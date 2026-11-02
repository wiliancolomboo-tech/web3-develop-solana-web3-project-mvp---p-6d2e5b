use anchor_lang::prelude::*;

#[event]
pub struct GrantInitializedEvent {
    pub grant: Pubkey,
    pub sponsor: Pubkey,
    pub student: Pubkey,
    pub supervisor: Pubkey,
    pub grant_id: u64,
    pub total_amount: u64,
    pub title: String,
    pub timestamp: i64,
}

#[event]
pub struct MilestoneSubmittedEvent {
    pub grant: Pubkey,
    pub milestone_index: u8,
    pub proof_uri: String,
    pub timestamp: i64,
}

#[event]
pub struct MilestoneApprovedEvent {
    pub grant: Pubkey,
    pub milestone_index: u8,
    pub supervisor: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct MilestoneClaimedEvent {
    pub grant: Pubkey,
    pub milestone_index: u8,
    pub student: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}