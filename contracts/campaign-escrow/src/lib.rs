//! # ads-bazaar-campaign-escrow
//!
//! Holds business-funded campaign budgets in escrow and releases them to
//! approved creators. This crate currently ships the **data model, storage
//! schema, error types, events and public API surface** for the full
//! escrow lifecycle; the state-transition logic for campaign creation,
//! funding, creator approval, proof review and payout release is left as
//! `todo!()` for contributors to design and implement (see inline TODOs and
//! `docs/ARCHITECTURE.md` at the repo root).
//!
//! Money movement is expected to go through the standard SEP-41 token
//! `Client` (`soroban_sdk::token::Client`) against `Campaign::asset.token`,
//! which is how a single contract deployment supports XLM, Naira-pegged
//! stablecoins, USDC, etc. without per-asset special-casing.
#![no_std]

mod error;
mod events;
mod storage;
mod types;

pub use error::Error;
pub use types::{Application, Campaign, ProtocolConfig};

use ads_bazaar_shared::{CampaignId, PayoutAsset};
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String};

/// Version string stored at `initialize` time. `upgrade` swaps the WASM
/// binary but does not bump this on its own — see the TODO on `upgrade`
/// below.
const INITIAL_VERSION: &str = "0.1.0";

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

    /// Create a new draft campaign owned by `business`. Not yet escrowed —
    /// call `fund_campaign` afterwards to deposit `total_budget`.
    ///
    /// TODO(contributors): implement. Should at minimum:
    /// - `business.require_auth()`
    /// - validate `total_budget > 0`, `max_creators > 0`
    /// - validate `application_deadline < completion_deadline` and both are
    ///   in the future (`env.ledger().timestamp()`)
    /// - allocate an id via `storage::next_campaign_id`, persist a
    ///   `Campaign` in `CampaignStatus::Draft` with `escrow_balance: 0`
    /// - emit `events::campaign_created`
    #[allow(unused_variables, clippy::too_many_arguments)]
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
        todo!("design + implement campaign creation — see doc comment above")
    }

    /// Transfer `campaign.total_budget` of `campaign.asset.token` from
    /// `business` into this contract, moving the campaign from `Draft` to
    /// `Funded`.
    ///
    /// TODO(contributors): implement using
    /// `soroban_sdk::token::Client::new(&env, &campaign.asset.token).transfer(...)`.
    /// Decide whether partial/top-up funding is allowed or funding is
    /// strictly all-at-once.
    #[allow(unused_variables)]
    pub fn fund_campaign(
        env: Env,
        business: Address,
        campaign_id: CampaignId,
    ) -> Result<(), Error> {
        business.require_auth();
        todo!("design + implement escrow funding — see doc comment above")
    }

    /// Creator applies to an active (`Funded`) campaign.
    ///
    /// TODO(contributors): implement. Consider: can a creator apply twice?
    /// What happens at `application_deadline`? Should this be permissionless
    /// or allow-listed?
    #[allow(unused_variables)]
    pub fn apply_to_campaign(
        env: Env,
        creator: Address,
        campaign_id: CampaignId,
        pitch_uri: String,
    ) -> Result<(), Error> {
        creator.require_auth();
        todo!("design + implement creator applications — see doc comment above")
    }

    /// Business approves a pending application and sets the agreed payout
    /// amount for that creator.
    ///
    /// TODO(contributors): implement. Must guard against approving more
    /// creators than `max_creators`, or approving a total `payout_amount`
    /// that exceeds `escrow_balance`.
    #[allow(unused_variables)]
    pub fn approve_creator(
        env: Env,
        business: Address,
        campaign_id: CampaignId,
        creator: Address,
        payout_amount: i128,
    ) -> Result<(), Error> {
        business.require_auth();
        todo!("design + implement creator approval — see doc comment above")
    }

    /// Approved creator submits proof of completed work.
    ///
    /// TODO(contributors): implement. Decide the proof format/verification
    /// story (off-chain URI only vs. on-chain hash commitment, oracle
    /// attestation, etc.) — this is likely the single biggest open design
    /// question in this contract.
    #[allow(unused_variables)]
    pub fn submit_proof(
        env: Env,
        creator: Address,
        campaign_id: CampaignId,
        proof_uri: String,
    ) -> Result<(), Error> {
        creator.require_auth();
        todo!("design + implement proof submission/verification — see doc comment above")
    }

    /// Release an approved creator's escrowed payout after proof is
    /// accepted, deducting the platform fee configured at `initialize`.
    ///
    /// TODO(contributors): implement. Decide who can trigger release
    /// (business only? auto-release after a timeout past proof submission?)
    /// and how the platform fee is collected (transferred to `admin`
    /// immediately, or accrued for later sweep).
    #[allow(unused_variables)]
    pub fn release_payment(
        env: Env,
        business: Address,
        campaign_id: CampaignId,
        creator: Address,
    ) -> Result<(), Error> {
        business.require_auth();
        todo!("design + implement payout release — see doc comment above")
    }

    /// Cancel a campaign and refund any remaining escrow balance to the
    /// business.
    ///
    /// TODO(contributors): implement. Decide whether cancellation is allowed
    /// once creators are already approved/active, and if so how their
    /// pending payouts are handled.
    #[allow(unused_variables)]
    pub fn cancel_campaign(
        env: Env,
        business: Address,
        campaign_id: CampaignId,
    ) -> Result<(), Error> {
        business.require_auth();
        todo!("design + implement cancellation/refund — see doc comment above")
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
