use anchor_lang::prelude::*;
use crate::state::*;
use crate::errors::ScholarStreamError;
use crate::events::MilestoneClaimedEvent;

#[derive(Accounts)]
#[instruction(milestone_index: u8)]
pub struct ClaimMilestone<'info> {
    #[account(
        mut,
        has_one = student @ ScholarStreamError::Unauthorized,
    )]
    pub grant_account: Account<'info, GrantAccount>,

    #[account(
        mut,
        seeds = [b"vault", grant_account.key().as_ref()],
        bump = grant_account.vault_bump
    )]
    pub vault: SystemAccount<'info>,

    #[account(mut)]
    pub student: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<ClaimMilestone>,
    milestone_index: u8,
) -> Result<()> {
    let grant = &mut ctx.accounts.grant_account;

    require!((milestone_index as usize) < grant.milestones.len(), ScholarStreamError::InvalidMilestoneIndex);
    require!(milestone_index == grant.current_milestone, ScholarStreamError::NonSequentialMilestone);

    let milestone = &mut grant.milestones[milestone_index as usize];
    require!(milestone.status == MilestoneStatus::Approved, ScholarStreamError::InvalidMilestoneState);

    let disbursement_amount = milestone.amount;
    require!(
        ctx.accounts.vault.lamports() >= disbursement_amount,
        ScholarStreamError::InsufficientVaultFunds
    );

    // Direct PDA native lamport disbursement
    **ctx.accounts.vault.to_account_info().try_borrow_mut_lamports()? -= disbursement_amount;
    **ctx.accounts.student.to_account_info().try_borrow_mut_lamports()? += disbursement_amount;

    milestone.status = MilestoneStatus::Claimed;
    grant.claimed_amount = grant.claimed_amount.checked_add(disbursement_amount).ok_or(ProgramError::ArithmeticOverflow)?;
    grant.current_milestone = grant.current_milestone.checked_add(1).unwrap_or(grant.current_milestone);

    let clock = Clock::get()?;
    emit!(MilestoneClaimedEvent {
        grant: grant.key(),
        milestone_index,
        student: ctx.accounts.student.key(),
        amount: disbursement_amount,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}