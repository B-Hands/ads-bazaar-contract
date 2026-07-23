//! Client for the narrow slice of `campaign-escrow` that this contract calls.
//!
//! Declared locally with `#[contractclient]` rather than depending on the
//! `ads-bazaar-campaign-escrow` crate: linking that crate into this one would
//! pull its `#[contractimpl]` exports into this contract's wasm, so both
//! contracts' entry points would ship in a single binary. Keep these
//! signatures in sync with `campaign-escrow/src/lib.rs`.
//!
//! Both methods are declared infallible even though the escrow contract
//! returns `Result<_, Error>`. The encoding is identical on success, and an
//! error from the callee traps the whole invocation — which is the behavior
//! we want, since neither a missing campaign nor a refused freeze leaves any
//! sensible way to continue raising the dispute.
#![allow(dead_code)]

use ads_bazaar_shared::CampaignId;
use soroban_sdk::{contractclient, Address, Env};

#[contractclient(name = "CampaignEscrowClient")]
pub trait CampaignEscrow {
    fn get_campaign_business(env: Env, campaign_id: CampaignId) -> Address;
    fn freeze_for_dispute(env: Env, campaign_id: CampaignId, creator: Address);
}
