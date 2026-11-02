use anchor_lang::prelude::*;

#[error_code]
pub enum ScholarStreamError {
    #[msg("Title string exceeds maximum length of 64 bytes.")]
    TitleTooLong,

    #[msg("Proof URI string exceeds maximum length of 128 bytes.")]
    ProofTooLong,

    #[msg("Grant must contain between 1 and 10 milestones.")]
    InvalidMilestoneCount,

    #[msg("Milestone arrays size mismatch.")]
    MilestoneMismatch,

    #[msg("Deadline must be set strictly in the future.")]
    InvalidDeadline,

    #[msg("Milestone amount must be strictly greater than zero.")]
    ZeroAmount,

    #[msg("Invalid milestone index requested.")]
    InvalidMilestoneIndex,

    #[msg("Milestones must be completed sequentially.")]
    NonSequentialMilestone,

    #[msg("Milestone is not in a valid state for this transition.")]
    InvalidMilestoneState,

    #[msg("Milestone deadline has already expired.")]
    MilestoneExpired,

    #[msg("Unauthorized signer for this operational flow.")]
    Unauthorized,

    #[msg("Insufficient vault balance to satisfy disbursement.")]
    InsufficientVaultFunds,
}