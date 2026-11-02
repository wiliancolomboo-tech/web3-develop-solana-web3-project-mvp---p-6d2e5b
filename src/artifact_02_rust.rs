use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

declare_id!("KNEU111111111111111111111111111111111111111");

#[program]
pub mod scholar_stream {
    use super::*;

    /// Initialize a new academic milestone-based grant escrow
    pub fn initialize_grant(
        ctx: Context<InitializeGrant>,
        grant_id: u64,
        total_amount: u64,
        milestones_count: u8,
        metadata_uri: String,
    ) -> Result<()> {
        require!(milestones_count > 0, ScholarError::ZeroMilestones);
        require!(milestones_count <= 10, ScholarError::TooManyMilestones);
        require!(total_amount > 0, ScholarError::ZeroAmount);
        require!(metadata_uri.len() <= 200, ScholarError::MetadataUriTooLong);

        let grant = &mut ctx.accounts.grant_account;
        grant.sponsor = ctx.accounts.sponsor.key();
        grant.student = ctx.accounts.student.key();
        grant.evaluator = ctx.accounts.evaluator.key();
        grant.grant_id = grant_id;
        grant.total_amount = total_amount;
        grant.amount_remaining = total_amount;
        grant.milestones_count = milestones_count;
        grant.current_milestone = 0;
        grant.status = GrantStatus::Active;
        grant.metadata_uri = metadata_uri;
        grant.bump = ctx.bumps.grant_account;
        grant.vault_bump = ctx.bumps.vault;

        // Transfer funds from sponsor to the PDA escrow vault
        let cpi_context = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.sponsor.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
            },
        );
        transfer(cpi_context, total_amount)?;

        emit!(GrantInitializedEvent {
            grant_id,
            sponsor: ctx.accounts.sponsor.key(),
            student: ctx.accounts.student.key(),
            total_amount,
            milestones_count,
        });

        Ok(())
    }

    /// Student submits proof of work (IPFS CID / Git commit hash) for active milestone
    pub fn submit_milestone(
        ctx: Context<SubmitMilestone>,
        milestone_index: u8,
        proof_hash: [u8; 32],
    ) -> Result<()> {
        let grant = &mut ctx.accounts.grant_account;
        require!(grant.status == GrantStatus::Active, ScholarError::GrantInactive);
        require!(grant.student == ctx.accounts.student.key(), ScholarError::UnauthorizedStudent);
        require!(milestone_index == grant.current_milestone, ScholarError::InvalidMilestoneOrder);

        emit!(MilestoneSubmittedEvent {
            grant_id: grant.grant_id,
            milestone_index,
            proof_hash,
            student: grant.student,
        });

        Ok(())
    }

    /// Designated faculty evaluator reviews and approves milestone, disbursing proportion of escrow
    pub fn approve_milestone(
        ctx: Context<ApproveMilestone>,
        milestone_index: u8,
    ) -> Result<()> {
        let grant = &mut ctx.accounts.grant_account;
        require!(grant.status == GrantStatus::Active, ScholarError::GrantInactive);
        require!(grant.evaluator == ctx.accounts.evaluator.key(), ScholarError::UnauthorizedEvaluator);
        require!(milestone_index == grant.current_milestone, ScholarError::InvalidMilestoneOrder);

        let remaining_milestones = (grant.milestones_count - grant.current_milestone) as u64;
        let payout_amount = if remaining_milestones == 1 {
            grant.amount_remaining
        } else {
            grant.amount_remaining / remaining_milestones
        };

        require!(payout_amount > 0, ScholarError::ZeroPayoutCalculated);
        require!(grant.amount_remaining >= payout_amount, ScholarError::InsufficientVaultFunds);

        grant.amount_remaining = grant.amount_remaining.checked_sub(payout_amount).ok_or(ScholarError::MathUnderflow)?;
        grant.current_milestone = grant.current_milestone.checked_add(1).ok_or(ScholarError::MathOverflow)?;

        if grant.current_milestone >= grant.milestones_count {
            grant.status = GrantStatus::Completed;
        }

        // Execute CPI transfer from PDA escrow vault directly to student
        let grant_id_bytes = grant.grant_id.to_le_bytes();
        let seeds = &[
            b"scholar_vault".as_ref(),
            grant_id_bytes.as_ref(),
            &[grant.vault_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        let cpi_context = CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.student.to_account_info(),
            },
            signer_seeds,
        );
        transfer(cpi_context, payout_amount)?;

        emit!(MilestoneApprovedEvent {
            grant_id: grant.grant_id,
            milestone_index,
            payout_amount,
            student: grant.student,
            is_completed: grant.status == GrantStatus::Completed,
        });

        Ok(())
    }

    /// Allows sponsor to cancel grant and reclaim remaining unreleased funds in case of student default
    pub fn refund_cancelled_grant(ctx: Context<RefundGrant>) -> Result<()> {
        let grant = &mut ctx.accounts.grant_account;
        require!(grant.sponsor == ctx.accounts.sponsor.key(), ScholarError::UnauthorizedSponsor);
        require!(grant.status == GrantStatus::Active, ScholarError::GrantInactive);

        let refund_amount = grant.amount_remaining;
        require!(refund_amount > 0, ScholarError::ZeroAmount);

        grant.amount_remaining = 0;
        grant.status = GrantStatus::Cancelled;

        let grant_id_bytes = grant.grant_id.to_le_bytes();
        let seeds = &[
            b"scholar_vault".as_ref(),
            grant_id_bytes.as_ref(),
            &[grant.vault_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        let cpi_context = CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.sponsor.to_account_info(),
            },
            signer_seeds,
        );
        transfer(cpi_context, refund_amount)?;

        emit!(GrantCancelledEvent {
            grant_id: grant.grant_id,
            refund_amount,
            sponsor: grant.sponsor,
        });

        Ok(())
    }
}

// -----------------------------------------------------------------------------
// ACCOUNT VALIDATION CONTEXTS
// -----------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(grant_id: u64)]
pub struct InitializeGrant<'info> {
    #[account(
        init,
        payer = sponsor,
        space = GrantAccount::LEN,
        seeds = [b"scholar_grant", grant_id.to_le_bytes().as_ref()],
        bump
    )]
    pub grant_account: Account<'info, GrantAccount>,

    /// CHECK: PDA derived vault for holding locked grant SOL safely
    #[account(
        mut,
        seeds = [b"scholar_vault", grant_id.to_le_bytes().as_ref()],
        bump
    )]
    pub vault: SystemAccount<'info>,

    #[account(mut)]
    pub sponsor: Signer<'info>,

    /// CHECK: Safe recipient student public key
    pub student: SystemAccount<'info>,

    /// CHECK: Safe authorized evaluator public key (Dean, Faculty Advisor, Judge)
    pub evaluator: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SubmitMilestone<'info> {
    #[account(
        mut,
        seeds = [b"scholar_grant", grant_account.grant_id.to_le_bytes().as_ref()],
        bump = grant_account.bump,
        has_one = student @ ScholarError::UnauthorizedStudent
    )]
    pub grant_account: Account<'info, GrantAccount>,

    pub student: Signer<'info>,
}

#[derive(Accounts)]
pub struct ApproveMilestone<'info> {
    #[account(
        mut,
        seeds = [b"scholar_grant", grant_account.grant_id.to_le_bytes().as_ref()],
        bump = grant_account.bump,
        has_one = evaluator @ ScholarError::UnauthorizedEvaluator,
        has_one = student @ ScholarError::StudentAccountMismatch
    )]
    pub grant_account: Account<'info, GrantAccount>,

    /// CHECK: PDA derived vault account
    #[account(
        mut,
        seeds = [b"scholar_vault", grant_account.grant_id.to_le_bytes().as_ref()],
        bump = grant_account.vault_bump
    )]
    pub vault: SystemAccount<'info>,

    pub evaluator: Signer<'info>,

    /// CHECK: Recipient student account verified by has_one
    #[account(mut)]
    pub student: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RefundGrant<'info> {
    #[account(
        mut,
        seeds = [b"scholar_grant", grant_account.grant_id.to_le_bytes().as_ref()],
        bump = grant_account.bump,
        has_one = sponsor @ ScholarError::UnauthorizedSponsor
    )]
    pub grant_account: Account<'info, GrantAccount>,

    /// CHECK: PDA derived vault account
    #[account(
        mut,
        seeds = [b"scholar_vault", grant_account.grant_id.to_le_bytes().as_ref()],
        bump = grant_account.vault_bump
    )]
    pub vault: SystemAccount<'info>,

    #[account(mut)]
    pub sponsor: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// -----------------------------------------------------------------------------
// DATA STRUCTURES & ERROR CODES
// -----------------------------------------------------------------------------

#[account]
pub struct GrantAccount {
    pub sponsor: Pubkey,          // 32
    pub student: Pubkey,          // 32
    pub evaluator: Pubkey,        // 32
    pub grant_id: u64,            // 8
    pub total_amount: u64,        // 8
    pub amount_remaining: u64,    // 8
    pub milestones_count: u8,     // 1
    pub current_milestone: u8,    // 1
    pub status: GrantStatus,      // 1
    pub bump: u8,                 // 1
    pub vault_bump: u8,           // 1
    pub metadata_uri: String,     // 4 + 200 = 204
}

impl GrantAccount {
    pub const LEN: usize = 8 + 32 + 32 + 32 + 8 + 8 + 8 + 1 + 1 + 1 + 1 + 1 + 204 + 64; // Safe pad
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum GrantStatus {
    Active,
    Completed,
    Cancelled,
}

#[error_code]
pub enum ScholarError {
    #[msg("Milestone count must be greater than zero.")]
    ZeroMilestones,
    #[msg("Exceeded maximum allowed milestones (10).")]
    TooManyMilestones,
    #[msg("Total grant funding amount must be greater than zero.")]
    ZeroAmount,
    #[msg("Metadata URI string exceeds 200 characters limit.")]
    MetadataUriTooLong,
    #[msg("Grant is not currently in active state.")]
    GrantInactive,
    #[msg("Only the designated student can execute this instruction.")]
    UnauthorizedStudent,
    #[msg("Only the designated faculty evaluator can approve milestones.")]
    UnauthorizedEvaluator,
    #[msg("Only the original sponsor can execute a refund on an active grant.")]
    UnauthorizedSponsor,
    #[msg("Submitted milestone index does not match next sequential milestone.")]
    InvalidMilestoneOrder,
    #[msg("Math underflow error during balance recalculation.")]
    MathUnderflow,
    #[msg("Math overflow error during milestone count increment.")]
    MathOverflow,
    #[msg("Calculated payout per milestone evaluated to 0 lamports.")]
    ZeroPayoutCalculated,
    #[msg("PDA Vault does not contain sufficient funds for milestone release.")]
    InsufficientVaultFunds,
    #[msg("Account student mismatch with grant record definition.")]
    StudentAccountMismatch,
}

// -----------------------------------------------------------------------------
// EVENT DEFINITIONS FOR OFF-CHAIN INDEXING
// -----------------------------------------------------------------------------

#[event]
pub struct GrantInitializedEvent {
    pub grant_id: u64,
    pub sponsor: Pubkey,
    pub student: Pubkey,
    pub total_amount: u64,
    pub milestones_count: u8,
}

#[event]
pub struct MilestoneSubmittedEvent {
    pub grant_id: u64,
    pub milestone_index: u8,
    pub proof_hash: [u8; 32],
    pub student: Pubkey,
}

#[event]
pub struct MilestoneApprovedEvent {
    pub grant_id: u64,
    pub milestone_index: u8,
    pub payout_amount: u64,
    pub student: Pubkey,
    pub is_completed: bool,
}

#[event]
pub struct GrantCancelledEvent {
    pub grant_id: u64,
    pub refund_amount: u64,
    pub sponsor: Pubkey,
}