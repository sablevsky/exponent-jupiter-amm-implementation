use anchor_lang::prelude::{AccountMeta, Pubkey};

pub struct RedeemSyAccounts {
    /** User public key */
    pub signer: Pubkey,
    /** Meta account public key */
    pub meta: Pubkey,
    /** Jito vault address */
    pub jito_vault: Pubkey,
    /** User's ATA for Base token */
    pub token_base_dst: Pubkey,
    /** Meta's ATA for Base token */
    pub token_vrt_escrow: Pubkey,
    /** User's ATA for SY token */
    pub token_sy_signer: Pubkey,
    /** SY token mint */
    pub mint_sy: Pubkey,
    /** Base token program (Usually Token Program) */
    pub base_token_program: Pubkey,
    /** SY token program (Usually Token Program) */
    pub token_program: Pubkey,
    pub jito_stake_pool: Pubkey,
    pub jito_token_meta_ata: Pubkey,
}

impl From<RedeemSyAccounts> for Vec<AccountMeta> {
    fn from(accounts: RedeemSyAccounts) -> Self {
        vec![
            AccountMeta::new(accounts.signer, true),
            AccountMeta::new(accounts.meta, false),
            AccountMeta::new_readonly(accounts.jito_vault, false),
            AccountMeta::new(accounts.token_base_dst, false),
            AccountMeta::new(accounts.token_vrt_escrow, false),
            AccountMeta::new(accounts.token_sy_signer, false),
            AccountMeta::new(accounts.mint_sy, false),
            AccountMeta::new_readonly(accounts.base_token_program, false),
            AccountMeta::new_readonly(accounts.token_program, false),
            AccountMeta::new(accounts.jito_stake_pool, false),
            AccountMeta::new_readonly(accounts.jito_token_meta_ata, false),
        ]
    }
}
