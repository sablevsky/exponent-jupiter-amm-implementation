use anchor_lang::prelude::{AccountMeta, Pubkey};

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
    pub event_authority: Pubkey,
    pub exponent_program_id: Pubkey,
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
            AccountMeta::new_readonly(accounts.event_authority, false),
            AccountMeta::new_readonly(accounts.exponent_program_id, false),
        ]
    }
}
