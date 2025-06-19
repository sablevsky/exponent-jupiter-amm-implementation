use crate::exponent::{precise_number::Number, CpiAccounts};
use anchor_lang::prelude::*;
use exponent_time_curve::math::fee_rate;

#[account]
#[derive(Debug)]
pub struct MarketTwo {
    /// Address to ALT
    pub address_lookup_table: Pubkey,

    /// Mint of the vault's PT token
    pub mint_pt: Pubkey,

    /// Mint of the SY program's SY token
    pub mint_sy: Pubkey,

    /// Link to yield-stripping vault
    pub vault: Pubkey,

    /// Mint for the market's LP tokens
    pub mint_lp: Pubkey,

    /// Holds the LP tokens that are earning emissions
    /// This is where LP holders "stake" their LP tokens
    pub token_lp_escrow: Pubkey,

    /// Token account that holds PT liquidity
    pub token_pt_escrow: Pubkey,

    /// Pass-through token account for SY moving from the depositor to the SY program
    pub token_sy_escrow: Pubkey,

    /// Token account that holds SY fees from trade_pt
    pub token_fee_treasury_sy: Pubkey,

    /// Fee treasury SY BPS
    pub fee_treasury_sy_bps: u16,

    /// Authority for CPI calls owned by the market struct
    pub self_address: Pubkey,

    /// Bump for signing the PDA
    pub signer_bump: [u8; 1],

    pub status_flags: u8,

    /// Link to the SY program ID
    pub sy_program: Pubkey,

    pub financials: MarketFinancials,

    pub emissions: MarketEmissions,

    pub lp_farm: LpFarm,

    pub max_lp_supply: u64,

    pub lp_escrow_amount: u64,

    /// Record of CPI accounts
    pub cpi_accounts: CpiAccounts,

    pub is_current_flash_swap: bool,

    pub liquidity_net_balance_limits: LiquidityNetBalanceLimits,
}

#[derive(AnchorDeserialize, AnchorSerialize, Clone, Default, Debug)]
pub struct MarketFinancials {
    /// Expiration timestamp, which is copied from the vault associated with the PT
    pub expiration_ts: u64,

    /// Balance of PT in the market
    /// This amount is tracked separately to prevent bugs from token transfers directly to the market
    pub pt_balance: u64,

    /// Balance of SY in the market
    /// This amount is tracked separately to prevent bugs from token transfers directly to the market
    pub sy_balance: u64,

    /// Initial log of fee rate, which decreases over time
    pub ln_fee_rate_root: f64,

    /// Last seen log of implied rate (APY) for PT
    /// Used to maintain continuity of the APY between trades over time
    pub last_ln_implied_rate: f64,

    /// Initial rate scalar, which increases over time
    pub rate_scalar_root: f64,
}

impl MarketFinancials {
    fn sec_remaining(&self, now: u64) -> u64 {
        if now > self.expiration_ts {
            0
        } else {
            self.expiration_ts - now
        }
    }

    /// Calculate asset balance from the SY balance and exchange rate
    pub fn asset_balance(&self, sy_exchange_rate: Number) -> Number {
        Number::from_natural_u64(self.sy_balance) * sy_exchange_rate
    }

    /// Calculate the current rate anchor
    pub fn current_rate_anchor(&self, sy_exchange_rate: Number, now: u64) -> f64 {
        let sec_remaining = self.sec_remaining(now);
        let asset = self.asset_balance(sy_exchange_rate).floor_u64();
        let current_rate_scalar = self.current_rate_scalar(now);
        exponent_time_curve::math::find_rate_anchor(
            self.pt_balance,
            asset,
            current_rate_scalar,
            self.last_ln_implied_rate.into(),
            sec_remaining,
        )
    }

    /// Calculate the current rate scalar
    pub fn current_rate_scalar(&self, now: u64) -> f64 {
        let sec_remaining = self.sec_remaining(now);
        exponent_time_curve::math::rate_scalar::<f64>(self.rate_scalar_root, sec_remaining)
    }

    /// Calculate the current fee rate base on the decay from the initial fee rate
    pub fn cur_fee_rate(&self, now: u64) -> f64 {
        fee_rate(self.ln_fee_rate_root.into(), self.sec_remaining(now))
    }
}

pub fn py_to_sy(sy_exchange_rate: Number, amount_py: u64) -> u64 {
    let sy = Number::from_natural_u64(amount_py) / sy_exchange_rate;
    sy.floor_u64()
}

/// Div-down flooring of SY tokens into PT & YT
/// Based on the current SY exchange rate
pub fn sy_to_py(sy_exchange_rate: Number, amount_sy: u64) -> u64 {
    let py = Number::from_natural_u64(amount_sy) * sy_exchange_rate;
    py.floor_u64()
}

// fn f64_to_u64_checked(value: f64) -> Option<u64> {
//     // Check for invalid values: NaN, infinity, or negative numbers
//     if !value.is_finite() || value < 0.0 {
//         return None;
//     }

//     // Check if the value exceeds the maximum u64 value
//     if value > u64::MAX as f64 {
//         return None; // Overflow
//     }

//     // Perform the conversion safely
//     Some(value as u64)
// }

// fn sy_magnitude_from_net_trader_asset(net_trader_asset: f64, sy_exchange_rate: Number) -> u64 {
//     // taking the floor before the absolute value is important
//     // if net_trader_asset is negative, we want to floor down towards -inf
//     // the reason for this is that: the trader is buying PT with asset, and so should be charged more asset

//     // if net_trader_asset is positive, we want to floor down towards 0
//     // this is because the trader is selling PT for asset, and so should be paid less asset

//     // the floor function returns the largest integer less than or equal to the number
//     // Example: -8.45 goes to -9

//     let is_negative = net_trader_asset.is_sign_negative();

//     let asset_magnitude: u64 =
//         f64_to_u64_checked(net_trader_asset.floor().abs()).expect("f64 overflow for u64");

//     let sy_magnitude = Number::from_natural_u64(asset_magnitude) / sy_exchange_rate;

//     if is_negative {
//         sy_magnitude.ceil_u64()
//     } else {
//         sy_magnitude.floor_u64()
//     }
// }

// fn net_trader_sy_from_net_trader_asset(net_trader_asset: f64, sy_exchange_rate: Number) -> i64 {
//     let sy_magnitude = sy_magnitude_from_net_trader_asset(net_trader_asset, sy_exchange_rate);
//     // buying PT means the trader is losing SY
//     let is_buy = net_trader_asset.is_sign_negative();

//     if is_buy {
//         // the trader is buying PT
//         // so their net sy change is negative
//         <u64 as TryInto<i64>>::try_into(sy_magnitude).expect("u64 overflow for i64") * -1
//     } else {
//         // the trader is selling PT
//         // so their net sy change is positive
//         sy_magnitude.try_into().expect("u64 overflow for i64")
//     }
// }

// /// Convert fee units from asset to SY units
// fn sy_fee_from_asset_fee(asset_fee: f64, sy_exchange_rate: Number) -> u64 {
//     let sy_exchange_rate = sy_exchange_rate.to_f64().unwrap();
//     let sy_fee = (asset_fee / sy_exchange_rate).floor();
//     f64_to_u64_checked(sy_fee).expect("f64 overflow for u64")
// }

#[derive(AnchorDeserialize, AnchorSerialize, Default, Clone, Debug)]
pub struct MarketEmissions {
    pub trackers: Vec<MarketEmission>,
}

#[derive(AnchorDeserialize, AnchorSerialize, Default, Clone, Debug)]
pub struct MarketEmission {
    /// Escrow account that receives the emissions from the SY program
    /// And then passes them through to the user
    pub token_escrow: Pubkey,

    /// Index for converting LP shares into earned emissions
    pub lp_share_index: Number,

    /// The difference between the staged amount and collected emission amount
    pub last_seen_staged: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, Debug)]
pub struct LpFarm {
    pub last_seen_timestamp: u32,
    pub farm_emissions: Vec<FarmEmission>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, Debug)]
pub struct FarmEmission {
    /// Mint for the emission token
    pub mint: Pubkey,
    /// Rate at which the emission token is emitted per second
    pub token_rate: u64,
    /// Expiration timestamp for the emission token
    pub expiry_timestamp: u32,
    /// Index for converting LP shares into earned emissions
    pub index: Number,
}

#[derive(AnchorDeserialize, AnchorSerialize, Default, Clone, Debug)]
pub struct LiquidityNetBalanceLimits {
    pub window_start_timestamp: u32,
    pub window_start_net_balance: u64,
    /// Maximum allowed negative change in basis points (10000 = 100%)
    pub max_net_balance_change_negative_percentage: u16,
    /// Maximum allowed positive change in basis points (10000 = 100%)
    /// Using u32 to allow for very large increases (up to ~429,496%)
    pub max_net_balance_change_positive_percentage: u32,
    pub window_duration_seconds: u32,
}

#[derive(Debug)]
pub struct TradeResult {
    /// The change to trader's PT balance and market's PT liquidity
    pub net_trader_pt: i64,
    /// The change to the trader's SY balance and market's SY liquidity
    pub net_trader_sy: i64,
    /// The part of the trade that was a fee
    pub sy_fee: u64,
}

pub struct LiqAddResult {
    pub pt_in: u64,
    pub sy_in: u64,
    pub lp_out: u64,
}

pub struct LiqRmResult {
    pub pt_out: u64,
    pub sy_out: u64,
}
