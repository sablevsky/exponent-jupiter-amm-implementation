use anchor_lang::prelude::*;
use crate::exponent::{precise_number::Number, CpiAccounts};

#[derive(Default, Debug)]
#[account]
pub struct Vault {
    /// Link to SY program
    pub sy_program: Pubkey,

    /// Mint for SY token
    pub mint_sy: Pubkey,

    /// Mint for the vault-specific YT token
    pub mint_yt: Pubkey,

    /// Mint for the vault-specific PT token
    pub mint_pt: Pubkey,

    /// Escrow account for holding deposited YT
    pub escrow_yt: Pubkey,

    /// Escrow account that holds temporary SY tokens
    /// As an interchange between users and the SY program
    pub escrow_sy: Pubkey,

    /// Link to a vault-owned YT position
    /// This account collects yield from all "unstaked" YT
    pub yield_position: Pubkey,

    /// Address lookup table key for vault
    pub address_lookup_table: Pubkey,

    /// start timestamp
    pub start_ts: u32,

    /// seconds duration
    pub duration: u32,

    /// Seed for CPI signing
    pub signer_seed: Pubkey,

    /// Authority for CPI signing
    pub authority: Pubkey,

    /// bump for signer authority PDA
    pub signer_bump: [u8; 1],

    /// Last seen SY exchange rate
    /// Not needed for live use, but only when the vault has matured, and we need to freeze earnings for YT holders
    /// This will not get updated after vault is mautured
    pub last_seen_sy_exchange_rate: Number,

    /// This is the all time high exchange rate for SY
    pub all_time_high_sy_exchange_rate: Number,

    /// This is the exchange rate for SY when the vault expires
    pub final_sy_exchange_rate: Number,

    /// How much SY is held in escrow
    pub total_sy_in_escrow: u64,

    /// The total SY set aside to back the PT holders
    /// This value is updated on every operation that touches the PT supply or the last seen exchange rate
    pub sy_for_pt: u64,

    /// Total supply of PT
    pub pt_supply: u64,

    /// Amount of SY staged for the treasury
    pub treasury_sy: u64,

    /// SY that has been earned by YT, but not yet collected
    pub uncollected_sy: u64,

    /// SY that has been staged for collection, but not yet collected
    /// It is strictly greater-than-or-equal to the treasury_sy
    /// And strictly less than or equal to the total_sy_in_escrow
    // pub uncollected_sy: u64,
    pub treasury_sy_token_account: Pubkey,

    pub interest_bps_fee: u16,

    pub min_op_size_strip: u64,

    pub min_op_size_merge: u64,

    pub status: u8,

    pub emissions: Vec<EmissionInfo>,

    pub cpi_accounts: CpiAccounts,

    pub claim_limits: ClaimLimits,

    pub max_py_supply: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone, Default, Debug)]
pub struct EmissionInfo {
    /// The token account for the emission where the vault authority is the authority
    pub token_account: Pubkey,
    // The initial index is used to track the first claimable index for yield positions after an emission has been added
    pub initial_index: Number,
    // The last seen index is used to track the last claimable index after vault expiration
    pub last_seen_index: Number,
    /// The final index is used to track the last claimable index after the vault expires
    pub final_index: Number,
    /// The treasury token account for this reward
    /// TODO - this account could be removed by using the token_account as the treasury token account
    pub treasury_token_account: Pubkey,
    /// The fee taken from emission collecting
    pub fee_bps: u16,
    /// The lambo fund
    pub treasury_emission: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone, Default, Debug)]
pub struct ClaimLimits {
    pub claim_window_start_timestamp: u32,
    pub total_claim_amount_in_window: u64,
    pub max_claim_amount_per_window: u64,
    pub claim_window_duration_seconds: u32,
}