use anchor_lang::prelude::{AccountMeta, Pubkey};

pub struct MintSyAccounts {
    pub depositor: Pubkey,
    pub meta: Pubkey,
    pub mint_sy: Pubkey,
    /// Depositor's base asset token account
    pub token_base_depositor: Pubkey,
    /// SY robot's base token account
    pub token_yield_bearing_escrow: Pubkey,
    pub token_sy_depositor: Pubkey,
    /// Base token program
    pub base_token_program: Pubkey,
    /// SY token program
    pub token_program: Pubkey,
}

impl From<MintSyAccounts> for Vec<AccountMeta> {
    fn from(accounts: MintSyAccounts) -> Self {
        vec![
            AccountMeta::new(accounts.depositor, true),
            AccountMeta::new(accounts.meta, false),
            AccountMeta::new(accounts.mint_sy, false),
            AccountMeta::new(accounts.token_base_depositor, false),
            AccountMeta::new(accounts.token_yield_bearing_escrow, false),
            AccountMeta::new(accounts.token_sy_depositor, false),
            AccountMeta::new_readonly(accounts.base_token_program, false),
            AccountMeta::new_readonly(accounts.token_program, false),
        ]
    }
}

pub struct RedeemSyAccounts {
    pub signer: Pubkey,
    pub meta: Pubkey,
    pub token_base_dst: Pubkey,
    pub token_yield_bearing_escrow: Pubkey,
    pub token_sy_signer: Pubkey,
    pub mint_sy: Pubkey,
    /// Base token program
    pub base_token_program: Pubkey,
    /// SY token program
    pub token_program: Pubkey,
}

impl From<RedeemSyAccounts> for Vec<AccountMeta> {
    fn from(accounts: RedeemSyAccounts) -> Self {
        vec![
            AccountMeta::new(accounts.signer, true),
            AccountMeta::new(accounts.meta, false),
            AccountMeta::new(accounts.token_base_dst, false),
            AccountMeta::new(accounts.token_yield_bearing_escrow, false),
            AccountMeta::new(accounts.token_sy_signer, false),
            AccountMeta::new(accounts.mint_sy, false),
            AccountMeta::new_readonly(accounts.base_token_program, false),
            AccountMeta::new_readonly(accounts.token_program, false),
        ]
    }
}

//? buy_pt and sell_pt instructions have the same accounts structure
#[derive(Copy, Clone, Debug)]
pub struct TradePt {
    pub trader: Pubkey,
    pub market: Pubkey,
    pub token_sy_trader: Pubkey,
    pub token_pt_trader: Pubkey,
    pub token_sy_escrow: Pubkey,
    pub token_pt_escrow: Pubkey,
    pub address_lookup_table: Pubkey,
    pub token_program: Pubkey,
    pub sy_program: Pubkey,
    pub token_fee_treasury_sy: Pubkey,
}

impl From<TradePt> for Vec<AccountMeta> {
    fn from(accounts: TradePt) -> Self {
        vec![
            AccountMeta::new(accounts.trader, true),
            AccountMeta::new(accounts.market, false),
            AccountMeta::new(accounts.token_sy_trader, false),
            AccountMeta::new(accounts.token_pt_trader, false),
            AccountMeta::new(accounts.token_sy_escrow, false),
            AccountMeta::new(accounts.token_pt_escrow, false),
            AccountMeta::new_readonly(accounts.address_lookup_table, false),
            AccountMeta::new_readonly(accounts.token_program, false),
            AccountMeta::new_readonly(accounts.sy_program, false),
            AccountMeta::new(accounts.token_fee_treasury_sy, false),
        ]
    }
}
