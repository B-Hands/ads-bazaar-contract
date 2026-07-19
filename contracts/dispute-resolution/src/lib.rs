//! # ads-bazaar-dispute-resolution
//!
//! Arbitrates disputes over campaign payouts held by `campaign-escrow`. As
//! with that contract, this crate ships the data model, storage schema,
//! errors and public API surface; the arbitration workflow itself
//! (assigning arbiters, evidence windows, resolving outcomes and calling
//! back into escrow) is left as `todo!()` for contributors — the
//! arbitration *model* (single trusted arbiter vs. staked jurors vs. an
//! oracle) is the biggest open design question in this repo.
#![no_std]

mod error;
mod events;
mod storage;
mod types;

pub use error::Error;
pub use types::Dispute;

use ads_bazaar_shared::{CampaignId, DisputeId, DisputeOutcome};
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String};

/// Version string stored at `initialize` time. `upgrade` swaps the WASM
/// binary but does not bump this on its own — see the TODO on `upgrade`
/// below.
const INITIAL_VERSION: &str = "0.1.0";

#[contract]
pub struct DisputeResolutionContract;

#[contractimpl]
impl DisputeResolutionContract {
    /// One-time setup. `escrow_contract` should be the deployed
    /// `campaign-escrow` contract's address — this contract will need to
    /// call back into it (`resolve_dispute_payout`) once resolution logic
    /// is implemented.
    pub fn initialize(env: Env, admin: Address, escrow_contract: Address) -> Result<(), Error> {
        if storage::is_initialized(&env) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();

        storage::set_admin(&env, &admin);
        storage::set_escrow_contract(&env, &escrow_contract);
        storage::set_version(&env, &String::from_str(&env, INITIAL_VERSION));
        Ok(())
    }

    /// Raise a dispute over a creator's payout on a given campaign.
    ///
    /// TODO(contributors): implement. Should call
    /// `campaign_escrow::Client::freeze_for_dispute` on the configured
    /// escrow contract once that hook exists, so funds can't be released
    /// mid-dispute. Decide who may raise a dispute (business, creator, or
    /// both) and whether there's a time window after proof submission.
    #[allow(unused_variables)]
    pub fn raise_dispute(
        env: Env,
        raised_by: Address,
        campaign_id: CampaignId,
        creator: Address,
        reason_uri: String,
    ) -> Result<DisputeId, Error> {
        raised_by.require_auth();
        todo!("design + implement dispute raising — see doc comment above")
    }

    /// Assign an arbiter to review a raised dispute.
    ///
    /// TODO(contributors): implement once the arbitration model is decided.
    #[allow(unused_variables)]
    pub fn assign_arbiter(
        env: Env,
        admin: Address,
        dispute_id: DisputeId,
        arbiter: Address,
    ) -> Result<(), Error> {
        admin.require_auth();
        todo!("design + implement arbiter assignment — see doc comment above")
    }

    /// Arbiter resolves a dispute with a final outcome, then calls back
    /// into `campaign-escrow::resolve_dispute_payout` to release/refund
    /// the frozen funds accordingly.
    ///
    /// TODO(contributors): implement.
    #[allow(unused_variables)]
    pub fn resolve_dispute(
        env: Env,
        arbiter: Address,
        dispute_id: DisputeId,
        outcome: DisputeOutcome,
    ) -> Result<(), Error> {
        arbiter.require_auth();
        todo!("design + implement dispute resolution — see doc comment above")
    }

    /// Read-only lookup of a dispute's current state.
    pub fn get_dispute(env: Env, dispute_id: DisputeId) -> Result<Dispute, Error> {
        storage::get_dispute(&env, dispute_id)
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
