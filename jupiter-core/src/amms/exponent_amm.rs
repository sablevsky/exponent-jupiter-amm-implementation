use anchor_lang::AccountDeserialize;
use anyhow::Result;
use exponent_time_curve::num::Num;
use jupiter_amm_interface::{
    try_get_account_data, AccountMap, Amm, AmmContext, KeyedAccount, Quote, QuoteParams, Swap,
    SwapAndAccountMetas, SwapParams,
};
use lazy_static::lazy_static;
use rust_decimal::Decimal;
use solana_sdk::{pubkey, pubkey::Pubkey};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use crate::exponent::{
    precise_number::Number, trade, trade_asset, MarketTwo, TradeAssetResult, TradePt, TradeResult,
    Vault,
};

mod exponent_swap_programs {
    use super::*;
    pub const EXPONENT_CORE: Pubkey = pubkey!("ExponentnaRg3CQbW6dqQNZKXp7gtZ9DGMp1cwC4HAS7");
}

lazy_static! {
    pub static ref EXPONENT_SWAP_PROGRAMS: HashMap<Pubkey, String> = {
        let mut m = HashMap::new();
        m.insert(exponent_swap_programs::EXPONENT_CORE, "Exponent".into());
        m
    };
}

#[derive(Clone)]
pub struct AdditionalMarketData {
    pub original_mint: Pubkey,
    pub virtual_exchange_rate: bool,
}

lazy_static! {
    pub static ref MARKET_ADDITIONAL_DATA: HashMap<Pubkey, AdditionalMarketData> = {
        let mut m = HashMap::new();

        let wfragsol_market_key = pubkey!("EJ4GPTCnNtemBVrT7QKhRfSKfM53aV2UJYGAC8gdVz5b");
        let wfragsol_additional_data = AdditionalMarketData {
            original_mint: pubkey!("WFRGSWjaz8tbAxsJitmbfRuFV2mSNwy7BMWcCwaA28U"),
            virtual_exchange_rate: true,
        };
        m.insert(wfragsol_market_key, wfragsol_additional_data);

        m
    };
}

#[derive(Clone)]
pub struct ExponentAmm {
    key: Pubkey,
    label: String,
    reserve_mints: [Pubkey; 2],
    exchange_rate: Option<Number>, //? Number
    market: MarketTwo,
    // vault: Option<Vault>,
    // epoch: Arc<AtomicU64>,
    timestamp: Arc<AtomicI64>,
    reserves: [u64; 2],
    program_id: Pubkey,
}

impl Amm for ExponentAmm {
    fn from_keyed_account(keyed_account: &KeyedAccount, amm_context: &AmmContext) -> Result<Self> {
        //? Get some domain state from KeyedAccount.
        //?? How can we use KeyedAccount.params?

        let market_state = MarketTwo::try_deserialize(&mut keyed_account.account.data.as_ref())?;

        let original_mint = MARKET_ADDITIONAL_DATA
            .get(&market_state.self_address)
            .unwrap()
            .original_mint
            .clone();

        let reserve_mints: [Pubkey; 2] = [original_mint, market_state.mint_pt];

        let label = EXPONENT_SWAP_PROGRAMS
            .get(&keyed_account.account.owner)
            .unwrap()
            .clone();

        // let epoch = amm_context.clock_ref.epoch.clone();
        let timestamp = amm_context.clock_ref.unix_timestamp.clone();

        Ok(Self {
            key: keyed_account.key,
            label,
            reserve_mints,
            exchange_rate: None,
            market: market_state,
            // epoch: epoch.clone(),
            timestamp: timestamp.clone(),
            reserves: Default::default(),
            program_id: keyed_account.account.owner,
        })
    }

    //? A human readable label of the underlying DEX
    fn label(&self) -> String {
        self.label.clone()
    }

    fn program_id(&self) -> Pubkey {
        self.program_id
    }

    //? Market state address
    fn key(&self) -> Pubkey {
        self.key
    }

    //? The mints that can be traded
    fn get_reserve_mints(&self) -> Vec<Pubkey> {
        self.reserve_mints.to_vec()
    }

    //? The accounts necessary to produce a quote
    fn get_accounts_to_update(&self) -> Vec<Pubkey> {
        // Update market and vault(to get exchange rate)
        vec![self.market.self_address, self.market.vault]
    }

    fn update(&mut self, account_map: &AccountMap) -> Result<()> {
        let market_data = try_get_account_data(account_map, &self.key)?;
        let market_state = MarketTwo::try_deserialize(&mut market_data.as_ref())?;
        // update market state
        self.market = market_state;

        let vault_data = try_get_account_data(account_map, &self.market.vault)?;
        let vault_state = Vault::try_deserialize(&mut vault_data.as_ref())?;

        let exchange_rate = vault_state.last_seen_sy_exchange_rate;
        // update exchange rate
        self.exchange_rate = Some(exchange_rate);

        // update reserves
        self.reserves = [
            self.market.financials.sy_balance,
            self.market.financials.pt_balance,
        ];

        Ok(())
    }

    fn quote(&self, quote_params: &QuoteParams) -> Result<Quote> {
        let time_now = self.timestamp.load(Ordering::Relaxed) as u64;
        let sy_exchange_rate = self.exchange_rate.unwrap();

        println!("\nquote_params: {:?}\n", quote_params);

        let is_buy_pt = quote_params.input_mint == self.reserve_mints[0];

        // ceil on asset balance when buying PT (make asset cheaper)
        // floor on asset balance when selling PT (make asset more expensive)
        let asset_balance = self.market.financials.asset_balance(sy_exchange_rate);
        let asset_balance = if is_buy_pt {
            asset_balance.ceil_u64()
        } else {
            asset_balance.floor_u64()
        };

        println!("\nsy_exchange_rate: {}\n", sy_exchange_rate);

        let current_rate_scalar = self.market.financials.current_rate_scalar(time_now);
        let current_rate_anchor = self
            .market
            .financials
            .current_rate_anchor(sy_exchange_rate, time_now);
        let current_fee_rate = self.market.financials.cur_fee_rate(time_now);

        if is_buy_pt {
            let TradeAssetResult {
                asset_fee,
                net_trader_pt,
            } = trade_asset(
                self.market.financials.pt_balance,
                asset_balance,
                current_rate_scalar,
                current_rate_anchor,
                current_fee_rate,
                Num::from_i64(-(quote_params.amount as i64)),
                false,
            );

            println!(
                "\nBUY PT:\nnet_trader_asset: {},\nasset_fee: {},\nnet_trader_pt: {} \n",
                -(quote_params.amount as i64),
                asset_fee,
                net_trader_pt
            );

            return Ok(Quote {
                fee_pct: Decimal::default(), //TODO calc percent
                in_amount: quote_params.amount,
                out_amount: net_trader_pt as u64,
                fee_amount: asset_fee as u64,
                fee_mint: self.market.mint_sy,
            });
        } else {
            let TradeResult {
                asset_fee,
                net_trader_asset,
            } = trade(
                self.market.financials.pt_balance,
                asset_balance,
                current_rate_scalar,
                current_rate_anchor,
                current_fee_rate,
                Num::from_i64(-(quote_params.amount as i64)),
                false,
            );

            println!(
                "\nSELL PT:\nnet_trader_pt: {},\n asset_fee: {},\nnet_trader_asset: {}\n",
                -(quote_params.amount as i64),
                asset_fee,
                net_trader_asset
            );

            return Ok(Quote {
                fee_pct: Decimal::default(), //TODO calc percent
                in_amount: quote_params.amount,
                out_amount: net_trader_asset as u64,
                fee_amount: asset_fee as u64,
                fee_mint: self.market.mint_sy,
            });
        }
    }

    fn supports_exact_out(&self) -> bool {
        //? Just for now. I guess that we have all mechanisms to implement exact out
        false
    }

    fn get_swap_and_account_metas(&self, swap_params: &SwapParams) -> Result<SwapAndAccountMetas> {
        let SwapParams {
            token_transfer_authority,
            ..
        } = swap_params;

        Ok(SwapAndAccountMetas {
            swap: Swap::TokenSwap, //TODO what swap method here? I think we need to contact jup team so they can add our Swap method
            //? do_cpi_trade_pt accounts
            account_metas: TradePt {
                trader: *token_transfer_authority,
                market: self.market.self_address,
                token_sy_trader: Pubkey::default(),
                token_pt_trader: Pubkey::default(),
                token_sy_escrow: self.market.token_sy_escrow,
                token_pt_escrow: self.market.token_pt_escrow,
                address_lookup_table: self.market.address_lookup_table,
                token_program: spl_token::id(),
                sy_program: self.market.sy_program,
                token_fee_treasury_sy: self.market.token_fee_treasury_sy,
            }
            .into(),
        })
    }

    fn clone_amm(&self) -> Box<dyn Amm + Send + Sync> {
        Box::new(self.clone())
    }
}
