use anchor_lang::prelude::*;
use crate::state::*;
use crate::errors::ScholarStreamError;
use crate::events::MilestoneSubmittedEvent;

#[derive(Accounts)]
#[instruction(milestone_index: u8)]
pub struct SubmitMilestone<'info> {
    #[account(
        mut,
        has_one = student @ ScholarStreamError::Unauthorized,
    )]
    pub grant_account: Account<'info, GrantAccount>,

    pub student: Signer<'info>,
}

pub fn handler(
    ctx: Context<SubmitMilestone>,
    milestone_index: u8,
    proof_uri: String,
) -> Result<()> {
    require!(proof_uri.len() <= GrantAccount::MAX_PROOF_URI_LEN, ScholarStreamError::ProofTooLong);
    let grant = &mut ctx.accounts.grant_account;

    require!((milestone_index as usize) < grant.milestones.len(), ScholarStreamError::InvalidMilestoneIndex);
    require!(milestone_index == grant.current_milestone, ScholarStreamError::NonSequentialMilestone);

    let clock = Clock::get()?;
    let milestone = &mut grant.milestones[milestone_index as usize];

    require!(clock.unix_timestamp <= milestone.deadline, ScholarStreamError::MilestoneExpired);
    require!(milestone.status == MilestoneStatus::Pending, ScholarStreamError::InvalidMilestoneState);

    milestone.status = MilestoneStatus::Submitted;
    milestone.proof_uri = proof_uri.clone();

    emit!(MilestoneSubmittedEvent {
        grant: grant.key(),
        milestone_index,
        proof_uri,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}