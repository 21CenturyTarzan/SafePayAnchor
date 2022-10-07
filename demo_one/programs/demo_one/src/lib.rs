use anchor_lang::prelude::*;
use anchor_spl::token::{self, CloseAccount, Mint, Token, SetAuthority, TokenAccount, Transfer};
use spl_token::instruction::AuthorityType;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod demo_one {
    use super::*;

    pub fn initialize_new_grant(ctx: Context<InitializeNewGrant>, application_idx: u64, state_bump: u8, _wallet_bump: u8, amount: u64) -> Result<()> {
        // Set the state attributes
        let state = &mut ctx.accounts.application_state;
        state.idx = application_idx;
        state.initializer = ctx.accounts.user_sending.key().clone();
        state.receiver = ctx.accounts.user_receiving.key().clone();
        state.mint_of_token = ctx.accounts.mint_of_token_being_sent.key().clone();
        state.escrow_wallet = ctx.accounts.escrow_wallet_state.key().clone();
        state.amount = amount;

        msg!("Initialized new Safe Transfer instance for {}", amount);

        // CPI time! we now need to call into the Token program to transfer our funds to the 
        // Escrow wallet. Our state account account is a PDA, which means that no private key
        // exists for the corresponding public key and therefore this key was not signed in the original 
        // transaction. Our program is the only entity that can programmatically sign for the PDA
        // and we can do this by specifying the PDA "derivation hash key" and using `CpiContext::new_with_signer()`.

        // This specific step is very different compared to Ethereum. In Ethereum, accounts need to first set allowances towards 
        // a specific contract (like ZeroEx, Uniswap, Curve..) before the contract is able to withdraw funds. In this other case,
        // the SafePay program can use Bob's signature to "authenticate" the `transfer()` instruction sent to the token contract.
        let bump_vector = state_bump.to_le_bytes();
        let mint_of_token_being_sent_pk = ctx.accounts.mint_of_token_being_sent.key().clone();
        let application_idx_bytes = application_idx.to_le_bytes();
        let inner = vec![
            b"state".as_ref(),
            ctx.accounts.user_sending.key.as_ref(),
            ctx.accounts.user_receiving.key.as_ref(),
            mint_of_token_being_sent_pk.as_ref(), 
            application_idx_bytes.as_ref(),
            bump_vector.as_ref(),
        ];
        let outer = vec![inner.as_slice()];

        // Below is the actual instruction that we are going to send to the Token program.
        let transfer_instruction = Transfer{
            from: ctx.accounts.wallet_to_withdraw_from.to_account_info(),
            to: ctx.accounts.escrow_wallet_state.to_account_info(),
            authority: ctx.accounts.user_sending.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            transfer_instruction,
            outer.as_slice(),
        );

        // The `?` at the end will cause the function to return early in case of an error.
        // This pattern is common in Rust.
        anchor_spl::token::transfer(cpi_ctx, state.amount)?;

        // Mark stage as deposited.
        state.stage = Stage::FundsDeposited.to_code();
        Ok(())
    }
}

// below is context sturcture which describes the context that will be passed in when function is called.

#[derive(Accounts)]
#[instruction(application_idx: u64, state_bump: u8, wallet_bump: u8)]
pub struct InitializeNewGrant <'info> {

    // Derived PDA
    #[account(
        init,
        payer=user_sending,
        seeds=[b"state".as_ref(), user_sending.key().as_ref(), user_receiving.key.as_ref(), mint_of_token_being_sent.key().as_ref(), application_idx.to_le_bytes().as_ref()],
        bump = state_bump,
    )]
    application_state: Account<'info, State>,

    #[account(
        init,
        payer = user_sending,
        seeds=[b"wallet".as_ref(), user_sending.key().as_ref(), user_receiving.key.as_ref(), mint_of_token_being_sent.key().as_ref(), application_idx.to_le_bytes().as_ref()],
        bump = wallet_bump,
        token::mint=mint_of_token_being_sent,
        token::authority=application_state,
    )]
    escrow_wallet_state : Account <'info, TokenAccount>,

    #[account(mut)]
    user_sending: Signer<'info>,                                // Alice
    user_receiving: AccountInfo<'info>,                         // Bob
    mint_of_token_being_sent: Account<'info, Mint>,             // USDC

    
    // Alice's USDC wallet that has already approved the escrow wallet
    #[account(
        mut,
        constraint=wallet_to_withdraw_from.owner == user_sending.key(),
        constraint=wallet_to_withdraw_from.mint == mint_of_token_being_sent.key()
    )]
    wallet_to_withdraw_from: Account<'info, TokenAccount>,      

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info,Rent>,
}



#[account]
pub struct State {
    pub idx : u64,
    pub initializer : Pubkey,    // Alice
    pub receiver : Pubkey,       // Bob
    pub mint_of_token : Pubkey,  // mint of token that alice want to send
    pub escrow_wallet : Pubkey,  //
    pub amount : u64,
    stage: u8,
}

// #[derive(Accounts)]
// pub struct