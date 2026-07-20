use soroban_sdk::contracterror;

/// Errors returned by the campaign-escrow contract.
///
/// TODO(contributors): extend as apply/approve/proof/dispute logic is filled
/// in — e.g. `ApplicationAlreadyExists`, `ProofAlreadySubmitted`,
/// `NotApprovedCreator`, `DisputeAlreadyRaised`.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    CampaignNotFound = 4,
    ApplicationNotFound = 5,
    InvalidStatus = 6,
    InvalidAmount = 7,
    DeadlinePassed = 8,
    MaxCreatorsReached = 9,
    InsufficientEscrowBalance = 10,
    NotCampaignOwner = 11,
    CampaignClosed = 12,
    /// Returned by any guarded state-changing function while the contract
    /// is paused via `pause`. See `require_not_paused` in `lib.rs`.
    ContractPaused = 11,
}
