use crate::exponent::precise_number::Number;
use anchor_lang::prelude::*;

/// ## Version History
/// * v15: migrate to new layout including new fields using bytemuck. (150584 ~= 148KB)
pub const FUND_ACCOUNT_CURRENT_VERSION: u16 = 15;

pub const FUND_WITHDRAWAL_FEE_RATE_BPS_LIMIT: u16 = 500;
pub const FUND_ACCOUNT_MAX_SUPPORTED_TOKENS: usize = 30;
pub const FUND_ACCOUNT_MAX_RESTAKING_VAULTS: usize = 30;

#[account(zero_copy)]
#[repr(C)]
pub struct FundAccount {
    data_version: u16,
    bump: u8,
    reserve_account_bump: u8,
    treasury_account_bump: u8,
    _padding: [u8; 9],
    pub(super) transfer_enabled: u8,

    address_lookup_table_enabled: u8,
    address_lookup_table_account: Pubkey,

    // informative
    reserve_account: Pubkey,
    treasury_account: Pubkey,

    /// receipt token information
    pub receipt_token_mint: Pubkey,
    pub(super) receipt_token_program: Pubkey,
    pub(super) receipt_token_decimals: u8,
    _padding2: [u8; 7],
    pub(super) receipt_token_supply_amount: u64,
    pub(super) one_receipt_token_as_sol: u64,
    pub(super) receipt_token_value_updated_slot: u64,
    pub(super) receipt_token_value: TokenValuePod,

    /// global withdrawal configurations
    pub(super) withdrawal_batch_threshold_interval_seconds: i64,
    pub(super) withdrawal_fee_rate_bps: u16,
    pub(super) withdrawal_enabled: u8,
    pub(super) deposit_enabled: u8,
    pub(super) donation_enabled: u8,
    _padding4: [u8; 3],

    /// SOL deposit & withdrawal
    pub(super) sol: AssetState,

    /// underlying assets
    _padding6: [u8; 15],
    num_supported_tokens: u8,
    supported_tokens: [SupportedToken; FUND_ACCOUNT_MAX_SUPPORTED_TOKENS],

    /// optional basket of underlying assets
    normalized_token: NormalizedToken,

    /// investments
    _padding7: [u8; 15],
    num_restaking_vaults: u8,
    restaking_vaults: [RestakingVault; FUND_ACCOUNT_MAX_RESTAKING_VAULTS],

    /// fund operation state
    pub(super) operation: OperationState,
}

impl FundAccount {
    pub fn exchange_rate(&self) -> Number {
        Number::from_natural_u64(self.one_receipt_token_as_sol)
            / Number::from_natural_u64(10_u64.pow(self.receipt_token_decimals as u32))
    }

    pub fn supported_token_exchange_rate(&self, index: usize) -> Number {
        let supported_token = &self.supported_tokens[index];
        Number::from_natural_u64(10_u64.pow(supported_token.decimals as u32))
            / Number::from_natural_u64(supported_token.one_token_as_receipt_token)
    }
}

const TOKEN_VALUE_MAX_NUMERATORS_SIZE: usize = 33;

#[zero_copy]
#[repr(C)]
pub struct TokenValuePod {
    numerator: [AssetPod; TOKEN_VALUE_MAX_NUMERATORS_SIZE],
    num_numerator: u64,
    denominator: u64,
}

#[zero_copy]
#[repr(C)]
pub struct AssetPod {
    discriminant: u8,
    _padding: [u8; 7],
    sol_amount: u64,
    token_amount: u64,
    token_mint: Pubkey,
    token_pricing_source: TokenPricingSourcePod,
}

#[zero_copy]
#[repr(C)]
pub struct SupportedToken {
    pub mint: Pubkey,
    pub program: Pubkey,
    pub decimals: u8,
    _padding: [u8; 7],

    pub pricing_source: TokenPricingSourcePod,

    /// informative
    pub one_token_as_sol: u64,

    /// token deposit & withdrawal
    pub token: AssetState,

    /// configuration: the amount requested to be unstaked as soon as possible regardless of current state, this value should be decreased by each unstaking requested amount.
    pub rebalancing_amount: u64,

    /// configuration: used for staking allocation strategy.
    pub sol_allocation_weight: u64,
    pub sol_allocation_capacity_amount: u64,

    // third parties state tracking
    pub pending_unstaking_amount_as_sol: u64,

    /// informative
    pub one_token_as_receipt_token: u64,

    _reserved: [u8; 48],
}

#[zero_copy]
#[repr(C)]
pub(super) struct OperationState {
    updated_slot: u64,
    updated_at: i64,
    expired_at: i64,

    _padding: [u8; 5],
    /// when the no_transition flag turned on, current command should not be transitioned to other command.
    /// the purpose of this flag is for internal testing by set boundary of the reset command operation.
    no_transition: u8,
    pub next_sequence: u16,
    pub num_operated: u64,

    next_command: OperationCommandEntryPod,

    _reserved: [u8; 128],
}

pub const FUND_ACCOUNT_OPERATION_COMMAND_MAX_ACCOUNT_SIZE: usize = 32;

#[zero_copy]
#[repr(C)]
pub struct OperationCommandEntryPod {
    num_required_accounts: u8,
    _padding: [u8; 7],
    required_accounts:
        [OperationCommandAccountMetaPod; FUND_ACCOUNT_OPERATION_COMMAND_MAX_ACCOUNT_SIZE],
    command: OperationCommandPod,
}

#[zero_copy]
#[repr(C)]
pub struct OperationCommandAccountMetaPod {
    pubkey: Pubkey,
    is_writable: u8,
    _padding: [u8; 7],
}

const FUND_ACCOUNT_OPERATION_COMMAND_BUFFER_SIZE: usize = 2535;

#[zero_copy]
#[repr(C)]
pub struct OperationCommandPod {
    discriminant: u8,
    buffer: [u8; FUND_ACCOUNT_OPERATION_COMMAND_BUFFER_SIZE],
}

pub const FUND_ACCOUNT_MAX_RESTAKING_VAULT_DELEGATIONS: usize = 30;
pub const FUND_ACCOUNT_RESTAKING_VAULT_MAX_COMPOUNDING_REWARD_TOKENS: usize = 10;

#[zero_copy]
#[repr(C)]
pub(super) struct RestakingVault {
    pub vault: Pubkey,
    pub program: Pubkey,

    pub supported_token_mint: Pubkey,
    pub receipt_token_mint: Pubkey,
    pub receipt_token_program: Pubkey,
    pub receipt_token_decimals: u8,
    _padding: [u8; 7],

    /// transient price
    pub one_receipt_token_as_sol: u64,
    pub receipt_token_pricing_source: TokenPricingSourcePod,
    pub receipt_token_operation_reserved_amount: u64,
    /// the amount of vrt being unrestaked
    pub receipt_token_operation_receivable_amount: u64,

    /// configuration: used for restaking allocation strategy.
    pub sol_allocation_weight: u64,
    pub sol_allocation_capacity_amount: u64,

    _padding2: [u8; 7],
    num_delegations: u8,
    delegations: [RestakingVaultDelegation; FUND_ACCOUNT_MAX_RESTAKING_VAULT_DELEGATIONS],

    /// auto-compounding
    _padding3: [u8; 7],
    num_compounding_reward_tokens: u8,
    compounding_reward_token_mints:
        [Pubkey; FUND_ACCOUNT_RESTAKING_VAULT_MAX_COMPOUNDING_REWARD_TOKENS],

    _reserved: [u8; 128],
}

#[zero_copy]
#[repr(C)]
pub(super) struct RestakingVaultDelegation {
    pub operator: Pubkey,

    /// configuration: used for delegation strategy.
    pub supported_token_allocation_weight: u64,
    pub supported_token_allocation_capacity_amount: u64,

    /// informative field; these values shall be synced from remote state periodically.
    pub supported_token_delegated_amount: u64,
    pub supported_token_undelegating_amount: u64,

    /// configuration: the amount requested to be undelegated as soon as possible regardless of current state, this value should be decreased by each undelegation requested amount.
    pub supported_token_redelegating_amount: u64,

    _reserved: [u8; 24],
}

#[zero_copy]
#[repr(C)]
pub struct WithdrawalBatch {
    pub batch_id: u64,
    pub num_requests: u64,
    pub receipt_token_amount: u64,
    pub enqueued_at: i64,
    _reserved: [u8; 32],
}

#[zero_copy]
#[repr(C)]
pub struct TokenPricingSourcePod {
    discriminant: u8,
    _padding: [u8; 7],
    address: Pubkey,
}

pub const FUND_ACCOUNT_MAX_QUEUED_WITHDRAWAL_BATCHES: usize = 10;

#[zero_copy]
#[repr(C)]
pub struct AssetState {
    token_mint: Pubkey,
    token_program: Pubkey,

    pub accumulated_deposit_capacity_amount: u64,
    pub accumulated_deposit_amount: u64,
    pub depositable: u8,
    _padding: [u8; 4],
    pub withdrawable: u8,
    pub normal_reserve_rate_bps: u16,
    pub normal_reserve_max_amount: u64,

    pub withdrawal_last_created_request_id: u64,
    pub withdrawal_last_processed_batch_id: u64,
    pub withdrawal_last_batch_enqueued_at: i64,
    pub withdrawal_last_batch_processed_at: i64,

    pub withdrawal_pending_batch: WithdrawalBatch,
    _padding2: [u8; 15],
    withdrawal_num_queued_batches: u8,
    withdrawal_queued_batches: [WithdrawalBatch; FUND_ACCOUNT_MAX_QUEUED_WITHDRAWAL_BATCHES],
    _reserved: [u8; 56],

    /// receipt token amount that users can request to withdraw with the given asset from the fund.
    /// it can be conditionally inaccurate on price changes among multiple assets, so make sure to update this properly before any use of it.
    /// do not make any hard limit constraints with this value from off-chain. a requested withdrawal amount will be adjusted on-chain based on the status.
    pub withdrawable_value_as_receipt_token_amount: u64,

    /// informative: reserved amount that users can claim for processed withdrawal requests, which is not accounted for as an asset of the fund.
    pub withdrawal_user_reserved_amount: u64,

    /// asset: receivable amount that the fund may charge the users requesting withdrawals.
    /// It is accrued during either the preparation of the withdrawal obligation or rebalancing of LST like fees from (un)staking or (un)restaking.
    /// And it shall be settled by the withdrawal fee normally. And it also can be written off by a donation operation.
    /// Then it costs the rebalancing expense to the capital of the fund itself as an operation cost instead of charging the users requesting withdrawals.
    pub operation_receivable_amount: u64,

    /// asset: remaining asset for cash-in/out
    pub operation_reserved_amount: u64,
}

#[zero_copy]
#[repr(C)]
pub(super) struct NormalizedToken {
    pub mint: Pubkey,
    pub program: Pubkey,
    pub decimals: u8,
    pub enabled: u8,
    _padding: [u8; 6],
    pub pricing_source: TokenPricingSourcePod,
    pub one_token_as_sol: u64,
    pub operation_reserved_amount: u64,
    _reserved: [u8; 64],
}
