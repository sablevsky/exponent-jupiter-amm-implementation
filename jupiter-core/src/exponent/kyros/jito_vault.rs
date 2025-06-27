//! The vault is responsible for holding tokens and minting VRT tokens.
use crate::exponent::precise_number::Number;
use anchor_lang::solana_program::pubkey::Pubkey;
use bytemuck::{Pod, Zeroable};
use jito_bytemuck::{
    types::{PodBool, PodU16, PodU64},
    AccountDeserialize, Discriminator,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
#[repr(C)]
pub struct DelegationState {
    /// The amount of stake that is currently active on the operator
    staked_amount: PodU64,

    /// Any stake that was deactivated in the current epoch
    enqueued_for_cooldown_amount: PodU64,

    /// Any stake that was deactivated in the previous epoch,
    /// to be available for re-delegation in the current epoch + 1
    cooling_down_amount: PodU64,

    reserved: [u8; 256],
}

#[derive(Debug, PartialEq, Eq)]
pub struct BurnSummary {
    /// How much of the VRT shall be transferred to the vault fee account
    pub vault_fee_amount: u64,
    /// How much of the VRT shall be transferred to the program fee account
    pub program_fee_amount: u64,
    /// How much of the staker's VRT shall be burned
    pub burn_amount: u64,
    /// How much of the staker's tokens shall be returned
    pub out_amount: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct MintSummary {
    pub vrt_to_depositor: u64,
    pub vrt_to_fee_wallet: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable, AccountDeserialize)]
#[repr(C)]
pub struct JitoVault {
    /// The base account of the VRT
    pub base: Pubkey,

    // ------------------------------------------
    // Token information and accounting
    // ------------------------------------------
    /// Mint of the VRT token
    pub vrt_mint: Pubkey,

    /// Mint of the token that is supported by the VRT
    pub supported_mint: Pubkey,

    /// The total number of VRT in circulation
    vrt_supply: PodU64,

    /// The total number of tokens deposited
    tokens_deposited: PodU64,

    /// The maximum deposit capacity allowed in the mint_to instruction.
    /// The deposited assets in the vault may exceed the deposit_capacity during other operations, such as vault balance updates.
    deposit_capacity: PodU64,

    /// Rolled-up stake state for all operators in the set
    pub delegation_state: DelegationState,

    /// The amount of additional assets that need unstaking to fulfill VRT withdrawals
    additional_assets_need_unstaking: PodU64,

    /// The amount of VRT tokens in VaultStakerWithdrawalTickets enqueued for cooldown
    vrt_enqueued_for_cooldown_amount: PodU64,

    /// The amount of VRT tokens cooling down
    vrt_cooling_down_amount: PodU64,

    /// The amount of VRT tokens ready to claim
    vrt_ready_to_claim_amount: PodU64,

    // ------------------------------------------
    // Admins
    // ------------------------------------------
    /// Vault admin
    pub admin: Pubkey,

    /// The delegation admin responsible for adding and removing delegations from operators.
    pub delegation_admin: Pubkey,

    /// The operator admin responsible for adding and removing operators.
    pub operator_admin: Pubkey,

    /// The node consensus network admin responsible for adding and removing support for NCNs.
    pub ncn_admin: Pubkey,

    /// The admin responsible for adding and removing slashers.
    pub slasher_admin: Pubkey,

    /// The admin responsible for setting the capacity
    pub capacity_admin: Pubkey,

    /// The admin responsible for setting the fees
    pub fee_admin: Pubkey,

    /// The delegate_admin responsible for delegating assets
    pub delegate_asset_admin: Pubkey,

    /// Fee wallet account
    pub fee_wallet: Pubkey,

    /// Optional mint signer
    pub mint_burn_admin: Pubkey,

    /// ( For future use ) Authority to update the vault's metadata
    pub metadata_admin: Pubkey,

    // ------------------------------------------
    // Indexing and counters
    // These are helpful when one needs to iterate through all the accounts
    // ------------------------------------------
    /// The index of the vault in the vault list
    vault_index: PodU64,

    /// Number of VaultNcnTicket accounts tracked by this vault
    ncn_count: PodU64,

    /// Number of VaultOperatorDelegation accounts tracked by this vault
    operator_count: PodU64,

    /// Number of VaultNcnSlasherTicket accounts tracked by this vault
    slasher_count: PodU64,

    /// The slot of the last fee change
    last_fee_change_slot: PodU64,

    /// The slot of the last time the delegations were updated
    last_full_state_update_slot: PodU64,

    /// The deposit fee in basis points
    deposit_fee_bps: PodU16,

    /// The withdrawal fee in basis points
    withdrawal_fee_bps: PodU16,

    /// The next epoch's withdrawal fee in basis points
    next_withdrawal_fee_bps: PodU16,

    /// Fee for each epoch
    reward_fee_bps: PodU16,

    /// (Copied from Config) The program fee in basis points
    program_fee_bps: PodU16,

    /// The bump seed for the PDA
    pub bump: u8,

    is_paused: PodBool,

    /// Reserved space
    reserved: [u8; 259],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitoVaultDiscriminator {
    Config = 1,
    Vault = 2,
    VaultNcnTicket = 3,
    VaultOperatorDelegation = 4,
    VaultNcnSlasherTicket = 5,
    VaultNcnSlasherOperatorTicket = 6,
    VaultStakerWithdrawalTicket = 7,
    VaultUpdateStateTracker = 8,
}

impl Discriminator for JitoVault {
    const DISCRIMINATOR: u8 = JitoVaultDiscriminator::Vault as u8;
}

impl JitoVault {
    pub fn vrt_supply(&self) -> u64 {
        self.vrt_supply.into()
    }

    /// Amount of jitoSOL deposited
    pub fn tokens_deposited(&self) -> u64 {
        self.tokens_deposited.into()
    }

    /// Exchange rate of the vault from VRT -> jitoSOL
    pub fn exchange_index(&self) -> Number {
        // If denominator is 0, exchange rate defaults to 1
        if self.vrt_supply() == 0 {
            return Number::ONE;
        }

        Number::from_ratio(self.tokens_deposited().into(), self.vrt_supply().into())
    }

    pub fn deserialize(data: &[u8]) -> &Self {
        bytemuck::try_from_bytes(&data[8..]).unwrap()
    }

    pub fn slot_staleness_check(&self, slot: u64) -> bool {
        u64::from(self.last_full_state_update_slot) == slot
    }
}
