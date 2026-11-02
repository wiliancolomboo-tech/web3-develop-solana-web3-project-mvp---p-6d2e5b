use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

declare_id!("EduVibE1111111111111111111111111111111111111");

pub const MAX_MILESTONES: usize = 5;
pub const MAX_TITLE_LEN: usize = 64;
pub const MAX_URI_LEN: usize = 128;

#[program]
pub mod eduvibe_protocol {
    use super::*;

    /// Initializes a new educational project grant with segregated milestone budget tranches.
    pub fn initialize_grant(
        ctx: Context<InitializeGrant>,
        grant_id: u64,
        title: String,
        milestone_amounts: Vec<u64>,
    ) -> Result<()> {
        require!(
            !title.is_empty() && title.len() <= MAX_TITLE_LEN,
            EduVibeError::InvalidTitleLength
        );
        require!(
            !milestone_amounts.is_empty() && milestone_amounts.len() <= MAX_MILESTONES,
            EduVibeError::InvalidMilestoneCount
        );

        let total_required: u64 = milestone_amounts.iter().try_fold(0u64, |acc, &amt| {
            require!(amt > 0, EduVibeError::ZeroMilestoneAmount);
            acc.checked_add(amt).ok_or(EduVibeError::ArithmeticOverflow)
        })?;

        let grant = &mut ctx.accounts.grant_account;
        grant.grant_id = grant_id;
        grant.grantor = ctx.accounts.grantor.key();
        grant.supervisor = ctx.accounts.supervisor.key();
        grant.student = ctx.accounts.student.key();
        grant.vault_bump = ctx.bumps.vault;
        grant.grant_bump = ctx.bumps.grant_account;
        grant.total_budget = total_required;
        grant.claimed_budget = 0;
        grant.current_milestone_index = 0;
        grant.title = title;
        grant.is_active = true;

        for (i, amt) in milestone_amounts.iter().enumerate() {
            grant.milestones[i] = MilestoneItem {
                amount: *amt,
                status: MilestoneStatus::Pending,
                proof_uri: [0u8; MAX_URI_LEN],
                proof_len: 0,
            };
        }
        grant.milestone_count = milestone_amounts.len() as u8;

        // Escrow transfer of total budget into PDA Vault
        let cpi_context = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.grantor.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
            },
        );
        transfer(cpi_context, total_required)?;

        emit!(GrantCreatedEvent {
            grant_id,
            grantor: grant.grantor,
            supervisor: grant.supervisor,
            student: grant.student,
            total_budget: total_required,
            milestones: grant.milestone_count,
        });

        Ok(())
    }

    /// Student submits verifiable IPFS/Arweave deliverable proof for the active milestone.
    pub fn submit_milestone_proof(
        ctx: Context<SubmitMilestone>,
        grant_id: u64,
        proof_uri: String,
    ) -> Result<()> {
        let grant = &mut ctx.accounts.grant_account;
        require!(grant.is_active, EduVibeError::GrantInactive);
        require!(
            grant.student == ctx.accounts.student.key(),
            EduVibeError::UnauthorizedStudent
        );

        let curr_idx = grant.current_milestone_index as usize;
        require!(
            curr_idx < grant.milestone_count as usize,
            EduVibeError::AllMilestonesCompleted
        );

        let milestone = &mut grant.milestones[curr_idx];
        require!(
            milestone.status == MilestoneStatus::Pending,
            EduVibeError::InvalidMilestoneStatus
        );

        let uri_bytes = proof_uri.as_bytes();
        require!(
            !uri_bytes.is_empty() && uri_bytes.len() <= MAX_URI_LEN,
            EduVibeError::InvalidProofUri
        );

        milestone.proof_uri[..uri_bytes.len()].copy_from_slice(uri_bytes);
        milestone.proof_len = uri_bytes.len() as u8;
        milestone.status = MilestoneStatus::Submitted;

        emit!(MilestoneSubmittedEvent {
            grant_id,
            milestone_index: curr_idx as u8,
            proof_uri,
        });

        Ok(())
    }

    /// Supervisor verifies deliverable and cryptographically triggers PDA escrow disbursement.
    pub fn approve_and_disburse(ctx: Context<ApproveMilestone>, grant_id: u64) -> Result<()> {
        let grant = &mut ctx.accounts.grant_account;
        require!(grant.is_active, EduVibeError::GrantInactive);
        require!(
            grant.supervisor == ctx.accounts.supervisor.key(),
            EduVibeError::UnauthorizedSupervisor
        );

        let curr_idx = grant.current_milestone_index as usize;
        require!(
            curr_idx < grant.milestone_count as usize,
            EduVibeError::AllMilestonesCompleted
        );

        let payout_amount: u64;
        {
            let milestone = &mut grant.milestones[curr_idx];
            require!(
                milestone.status == MilestoneStatus::Submitted,
                EduVibeError::MilestoneNotSubmitted
            );

            milestone.status = MilestoneStatus::Approved;
            payout_amount = milestone.amount;
        }

        // Advance state
        grant.claimed_budget = grant
            .claimed_budget
            .checked_add(payout_amount)
            .ok_or(EduVibeError::ArithmeticOverflow)?;
        grant.current_milestone_index = grant
            .current_milestone_index
            .checked_add(1)
            .ok_or(EduVibeError::ArithmeticOverflow)?;

        if grant.current_milestone_index == grant.milestone_count {
            grant.is_active = false;
        }

        // PDA signer seeds for program balance disbursement
        let grant_id_bytes = grant_id.to_le_bytes();
        let signer_seeds: &[&[&[u8]]] = &[&[
            b"eduvibe_vault",
            grant_id_bytes.as_ref(),
            &[grant.vault_bump],
        ]];

        **ctx.accounts.vault.try_borrow_mut_lamports()? = ctx
            .accounts
            .vault
            .lamports()
            .checked_sub(payout_amount)
            .ok_or(EduVibeError::InsufficientVaultBalance)?;

        **ctx.accounts.student.try_borrow_mut_lamports()? = ctx
            .accounts
            .student
            .lamports()
            .checked_add(payout_amount)
            .ok_or(EduVibeError::ArithmeticOverflow)?;

        emit!(MilestoneReleasedEvent {
            grant_id,
            milestone_index: curr_idx as u8,
            recipient: ctx.accounts.student.key(),
            disbursed_amount: payout_amount,
        });

        Ok(())
    }

    /// Emergency/Dispute cancellation: allows grantor to reclaim unspent escrow after faculty cancellation.
    pub fn cancel_and_clawback(ctx: Context<CancelGrant>, grant_id: u64) -> Result<()> {
        let grant = &mut ctx.accounts.grant_account;
        require!(grant.is_active, EduVibeError::GrantInactive);
        require!(
            grant.grantor == ctx.accounts.grantor.key(),
            EduVibeError::UnauthorizedGrantor
        );

        let unspent_amount = ctx.accounts.vault.lamports();
        require!(unspent_amount > 0, EduVibeError::NoFundsRemaining);

        grant.is_active = false;

        **ctx.accounts.vault.try_borrow_mut_lamports()? = 0;
        **ctx.accounts.grantor.try_borrow_mut_lamports()? = ctx
            .accounts
            .grantor
            .lamports()
            .checked_add(unspent_amount)
            .ok_or(EduVibeError::ArithmeticOverflow)?;

        emit!(GrantCancelledEvent {
            grant_id,
            refund_amount: unspent_amount,
        });

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Account Structures & Contexts
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(grant_id: u64)]
pub struct InitializeGrant<'info> {
    #[account(
        init,
        payer = grantor,
        space = 8 + GrantState::SPACE,
        seeds = [b"eduvibe_grant", grant_id.to_le_bytes().as_ref()],
        bump
    )]
    pub grant_account: Account<'info, GrantState>,

    /// CHECK: PDA Program Vault holding escrowed SOL
    #[account(
        mut,
        seeds = [b"eduvibe_vault", grant_id.to_le_bytes().as_ref()],
        bump
    )]
    pub vault: SystemAccount<'info>,

    #[account(mut)]
    pub grantor: Signer<'info>,

    /// CHECK: Supervisor public key for milestone signoff
    pub supervisor: SystemAccount<'info>,

    /// CHECK: Student public key who receives disbursed funds
    pub student: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(grant_id: u64)]
pub struct SubmitMilestone<'info> {
    #[account(
        mut,
        seeds = [b"eduvibe_grant", grant_id.to_le_bytes().as_ref()],
        bump = grant_account.grant_bump,
    )]
    pub grant_account: Account<'info, GrantState>,

    pub student: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(grant_id: u64)]
pub struct ApproveMilestone<'info> {
    #[account(
        mut,
        seeds = [b"eduvibe_grant", grant_id.to_le_bytes().as_ref()],
        bump = grant_account.grant_bump,
    )]
    pub grant_account: Account<'info, GrantState>,

    /// CHECK: PDA Program Vault
    #[account(
        mut,
        seeds = [b"eduvibe_vault", grant_id.to_le_bytes().as_ref()],
        bump = grant_account.vault_bump,
    )]
    pub vault: SystemAccount<'info>,

    pub supervisor: Signer<'info>,

    /// CHECK: Student target wallet matching grant_account.student
    #[account(
        mut,
        constraint = student.key() == grant_account.student @ EduVibeError::UnauthorizedStudent
    )]
    pub student: SystemAccount<'info>,
}

#[derive(Accounts)]
#[instruction(grant_id: u64)]
pub struct CancelGrant<'info> {
    #[account(
        mut,
        seeds = [b"eduvibe_grant", grant_id.to_le_bytes().as_ref()],
        bump = grant_account.grant_bump,
    )]
    pub grant_account: Account<'info, GrantState>,

    /// CHECK: PDA Program Vault
    #[account(
        mut,
        seeds = [b"eduvibe_vault", grant_id.to_le_bytes().as_ref()],
        bump = grant_account.vault_bump,
    )]
    pub vault: SystemAccount<'info>,

    #[account(
        mut,
        constraint = grantor.key() == grant_account.grantor @ EduVibeError::UnauthorizedGrantor
    )]
    pub grantor: Signer<'info>,
}

// ---------------------------------------------------------------------------
// State Representation
// ---------------------------------------------------------------------------

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum MilestoneStatus {
    Pending,
    Submitted,
    Approved,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct MilestoneItem {
    pub amount: u64,
    pub status: MilestoneStatus,
    pub proof_uri: [u8; MAX_URI_LEN],
    pub proof_len: u8,
}

#[account]
pub struct GrantState {
    pub grant_id: u64,
    pub grantor: Pubkey,
    pub supervisor: Pubkey,
    pub student: Pubkey,
    pub vault_bump: u8,
    pub grant_bump: u8,
    pub total_budget: u64,
    pub claimed_budget: u64,
    pub current_milestone_index: u8,
    pub milestone_count: u8,
    pub is_active: bool,
    pub title: String,
    pub milestones: [MilestoneItem; MAX_MILESTONES],
}

impl GrantState {
    // 8 (disc) + 8 (id) + 32*3 (keys) + 1 + 1 (bumps) + 8*2 (budgets) + 1 + 1 (indices) + 1 (bool)
    // + (4 + 64) (string) + 5 * (8 + 1 + 128 + 1) (milestones)
    pub const SPACE: usize = 8 + 32 + 32 + 32 + 1 + 1 + 8 + 8 + 1 + 1 + 1 + (4 + MAX_TITLE_LEN)
        + (MAX_MILESTONES * (8 + 1 + MAX_URI_LEN + 1));
}

// ---------------------------------------------------------------------------
// Events & Errors
// ---------------------------------------------------------------------------

#[event]
pub struct GrantCreatedEvent {
    pub grant_id: u64,
    pub grantor: Pubkey,
    pub supervisor: Pubkey,
    pub student: Pubkey,
    pub total_budget: u64,
    pub milestones: u8,
}

#[event]
pub struct MilestoneSubmittedEvent {
    pub grant_id: u64,
    pub milestone_index: u8,
    pub proof_uri: String,
}

#[event]
pub struct MilestoneReleasedEvent {
    pub grant_id: u64,
    pub milestone_index: u8,
    pub recipient: Pubkey,
    pub disbursed_amount: u64,
}

#[event]
pub struct GrantCancelledEvent {
    pub grant_id: u64,
    pub refund_amount: u64,
}

#[error_code]
pub enum EduVibeError {
    #[msg("Invalid title length: must be between 1 and 64 characters")]
    InvalidTitleLength,
    #[msg("Invalid milestone count: must be between 1 and 5")]
    InvalidMilestoneCount,
    #[msg("Milestone disbursement amount must be greater than zero")]
    ZeroMilestoneAmount,
    #[msg("Arithmetic overflow occurred during calculation")]
    ArithmeticOverflow,
    #[msg("Grant is marked inactive")]
    GrantInactive,
    #[msg("Signer is not authorized as the registered student")]
    UnauthorizedStudent,
    #[msg("Signer is not authorized as the registered supervisor")]
    UnauthorizedSupervisor,
    #[msg("Signer is not authorized as the registered grantor")]
    UnauthorizedGrantor,
    #[msg("All milestones for this grant are already completed")]
    AllMilestonesCompleted,
    #[msg("Milestone is not in Pending status")]
    InvalidMilestoneStatus,
    #[msg("Proof URI length exceeds maximum allowed or is empty")]
    InvalidProofUri,
    #[msg("Milestone has not been submitted for approval yet")]
    MilestoneNotSubmitted,
    #[msg("Insufficient lamports in PDA vault to execute disbursement")]
    InsufficientVaultBalance,
    #[msg("No remaining funds to clawback")]
    NoFundsRemaining,
}