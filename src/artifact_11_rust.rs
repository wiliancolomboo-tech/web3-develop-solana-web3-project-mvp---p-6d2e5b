use anchor_lang::prelude::*;
use crate::state::*;
use crate::errors::ScholarStreamError;
use crate::events::MilestoneApprovedEvent;

#[derive(Accounts)]
#[instruction(milestone_index: u8)]
pub struct ApproveMilestone<'info> {
    #[account(
        mut,
        has_one = supervisor @ ScholarStreamError::Unauthorized,
    )]
    pub grant_account: Account<'info, GrantAccount>,

    pub supervisor: Signer<'info>,
}

pub fn handler(
    ctx: Context<ApproveMilestone>,
    milestone_index: u8,
) -> Result<()> {
    let grant = &mut ctx.accounts.grant_account;

    require!((milestone_index as usize) < grant.milestones.len(), ScholarStreamError::InvalidMilestoneIndex);
    require!(milestone_index == grant.current_milestone, ScholarStreamError::NonSequentialMilestone);

    let milestone = &mut grant.milestones[milestone_index as usize];
    require!(milestone.status == MilestoneStatus::Submitted, ScholarStreamError::InvalidMilestoneState);

    milestone.status = MilestoneStatus::Approved;

    let clock = Clock::get()?;
    emit!(MilestoneApprovedEvent {
        grant: grant.key(),
        milestone_index,
        supervisor: ctx.accounts.supervisor.key(),
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}