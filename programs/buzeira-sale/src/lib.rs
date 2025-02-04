use anchor_lang::prelude::*;
use anchor_lang::solana_program::system_instruction;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

// This is your program's public key and it will update
// automatically when you build the project.
declare_id!("Fgrg9Ft47mgZ3R7fqo4rdBpaxvCdwrjmgYF8FBapuyfm");

#[program]
mod buzeira_sale {
    use super::*;
    pub fn init(
        ctx: Context<Init>,
        admin: Pubkey,
        token_mint: Pubkey,
        sale_duration: u64,
        token_price: u64,
    ) -> Result<()> {
        let now_ts: u64 = Clock::get()?.unix_timestamp as u64;
        ctx.accounts.protocol_status.admin = admin;
        ctx.accounts.protocol_status.token_mint = token_mint;
        ctx.accounts.protocol_status.end_time = sale_duration + now_ts;
        ctx.accounts.protocol_status.token_price = token_price;
        ctx.accounts.protocol_status.total_participants = 0;
        ctx.accounts.protocol_status.total_sale_amount = 0;

        Ok(())
    }

    pub fn buy_token(ctx: Context<ManageToken>, sol_amount: u64) -> Result<()> {
        let protocol_status = &mut ctx.accounts.protocol_status;
        let token_price = protocol_status.token_price as f64 / 1000000000.0;
        let token_amount = (sol_amount as f64 / token_price) as u64;
        let now_ts: u64 = Clock::get()?.unix_timestamp as u64;

        require!(protocol_status.end_time >= now_ts, ErrorCode::InvalidSale);

        let destination = &ctx.accounts.to_ata;
        let source = &ctx.accounts.from_ata;
        let token_program = &ctx.accounts.token_program;
        let authority = &ctx.accounts.vault;

        //SOL tranfer from signer to the contract

        let ix = system_instruction::transfer(
            &ctx.accounts.signer.key(),
            &ctx.accounts.vault.key(),
            sol_amount,
        );

        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.signer.to_account_info(),
                ctx.accounts.vault.to_account_info(),
            ],
        )?;

        // Transfer tokens to the buyer
        let cpi_accounts = Transfer {
            from: source.to_account_info().clone(),
            to: destination.to_account_info().clone(),
            authority: authority.to_account_info().clone(),
        };
        let cpi_program = token_program.to_account_info();

        let vault_bump = ctx.bumps.vault;

        let seeds = &[b"vault".as_ref(), &[vault_bump]];
        let signer = &[&seeds[..]];

        token::transfer(
            CpiContext::new(cpi_program, cpi_accounts).with_signer(signer),
            token_amount,
        )?;

        protocol_status.total_sale_amount += sol_amount;

        if token_amount > 0 {
            protocol_status.total_participants += 1;
        }

        Ok(())
    }

    pub fn withdraw_token(
        ctx: Context<ManageToken>,
        sol_amount: u64,
        token_amount: u64,
    ) -> Result<()> {
        let protocol_status = &mut ctx.accounts.protocol_status;
        let now_ts: u64 = Clock::get()?.unix_timestamp as u64;

        require!(
            protocol_status.end_time < now_ts,
            ErrorCode::InvalidWithdraw
        );

        require!(
            protocol_status.admin == ctx.accounts.signer.key(),
            ErrorCode::InvalidCaller
        );

        let destination = &ctx.accounts.to_ata;
        let source = &ctx.accounts.from_ata;
        let token_program = &ctx.accounts.token_program;
        let authority = &ctx.accounts.vault;
        // Transfer tokens to the admin
        let cpi_accounts = Transfer {
            from: source.to_account_info().clone(),
            to: destination.to_account_info().clone(),
            authority: authority.to_account_info().clone(),
        };
        let cpi_program = token_program.to_account_info();

        let vault_bump = ctx.bumps.vault;

        let seeds = &[b"vault".as_ref(), &[vault_bump]];
        let signer = &[&seeds[..]];

        token::transfer(
            CpiContext::new(cpi_program, cpi_accounts).with_signer(signer),
            token_amount,
        )?;

        // Withdraw SOL
        **ctx
            .accounts
            .vault
            .to_account_info()
            .try_borrow_mut_lamports()? -= sol_amount;
        **ctx
            .accounts
            .signer
            .to_account_info()
            .try_borrow_mut_lamports()? += sol_amount;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Init<'info> {
    pub mint: Account<'info, Mint>,
    #[account(
        init, 
        payer = signer,
        seeds = [b"protocol_status"],
        bump,
        space = 8 + 32 + 32 + 8 + 8 + 8 + 8
    )]
    pub protocol_status: Account<'info, ProtocolStatus>,
    #[account(
        init,
        payer = signer,
        space = 8,
        seeds = [b"vault"],
        bump,
    )]
    /// CHECK:
    pub vault: AccountInfo<'info>,
    #[account(
        init,
        payer = signer,
        token::mint = mint,
        token::authority = vault,
        seeds = [b"vault_ata"],
        bump
    )]
    pub vault_ata: Account<'info, TokenAccount>,
    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ManageToken<'info> {
    #[account(
        mut,
        seeds = [b"protocol_status"],
        bump,
    )]
    pub protocol_status: Account<'info, ProtocolStatus>,

    #[account(
        mut,
        seeds = [b"vault"],
        bump,
    )]
    /// CHECK:
    pub vault: AccountInfo<'info>,

    pub mint: Account<'info, Mint>,

    #[account( 
        mut,
        token::mint = mint,
        token::authority = vault,
        seeds = [b"vault_ata"],
        bump
        )]
    pub from_ata: Account<'info, TokenAccount>,

    #[account( 
        mut,
        token::mint = mint,
        token::authority = signer,
        )]
    pub to_ata: Account<'info, TokenAccount>,

    #[account(mut)]
    pub signer: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

#[account]
pub struct ProtocolStatus {
    admin: Pubkey,
    token_mint: Pubkey,
    end_time: u64,
    token_price: u64,
    total_participants: u64,
    total_sale_amount: u64,
}

#[error_code]
pub enum ErrorCode {
    #[msg("invalid sale")]
    InvalidSale,
    #[msg("Sale isn't ended")]
    InvalidWithdraw,
    #[msg("invalid caller")]
    InvalidCaller,
}
