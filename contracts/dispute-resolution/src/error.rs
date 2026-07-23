use soroban_sdk::contracterror;

/// Errors returned by the dispute-resolution contract.
///
/// TODO(contributors): extend as arbitration logic is filled in — e.g.
/// `DisputeAlreadyRaised`, `NotAssignedArbiter`, `EvidenceWindowClosed`.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    DisputeNotFound = 4,
    InvalidStatus = 5,
    /// A dispute over this `(campaign_id, creator)` payout is already open.
    DisputeAlreadyRaised = 6,
    /// `reason_uri` must be non-empty — an arbiter needs something to review.
    InvalidReason = 7,
}
