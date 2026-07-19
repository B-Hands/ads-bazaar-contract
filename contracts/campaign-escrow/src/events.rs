//! Event definitions for the campaign-escrow contract, using the
//! `#[contractevent]` macro so events are part of the contract's on-chain
//! interface spec (discoverable by indexers/SDKs), not just ad-hoc
//! `env.events().publish(...)` calls.
//!
//! None of these are published yet — wire up `.publish(&env)` calls at the
//! matching point in `lib.rs` as each `todo!()` handler is implemented.
#![allow(dead_code)]

use ads_bazaar_shared::CampaignId;
use soroban_sdk::{contractevent, Address, BytesN};

#[contractevent]
#[derive(Clone, Debug)]
pub struct CampaignCreated {
    #[topic]
    pub business: Address,
    pub campaign_id: CampaignId,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct CampaignFunded {
    #[topic]
    pub campaign_id: CampaignId,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct CreatorApplied {
    #[topic]
    pub campaign_id: CampaignId,
    #[topic]
    pub creator: Address,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct CreatorApproved {
    #[topic]
    pub campaign_id: CampaignId,
    #[topic]
    pub creator: Address,
    pub payout_amount: i128,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct ProofSubmitted {
    #[topic]
    pub campaign_id: CampaignId,
    #[topic]
    pub creator: Address,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct PaymentReleased {
    #[topic]
    pub campaign_id: CampaignId,
    #[topic]
    pub creator: Address,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct CampaignCancelled {
    #[topic]
    pub campaign_id: CampaignId,
    pub refunded_amount: i128,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct ContractUpgraded {
    pub new_wasm_hash: BytesN<32>,
}
