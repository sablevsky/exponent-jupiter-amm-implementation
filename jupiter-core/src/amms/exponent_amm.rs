use anchor_lang::prelude::AccountMeta;
use anchor_lang::AccountDeserialize;
use anyhow::Result;
use exponent_time_curve::num::Num;
use jupiter_amm_interface::{
    try_get_account_data, AccountMap, Amm, AmmContext, KeyedAccount, Quote, QuoteParams, Swap,
    SwapAndAccountMetas, SwapParams,
};
use lazy_static::lazy_static;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use solana_sdk::{address_lookup_table::state::AddressLookupTable, pubkey, pubkey::Pubkey};
use spl_associated_token_account::get_associated_token_address;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use crate::exponent::{
    cpi_contexts_to_account_metas, precise_number::Number, trade, trade_asset, unique_cpi_contexts,
    CpiAccounts, FundAccount, MarketTwo, MintSyAccounts, RedeemSyAccounts, TradeAssetResult,
    TradePt, TradeResult,
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
    pub fund_account: Pubkey,
    pub sy_meta_address: Pubkey,
}

lazy_static! {
    pub static ref MARKET_ADDITIONAL_DATA: HashMap<Pubkey, AdditionalMarketData> = {
        let mut m = HashMap::new();

        let wfragsol_market_key = pubkey!("EJ4GPTCnNtemBVrT7QKhRfSKfM53aV2UJYGAC8gdVz5b");
        let wfragsol_additional_data = AdditionalMarketData {
            original_mint: pubkey!("WFRGSWjaz8tbAxsJitmbfRuFV2mSNwy7BMWcCwaA28U"),
            virtual_exchange_rate: true,
            fund_account: pubkey!("3TK9fNePM4qdKC4dwvDe8Bamv14prDqdVfuANxPeiryb"), //? Can be generated from FRAGMETRIC_PROGRAM_ID
            sy_meta_address: pubkey!("8EC8D6FG4ATRTScZvziTgtrcMv9Edvwv3hHmZNdnTCg"),
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
    exchange_rate: Option<Number>, //? I.e. How much sol should be paid for 1 wfrag_sol
    market: MarketTwo,
    market_additional_data: AdditionalMarketData,
    market_lookup_table_accounts: Option<Vec<Pubkey>>,
    timestamp: Arc<AtomicI64>,
    reserves: [u64; 2],
    program_id: Pubkey,
}

impl Amm for ExponentAmm {
    fn from_keyed_account(keyed_account: &KeyedAccount, amm_context: &AmmContext) -> Result<Self> {
        let market_state = MarketTwo::try_deserialize(&mut keyed_account.account.data.as_ref())?;

        let market_additional_data = MARKET_ADDITIONAL_DATA
            .get(&market_state.self_address)
            .unwrap();

        let reserve_mints: [Pubkey; 2] =
            [market_additional_data.original_mint, market_state.mint_pt];

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
            market_additional_data: market_additional_data.clone(),
            market_lookup_table_accounts: None,
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
        // Update market, fund account(to get exchange rate) and lookup table
        vec![
            self.market.self_address,
            self.market_additional_data.fund_account,
            self.market.address_lookup_table,
        ]
    }

    fn update(&mut self, account_map: &AccountMap) -> Result<()> {
        let market_data = try_get_account_data(account_map, &self.key)?;
        let market_state = MarketTwo::try_deserialize(&mut market_data.as_ref())?;
        // update market state
        self.market = market_state;

        let fund_account_data =
            try_get_account_data(account_map, &self.market_additional_data.fund_account)?;
        let fund_account: &FundAccount =
            bytemuck::from_bytes(&fund_account_data[8..std::mem::size_of::<FundAccount>() + 8]);
        let exchange_rate = fund_account.exchange_rate();
        // set exchange rate
        self.exchange_rate = Some(exchange_rate);

        // update reserves
        self.reserves = [
            self.market.financials.sy_balance,
            self.market.financials.pt_balance,
        ];

        let market_lookup_table_data =
            try_get_account_data(account_map, &self.market.address_lookup_table)?;
        let lookup_table_addresses = AddressLookupTable::deserialize(&market_lookup_table_data)
            .unwrap()
            .addresses
            .to_vec();

        self.market_lookup_table_accounts = Some(lookup_table_addresses);

        // println!("\nlookup_table: {:?}\n", lookup_table_addresses);

        Ok(())
    }

    fn quote(&self, quote_params: &QuoteParams) -> Result<Quote> {
        let time_now = self.timestamp.load(Ordering::Relaxed) as u64;
        let sy_exchange_rate = self.exchange_rate.unwrap();

        let virtual_exchange_rate = self.market_additional_data.virtual_exchange_rate;

        // println!("\nquote_params: {:?}\n", quote_params);

        let is_buy_pt = quote_params.input_mint == self.reserve_mints[0];

        // ceil on asset balance when buying PT (make asset cheaper)
        // floor on asset balance when selling PT (make asset more expensive)
        let asset_balance = self.market.financials.asset_balance(sy_exchange_rate);
        let asset_balance = if is_buy_pt {
            asset_balance.ceil_u64()
        } else {
            asset_balance.floor_u64()
        };

        // println!("\nsy_exchange_rate: {}\n", sy_exchange_rate);

        let current_rate_scalar = self.market.financials.current_rate_scalar(time_now);
        let current_rate_anchor = self
            .market
            .financials
            .current_rate_anchor(sy_exchange_rate, time_now);
        let current_fee_rate = self.market.financials.cur_fee_rate(time_now);

        if is_buy_pt {
            let net_trader_asset = if virtual_exchange_rate {
                let trader_asset: Number =
                    Number::from_natural_u64(quote_params.amount) * sy_exchange_rate;
                -(trader_asset.floor_u64().to_i64().unwrap())
            } else {
                -(quote_params.amount as i64)
            };

            let TradeAssetResult {
                asset_fee,
                net_trader_pt: out_amount,
            } = trade_asset(
                self.market.financials.pt_balance,
                asset_balance,
                current_rate_scalar,
                current_rate_anchor,
                current_fee_rate,
                Num::from_i64(net_trader_asset),
                false,
            );

            // println!(
            //     "\nBUY PT:\nnet_trader_asset: {},\nasset_fee: {},\nnet_trader_pt: {} \n",
            //     -(quote_params.amount as i64),
            //     asset_fee,
            //     out_amount
            // );

            return Ok(Quote {
                fee_pct: Decimal::default(), //TODO How to calculate fee_pct in a proper way?
                in_amount: quote_params.amount,
                out_amount: out_amount as u64,
                fee_amount: asset_fee as u64,
                fee_mint: self.market.mint_sy,
            });
        } else {
            let net_trader_pt = if virtual_exchange_rate {
                let trader_asset = Number::from_natural_u64(quote_params.amount) / sy_exchange_rate;
                -(trader_asset.floor_u64().to_i64().unwrap())
            } else {
                -(quote_params.amount as i64)
            };

            let TradeResult {
                asset_fee,
                net_trader_asset,
            } = trade(
                self.market.financials.pt_balance,
                asset_balance,
                current_rate_scalar,
                current_rate_anchor,
                current_fee_rate,
                Num::from_i64(net_trader_pt),
                false,
            );

            // println!(
            //     "\nSELL PT:\nnet_trader_pt: {},\n asset_fee: {},\nnet_trader_asset: {}\n",
            //     -(quote_params.amount as i64),
            //     asset_fee,
            //     net_trader_asset
            // );

            return Ok(Quote {
                fee_pct: Decimal::default(), //TODO How to calculate fee_pct in a proper way?
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
            source_mint,
            ..
        } = swap_params;

        let is_buy_pt = *source_mint == self.reserve_mints[0];

        let user_base_token_ata = get_associated_token_address(
            &token_transfer_authority,
            &self.market_additional_data.original_mint,
        );
        let user_sy_token_ata =
            get_associated_token_address(&token_transfer_authority, &self.market.mint_sy);
        let user_pt_token_ata =
            get_associated_token_address(&token_transfer_authority, &self.market.mint_pt);

        let meta_base_token_ata = get_associated_token_address(
            &self.market_additional_data.sy_meta_address,
            &self.market_additional_data.original_mint,
        );

        let trade_metas: Vec<AccountMeta> = TradePt {
            trader: *token_transfer_authority,
            market: self.market.self_address,
            token_sy_trader: user_sy_token_ata,
            token_pt_trader: user_pt_token_ata,
            token_sy_escrow: self.market.token_sy_escrow,
            token_pt_escrow: self.market.token_pt_escrow,
            address_lookup_table: self.market.address_lookup_table,
            token_program: spl_token::id(),
            sy_program: self.market.sy_program,
            token_fee_treasury_sy: self.market.token_fee_treasury_sy,
        }
        .into();

        let CpiAccounts {
            get_sy_state,
            deposit_sy,
            withdraw_sy,
            ..
        } = self.market.cpi_accounts.clone();

        if is_buy_pt {
            let unique_cpi =
                unique_cpi_contexts(&[get_sy_state.as_slice(), deposit_sy.as_slice()].concat());

            let remaining_accounts: Vec<AccountMeta> = cpi_contexts_to_account_metas(
                &unique_cpi,
                self.market_lookup_table_accounts.as_ref().unwrap(),
            )
            .into();

            let mint_sy_remaining_accounts: Vec<AccountMeta> = MintSyAccounts {
                depositor: *token_transfer_authority,
                meta: self.market_additional_data.sy_meta_address,
                mint_sy: self.market.mint_sy,
                token_base_depositor: user_base_token_ata, //? user wfragSOL ata
                token_yield_bearing_escrow: meta_base_token_ata, //? ATA wsol and meta?
                token_sy_depositor: user_sy_token_ata,     //? user syToken ata
                base_token_program: spl_token::id(),
                token_program: spl_token::id(),
            }
            .into();

            let account_metas = [
                remaining_accounts.as_slice(),
                mint_sy_remaining_accounts.as_slice(),
                trade_metas.as_slice(),
            ]
            .concat();

            //? Param that is needed for buy_pt instruction
            let mint_sy_rem_accounts_until = mint_sy_remaining_accounts.len();

            Ok(SwapAndAccountMetas {
                swap: Swap::TokenSwap, //TODO change swap method here
                //? do_cpi_trade_pt accounts
                account_metas,
            })
        } else {
            let unique_cpi =
                unique_cpi_contexts(&[get_sy_state.as_slice(), withdraw_sy.as_slice()].concat());

            let remaining_accounts: Vec<AccountMeta> = cpi_contexts_to_account_metas(
                &unique_cpi,
                self.market_lookup_table_accounts.as_ref().unwrap(),
            )
            .into();

            let redeem_sy_accounts: Vec<AccountMeta> = RedeemSyAccounts {
                signer: *token_transfer_authority,
                meta: self.market_additional_data.sy_meta_address,
                token_base_dst: user_base_token_ata, //? user wfragSOL ata
                token_yield_bearing_escrow: meta_base_token_ata, //? ATA wsol and meta
                token_sy_signer: user_sy_token_ata,  //? user syToken ata
                mint_sy: self.market.mint_sy,
                base_token_program: spl_token::id(),
                token_program: spl_token::id(),
            }
            .into();

            let account_metas = [
                remaining_accounts.as_slice(),
                redeem_sy_accounts.as_slice(),
                trade_metas.as_slice(),
            ]
            .concat();

            //? Param that is needed for sell_pt instruction
            let redeem_sy_rem_accounts_until = redeem_sy_accounts.len();

            Ok(SwapAndAccountMetas {
                swap: Swap::TokenSwap, //TODO change swap method here
                account_metas,
            })
        }
    }

    fn clone_amm(&self) -> Box<dyn Amm + Send + Sync> {
        Box::new(self.clone())
    }
}
