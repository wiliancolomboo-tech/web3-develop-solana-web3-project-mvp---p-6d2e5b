use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};
use crate::state::*;
use crate::errors::ScholarStreamError;
use crate::events::GrantInitializedEvent;

#[derive(Accounts)]
#[instruction(grant_id: u64, title: String, milestone_amounts: Vec<u64>, milestone_deadlines: Vec<i64>)]
pub struct InitializeGrant<'info> {
    #[account(mut)]
    pub sponsor: Signer<'info>,

    /// CHECK: Validated as recipient of student grant
    pub student: AccountInfo<'info>,

    /// CHECK: Validated faculty supervisor
    pub supervisor: AccountInfo<'info>,

    #[account(
        init,
        payer = sponsor,
        space = GrantAccount::space(milestone_amounts.len()),
        seeds = [b"grant", sponsor.key().as_ref(), &grant_id.to_le_bytes()],
        bump
    )]
    pub grant_account: Account<'info, GrantAccount>,

    #[account(
        mut,
        seeds = [b"vault", grant_account.key().as_ref()],
        bump
    )]
    pub vault: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitializeGrant>,
    grant_id: u64,
    title: String,
    milestone_amounts: Vec<u64>,
    milestone_deadlines: Vec<i64>,
) -> Result<()> {
    require!(title.len() <= GrantAccount::MAX_TITLE_LEN, ScholarStreamError::TitleTooLong);
    require!(
        !milestone_amounts.is_empty() && milestone_amounts.len() <= GrantAccount::MAX_MILESTONES,
        ScholarStreamError::InvalidMilestoneCount
    );
    require!(
        milestone_amounts.len() == milestone_deadlines.len(),
        ScholarStreamError::MilestoneMismatch
    );

    let clock = Clock::get()?;
    let mut total_amount: u64 = 0;
    let mut milestones: Vec<Milestone> = Vec::with_capacity(milestone_amounts.len());

    for (i, (&amount, &deadline)) in milestone_amounts.iter().zip(milestone_deadlines.iter()).enumerate() {
        require!(amount > 0, ScholarStreamError::ZeroAmount);
        require!(deadline > clock.unix_timestamp, ScholarStreamError::InvalidDeadline);

        if i > 0 {
            require!(deadline > milestone_deadlines[i - 1], ScholarStreamError::InvalidDeadline);
        }

        total_amount = total_amount.checked_add(amount).ok_or(ProgramError::ArithmeticOverflow)?;

        milestones.push(Milestone {
            amount,
            deadline,
            status: MilestoneStatus::Pending,
            proof_uri: String::new(),
        });
    }

    // Transfer total fund amount into the PDA vault
    let transfer_ctx = CpiContext::new(
        ctx.accounts.system_program.to_account_info(),
        Transfer {
            from: ctx.accounts.sponsor.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
        },
    );
    transfer(transfer_ctx, total_amount)?;

    let grant = &mut ctx.accounts.grant_account;
    grant.sponsor = ctx.accounts.sponsor.key();
    grant.student = ctx.accounts.student.key();
    grant.supervisor = ctx.accounts.supervisor.key();
    grant.grant_id = grant_id;
    grant.total_amount = total_amount;
    grant.claimed_amount = 0;
    grant.total_milestones = milestones.len() as u8;
    grant.current_milestone = 0;
    grant.bump = ctx.bumps.grant_account;
    grant.vault_bump = ctx.bumps.vault;
    grant.title = title.clone();
    grant.milestones = milestones;

    emit!(GrantInitializedEvent {
        grant: grant.key(),
        sponsor: grant.sponsor,
        student: grant.student,
        supervisor: grant.supervisor,
        grant_id,
        total_amount,
        title,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}