use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum MilestoneStatus {
    Pending,
    Submitted,
    Approved,
    Claimed,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Milestone {
    pub amount: u64,
    pub deadline: i64,
    pub status: MilestoneStatus,
    pub proof_uri: String, // IPFS CID or Arweave Tx Hash
}

#[account]
pub struct GrantAccount {
    pub sponsor: Pubkey,
    pub student: Pubkey,
    pub supervisor: Pubkey,
    pub grant_id: u64,
    pub total_amount: u64,
    pub claimed_amount: u64,
    pub total_milestones: u8,
    pub current_milestone: u8,
    pub bump: u8,
    pub vault_bump: u8,
    pub title: String,
    pub milestones: Vec<Milestone>,
}

impl GrantAccount {
    pub const MAX_TITLE_LEN: usize = 64;
    pub const MAX_MILESTONES: usize = 10;
    pub const MAX_PROOF_URI_LEN: usize = 128;

    pub fn space(num_milestones: usize) -> usize {
        8 +  // discriminator
        32 + // sponsor
        32 + // student
        32 + // supervisor
        8 +  // grant_id
        8 +  // total_amount
        8 +  // claimed_amount
        1 +  // total_milestones
        1 +  // current_milestone
        1 +  // bump
        1 +  // vault_bump
        (4 + Self::MAX_TITLE_LEN) + // title
        (4 + (num_milestones * (8 + 8 + 1 + (4 + Self::MAX_PROOF_URI_LEN)))) // milestones
    }
}