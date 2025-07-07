use anchor_lang::prelude::{AccountMeta, Pubkey};

pub struct MintSyAccounts {
    /** User public key */
    pub depositor: Pubkey,
    /** Meta account public key */
    pub meta: Pubkey,
    /** SY token mint */
    pub mint_sy: Pubkey,
    /** User's ATA for Base token */
    pub token_base_depositor: Pubkey,
    /** Meta's ATA for Base token */
    pub token_yield_bearing_escrow: Pubkey,
    /** User's ATA for SY token */
    pub token_sy_depositor: Pubkey,
    /** Base token program (Usually Token Program) */
    pub base_token_program: Pubkey,
    /** SY token program (Usually Token Program) */
    pub token_program: Pubkey,
    pub wfragsol_fund_account: Pubkey,
    pub jito_token_meta_ata: Pubkey,
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
            AccountMeta::new(accounts.wfragsol_fund_account, false),
            AccountMeta::new_readonly(accounts.jito_token_meta_ata, false),
        ]
    }
}
