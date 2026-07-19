//! Event definitions for the dispute-resolution contract. See the
//! campaign-escrow crate's `events.rs` for more detail on the
//! `#[contractevent]` pattern used here. None of these are published yet —
//! wire up `.publish(&env)` calls as each `todo!()` handler is implemented.
#![allow(dead_code)]

use ads_bazaar_shared::DisputeId;
use soroban_sdk::{contractevent, Address, BytesN};

#[contractevent]
#[derive(Clone, Debug)]
pub struct DisputeRaised {
    #[topic]
    pub dispute_id: DisputeId,
    #[topic]
    pub raised_by: Address,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct DisputeResolved {
    #[topic]
    pub dispute_id: DisputeId,
}

#[contractevent]
#[derive(Clone, Debug)]
pub struct ContractUpgraded {
    pub new_wasm_hash: BytesN<32>,
}
