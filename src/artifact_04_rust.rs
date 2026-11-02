use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

declare_id!("CPUsP84W9k2vQ9f2n6hR4K8yT7B1gUvGk6yK3w2m5zVw");

pub const MAX_TITLE_LEN: usize = 64;
pub const MAX_URI_LEN: usize = 128;
pub const GRANT_SEED_PREFIX: &[u8] = b"grant";
pub const VAULT_SEED_PREFIX: &[u8] = b"vault";

#[program]
pub mod campusflow {
    use super::*;

    /// Initialize a new educational grant escrow with milestone requirements
    pub fn initialize_grant(
        ctx: Context<InitializeGrant>,
        grant_id: u64,
        title: String,
        amount: u64,
        deadline: i64,
        milestone_hash: [u8; 32],
    ) -> Result<()> {
        require!(title.len() <= MAX_TITLE_LEN, CampusFlowError::StringTooLong);
        require!(amount > 0, CampusFlowError::InvalidAmount);
        
        let clock = Clock::get()?;
        require!(deadline > clock.unix_timestamp, CampusFlowError::InvalidDeadline);

        let grant = &mut ctx.accounts.grant;
        grant.sponsor = ctx.accounts.sponsor.key();
        grant.recipient = ctx.accounts.recipient.key();
        grant.grant_id = grant_id;
        grant.title = title;
        grant.amount = amount;
        grant.deadline = deadline;
        grant.milestone_hash = milestone_hash;
        grant.state = GrantState::Initialized;
        grant.proof_uri = String::new();
        grant.bump = ctx.bumps.grant;
        grant.vault_bump = ctx.bumps.vault;

        // Escrow funds: Transfer native SOL from sponsor to the Vault PDA
        let cpi_context = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.sponsor.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
            },
        );
        transfer(cpi_context, amount)?;

        emit!(GrantCreatedEvent {
            grant_id,
            sponsor: ctx.accounts.sponsor.key(),
            recipient: ctx.accounts.recipient.key(),
            amount,
            deadline,
        });

        Ok(())
    }

    /// Recipient submits verifiable milestone proof (IPFS/Arweave URI + SHA256 proof hash)
    pub fn submit_milestone(
        ctx: Context<SubmitMilestone>,
        proof_uri: String,
        submitted_hash: [u8; 32],
    ) -> Result<()> {
        require!(proof_uri.len() <= MAX_URI_LEN, CampusFlowError::StringTooLong);

        let grant = &mut ctx.accounts.grant;
        require!(grant.state == GrantState::Initialized, CampusFlowError::InvalidState);

        let clock = Clock::get()?;
        require!(clock.unix_timestamp <= grant.deadline, CampusFlowError::GrantExpired);

        grant.proof_uri = proof_uri.clone();
        grant.state = GrantState::MilestonePending;

        emit!(MilestoneSubmittedEvent {
            grant_id: grant.grant_id,
            recipient: grant.recipient,
            proof_uri,
            submitted_hash,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Sponsor reviews and approves milestone, triggering trustless vault release to student
    pub fn verify_and_disburse(ctx: Context<VerifyAndDisburse>) -> Result<()> {
        let grant = &mut ctx.accounts.grant;
        require!(grant.state == GrantState::MilestonePending, CampusFlowError::InvalidState);

        let grant_key = grant.key();
        let seeds: &[&[&[u8]]] = &[&[
            VAULT_SEED_PREFIX,
            grant_key.as_ref(),
            &[grant.vault_bump],
        ]];

        let disbursement_amount = grant.amount;

        // Safe transfer native SOL out of Vault PDA to Recipient
        **ctx.accounts.vault.to_account_info().try_borrow_mut_lamports()? = ctx
            .accounts
            .vault
            .to_account_info()
            .lamports()
            .checked_sub(disbursement_amount)
            .ok_or(CampusFlowError::NumericalUnderflow)?;

        **ctx.accounts.recipient.to_account_info().try_borrow_mut_lamports()? = ctx
            .accounts
            .recipient
            .to_account_info()
            .lamports()
            .checked_add(disbursement_amount)
            .ok_or(CampusFlowError::NumericalOverflow)?;

        grant.state = GrantState::Completed;

        emit!(GrantDisbursedEvent {
            grant_id: grant.grant_id,
            recipient: grant.recipient,
            amount: disbursement_amount,
            completed_at: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Sponsor refunds capital if the grant deadline has lapsed without verified completion
    pub fn cancel_grant(ctx: Context<CancelGrant>) -> Result<()> {
        let grant = &mut ctx.accounts.grant;
        require!(
            grant.state == GrantState::Initialized || grant.state == GrantState::MilestonePending,
            CampusFlowError::InvalidState
        );

        let clock = Clock::get()?;
        require!(clock.unix_timestamp > grant.deadline, CampusFlowError::DeadlineNotMet);

        let refund_amount = ctx.accounts.vault.to_account_info().lamports();
        require!(refund_amount > 0, CampusFlowError::VaultEmpty);

        **ctx.accounts.vault.to_account_info().try_borrow_mut_lamports()? = 0;
        **ctx.accounts.sponsor.to_account_info().try_borrow_mut_lamports()? = ctx
            .accounts
            .sponsor
            .to_account_info()
            .lamports()
            .checked_add(refund_amount)
            .ok_or(CampusFlowError::NumericalOverflow)?;

        grant.state = GrantState::Refunded;

        emit!(GrantCancelledEvent {
            grant_id: grant.grant_id,
            sponsor: grant.sponsor,
            refund_amount,
            cancelled_at: clock.unix_timestamp,
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
    #[account(mut)]
    pub sponsor: Signer<'info>,

    /// CHECK: The recipient of the educational stipend
    pub recipient: AccountInfo<'info>,

    #[account(
        init,
        payer = sponsor,
        space = 8 + GrantAccount::INIT_SPACE,
        seeds = [GRANT_SEED_PREFIX, sponsor.key().as_ref(), &grant_id.to_le_bytes()],
        bump
    )]
    pub grant: Account<'info, GrantAccount>,

    /// CHECK: PDA acting as the Escrow SOL Vault
    #[account(
        mut,
        seeds = [VAULT_SEED_PREFIX, grant.key().as_ref()],
        bump
    )]
    pub vault: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SubmitMilestone<'info> {
    #[account(
        mut,
        has_one = recipient @ CampusFlowError::UnauthorizedRecipient
    )]
    pub grant: Account<'info, GrantAccount>,

    pub recipient: Signer<'info>,
}

#[derive(Accounts)]
pub struct VerifyAndDisburse<'info> {
    #[account(
        mut,
        has_one = sponsor @ CampusFlowError::UnauthorizedSponsor,
        has_one = recipient @ CampusFlowError::UnauthorizedRecipient
    )]
    pub grant: Account<'info, GrantAccount>,

    pub sponsor: Signer<'info>,

    /// CHECK: Verified recipient matching grant.recipient
    #[account(mut)]
    pub recipient: AccountInfo<'info>,

    /// CHECK: PDA Vault containing funds
    #[account(
        mut,
        seeds = [VAULT_SEED_PREFIX, grant.key().as_ref()],
        bump = grant.vault_bump
    )]
    pub vault: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CancelGrant<'info> {
    #[account(
        mut,
        has_one = sponsor @ CampusFlowError::UnauthorizedSponsor
    )]
    pub grant: Account<'info, GrantAccount>,

    #[account(mut)]
    pub sponsor: Signer<'info>,

    /// CHECK: Vault account holding funds to refund
    #[account(
        mut,
        seeds = [VAULT_SEED_PREFIX, grant.key().as_ref()],
        bump = grant.vault_bump
    )]
    pub vault: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

// -----------------------------------------------------------------------------
// DATA STATE STRUCTURES
// -----------------------------------------------------------------------------

#[account]
#[derive(InitSpace)]
pub struct GrantAccount {
    pub sponsor: Pubkey,            // 32
    pub recipient: Pubkey,          // 32
    pub grant_id: u64,              // 8
    #[max_len(64)]
    pub title: String,              // 4 + 64
    pub amount: u64,                // 8
    pub deadline: i64,              // 8
    pub milestone_hash: [u8; 32],   // 32
    pub state: GrantState,          // 1 + 1
    #[max_len(128)]
    pub proof_uri: String,          // 4 + 128
    pub bump: u8,                   // 1
    pub vault_bump: u8,             // 1
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum GrantState {
    Initialized,
    MilestonePending,
    Completed,
    Refunded,
}

// -----------------------------------------------------------------------------
// EVENTS
// -----------------------------------------------------------------------------

#[event]
pub struct GrantCreatedEvent {
    pub grant_id: u64,
    pub sponsor: Pubkey,
    pub recipient: Pubkey,
    pub amount: u64,
    pub deadline: i64,
}

#[event]
pub struct MilestoneSubmittedEvent {
    pub grant_id: u64,
    pub recipient: Pubkey,
    pub proof_uri: String,
    pub submitted_hash: [u8; 32],
    pub timestamp: i64,
}

#[event]
pub struct GrantDisbursedEvent {
    pub grant_id: u64,
    pub recipient: Pubkey,
    pub amount: u64,
    pub completed_at: i64,
}

#[event]
pub struct GrantCancelledEvent {
    pub grant_id: u64,
    pub sponsor: Pubkey,
    pub refund_amount: u64,
    pub cancelled_at: i64,
}

// -----------------------------------------------------------------------------
// ERROR CODES
// -----------------------------------------------------------------------------

#[error_code]
pub enum CampusFlowError {
    #[msg("String exceeds maximum allowable length.")]
    StringTooLong,
    #[msg("Amount must be greater than zero.")]
    InvalidAmount,
    #[msg("Deadline must be set in the future.")]
    InvalidDeadline,
    #[msg("Grant is not in the required state for this instruction.")]
    InvalidState,
    #[msg("Grant deadline has expired.")]
    GrantExpired,
    #[msg("Grant deadline has not passed yet.")]
    DeadlineNotMet,
    #[msg("Caller is not the designated recipient.")]
    UnauthorizedRecipient,
    #[msg("Caller is not the designated sponsor.")]
    UnauthorizedSponsor,
    #[msg("Arithmetic underflow occurred.")]
    NumericalUnderflow,
    #[msg("Arithmetic overflow occurred.")]
    NumericalOverflow,
    #[msg("Escrow vault has zero balance.")]
    VaultEmpty,
}