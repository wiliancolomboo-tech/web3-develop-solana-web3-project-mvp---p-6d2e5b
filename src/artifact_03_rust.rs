use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

declare_id!("CampCred11111111111111111111111111111111111");

#[program]
pub mod campus_cred {
    use super::*;

    /// Initializes a university grant pool with a deterministic escrow vault.
    pub fn initialize_grant(
        ctx: Context<InitializeGrant>,
        grant_id: String,
        title: String,
        total_milestones: u8,
        total_payout_lamports: u64,
    ) -> Result<()> {
        require!(grant_id.as_bytes().len() <= 32, CampusCredError::StringTooLong);
        require!(title.as_bytes().len() <= 64, CampusCredError::StringTooLong);
        require!(total_milestones > 0, CampusCredError::InvalidMilestoneCount);
        require!(total_payout_lamports > 0, CampusCredError::InvalidAmount);

        let grant = &mut ctx.accounts.grant;
        grant.authority = ctx.accounts.authority.key();
        grant.grant_id = grant_id;
        grant.title = title;
        grant.total_milestones = total_milestones;
        grant.completed_milestones = 0;
        grant.total_budget = total_payout_lamports;
        grant.remaining_budget = total_payout_lamports;
        grant.is_active = true;
        grant.vault_bump = ctx.bumps.vault;
        grant.grant_bump = ctx.bumps.grant;

        // Deposit initial funding lamports into the Program-controlled escrow vault
        let cpi_context = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.authority.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
            },
        );
        transfer(cpi_context, total_payout_lamports)?;

        emit!(GrantCreatedEvent {
            grant_pubkey: grant.key(),
            authority: grant.authority,
            total_budget: total_payout_lamports,
            total_milestones,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Assigns a student contributor to a grant milestone.
    pub fn submit_milestone(
        ctx: Context<SubmitMilestone>,
        milestone_index: u8,
        submission_uri: String,
        commit_hash: [u8; 32],
    ) -> Result<()> {
        require!(submission_uri.as_bytes().len() <= 128, CampusCredError::StringTooLong);
        let grant = &ctx.accounts.grant;
        require!(grant.is_active, CampusCredError::GrantInactive);
        require!(milestone_index < grant.total_milestones, CampusCredError::InvalidMilestoneIndex);

        let milestone = &mut ctx.accounts.milestone_submission;
        require!(!milestone.is_submitted, CampusCredError::MilestoneAlreadySubmitted);

        milestone.grant = grant.key();
        milestone.student = ctx.accounts.student.key();
        milestone.milestone_index = milestone_index;
        milestone.submission_uri = submission_uri.clone();
        milestone.commit_hash = commit_hash;
        milestone.is_submitted = true;
        milestone.is_approved = false;
        milestone.bump = ctx.bumps.milestone_submission;

        emit!(MilestoneSubmittedEvent {
            grant_pubkey: grant.key(),
            student: ctx.accounts.student.key(),
            milestone_index,
            submission_uri,
            commit_hash,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Evaluator reviews deliverable, triggers automated SOL release and writes Soulbound credential.
    pub fn evaluate_and_payout(
        ctx: Context<EvaluateAndPayout>,
        payout_lamports: u64,
        merit_score: u8,
    ) -> Result<()> {
        let grant = &mut ctx.accounts.grant;
        require!(grant.is_active, CampusCredError::GrantInactive);
        require!(payout_lamports <= grant.remaining_budget, CampusCredError::InsufficientVaultBalance);
        require!(merit_score <= 100, CampusCredError::InvalidScore);

        let milestone = &mut ctx.accounts.milestone_submission;
        require!(milestone.is_submitted, CampusCredError::MilestoneNotSubmitted);
        require!(!milestone.is_approved, CampusCredError::MilestoneAlreadyApproved);

        // State mutation
        milestone.is_approved = true;
        grant.completed_milestones = grant.completed_milestones.checked_add(1).ok_or(CampusCredError::MathOverflow)?;
        grant.remaining_budget = grant.remaining_budget.checked_sub(payout_lamports).ok_or(CampusCredError::MathOverflow)?;

        if grant.completed_milestones == grant.total_milestones {
            grant.is_active = false;
        }

        // Program Derived Address Signer Seeds for native vault transfer
        let grant_key = grant.key();
        let seeds = &[
            b"vault",
            grant_key.as_ref(),
            &[grant.vault_bump],
        ];
        let signer = &[&seeds[..]];

        // Release bounty directly to the student
        let cpi_context = CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.student.to_account_info(),
            },
            signer,
        );
        transfer(cpi_context, payout_lamports)?;

        // Populate Soulbound Attestation record
        let attestation = &mut ctx.accounts.merit_attestation;
        attestation.grant = grant.key();
        attestation.recipient = ctx.accounts.student.key();
        attestation.evaluator = ctx.accounts.authority.key();
        attestation.milestone_index = milestone.milestone_index;
        attestation.merit_score = merit_score;
        attestation.payout_awarded = payout_lamports;
        attestation.timestamp = Clock::get()?.unix_timestamp;
        attestation.bump = ctx.bumps.merit_attestation;

        emit!(MilestoneApprovedEvent {
            grant_pubkey: grant.key(),
            student: ctx.accounts.student.key(),
            milestone_index: milestone.milestone_index,
            payout_lamports,
            merit_score,
            timestamp: attestation.timestamp,
        });

        Ok(())
    }
}

// ---------------------------------------------------------------------------------
// Account Contexts
// ---------------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(grant_id: String)]
pub struct InitializeGrant<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + GrantAccount::INIT_SPACE,
        seeds = [b"grant", authority.key().as_ref(), grant_id.as_bytes()],
        bump
    )]
    pub grant: Account<'info, GrantAccount>,

    /// CHECK: PDA acting as the secure decentralized escrow vault
    #[account(
        mut,
        seeds = [b"vault", grant.key().as_ref()],
        bump
    )]
    pub vault: SystemAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(milestone_index: u8)]
pub struct SubmitMilestone<'info> {
    #[account(mut)]
    pub grant: Account<'info, GrantAccount>,

    #[account(
        init,
        payer = student,
        space = 8 + MilestoneSubmission::INIT_SPACE,
        seeds = [b"milestone", grant.key().as_ref(), &[milestone_index]],
        bump
    )]
    pub milestone_submission: Account<'info, MilestoneSubmission>,

    #[account(mut)]
    pub student: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct EvaluateAndPayout<'info> {
    #[account(
        mut,
        has_one = authority @ CampusCredError::UnauthorizedEvaluator
    )]
    pub grant: Account<'info, GrantAccount>,

    /// CHECK: Validated by PDA seeds in Transfer CPI
    #[account(
        mut,
        seeds = [b"vault", grant.key().as_ref()],
        bump = grant.vault_bump
    )]
    pub vault: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [b"milestone", grant.key().as_ref(), &[milestone_submission.milestone_index]],
        bump = milestone_submission.bump,
        has_one = grant @ CampusCredError::InvalidGrantMilestoneRelation
    )]
    pub milestone_submission: Account<'info, MilestoneSubmission>,

    #[account(
        init,
        payer = authority,
        space = 8 + MeritAttestation::INIT_SPACE,
        seeds = [b"attestation", grant.key().as_ref(), student.key().as_ref(), &[milestone_submission.milestone_index]],
        bump
    )]
    pub merit_attestation: Account<'info, MeritAttestation>,

    /// CHECK: Verified against student key in milestone submission
    #[account(
        mut,
        address = milestone_submission.student @ CampusCredError::StudentKeyMismatch
    )]
    pub student: SystemAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// ---------------------------------------------------------------------------------
// State Schemas
// ---------------------------------------------------------------------------------

#[account]
#[derive(InitSpace)]
pub struct GrantAccount {
    pub authority: Pubkey,
    #[max_len(32)]
    pub grant_id: String,
    #[max_len(64)]
    pub title: String,
    pub total_milestones: u8,
    pub completed_milestones: u8,
    pub total_budget: u64,
    pub remaining_budget: u64,
    pub is_active: bool,
    pub vault_bump: u8,
    pub grant_bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct MilestoneSubmission {
    pub grant: Pubkey,
    pub student: Pubkey,
    pub milestone_index: u8,
    #[max_len(128)]
    pub submission_uri: String,
    pub commit_hash: [u8; 32],
    pub is_submitted: bool,
    pub is_approved: bool,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct MeritAttestation {
    pub grant: Pubkey,
    pub recipient: Pubkey,
    pub evaluator: Pubkey,
    pub milestone_index: u8,
    pub merit_score: u8,
    pub payout_awarded: u64,
    pub timestamp: i64,
    pub bump: u8,
}

// ---------------------------------------------------------------------------------
// Events & Errors
// ---------------------------------------------------------------------------------

#[event]
pub struct GrantCreatedEvent {
    pub grant_pubkey: Pubkey,
    pub authority: Pubkey,
    pub total_budget: u64,
    pub total_milestones: u8,
    pub timestamp: i64,
}

#[event]
pub struct MilestoneSubmittedEvent {
    pub grant_pubkey: Pubkey,
    pub student: Pubkey,
    pub milestone_index: u8,
    pub submission_uri: String,
    pub commit_hash: [u8; 32],
    pub timestamp: i64,
}

#[event]
pub struct MilestoneApprovedEvent {
    pub grant_pubkey: Pubkey,
    pub student: Pubkey,
    pub milestone_index: u8,
    pub payout_lamports: u64,
    pub merit_score: u8,
    pub timestamp: i64,
}

#[error_code]
pub enum CampusCredError {
    #[msg("String parameter exceeds allocated capacity.")]
    StringTooLong,
    #[msg("Milestone count must be greater than zero.")]
    InvalidMilestoneCount,
    #[msg("Initial budget amount must be strictly positive.")]
    InvalidAmount,
    #[msg("Provided milestone index is out of bounds.")]
    InvalidMilestoneIndex,
    #[msg("Grant is not currently active.")]
    GrantInactive,
    #[msg("Milestone has already been submitted.")]
    MilestoneAlreadySubmitted,
    #[msg("Milestone is not marked as submitted.")]
    MilestoneNotSubmitted,
    #[msg("Milestone has already been approved and paid out.")]
    MilestoneAlreadyApproved,
    #[msg("Requested payout exceeds remaining grant vault balance.")]
    InsufficientVaultBalance,
    #[msg("Merit score must be an integer between 0 and 100.")]
    InvalidScore,
    #[msg("Arithmetic overflow occurred during balance recalculation.")]
    MathOverflow,
    #[msg("Unauthorized evaluator for this grant.")]
    UnauthorizedEvaluator,
    #[msg("Milestone submission does not belong to specified grant.")]
    InvalidGrantMilestoneRelation,
    #[msg("Provided student pubkey does not match milestone submitter.")]
    StudentKeyMismatch,
}