//! # ads-bazaar-campaign-escrow
//!
//! Holds business-funded campaign budgets in escrow and releases them to
//! approved creators. This crate implements the full escrow lifecycle:
//! campaign creation, funding, creator applications, selection, proof
//! submission/review, payout release (with platform fee), cancellation,
//! expiry and surplus reclaim.
//!
//! Money movement goes through the standard SEP-41 token `Client`
//! (`soroban_sdk::token::Client`) against `Campaign::asset.token`, which is
//! how a single contract deployment supports XLM, Naira-pegged stablecoins,
//! USDC, etc. without per-asset special-casing.
#![no_std]

mod error;
mod events;
mod storage;
mod types;

pub use error::Error;
pub use types::{Application, Campaign, ProtocolConfig};

use ads_bazaar_shared::{ApplicationStatus, CampaignId, CampaignStatus, PayoutAsset};
use soroban_sdk::{contract, contractimpl, token, Address, BytesN, Env, String};

/// Version string stored at `initialize` time. `upgrade` swaps the WASM
/// binary but does not bump this on its own — see the TODO on `upgrade`
/// below.
const INITIAL_VERSION: &str = "0.1.0";

/// Require that `admin` matches the address stored at `initialize` time.
/// Returns `Error::Unauthorized` for any other caller. Used by `pause` and
/// `unpause`.
fn require_admin(env: &Env, admin: &Address) -> Result<(), Error> {
    admin.require_auth();
    let stored_admin = storage::get_admin(env)?;
    if stored_admin != *admin {
        return Err(Error::Unauthorized);
    }
    Ok(())
}

/// Guard called at the top of every state-changing function. Returns
/// `Error::ContractPaused` if the contract is currently paused via `pause`.
/// Read-only functions intentionally do not call this, so users can still
/// read their data while the contract is paused.
fn require_not_paused(env: &Env) -> Result<(), Error> {
    if storage::get_paused(env) {
        return Err(Error::ContractPaused);
    }
    Ok(())
}

#[contract]
pub struct CampaignEscrowContract;

#[contractimpl]
impl CampaignEscrowContract {
    /// One-time setup. Must be called before any other function.
    ///
    /// `dispute_contract` is the only address permitted to call
    /// `freeze_for_dispute` / `resolve_dispute_payout` once those are
    /// implemented — it should be the deployed `dispute-resolution`
    /// contract's address.
    pub fn initialize(
        env: Env,
        admin: Address,
        dispute_contract: Address,
        fee_bps: i128,
    ) -> Result<(), Error> {
        if storage::is_initialized(&env) {
            return Err(Error::AlreadyInitialized);
        }
        if !(0..=ads_bazaar_shared::BASIS_POINTS_DENOMINATOR).contains(&fee_bps) {
            return Err(Error::InvalidAmount);
        }
        admin.require_auth();

        storage::set_admin(&env, &admin);
        // No separate fee-collection destination exists yet (see the TODO
        // on `release_payment` below) — treasury defaults to admin until a
        // future issue adds a dedicated setter.
        storage::set_treasury(&env, &admin);
        storage::set_dispute_contract(&env, &dispute_contract);
        storage::set_fee_bps(&env, fee_bps);
        storage::set_version(&env, &String::from_str(&env, INITIAL_VERSION));
        Ok(())
    }

    /// Freeze all state-changing operations. Callable only by the admin set
    /// at `initialize`. Emits `events::ContractPaused`. View functions are
    /// unaffected and remain readable.
    pub fn pause(env: Env, admin: Address) -> Result<(), Error> {
        require_admin(&env, &admin)?;
        storage::set_paused(&env, true);
        events::ContractPaused { admin }.publish(&env);
        Ok(())
    }

    /// Resume state-changing operations after a `pause`. Callable only by
    /// the admin set at `initialize`. Emits `events::ContractUnpaused`.
    pub fn unpause(env: Env, admin: Address) -> Result<(), Error> {
        require_admin(&env, &admin)?;
        storage::set_paused(&env, false);
        events::ContractUnpaused { admin }.publish(&env);
        Ok(())
    }

    /// Read-only: current pause state. Accessible regardless of whether the
    /// contract is paused.
    pub fn is_paused(env: Env) -> bool {
        storage::get_paused(&env)
    }

    /// Update the platform fee for future `claim_payment` calls.
    /// The fee is read at claim time, so a fee change affects pending campaigns.
    /// Callable only by the admin.
    ///
    /// Capped at 1,000 bps (10%), deliberately tighter than the 0..=10,000
    /// range `initialize` allows — a sane ceiling for adjusting an already-
    /// live fee, even though the wider range remains available at deploy time.
    pub fn update_fee_bps(env: Env, admin: Address, new_fee_bps: i128) -> Result<(), Error> {
        require_admin(&env, &admin)?;
        if !(0..=1_000).contains(&new_fee_bps) {
            return Err(Error::FeeTooHigh);
        }
        storage::set_fee_bps(&env, new_fee_bps);
        events::FeeUpdated { admin, new_fee_bps }.publish(&env);
        Ok(())
    }

    /// Update the treasury address where platform fees are sent.
    /// Callable only by the admin.
    pub fn update_treasury(env: Env, admin: Address, new_treasury: Address) -> Result<(), Error> {
        require_admin(&env, &admin)?;
        storage::set_treasury(&env, &new_treasury);
        events::TreasuryUpdated {
            admin,
            new_treasury,
        }
        .publish(&env);
        Ok(())
    }

    /// Create a new draft campaign owned by `business`. Not yet escrowed —
    /// call `fund_campaign` afterwards to deposit `total_budget`.
    ///
    /// Validates `total_budget > 0`, `max_creators > 0`, that both deadlines
    /// are in the future and that `application_deadline < completion_deadline`.
    #[allow(clippy::too_many_arguments)]
    pub fn create_campaign(
        env: Env,
        business: Address,
        asset: PayoutAsset,
        total_budget: i128,
        max_creators: u32,
        application_deadline: u64,
        completion_deadline: u64,
        metadata_uri: String,
    ) -> Result<CampaignId, Error> {
        require_not_paused(&env)?;
        if !storage::is_initialized(&env) {
            return Err(Error::NotInitialized);
        }
        if total_budget <= 0 || max_creators == 0 {
            return Err(Error::InvalidAmount);
        }
        let now = env.ledger().timestamp();
        if application_deadline <= now || completion_deadline <= now {
            return Err(Error::DeadlineInPast);
        }
        if application_deadline >= completion_deadline {
            return Err(Error::InvalidDeadlineOrder);
        }

        business.require_auth();

        let id = storage::next_campaign_id(&env);
        let campaign = Campaign {
            id,
            business: business.clone(),
            asset,
            total_budget,
            escrow_balance: 0,
            committed_payouts: 0,
            // Snapshotted at creation so a later admin fee change (see
            // `update_fee_bps`) doesn't retroactively affect this campaign.
            fee_bps: storage::get_fee_bps(&env)?,
            max_creators,
            approved_count: 0,
            application_deadline,
            completion_deadline,
            metadata_uri,
            status: CampaignStatus::Draft,
        };
        storage::set_campaign(&env, &campaign);
        events::CampaignCreated {
            business,
            campaign_id: id,
        }
        .publish(&env);
        Ok(id)
    }

    /// Transfer `campaign.total_budget` of `campaign.asset.token` from
    /// `business` into this contract, moving the campaign from `Draft` to
    /// `Funded`. Funding is strictly all-at-once.
    pub fn fund_campaign(
        env: Env,
        business: Address,
        campaign_id: CampaignId,
    ) -> Result<(), Error> {
        require_not_paused(&env)?;
        business.require_auth();
        let mut campaign = storage::get_campaign(&env, campaign_id)?;
        if campaign.status != CampaignStatus::Draft {
            return Err(Error::InvalidStatus);
        }
        if campaign.business != business {
            return Err(Error::NotCampaignOwner);
        }

        let token = token::Client::new(&env, &campaign.asset.token);
        token.transfer(
            &business,
            env.current_contract_address(),
            &campaign.total_budget,
        );
        campaign.escrow_balance = campaign.total_budget;
        campaign.status = CampaignStatus::Funded;
        storage::set_campaign(&env, &campaign);
        events::CampaignFunded {
            campaign_id,
            amount: campaign.total_budget,
        }
        .publish(&env);
        Ok(())
    }

    /// Creator applies to a funded (`Funded`) campaign before its application
    /// deadline. A creator may apply only once per campaign.
    pub fn apply_to_campaign(
        env: Env,
        creator: Address,
        campaign_id: CampaignId,
        pitch_uri: String,
    ) -> Result<(), Error> {
        require_not_paused(&env)?;
        creator.require_auth();
        let campaign = storage::get_campaign(&env, campaign_id)?;
        if campaign.status != CampaignStatus::Funded && campaign.status != CampaignStatus::Active {
            return Err(Error::InvalidStatus);
        }
        if env.ledger().timestamp() > campaign.application_deadline {
            return Err(Error::ApplicationDeadlinePassed);
        }
        if storage::get_application(&env, campaign_id, &creator).is_ok() {
            return Err(Error::AlreadyApplied);
        }

        let application = Application {
            campaign_id,
            creator: creator.clone(),
            pitch_uri,
            proof_uri: None,
            payout_amount: 0,
            proof_approved: false,
            status: ApplicationStatus::Pending,
        };
        storage::set_application(&env, &application);
        events::CreatorApplied {
            campaign_id,
            creator,
        }
        .publish(&env);
        Ok(())
    }

    /// Business approves a pending application, selecting the creator and
    /// setting their agreed `payout_amount`. Guards against selecting more
    /// than `max_creators`, double-selection, and over-committing escrow.
    pub fn approve_creator(
        env: Env,
        business: Address,
        campaign_id: CampaignId,
        creator: Address,
        payout_amount: i128,
    ) -> Result<(), Error> {
        require_not_paused(&env)?;
        business.require_auth();
        let mut campaign = storage::get_campaign(&env, campaign_id)?;
        if campaign.business != business {
            return Err(Error::NotCampaignOwner);
        }
        if campaign.status != CampaignStatus::Funded && campaign.status != CampaignStatus::Active {
            return Err(Error::InvalidStatus);
        }
        if payout_amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let mut application = storage::get_application(&env, campaign_id, &creator)?;
        if application.status != ApplicationStatus::Pending {
            return Err(Error::AlreadySelected);
        }

        if campaign.approved_count >= campaign.max_creators {
            return Err(Error::MaxCreatorsReached);
        }
        if campaign.committed_payouts + payout_amount > campaign.escrow_balance {
            return Err(Error::InsufficientEscrowBalance);
        }

        application.payout_amount = payout_amount;
        application.status = ApplicationStatus::Approved;
        storage::set_application(&env, &application);

        campaign.approved_count += 1;
        campaign.committed_payouts += payout_amount;
        if campaign.status == CampaignStatus::Funded {
            campaign.status = CampaignStatus::Active;
        }
        storage::set_campaign(&env, &campaign);
        events::CreatorApproved {
            campaign_id,
            creator,
            payout_amount,
        }
        .publish(&env);
        Ok(())
    }

    /// Approved creator submits proof of completed work. May only be called
    /// before the content deadline.
    pub fn submit_proof(
        env: Env,
        creator: Address,
        campaign_id: CampaignId,
        proof_uri: String,
    ) -> Result<(), Error> {
        require_not_paused(&env)?;
        creator.require_auth();
        let campaign = storage::get_campaign(&env, campaign_id)?;
        if env.ledger().timestamp() > campaign.completion_deadline {
            return Err(Error::ContentDeadlinePassed);
        }

        let mut application = storage::get_application(&env, campaign_id, &creator)?;
        if application.status != ApplicationStatus::Approved {
            return Err(Error::InvalidStatus);
        }

        application.proof_uri = Some(proof_uri);
        application.status = ApplicationStatus::ProofSubmitted;
        application.proof_approved = false;
        storage::set_application(&env, &application);
        events::ProofSubmitted {
            campaign_id,
            creator,
        }
        .publish(&env);
        Ok(())
    }

    /// Business accepts a submitted proof, marking the submission payable.
    pub fn approve_submission(
        env: Env,
        business: Address,
        campaign_id: CampaignId,
        creator: Address,
    ) -> Result<(), Error> {
        require_not_paused(&env)?;
        business.require_auth();
        let campaign = storage::get_campaign(&env, campaign_id)?;
        if campaign.business != business {
            return Err(Error::NotCampaignOwner);
        }

        let mut application = storage::get_application(&env, campaign_id, &creator)?;
        if application.status != ApplicationStatus::ProofSubmitted {
            return Err(Error::InvalidStatus);
        }
        application.proof_approved = true;
        storage::set_application(&env, &application);
        Ok(())
    }

    /// Business rejects a submitted proof, returning the creator to the
    /// selected state so they may re-submit proof.
    pub fn reject_submission(
        env: Env,
        business: Address,
        campaign_id: CampaignId,
        creator: Address,
    ) -> Result<(), Error> {
        require_not_paused(&env)?;
        business.require_auth();
        let campaign = storage::get_campaign(&env, campaign_id)?;
        if campaign.business != business {
            return Err(Error::NotCampaignOwner);
        }

        let mut application = storage::get_application(&env, campaign_id, &creator)?;
        if application.status != ApplicationStatus::ProofSubmitted {
            return Err(Error::InvalidStatus);
        }
        application.proof_uri = None;
        application.proof_approved = false;
        application.status = ApplicationStatus::Approved;
        storage::set_application(&env, &application);
        Ok(())
    }

    /// Release an approved creator's escrowed payout, deducting the platform
    /// fee configured at `initialize`. Callable by the creator once their
    /// submission is approved, or automatically once the content deadline has
    /// passed (auto-approval).
    pub fn claim_payment(env: Env, creator: Address, campaign_id: CampaignId) -> Result<(), Error> {
        require_not_paused(&env)?;
        creator.require_auth();
        let mut campaign = storage::get_campaign(&env, campaign_id)?;

        let mut application = storage::get_application(&env, campaign_id, &creator)?;
        if application.status != ApplicationStatus::ProofSubmitted {
            return Err(Error::SubmissionNotPayable);
        }

        let auto_approved = env.ledger().timestamp() > campaign.completion_deadline;
        if !application.proof_approved && !auto_approved {
            return Err(Error::SubmissionNotPayable);
        }

        // Use the fee snapshotted at campaign creation, not the current
        // instance value — an admin fee change (`update_fee_bps`) must not
        // retroactively affect a campaign's already-agreed payouts.
        let fee_bps = campaign.fee_bps;
        let fee = application
            .payout_amount
            .checked_mul(fee_bps)
            .ok_or(Error::InvalidAmount)?
            / ads_bazaar_shared::BASIS_POINTS_DENOMINATOR;
        let net = application
            .payout_amount
            .checked_sub(fee)
            .ok_or(Error::InvalidAmount)?;

        let token = token::Client::new(&env, &campaign.asset.token);
        let contract = env.current_contract_address();
        if fee > 0 {
            token.transfer(&contract, &storage::get_treasury(&env)?, &fee);
        }
        token.transfer(&contract, &creator, &net);

        application.status = ApplicationStatus::Paid;
        storage::set_application(&env, &application);

        campaign.escrow_balance -= application.payout_amount;
        campaign.committed_payouts -= application.payout_amount;
        if campaign.escrow_balance == 0 {
            campaign.status = CampaignStatus::Completed;
        }
        storage::set_campaign(&env, &campaign);
        events::PaymentReleased {
            campaign_id,
            creator,
            amount: net,
        }
        .publish(&env);
        Ok(())
    }

    /// Cancel a campaign and refund the unallocated (never-committed) portion
    /// of the escrow to the business. Allowed at any point before full payout
    /// completion. Payouts already committed to approved creators remain
    /// reserved and can still be claimed via `claim_payment` afterward.
    pub fn cancel_campaign(
        env: Env,
        business: Address,
        campaign_id: CampaignId,
    ) -> Result<(), Error> {
        require_not_paused(&env)?;
        business.require_auth();
        let mut campaign = storage::get_campaign(&env, campaign_id)?;
        if campaign.business != business {
            return Err(Error::NotCampaignOwner);
        }
        if campaign.status == CampaignStatus::Cancelled
            || campaign.status == CampaignStatus::Completed
        {
            return Err(Error::InvalidStatus);
        }

        let token = token::Client::new(&env, &campaign.asset.token);
        let contract = env.current_contract_address();
        // Never refund more than the unallocated balance. `committed_payouts`
        // is reserved for approved creators who are still owed payment and can
        // `claim_payment` even after the campaign is cancelled.
        let refund = campaign
            .escrow_balance
            .checked_sub(campaign.committed_payouts)
            .ok_or(Error::InvalidAmount)?;
        if refund > 0 {
            token.transfer(&contract, &business, &refund);
        }
        // Leave `committed_payouts` intact so approved-but-unpaid creators can
        // still claim their payouts afterward.
        campaign.escrow_balance = campaign.committed_payouts;
        campaign.status = CampaignStatus::Cancelled;
        storage::set_campaign(&env, &campaign);
        events::CampaignCancelled {
            campaign_id,
            refunded_amount: refund,
        }
        .publish(&env);
        Ok(())
    }

    /// Expire a campaign past its content deadline, refunding the unallocated
    /// (never-committed) portion of the escrow balance to the business. Fails
    /// if called before the content deadline is reached. Any payout already
    /// committed to an approved creator remains reserved and claimable.
    pub fn expire_campaign(
        env: Env,
        business: Address,
        campaign_id: CampaignId,
    ) -> Result<(), Error> {
        require_not_paused(&env)?;
        business.require_auth();
        let mut campaign = storage::get_campaign(&env, campaign_id)?;
        if campaign.business != business {
            return Err(Error::NotCampaignOwner);
        }
        if env.ledger().timestamp() <= campaign.completion_deadline {
            return Err(Error::DeadlineNotReached);
        }
        if campaign.status == CampaignStatus::Cancelled
            || campaign.status == CampaignStatus::Completed
        {
            return Err(Error::InvalidStatus);
        }

        let token = token::Client::new(&env, &campaign.asset.token);
        let contract = env.current_contract_address();
        // Only the unallocated balance is refundable; committed payouts stay
        // reserved for approved creators who can still `claim_payment`.
        let refund = campaign
            .escrow_balance
            .checked_sub(campaign.committed_payouts)
            .ok_or(Error::InvalidAmount)?;
        if refund > 0 {
            token.transfer(&contract, &business, &refund);
        }
        // Leave `committed_payouts` intact so approved-but-unpaid creators can
        // still claim their payouts afterward.
        campaign.escrow_balance = campaign.committed_payouts;
        campaign.status = CampaignStatus::Cancelled;
        storage::set_campaign(&env, &campaign);
        events::CampaignCancelled {
            campaign_id,
            refunded_amount: refund,
        }
        .publish(&env);
        Ok(())
    }

    /// Reclaim any unallocated (surplus) escrow back to the business. Surplus
    /// is whatever escrow remains once committed payouts are excluded, so it
    /// can be called while approved creators are still owed payment — those
    /// reserved payouts remain claimable afterward.
    pub fn reclaim_surplus(
        env: Env,
        business: Address,
        campaign_id: CampaignId,
    ) -> Result<(), Error> {
        require_not_paused(&env)?;
        business.require_auth();
        let mut campaign = storage::get_campaign(&env, campaign_id)?;
        if campaign.business != business {
            return Err(Error::NotCampaignOwner);
        }
        if campaign.status == CampaignStatus::Cancelled {
            return Err(Error::InvalidStatus);
        }

        let token = token::Client::new(&env, &campaign.asset.token);
        let contract = env.current_contract_address();
        // Surplus is the unallocated balance only; committed payouts stay
        // reserved for approved creators who can still `claim_payment`.
        let surplus = campaign
            .escrow_balance
            .checked_sub(campaign.committed_payouts)
            .ok_or(Error::InvalidAmount)?;
        if surplus > 0 {
            token.transfer(&contract, &business, &surplus);
        }
        // Leave `committed_payouts` intact so approved-but-unpaid creators can
        // still claim their payouts afterward.
        campaign.escrow_balance = campaign.committed_payouts;
        if campaign.status != CampaignStatus::Completed {
            campaign.status = CampaignStatus::Completed;
        }
        storage::set_campaign(&env, &campaign);
        events::SurplusReclaimed {
            campaign_id,
            amount: surplus,
        }
        .publish(&env);
        Ok(())
    }

    /// Freeze a campaign's escrow so funds cannot be released while a
    /// dispute is under review. Callable only by the trusted
    /// `dispute-resolution` contract set at `initialize`.
    ///
    /// TODO(contributors): implement once `dispute-resolution`'s call
    /// interface is finalized. This should be an authenticated
    /// contract-to-contract call (verify `env.current_contract_address()`
    /// caller via `require_auth` on the dispute contract's own invocation,
    /// or restrict by checking `get_dispute_contract` matches the invoker).
    #[allow(unused_variables)]
    pub fn freeze_for_dispute(
        env: Env,
        campaign_id: CampaignId,
        creator: Address,
    ) -> Result<(), Error> {
        require_not_paused(&env)?;
        todo!("design + implement dispute freeze hook — see doc comment above")
    }

    /// Apply a dispute outcome (from `dispute-resolution`) by releasing or
    /// refunding the frozen escrow amount accordingly.
    ///
    /// TODO(contributors): implement alongside `freeze_for_dispute`.
    #[allow(unused_variables)]
    pub fn resolve_dispute_payout(
        env: Env,
        campaign_id: CampaignId,
        creator: Address,
        creator_bps: i128,
    ) -> Result<(), Error> {
        require_not_paused(&env)?;
        todo!("design + implement dispute payout resolution — see doc comment above")
    }

    /// Read-only lookup of a campaign's current state.
    pub fn get_campaign(env: Env, campaign_id: CampaignId) -> Result<Campaign, Error> {
        storage::get_campaign(&env, campaign_id)
    }

    /// Read-only lookup of a creator's application to a campaign.
    pub fn get_application(
        env: Env,
        campaign_id: CampaignId,
        creator: Address,
    ) -> Result<Application, Error> {
        storage::get_application(&env, campaign_id, &creator)
    }

    /// Read-only lookup of protocol-level config (admin, treasury, fee_bps)
    /// so the frontend can compute a fee breakdown before funding a
    /// campaign. Requires no auth. Errors with `Error::NotInitialized` if
    /// called before `initialize`.
    pub fn get_protocol_config(env: Env) -> Result<ProtocolConfig, Error> {
        let admin = storage::get_admin(&env)?;
        let treasury = storage::get_treasury(&env)?;
        let fee_bps = storage::get_fee_bps(&env)?;

        storage::extend_instance_ttl(&env);

        Ok(ProtocolConfig {
            admin,
            treasury,
            fee_bps,
        })
    }

    /// Read-only lookup of the WASM version string set at `initialize`.
    pub fn version(env: Env) -> Result<String, Error> {
        storage::get_version(&env)
    }

    /// Replace this contract's WASM binary in place via Soroban's native
    /// upgrade mechanism, preserving the contract address and all existing
    /// storage. Admin-only.
    ///
    /// TODO(contributors): this does not bump the stored `Version` — decide
    /// whether `upgrade` should take a new version string to persist, or
    /// whether version tracking should be derived from the wasm hash instead.
    pub fn upgrade(env: Env, admin: Address, new_wasm_hash: BytesN<32>) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin = storage::get_admin(&env)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        env.deployer()
            .update_current_contract_wasm(new_wasm_hash.clone());
        events::ContractUpgraded { new_wasm_hash }.publish(&env);
        Ok(())
    }
}

#[cfg(test)]
mod test;
