use anchor_lang::prelude::*;
use anchor_lang::system_program;

declare_id!("V1beGrant1111111111111111111111111111111111");

#[program]
pub mod aegis_edu {
    use super::*;

    pub fn initialize_grant(
        ctx: Context<InitializeGrant>,
        title: String,
        total_milestones: u8,
        total_budget: u64,
    ) -> Result<()> {
        require!(title.len() <= 64, AegisError::TitleTooLong);
        require!(total_milestones > 0, AegisError::ZeroMilestones);
        require!(total_budget > 0, AegisError::ZeroBudget);

        let grant = &mut ctx.accounts.grant;
        grant.authority = ctx.accounts.authority.key();
        grant.recipient = ctx.accounts.recipient.key();
        grant.total_budget = total_budget;
        grant.funded_amount = 0;
        grant.released_amount = 0;
        grant.total_milestones = total_milestones;
        grant.completed_milestones = 0;
        grant.title = title;
        grant.is_active = true;
        grant.bump = ctx.bumps.grant;
        grant.vault_bump = ctx.bumps.vault;

        emit!(GrantInitialized {
            grant: grant.key(),
            authority: grant.authority,
            recipient: grant.recipient,
            total_budget,
            total_milestones,
        });

        Ok(())
    }

    pub fn fund_grant(ctx: Context<FundGrant>, amount: u64) -> Result<()> {
        require!(amount > 0, AegisError::ZeroFundingAmount);
        let grant = &mut ctx.accounts.grant;
        require!(grant.is_active, AegisError::GrantInactive);

        let new_funded = grant
            .funded_amount
            .checked_add(amount)
            .ok_or(AegisError::NumericalOverflow)?;
        require!(new_funded <= grant.total_budget, AegisError::FundingExceedsBudget);

        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.funder.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                },
            ),
            amount,
        )?;

        grant.funded_amount = new_funded;

        emit!(GrantFunded {
            grant: grant.key(),
            funder: ctx.accounts.funder.key(),
            amount,
            total_funded: grant.funded_amount,
        });

        Ok(())
    }

    pub fn submit_milestone(
        ctx: Context<SubmitMilestone>,
        milestone_index: u8,
        proof_hash: [u8; 32],
    ) -> Result<()> {
        let grant = &ctx.accounts.grant;
        require!(grant.is_active, AegisError::GrantInactive);
        require!(
            milestone_index == grant.completed_milestones,
            AegisError::InvalidMilestoneOrder
        );

        let milestone = &mut ctx.accounts.milestone;
        milestone.grant = grant.key();
        milestone.index = milestone_index;
        milestone.proof_hash = proof_hash;
        milestone.is_completed = false;
        milestone.bump = ctx.bumps.milestone;

        emit!(MilestoneSubmitted {
            grant: grant.key(),
            milestone_index,
            proof_hash,
        });

        Ok(())
    }

    pub fn approve_milestone(ctx: Context<ApproveMilestone>, milestone_index: u8) -> Result<()> {
        let grant = &mut ctx.accounts.grant;
        require!(grant.is_active, AegisError::GrantInactive);
        require!(
            milestone_index == grant.completed_milestones,
            AegisError::InvalidMilestoneOrder
        );

        let milestone = &mut ctx.accounts.milestone;
        require!(!milestone.is_completed, AegisError::MilestoneAlreadyApproved);

        let tranche_amount = grant
            .total_budget
            .checked_div(grant.total_milestones as u64)
            .ok_or(AegisError::NumericalOverflow)?;

        let current_vault_balance = ctx.accounts.vault.lamports();
        require!(
            current_vault_balance >= tranche_amount,
            AegisError::InsufficientVaultBalance
        );

        let grant_key = grant.key();
        let signer_seeds: &[&[&[u8]]] = &[&[
            b"vault",
            grant_key.as_ref(),
            &[grant.vault_bump],
        ]];

        system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.recipient.to_account_info(),
                },
                signer_seeds,
            ),
            tranche_amount,
        )?;

        milestone.is_completed = true;
        grant.completed_milestones = grant
            .completed_milestones
            .checked_add(1)
            .ok_or(AegisError::NumericalOverflow)?;
        grant.released_amount = grant
            .released_amount
            .checked_add(tranche_amount)
            .ok_or(AegisError::NumericalOverflow)?;

        if grant.completed_milestones == grant.total_milestones {
            grant.is_active = false;
        }

        emit!(MilestoneApproved {
            grant: grant.key(),
            milestone_index,
            payout_amount: tranche_amount,
            recipient: ctx.accounts.recipient.key(),
        });

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(title: String, total_milestones: u8, total_budget: u64)]
pub struct InitializeGrant<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    /// CHECK: Recipient is arbitrary student account validated off-chain
    pub recipient: AccountInfo<'info>,
    #[account(
        init,
        payer = authority,
        space = Grant::LEN,
        seeds = [b"grant", authority.key().as_ref(), title.as_bytes()],
        bump
    )]
    pub grant: Account<'info, Grant>,
    #[account(
        mut,
        seeds = [b"vault", grant.key().as_ref()],
        bump
    )]
    pub vault: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FundGrant<'info> {
    #[account(mut)]
    pub funder: Signer<'info>,
    #[account(
        mut,
        seeds = [b"grant", grant.authority.as_ref(), grant.title.as_bytes()],
        bump = grant.bump
    )]
    pub grant: Account<'info, Grant>,
    #[account(
        mut,
        seeds = [b"vault", grant.key().as_ref()],
        bump = grant.vault_bump
    )]
    pub vault: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(milestone_index: u8)]
pub struct SubmitMilestone<'info> {
    #[account(mut, address = grant.recipient @ AegisError::UnauthorizedRecipient)]
    pub submitter: Signer<'info>,
    #[account(
        mut,
        seeds = [b"grant", grant.authority.as_ref(), grant.title.as_bytes()],
        bump = grant.bump
    )]
    pub grant: Account<'info, Grant>,
    #[account(
        init,
        payer = submitter,
        space = Milestone::LEN,
        seeds = [b"milestone", grant.key().as_ref(), &[milestone_index]],
        bump
    )]
    pub milestone: Account<'info, Milestone>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(milestone_index: u8)]
pub struct ApproveMilestone<'info> {
    #[account(mut, address = grant.authority @ AegisError::UnauthorizedAuthority)]
    pub authority: Signer<'info>,
    /// CHECK: Target student recipient verified matching stored grant key
    #[account(mut, address = grant.recipient @ AegisError::UnauthorizedRecipient)]
    pub recipient: AccountInfo<'info>,
    #[account(
        mut,
        seeds = [b"grant", grant.authority.as_ref(), grant.title.as_bytes()],
        bump = grant.bump
    )]
    pub grant: Account<'info, Grant>,
    #[account(
        mut,
        seeds = [b"milestone", grant.key().as_ref(), &[milestone_index]],
        bump = milestone.bump
    )]
    pub milestone: Account<'info, Milestone>,
    #[account(
        mut,
        seeds = [b"vault", grant.key().as_ref()],
        bump = grant.vault_bump
    )]
    pub vault: SystemAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct Grant {
    pub authority: Pubkey,
    pub recipient: Pubkey,
    pub total_budget: u64,
    pub funded_amount: u64,
    pub released_amount: u64,
    pub total_milestones: u8,
    pub completed_milestones: u8,
    pub is_active: bool,
    pub bump: u8,
    pub vault_bump: u8,
    pub title: String,
}

impl Grant {
    pub const LEN: usize = 8 + 32 + 32 + 8 + 8 + 8 + 1 + 1 + 1 + 1 + 1 + (4 + 64);
}

#[account]
pub struct Milestone {
    pub grant: Pubkey,
    pub index: u8,
    pub proof_hash: [u8; 32],
    pub is_completed: bool,
    pub bump: u8,
}

impl Milestone {
    pub const LEN: usize = 8 + 32 + 1 + 32 + 1 + 1;
}

#[event]
pub struct GrantInitialized {
    pub grant: Pubkey,
    pub authority: Pubkey,
    pub recipient: Pubkey,
    pub total_budget: u64,
    pub total_milestones: u8,
}

#[event]
pub struct GrantFunded {
    pub grant: Pubkey,
    pub funder: Pubkey,
    pub amount: u64,
    pub total_funded: u64,
}

#[event]
pub struct MilestoneSubmitted {
    pub grant: Pubkey,
    pub milestone_index: u8,
    pub proof_hash: [u8; 32],
}

#[event]
pub struct MilestoneApproved {
    pub grant: Pubkey,
    pub milestone_index: u8,
    pub payout_amount: u64,
    pub recipient: Pubkey,
}

#[error_code]
pub enum AegisError {
    #[msg("Grant title exceeds 64 bytes")]
    TitleTooLong,
    #[msg("Total milestones must be greater than zero")]
    ZeroMilestones,
    #[msg("Total budget must be greater than zero")]
    ZeroBudget,
    #[msg("Funding amount must be greater than zero")]
    ZeroFundingAmount,
    #[msg("Grant is not currently active")]
    GrantInactive,
    #[msg("Funding exceeds allocated total budget")]
    FundingExceedsBudget,
    #[msg("Milestone submitted out of sequential order")]
    InvalidMilestoneOrder,
    #[msg("Milestone has already been approved and paid")]
    MilestoneAlreadyApproved,
    #[msg("Arithmetic calculation resulted in an overflow")]
    NumericalOverflow,
    #[msg("Vault does not hold sufficient lamports for tranche release")]
    InsufficientVaultBalance,
    #[msg("Caller is unauthorized to approve this grant")]
    UnauthorizedAuthority,
    #[msg("Caller or recipient does not match grant student address")]
    UnauthorizedRecipient,
}