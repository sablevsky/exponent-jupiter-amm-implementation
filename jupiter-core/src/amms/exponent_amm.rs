use anchor_lang::AccountDeserialize;
use anchor_lang::{prelude::*, solana_program::borsh1};
use anyhow::Result;
use exponent_time_curve::num::Num;
use jupiter_amm_interface::{
    try_get_account_data, AccountMap, Amm, AmmContext, KeyedAccount, Quote, QuoteParams, Swap,
    SwapAndAccountMetas, SwapParams,
};
use lazy_static::lazy_static;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;
use solana_sdk::{address_lookup_table::state::AddressLookupTable, pubkey, pubkey::Pubkey};
use spl_associated_token_account::get_associated_token_address;
use spl_stake_pool::state::StakePool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use crate::exponent::kyros;
use crate::exponent::{
    fragmetric,
    kyros::JitoVault,
    math::{trade, trade_asset, TradeAssetResult},
    precise_number::Number,
    state::{
        cpi_contexts_to_account_metas, CpiAccounts, CpiInterfaceContext, FundAccount, MarketTwo,
    },
    unique_cpi_contexts, TradePt, TradeResult,
};

mod exponent_swap_programs {
    use super::*;
    pub const EXPONENT_CORE: Pubkey = pubkey!("ExponentnaRg3CQbW6dqQNZKXp7gtZ9DGMp1cwC4HAS7");
    pub const EXPONENT_EVENT_AUTHORITY: Pubkey =
        pubkey!("2qFqt7c5teKuuTMT7FCG24DzvsUwicuYovGpKAoB2XnK");
}

mod exponent_hardcoded_amm_data {
    use super::*;

    pub const WFRAGSOL_MARKET: Pubkey = pubkey!("EJ4GPTCnNtemBVrT7QKhRfSKfM53aV2UJYGAC8gdVz5b");
    pub const WFRAGSOL_MINT: Pubkey = pubkey!("WFRGSWjaz8tbAxsJitmbfRuFV2mSNwy7BMWcCwaA28U");
    pub const WFRAGSOL_FUND_ACCOUNT: Pubkey =
        pubkey!("3TK9fNePM4qdKC4dwvDe8Bamv14prDqdVfuANxPeiryb");
    pub const WFRAGSOL_SY_META_ADDRESS: Pubkey =
        pubkey!("8EC8D6FG4ATRTScZvziTgtrcMv9Edvwv3hHmZNdnTCg");

    pub const KYSOL_MARKET: Pubkey = pubkey!("3xckb8Z5NfqptY4Pzg3KQ1bPr8ufB8CE4gJo4YMVsXvi");
    pub const KYSOL_MINT: Pubkey = pubkey!("kySo1nETpsZE2NWe5vj2C64mPSciH1SppmHb4XieQ7B");
    pub const KYSOL_SY_META_ADDRESS: Pubkey =
        pubkey!("4u2L26Bu8Cs1ZbWCPhv6mUtUxeqL5xC8cPh81DoW15Ad");

    pub const JITO_STAKE_POOL: Pubkey = pubkey!("Jito4APyf642JPZPx3hGc6WWJ8zPKtRbRs4P815Awbb");
    pub const JITO_VAULT: Pubkey = pubkey!("CQpvXgoaaawDCLh8FwMZEwQqnPakRUZ5BnzhjnEBPJv");
    pub const JITO_TOKEN_MINT: Pubkey = pubkey!("jtojtomepa8beP8AuQc6eXt5FriJwfFMwQx2v2f9mCL");
}

lazy_static! {
    pub static ref EXPONENT_SWAP_PROGRAMS: HashMap<Pubkey, String> = {
        let mut m = HashMap::new();
        m.insert(exponent_swap_programs::EXPONENT_CORE, "Exponent".into());
        m
    };
}

//? Common data for all markets
#[derive(Clone)]
pub struct AdditionalMarketData {
    pub original_mint: Pubkey,
    pub is_virtual_exchange_rate: bool,
    pub sy_meta_address: Pubkey,
}

pub fn get_market_additional_data(
    market_pubkey: Pubkey,
) -> Result<AdditionalMarketData, anyhow::Error> {
    if market_pubkey == exponent_hardcoded_amm_data::WFRAGSOL_MARKET {
        Ok(AdditionalMarketData {
            original_mint: exponent_hardcoded_amm_data::WFRAGSOL_MINT,
            is_virtual_exchange_rate: true,
            sy_meta_address: exponent_hardcoded_amm_data::WFRAGSOL_SY_META_ADDRESS,
        })
    } else if market_pubkey == exponent_hardcoded_amm_data::KYSOL_MARKET {
        Ok(AdditionalMarketData {
            original_mint: exponent_hardcoded_amm_data::KYSOL_MINT,
            is_virtual_exchange_rate: true,
            sy_meta_address: exponent_hardcoded_amm_data::KYSOL_SY_META_ADDRESS,
        })
    } else {
        Err(anyhow::anyhow!("Unknown market pubkey"))
    }
}

//? Stores specific data for each market
#[derive(Clone)]
pub enum ExponentAmmType {
    WFragSol {
        fund_account: Pubkey,
    },
    KySol {
        jito_stake_pool: Pubkey,
        jito_vault: Pubkey,
    },
}

pub fn get_amm_type_from_market(market_pubkey: Pubkey) -> Result<ExponentAmmType, anyhow::Error> {
    if market_pubkey == exponent_hardcoded_amm_data::WFRAGSOL_MARKET {
        Ok(ExponentAmmType::WFragSol {
            fund_account: exponent_hardcoded_amm_data::WFRAGSOL_FUND_ACCOUNT,
        })
    } else if market_pubkey == exponent_hardcoded_amm_data::KYSOL_MARKET {
        Ok(ExponentAmmType::KySol {
            jito_stake_pool: exponent_hardcoded_amm_data::JITO_STAKE_POOL,
            jito_vault: exponent_hardcoded_amm_data::JITO_VAULT,
        })
    } else {
        Err(anyhow::anyhow!("Unknown market pubkey"))
    }
}

#[derive(Clone)]
pub struct ExponentAmm {
    key: Pubkey,
    label: String,
    amm_type: ExponentAmmType,
    reserve_mints: [Pubkey; 2],
    sy_exchange_rate: Option<Number>, //? I.e. How much sol should be paid for 1 sy_wfrag_sol
    market: MarketTwo,
    market_additional_data: AdditionalMarketData,
    market_lookup_table_accounts: Option<Vec<Pubkey>>,
    timestamp: Arc<AtomicI64>,
    reserves: [u64; 2],
    program_id: Pubkey,
}

impl ExponentAmm {
    fn get_sy_exchange_rate(&mut self, account_map: &AccountMap) -> Result<Number, anyhow::Error> {
        match self.amm_type {
            ExponentAmmType::WFragSol { fund_account, .. } => {
                let fund_account_data = try_get_account_data(account_map, &fund_account)?;
                let fund_account: &FundAccount = bytemuck::from_bytes(
                    &fund_account_data[8..std::mem::size_of::<FundAccount>() + 8],
                );
                Ok(fund_account.exchange_rate())
            }
            ExponentAmmType::KySol {
                jito_stake_pool,
                jito_vault,
                ..
            } => {
                let jito_vault_account_data = try_get_account_data(account_map, &jito_vault)?;
                let jito_vault = JitoVault::deserialize(&jito_vault_account_data);
                let jito_vault_exchange_rate = jito_vault.exchange_index();

                let stake_pool_account_data = try_get_account_data(account_map, &jito_stake_pool)?;
                let stake_pool: StakePool =
                    borsh1::try_from_slice_unchecked(&mut stake_pool_account_data.as_ref())?;
                let stake_pool_rate = Number::from_natural_u64(stake_pool.total_lamports)
                    .checked_div(&Number::from_natural_u64(stake_pool.pool_token_supply))
                    .unwrap();

                let exchange_rate = stake_pool_rate
                    .checked_mul(&jito_vault_exchange_rate)
                    .unwrap();

                Ok(exchange_rate)
            }
        }
    }

    fn get_specific_accounts_to_update(&self) -> Vec<Pubkey> {
        match self.amm_type {
            ExponentAmmType::WFragSol { fund_account, .. } => vec![fund_account],
            ExponentAmmType::KySol {
                jito_stake_pool,
                jito_vault,
                ..
            } => vec![jito_stake_pool, jito_vault],
        }
    }

    fn get_trade_metas(&self, wallet_address: Pubkey) -> Vec<AccountMeta> {
        let user_sy_token_ata = get_associated_token_address(&wallet_address, &self.market.mint_sy);
        let user_pt_token_ata = get_associated_token_address(&wallet_address, &self.market.mint_pt);

        TradePt {
            trader: wallet_address,
            market: self.market.self_address,
            token_sy_trader: user_sy_token_ata,
            token_pt_trader: user_pt_token_ata,
            token_sy_escrow: self.market.token_sy_escrow,
            token_pt_escrow: self.market.token_pt_escrow,
            address_lookup_table: self.market.address_lookup_table,
            token_program: spl_token::id(),
            sy_program: self.market.sy_program,
            token_fee_treasury_sy: self.market.token_fee_treasury_sy,
            event_authority: exponent_swap_programs::EXPONENT_EVENT_AUTHORITY,
            exponent_program_id: exponent_swap_programs::EXPONENT_CORE,
        }
        .into()
    }

    fn get_mint_sy_metas(&self, wallet_address: Pubkey) -> Vec<AccountMeta> {
        let user_base_token_ata = get_associated_token_address(
            &wallet_address,
            &self.market_additional_data.original_mint,
        );
        let user_sy_token_ata = get_associated_token_address(&wallet_address, &self.market.mint_sy);
        let meta_base_token_ata = get_associated_token_address(
            &self.market_additional_data.sy_meta_address,
            &self.market_additional_data.original_mint,
        );
        let jito_token_meta_ata = get_associated_token_address(
            &self.market_additional_data.sy_meta_address,
            &exponent_hardcoded_amm_data::JITO_TOKEN_MINT,
        );

        match self.amm_type {
            ExponentAmmType::WFragSol { .. } => fragmetric::MintSyAccounts {
                depositor: wallet_address,
                meta: self.market_additional_data.sy_meta_address,
                mint_sy: self.market.mint_sy,
                token_base_depositor: user_base_token_ata,
                token_yield_bearing_escrow: meta_base_token_ata,
                token_sy_depositor: user_sy_token_ata,
                base_token_program: spl_token::id(),
                token_program: spl_token::id(),
                wfragsol_fund_account: exponent_hardcoded_amm_data::WFRAGSOL_FUND_ACCOUNT,
                jito_token_meta_ata,
            }
            .into(),
            ExponentAmmType::KySol { jito_vault, .. } => kyros::MintSyAccounts {
                depositor: wallet_address,
                meta: self.market_additional_data.sy_meta_address,
                mint_sy: self.market.mint_sy,
                token_base_depositor: user_base_token_ata,
                token_vrt_escrow: meta_base_token_ata,
                token_sy_depositor: user_sy_token_ata,
                jito_vault,
                base_token_program: spl_token::id(),
                token_program: spl_token::id(),
                jito_stake_pool: exponent_hardcoded_amm_data::JITO_STAKE_POOL,
                jito_token_meta_ata,
            }
            .into(),
        }
    }

    fn get_redeem_sy_metas(&self, wallet_address: Pubkey) -> Vec<AccountMeta> {
        let user_base_token_ata = get_associated_token_address(
            &wallet_address,
            &self.market_additional_data.original_mint,
        );
        let user_sy_token_ata = get_associated_token_address(&wallet_address, &self.market.mint_sy);
        let meta_base_token_ata = get_associated_token_address(
            &self.market_additional_data.sy_meta_address,
            &self.market_additional_data.original_mint,
        );
        let jito_token_meta_ata = get_associated_token_address(
            &self.market_additional_data.sy_meta_address,
            &exponent_hardcoded_amm_data::JITO_TOKEN_MINT,
        );

        match self.amm_type {
            ExponentAmmType::WFragSol { .. } => fragmetric::RedeemSyAccounts {
                signer: wallet_address,
                meta: self.market_additional_data.sy_meta_address,
                token_base_dst: user_base_token_ata,
                token_yield_bearing_escrow: meta_base_token_ata,
                token_sy_signer: user_sy_token_ata,
                mint_sy: self.market.mint_sy,
                base_token_program: spl_token::id(),
                token_program: spl_token::id(),
                wfragsol_fund_account: exponent_hardcoded_amm_data::WFRAGSOL_FUND_ACCOUNT,
                jito_token_meta_ata,
            }
            .into(),
            ExponentAmmType::KySol { jito_vault, .. } => kyros::RedeemSyAccounts {
                signer: wallet_address,
                meta: self.market_additional_data.sy_meta_address,
                jito_vault,
                token_base_dst: user_base_token_ata,
                token_vrt_escrow: meta_base_token_ata,
                token_sy_signer: user_sy_token_ata,
                mint_sy: self.market.mint_sy,
                base_token_program: spl_token::id(),
                token_program: spl_token::id(),
                jito_stake_pool: exponent_hardcoded_amm_data::JITO_STAKE_POOL,
                jito_token_meta_ata,
            }
            .into(),
        }
    }

    fn get_remaining_accounts_metas(&self, contexts: &[CpiInterfaceContext]) -> Vec<AccountMeta> {
        let unique_cpi = unique_cpi_contexts(contexts);
        cpi_contexts_to_account_metas(
            &unique_cpi,
            self.market_lookup_table_accounts.as_ref().unwrap(),
        )
        .into()
    }

    fn get_trade_func_math_args(&self, is_buy_pt: bool) -> TradeFuncMathArgs {
        let time_now = self.timestamp.load(Ordering::Relaxed) as u64;
        let sy_exchange_rate = self.sy_exchange_rate.unwrap();

        let market_asset_balance = self.market.financials.asset_balance(sy_exchange_rate);
        let asset_balance = if is_buy_pt {
            market_asset_balance.ceil_u64()
        } else {
            market_asset_balance.floor_u64()
        };
        let current_rate_scalar = self.market.financials.current_rate_scalar(time_now);
        let current_rate_anchor = self
            .market
            .financials
            .current_rate_anchor(sy_exchange_rate, time_now);
        let current_fee_rate = self.market.financials.cur_fee_rate(time_now);

        return TradeFuncMathArgs {
            market_asset: asset_balance,
            current_rate_scalar,
            current_rate_anchor,
            current_fee_rate,
        };
    }

    fn get_quote(&self, quote_params: &QuoteParams) -> Result<Quote> {
        let is_virtual_exchange_rate = self.market_additional_data.is_virtual_exchange_rate;
        let sy_exchange_rate = self.sy_exchange_rate.unwrap();
        let is_buy_pt = false;

        let TradeFuncMathArgs {
            market_asset,
            current_rate_scalar,
            current_rate_anchor,
            current_fee_rate,
        } = self.get_trade_func_math_args(is_buy_pt);

        if is_buy_pt {
            //? Adjust the amount for markets with virtual exchange rate
            let net_trader_asset = if is_virtual_exchange_rate {
                let trader_asset: Number =
                    Number::from_natural_u64(quote_params.amount) * sy_exchange_rate;
                -(trader_asset.floor_u64().to_i64().unwrap())
            } else {
                -(quote_params.amount as i64)
            };

            let TradeAssetResult {
                asset_fee: fee_amount,
                net_trader_pt: out_amount,
            } = trade_asset(
                self.market.financials.pt_balance,
                market_asset,
                current_rate_scalar,
                current_rate_anchor,
                current_fee_rate,
                Num::from_i64(net_trader_asset),
                false,
            );

            let fee_pct = Decimal::from_u64(fee_amount as u64)
                .unwrap()
                .checked_div(Decimal::from_u64(out_amount as u64).unwrap())
                .unwrap();

            // println!(
            //     "\nBUY PT:\ninput_mint: {},\noutput_mint: {},\nin_amount: {},\nout_amount: {}\nfee_amount: {}\nfee_pct: {}\n",
            //     quote_params.input_mint,
            //     quote_params.output_mint,
            //     quote_params.amount,
            //     out_amount as u64,
            //     fee_amount as u64,
            //     fee_pct,
            // );

            return Ok(Quote {
                fee_pct,
                fee_amount: fee_amount as u64,
                fee_mint: quote_params.input_mint,
                in_amount: quote_params.amount,
                out_amount: out_amount as u64,
            });
        } else {
            let TradeResult {
                asset_fee: sy_fee,
                net_trader_asset: sy_out_amount,
            } = trade(
                self.market.financials.pt_balance,
                market_asset,
                current_rate_scalar,
                current_rate_anchor,
                current_fee_rate,
                Num::from_i64(-(quote_params.amount as i64)),
                false,
            );

            //? Adjust the amount for markets with virtual exchange rate
            let out_amount = if is_virtual_exchange_rate {
                let amount = Number::from_natural_u64(sy_out_amount as u64) / sy_exchange_rate;
                amount.floor_u64()
            } else {
                sy_out_amount as u64
            };

            //? Adjust the amount for markets with virtual exchange rate
            let fee_amount = if is_virtual_exchange_rate {
                let asset_fee = Number::from_natural_u64(sy_fee as u64) / sy_exchange_rate;
                asset_fee.floor_u64()
            } else {
                sy_fee as u64
            };

            let fee_pct = Decimal::from_u64(fee_amount)
                .unwrap()
                .checked_div(Decimal::from_u64(out_amount).unwrap())
                .unwrap();

            // println!(
            //     "\nSELL PT:\ninput_mint: {},\noutput_mint: {},\nnet_trader_asset: {},\nnet_trader_pt: {}\nfee_amount: {}\nfee_pct: {}\n",
            //     quote_params.input_mint,
            //     quote_params.output_mint,
            //     -(quote_params.amount as i64),
            //     out_amount,
            //     fee_amount,
            //     fee_pct,
            // );

            return Ok(Quote {
                fee_pct,
                in_amount: quote_params.amount,
                out_amount,
                fee_amount: fee_amount,
                fee_mint: quote_params.output_mint,
            });
        }
    }
}

struct TradeFuncMathArgs {
    market_asset: u64,
    current_rate_scalar: f64,
    current_rate_anchor: f64,
    current_fee_rate: f64,
}

impl Amm for ExponentAmm {
    fn from_keyed_account(keyed_account: &KeyedAccount, amm_context: &AmmContext) -> Result<Self> {
        let market_state = MarketTwo::try_deserialize(&mut keyed_account.account.data.as_ref())?;

        let market_additional_data = get_market_additional_data(market_state.self_address).unwrap();

        let reserve_mints: [Pubkey; 2] =
            [market_additional_data.original_mint, market_state.mint_pt];

        let label = EXPONENT_SWAP_PROGRAMS
            .get(&keyed_account.account.owner)
            .unwrap()
            .clone();

        // let epoch = amm_context.clock_ref.epoch.clone();
        let timestamp = amm_context.clock_ref.unix_timestamp.clone();

        let amm_type = get_amm_type_from_market(market_state.self_address).unwrap();

        Ok(Self {
            key: keyed_account.key,
            label,
            amm_type,
            reserve_mints,
            sy_exchange_rate: None,
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
        let specific_accounts = self.get_specific_accounts_to_update();

        // Update market, fund account(to get exchange rate) and lookup table
        let mut accounts = vec![self.market.self_address, self.market.address_lookup_table];
        accounts.extend(specific_accounts);
        accounts
    }

    fn update(&mut self, account_map: &AccountMap) -> Result<()> {
        let market_data = try_get_account_data(account_map, &self.key)?;
        let market_state = MarketTwo::try_deserialize(&mut market_data.as_ref())?;
        // update market state
        self.market = market_state;

        self.sy_exchange_rate = Some(self.get_sy_exchange_rate(account_map)?);

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

        Ok(())
    }

    fn quote(&self, quote_params: &QuoteParams) -> Result<Quote> {
        self.get_quote(quote_params)
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
        let exchange_rate_f64 = self.sy_exchange_rate.unwrap().to_f64().unwrap();
        let is_virtual_exchange_rate = self.market_additional_data.is_virtual_exchange_rate;

        let trade_metas: Vec<AccountMeta> = self.get_trade_metas(*token_transfer_authority);

        let CpiAccounts {
            get_sy_state,
            deposit_sy,
            withdraw_sy,
            ..
        } = self.market.cpi_accounts.clone();

        if is_buy_pt {
            let remaining_accounts: Vec<AccountMeta> = self.get_remaining_accounts_metas(
                &[get_sy_state.as_slice(), deposit_sy.as_slice()].concat(),
            );

            let mint_sy_remaining_accounts: Vec<AccountMeta> =
                self.get_mint_sy_metas(*token_transfer_authority);

            let exponent_program_account_meta = vec![AccountMeta::new_readonly(
                exponent_swap_programs::EXPONENT_CORE,
                false,
            )];

            let account_metas = [
                trade_metas.as_slice(),
                mint_sy_remaining_accounts.as_slice(),
                exponent_program_account_meta.as_slice(),
                remaining_accounts.as_slice(),
            ]
            .concat();

            //? Param that is needed for buy_pt instruction
            let mint_sy_rem_accounts_until = mint_sy_remaining_accounts.len();

            Ok(SwapAndAccountMetas {
                swap: Swap::Exponent {
                    exchange_rate: exchange_rate_f64,
                    is_virtual_exchange_rate,
                    rem_accounts_until: mint_sy_rem_accounts_until,
                },
                account_metas,
            })
        } else {
            let remaining_accounts: Vec<AccountMeta> = self.get_remaining_accounts_metas(
                &[get_sy_state.as_slice(), withdraw_sy.as_slice()].concat(),
            );

            let redeem_sy_accounts: Vec<AccountMeta> =
                self.get_redeem_sy_metas(*token_transfer_authority);

            let account_metas = [
                trade_metas.as_slice(),
                redeem_sy_accounts.as_slice(),
                remaining_accounts.as_slice(),
            ]
            .concat();

            //? Param that is needed for sell_pt instruction
            let redeem_sy_rem_accounts_until = redeem_sy_accounts.len();

            Ok(SwapAndAccountMetas {
                swap: Swap::Exponent {
                    exchange_rate: exchange_rate_f64,
                    is_virtual_exchange_rate,
                    rem_accounts_until: redeem_sy_rem_accounts_until,
                },
                account_metas,
            })
        }
    }

    fn clone_amm(&self) -> Box<dyn Amm + Send + Sync> {
        Box::new(self.clone())
    }
}
