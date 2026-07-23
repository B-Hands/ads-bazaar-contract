//! Types, enums and constants shared across the AdsBazaar contract suite.
//!
//! Kept dependency-light on purpose: this crate must stay `no_std` and
//! import only `soroban-sdk` so every contract in the workspace can depend
//! on it without pulling in unrelated logic.
#![no_std]

use soroban_sdk::{contracttype, Address};

/// Identifier for a campaign, unique within a single campaign-escrow contract instance.
pub type CampaignId = u64;

/// Identifier for a dispute, unique within a single dispute-resolution contract instance.
pub type DisputeId = u64;

/// Basis-point denominator used for fee and split calculations (100.00%).
pub const BASIS_POINTS_DENOMINATOR: i128 = 10_000;

/// Lifecycle of a campaign inside the escrow contract.
///
/// TODO(contributors): confirm this covers every state needed once
/// milestone-based (partial) payouts are designed, not just all-or-nothing release.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CampaignStatus {
    /// Created by a business but not yet funded.
    Draft,
    /// Escrow balance has been deposited; open for creator applications.
    Funded,
    /// At least one creator has been approved and is actively producing content.
    Active,
    /// All approved creators have been paid out.
    Completed,
    /// Cancelled before completion; any escrowed funds refunded to the business.
    Cancelled,
    /// Campaign has been flagged for dispute; funds are locked pending admin resolution.
    Disputed,
}

/// Status of a single creator's application to a campaign.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationStatus {
    Pending,
    Approved,
    Rejected,
    ProofSubmitted,
    Paid,
}

/// Status of a dispute raised against a campaign application.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisputeStatus {
    Raised,
    UnderReview,
    Resolved,
}

/// Outcome recorded once a dispute is resolved.
///
/// TODO(contributors): this is intentionally coarse-grained (winner-takes-all
/// or split) — revisit once the arbitration model (single arbiter vs. jury vs.
/// oracle) is decided in `dispute-resolution`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisputeOutcome {
    /// No outcome recorded yet — the dispute is still open.
    Pending,
    CreatorFavored,
    BusinessFavored,
    /// Split expressed in basis points awarded to the creator (remainder to business).
    Split(i128),
}

/// Describes the Stellar asset a campaign is funded and paid out in.
///
/// `token` is the Stellar Asset Contract (SAC) or any SEP-41-compatible
/// token contract address — this is how multi-currency support (XLM,
/// NGNC-style Naira stablecoins, USDC, etc.) is represented at the
/// contract level without the escrow logic needing to special-case assets.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PayoutAsset {
    pub token: Address,
    /// Human-readable symbol for off-chain display (e.g. "USDC", "NGNC"). Not
    /// trusted for any on-chain logic — purely informational.
    pub symbol: soroban_sdk::String,
}
