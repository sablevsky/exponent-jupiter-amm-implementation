//? Copied as it is from new-trade-sy-ix branch

use exponent_time_curve::num::Num;

const DAY_SEC: u64 = 86400;
const YEAR_SEC: u64 = DAY_SEC * 365;

/// Get the proportion of PT in the market
pub fn proportion_pt<N: Num>(pt: u64, asset: u64) -> N {
    let pt = N::from_u64(pt);
    let asset = N::from_u64(asset);
    pt / (pt + asset)
}

/// Logit function
pub fn logit<N: Num>(p: N) -> N {
    (p / (N::one() - p)).ln()
}

pub fn exchange_rate<N: Num>(logit: N, rate_scalar: N, rate_anchor: N) -> N {
    logit / rate_scalar + rate_anchor
}

/// Calculate the fee rate for a given time remaining and fee rate root
pub fn fee_rate<N: Num>(ln_fee_rate_root: N, sec_remaining: u64) -> N {
    let normalized_sec_remaining = normalized_sec_remaining::<N>(sec_remaining);
    let ln_fee_rate = ln_fee_rate_root * normalized_sec_remaining;
    ln_fee_rate.exp()
}

pub struct MarketProportions {
    pub pt: u64,
    pub sy: u64,
}

/// Calculate the proportional shares of PT and SY for a quantity of LP tokens
pub fn lp_proportion<N: Num>(
    lp_tokens: u64,
    market_pt: u64,
    market_sy: u64,
    total_lp: u64,
) -> MarketProportions {
    assert!(total_lp > 0);

    let lp_tokens = N::from_u64(lp_tokens);
    let market_pt = N::from_u64(market_pt);
    let market_sy = N::from_u64(market_sy);
    let total_lp = N::from_u64(total_lp);

    let pt = market_pt * lp_tokens / total_lp;
    let sy = market_sy * lp_tokens / total_lp;

    MarketProportions {
        pt: pt.to_u64(),
        sy: sy.to_u64(),
    }
}

/// Find the initial scalar root
/// It will be small for high divergence between rate_max and rate_expected
/// And it will be small if the tail is wide
pub fn rate_scalar_root<N: Num>(rate_expected: N, rate_max: N, tail_width: Option<N>) -> N {
    // "tail width" defines the range of normal trading proportions
    // the expected trading proportions should be between [tail_width, 1 - tail_width]
    let tail_width = tail_width.unwrap_or(N::from_ratio(1, 10));

    // the tail width cannot be zero, nor greater than 1/2
    assert!(tail_width > N::zero() && tail_width < N::from_ratio(1, 2));

    // small values of tail width will result in a large scalar root
    let s = ((N::one() - tail_width) / tail_width).ln();

    // low rate
    let a = s / (rate_max - rate_expected);
    // high rate
    let b = s / (rate_expected - N::one());

    a.min(b)
}

/// Normalize the seconds remaining by year, which is useful when computing the implied yield rate
pub fn normalized_sec_remaining<N: Num>(sec_remaining: u64) -> N {
    N::from_ratio(sec_remaining, YEAR_SEC)
}

/// The rate scalar grows as the seconds remaining decrease
///
/// The sensitivity of the curve has an inverse relationship to the rate scalar
/// As the rate scalar grows, the sensitivity goes down
pub fn rate_scalar<N: Num>(rate_scalar_root: N, sec_remaining: u64) -> N {
    // if sec_remaining is zero, the rate scalar is effectively infinite
    if sec_remaining == 0 {
        return N::max();
    }

    let normalized_sec_remaining = normalized_sec_remaining::<N>(sec_remaining);

    // root * YEAR_SEC / sec_remaining
    rate_scalar_root / normalized_sec_remaining
}

/// Natural log fo the implied interest rate
pub fn ln_implied_rate<N: Num>(
    pt: u64,
    asset: u64,
    rate_scalar: N,
    rate_anchor: N,
    sec_remaining: u64,
) -> N {
    let l_p = logit(proportion_pt::<N>(pt, asset));
    let rate = exchange_rate(l_p, rate_scalar, rate_anchor);
    let ln_rate = rate.ln();
    let normalized_sec_remaining = normalized_sec_remaining::<N>(sec_remaining);

    ln_rate / normalized_sec_remaining
}

/// Exchange rate is e^rt
pub fn exchange_rate_from_ln_implied_rate<N: Num>(ln_implied_rate: N, sec_remaining: u64) -> N {
    let rt = ln_implied_rate * normalized_sec_remaining::<N>(sec_remaining);

    rt.exp()
}

pub struct AddLiquidityResult {
    pub lp_tokens_out: u64,
    pub sy_in: u64,
    pub pt_in: u64,
}

/// Calculate the amount of LP tokens received, and SY & PT put in
/// This is based on an intended amount of SY and PT
pub fn add_liquidity<N: Num>(
    intent_sy: u64,
    intent_pt: u64,
    market_total_lp: u64,
    market_total_sy: u64,
    market_total_pt: u64,
) -> AddLiquidityResult {
    let intent_sy = N::from_u64(intent_sy);
    let intent_pt = N::from_u64(intent_pt);
    let market_total_lp = N::from_u64(market_total_lp);
    let market_total_sy = N::from_u64(market_total_sy);
    let market_total_pt = N::from_u64(market_total_pt);

    let lp_from_pt = market_total_lp * intent_pt / market_total_pt;
    let lp_from_sy = market_total_lp * intent_sy / market_total_sy;

    if lp_from_pt < lp_from_sy {
        let lp_tokens_out = lp_from_pt;
        let pt_in = intent_pt.to_u64();
        let sy_in = ((market_total_sy * lp_tokens_out + market_total_lp - N::one())
            / market_total_lp)
            .to_u64();

        AddLiquidityResult {
            lp_tokens_out: lp_tokens_out.to_u64(),
            sy_in,
            pt_in,
        }
    } else {
        let lp_tokens_out = lp_from_sy;
        let sy_in = intent_sy.to_u64();
        let pt_in = ((market_total_pt * lp_tokens_out + market_total_lp - N::one())
            / market_total_lp)
            .to_u64();

        AddLiquidityResult {
            lp_tokens_out: lp_tokens_out.to_u64(),
            sy_in,
            pt_in,
        }
    }
}

pub struct RemoveLiquidityResult {
    pub sy_out: u64,
    pub pt_out: u64,
}

/// Remove liquidity from the pool, returning the amount of SY and PT received for LP tokens in
pub fn rm_liquidity<N: Num>(
    lp_in: u64,
    market_total_lp: u64,
    market_total_sy: u64,
    market_total_pt: u64,
) -> RemoveLiquidityResult {
    assert!(market_total_lp >= lp_in);
    let lp_in = N::from_u64(lp_in);
    let market_total_lp = N::from_u64(market_total_lp);
    let market_total_sy = N::from_u64(market_total_sy);
    let market_total_pt = N::from_u64(market_total_pt);

    let sy_out = market_total_sy * lp_in / market_total_lp;
    let pt_out = market_total_pt * lp_in / market_total_lp;

    RemoveLiquidityResult {
        sy_out: sy_out.to_u64(),
        pt_out: pt_out.to_u64(),
    }
}

/// Calculate amount of SY owned by an amount of LP tokens
pub fn lp_to_sy<N: Num>(
    lp_amount: u64,
    market_total_lp: u64,
    market_total_sy: u64,
    market_total_pt: u64,
) -> u64 {
    let r = rm_liquidity::<N>(lp_amount, market_total_lp, market_total_sy, market_total_pt);

    r.sy_out
}

/// Find a rate anchor that preserves the implied rate even though the exchange rate changes with time
pub fn find_rate_anchor<N: Num>(
    pt: u64,
    asset: u64,
    rate_scalar: N,
    last_ln_implied_rate: N,
    sec_remaining: u64,
) -> N {
    let new_exchange_rate =
        exchange_rate_from_ln_implied_rate::<N>(last_ln_implied_rate, sec_remaining);

    let p = logit(proportion_pt::<N>(pt, asset));

    // solve for rate anchor, which is a vertical translation on the curve
    new_exchange_rate - p / rate_scalar
}

/// Calculate the amount of asset fee on a trade
/// If net_trader_asset is positive, the trader is selling PT to receive asset, and the fee is positive (meaning they receive less asset)
/// If net_trader_asset is negative, the trader is selling asset to receive PT, and the fee is positive (meaning they pay more asset to receive PT)
pub fn asset_fee<N: Num>(net_trader_asset: N, fee_rate: N) -> N {
    assert!(fee_rate >= N::one());
    let is_sell_pt = net_trader_asset > N::zero();
    if is_sell_pt {
        // selling PT to buy asset
        net_trader_asset * (fee_rate - N::one())
    } else {
        // buying PT and spending asset
        -net_trader_asset * (fee_rate - N::one()) / fee_rate
    }
}

/// Result of a trade where the input is a fixed amount of PT
pub struct TradeResult<N: Num> {
    /// The change of asset for the trader after the fee is taken into account
    pub net_trader_asset: N,

    /// Positive asset fee
    ///
    /// When receiving asset, a trader gets *less* by the fee amount
    /// When sending asset, a trader sends *more* by the fee amount
    ///
    /// The reason for breaking out the fee separately is so that the product can
    /// take a cut of this fee to the treasury
    pub asset_fee: N,
}

/// Result of a trade where the input is a fixed amount of asset
pub struct TradeAssetResult<N: Num> {
    /// The change of PT for the trader
    pub net_trader_pt: N,

    /// Positive asset fee
    ///
    /// When receiving asset, a trader gets *less* by the fee amount
    /// When sending asset, a trader sends *more* by the fee amount
    ///
    /// The reason for breaking out the fee separately is so that the product can
    /// take a cut of this fee to the treasury
    pub asset_fee: N,
}

pub fn trade<N: Num>(
    market_pt: u64,
    market_asset: u64,
    rate_scalar: N,
    rate_anchor: N,
    fee_rate: N,
    net_trader_pt: N,
    is_current_flash_swap: bool,
) -> TradeResult<N> {
    // assert that the user is selling PT into the market
    // or the market has more PT than the user is buying
    assert!(net_trader_pt < N::zero() || market_pt > net_trader_pt.to_u64());

    let market_pt = N::from_u64(market_pt);
    let new_pt = market_pt - net_trader_pt;

    let p = new_pt / (market_pt + N::from_u64(market_asset));
    let l_p = logit(p);
    let er = exchange_rate(l_p, rate_scalar, rate_anchor);

    println!("er in trade: {}", er);

    assert!(er > N::one(), "Asset cannot be worth less than PT");

    // negate the trader PT to get the net change in asset for the trader
    let pre_fee_net_trader_asset = -net_trader_pt / er;

    // If the market is currently performing a flash swap, the fee is relative to the borrowed amount.
    let fee = if is_current_flash_swap {
        // 1 / er represents the PT price in asset terms
        // Therefore, (1 - (1/er)) represents the YT price in asset terms
        let yt_value = (N::one() - N::one() / er) * net_trader_pt.abs();
        asset_fee(yt_value, fee_rate)
    } else {
        asset_fee(pre_fee_net_trader_asset, fee_rate)
    };

    // subtract the fee from the net trader asset
    // if net_trader_asset is negative, the user is buying PT and selling asset and the "fee" value is positive in order to increase the magnitude of net_trader_asset (increasing the amount of asset the user must pay)
    // if net_trader_asset is positive, the user is selling PT and buying asset and the "fee" value is positive in order to decrease the magnitude of net_trader_asset
    let net_trader_asset = pre_fee_net_trader_asset - fee;

    TradeResult {
        net_trader_asset,
        asset_fee: fee,
    }
}

pub fn trade_asset<N: Num>(
    market_pt: u64,
    market_asset: u64,
    rate_scalar: N,
    rate_anchor: N,
    fee_rate: N,
    net_trader_asset: N,
    is_current_flash_swap: bool,
) -> TradeAssetResult<N> {
    // assert that the user is selling asset into the market
    // or the market has more asset than the user is buying
    assert!(net_trader_asset < N::zero() || market_asset > net_trader_asset.to_u64());

    let market_pt = N::from_u64(market_pt);
    let market_asset = N::from_u64(market_asset);

    // Subtract the asset amount before computing exchange rate
    let new_asset = market_asset - net_trader_asset;

    let p = market_pt / (market_pt + new_asset);
    let l_p = logit(p);
    let er = exchange_rate(l_p, rate_scalar, rate_anchor);

    println!("er in trade_asset: {}", er);

    assert!(er > N::one(), "Asset cannot be worth less than PT");

    // Calculate the PT amount based on the asset amount and exchange rate
    let net_trader_pt = -net_trader_asset * er;

    // Calculate fee based on asset amount
    let fee = if is_current_flash_swap {
        // 1 / er represents the PT price in asset terms
        // Therefore, (1 - (1/er)) represents the YT price in asset terms
        let yt_value = (N::one() - N::one() / er) * net_trader_asset.abs();
        asset_fee(yt_value, fee_rate)
    } else {
        asset_fee(net_trader_asset, fee_rate)
    };

    TradeAssetResult {
        net_trader_pt,
        asset_fee: fee,
    }
}

// #[cfg(test)]
// mod test {
//     use super::*;

//     // Helper function to compare floating point numbers
//     fn assert_close(a: f64, b: f64, epsilon: f64) {
//         assert!((a - b).abs() < epsilon, "{} is not close to {}", a, b);
//     }

//     #[test]
//     fn test_fee_rate() {
//         let ln_fee_rate_root = 1.10.ln();

//         // The fee should be the same as the root at 1 year
//         assert_eq!(1.1, fee_rate(ln_fee_rate_root, YEAR_SEC));

//         // longer time frame
//         // fee rate should be a little more than double with double the time
//         let fr = fee_rate(ln_fee_rate_root, YEAR_SEC * 2);
//         assert!(1.2 < fr);
//         assert!(1.22 > fr);

//         // shorter time frame
//         // fee rate should be a little less than half with half the time
//         let fr = fee_rate(ln_fee_rate_root, YEAR_SEC / 2);
//         assert!(1.05 > fr);
//         assert!(1.048 < fr);

//         // zero time remaining
//         // fee rate should be 1.0 (ie, 0 fees)
//         assert_eq!(1.0, fee_rate(ln_fee_rate_root, 0));
//     }

//     #[test]
//     fn test_add_remove_liquidity_roundtrip() {
//         let market_start_total_lp = 1000;
//         let market_start_total_sy = 1000;
//         let market_start_total_pt = 1000;

//         let add_result = add_liquidity::<f64>(
//             100,
//             100,
//             market_start_total_lp,
//             market_start_total_sy,
//             market_start_total_pt,
//         );
//         let rm_result = rm_liquidity::<f64>(
//             add_result.lp_tokens_out,
//             market_start_total_lp + add_result.lp_tokens_out,
//             market_start_total_sy + add_result.sy_in,
//             market_start_total_pt + add_result.pt_in,
//         );

//         assert_close(rm_result.sy_out as f64, add_result.sy_in as f64, 1e-6);
//         assert_close(rm_result.pt_out as f64, add_result.pt_in as f64, 1e-6);
//     }

//     #[test]
//     fn test_proportion_pt() {
//         let p = proportion_pt::<f64>(0, 1);
//         assert_eq!(p, 0.0);

//         let p = proportion_pt::<f64>(100, 100);
//         assert_eq!(p, 0.5);

//         let p = proportion_pt::<f64>(200, 100);
//         assert_eq!(p, 200.0 / 300.0);
//     }

//     #[test]
//     fn test_logit() {
//         // the logit should be 0 at the midpoint
//         let l = logit::<f64>(0.5);
//         assert_eq!(l, 0.0);

//         let l = logit::<f64>(0.5001);
//         assert!(l > 0.0);

//         let l = logit::<f64>(0.4999);
//         assert!(l < 0.0);
//     }

//     #[test]
//     fn test_rate_scalar() {
//         // the scalar should increase with time
//         let r_0 = rate_scalar::<f64>(5.0, 1000);
//         let r_1 = rate_scalar::<f64>(5.0, 500);
//         let r_2 = rate_scalar::<f64>(5.0, 250);
//         assert!(r_1 > r_0);

//         // rate scalar increases linearly as time goes to 0
//         assert_eq!(r_1, r_0 * 2.0);
//         assert_eq!(r_2, r_0 * 4.0);
//     }

//     #[test]
//     fn test_rate_scalar_infinity_at_zero() {
//         let r = rate_scalar::<f64>(5.0, 0);
//         assert_eq!(r, f64::MAX);
//     }

//     #[test]
//     fn test_ln_implied_rate() {
//         let pt = 200;
//         let asset = 100;
//         let rate_scalar = 5.0;
//         let rate_anchor = 1.01;
//         let sec_remaining = 1000;

//         // implied rate should increase as time-to-expiry goes down
//         let r_0 = ln_implied_rate::<f64>(pt, asset, rate_scalar, rate_anchor, sec_remaining);
//         let r_1 = ln_implied_rate::<f64>(pt, asset, rate_scalar, rate_anchor, sec_remaining / 2);

//         assert!(r_1 > r_0);
//         // the ln_implied_rate should increase linearly as time decreases
//         assert!(r_1 == r_0 * 2.0)
//     }

//     #[test]
//     fn test_ln_implied_rate_equals_rate_anchor() {
//         let pt = 100;
//         let asset = 100;
//         let rate_scalar = 5.0;
//         let rate_anchor = 1.10;

//         let lir = ln_implied_rate(pt, asset, rate_scalar, rate_anchor, YEAR_SEC);
//         assert_eq!(lir.exp(), rate_anchor);

//         // at an equal proportion, the ln_implied_rate is the same with different scalars
//         let lir = ln_implied_rate(pt, asset, rate_scalar * 4.0, rate_anchor, YEAR_SEC);
//         assert_eq!(lir.exp(), rate_anchor);
//     }

//     #[test]
//     fn test_exchange_rate() {
//         let pt = 200;
//         let asset = 100;
//         let rate_scalar = 5.0;
//         let rate_anchor = 1.01;

//         let p = logit(proportion_pt::<f64>(pt, asset));
//         let er = exchange_rate(p, rate_scalar, rate_anchor);

//         // the exchange rate should be greater than the rate anchor
//         assert!(er > rate_anchor);
//     }

//     #[test]
//     fn test_exchange_rate_implied_rate_commute() {
//         let pt = 200;
//         let asset = 100;
//         let rate_scalar = 5.0;
//         let rate_anchor = 1.01;
//         let sec_remaining = 1000;

//         let p = logit(proportion_pt::<f64>(pt, asset));
//         let er = exchange_rate(p, rate_scalar, rate_anchor);
//         let ln_ir = ln_implied_rate(pt, asset, rate_scalar, rate_anchor, sec_remaining);

//         let er_2 = exchange_rate_from_ln_implied_rate::<f64>(ln_ir, sec_remaining);

//         // check that the rates are equal within an epsilon
//         assert!((er - er_2).abs() < 1e-10);
//     }

//     #[test]
//     fn add_liquidity_even() {
//         let market_total_lp = 1000;
//         let market_total_sy = 1000;
//         let market_total_pt = 1000;

//         let add_result =
//             add_liquidity::<f64>(100, 100, market_total_lp, market_total_sy, market_total_pt);

//         // With even amounts of SY and PT, the LP tokens out should be equal to the input
//         assert!(add_result.lp_tokens_out == 100);

//         // the amount of SY and PT in should be equal
//         assert_eq!(add_result.sy_in, add_result.pt_in);
//     }

//     #[test]
//     fn test_add_liquidity_uneven() {
//         let market_total_lp = 1000;
//         let market_total_sy = 1000;
//         let market_total_pt = 1000;

//         let add_result =
//             add_liquidity::<f64>(100, 200, market_total_lp, market_total_sy, market_total_pt);

//         assert_eq!(add_result.lp_tokens_out, 100);
//         assert_eq!(add_result.sy_in, 100);
//         assert_eq!(add_result.pt_in, 100);
//     }

//     #[test]
//     fn test_add_liquidity_unbalanced_pool() {
//         let market_total_lp = 1000;
//         let market_total_sy = 500;
//         let market_total_pt = 2000;

//         let add_result =
//             add_liquidity::<f64>(50, 200, market_total_lp, market_total_sy, market_total_pt);

//         assert_eq!(add_result.lp_tokens_out, 100);
//         assert_eq!(add_result.sy_in, 50);
//         assert_eq!(add_result.pt_in, 200);

//         // Try adding way too much PT
//         let add_result =
//             add_liquidity::<f64>(50, 400, market_total_lp, market_total_sy, market_total_pt);
//         assert_eq!(add_result.lp_tokens_out, 100);
//         assert_eq!(add_result.sy_in, 50);
//         // PT will be reduced to balance the SY
//         assert_eq!(add_result.pt_in, 200);
//     }

//     /// The recomputation of the rate anchor based on time decay should preserve the implied rate
//     #[test]
//     fn test_find_new_rate_anchor() {
//         let pt = 200;
//         let asset = 100;
//         let rate_anchor = 1.01;
//         let sec_remaining = 1000;
//         let rs = rate_scalar(10.0, sec_remaining);

//         let p = logit(proportion_pt::<f64>(pt, asset));
//         let er_0 = exchange_rate(p, rs, rate_anchor);
//         let ln_ir_0 = ln_implied_rate::<f64>(pt, asset, rs, rate_anchor, sec_remaining);

//         // decrease seconds remaining til expiry
//         let sec_remaining = sec_remaining / 2;

//         // recompute rate scalar and rate anchor
//         let rs = rate_scalar(10.0, sec_remaining);
//         let new_rate_anchor = find_rate_anchor(pt, asset, rs, ln_ir_0, sec_remaining);

//         let er_1 = exchange_rate(p, rs, new_rate_anchor);
//         let ln_ir_1 = ln_implied_rate(pt, asset, rs, new_rate_anchor, sec_remaining);

//         // The implied rate should stay the same
//         assert!((ln_ir_1 - ln_ir_0).abs() < 1e-10);

//         // The exchange rate should go up
//         assert!(er_1 < er_0);
//     }

//     #[test]
//     fn test_fee_sell_pt() {
//         let fee_rate = 1.01;
//         let net_trader_asset = 100.0;

//         // trader is buying asset
//         let fee = asset_fee(net_trader_asset, fee_rate);

//         // the fee should be 1% of 100
//         assert_close(fee, 1.0, 1e-10);
//     }

//     #[test]
//     fn test_fee_buy_pt() {
//         let fee_rate = 1.01;
//         let net_trader_asset = -100.0;

//         // trader is buying asset
//         let fee = asset_fee(net_trader_asset, fee_rate);

//         // the fee should be just a little less than 1.0
//         assert!(fee > 0.99);
//         assert!(fee < 1.0);
//     }

//     /// Test that the fee decays linearly
//     #[test]
//     fn test_fee_decay() {
//         let fee_root = 1.10;
//         let ln_fee_root = fee_root.ln();
//         let sec_remaining = 10_000;

//         let r1 = fee_rate(ln_fee_root, sec_remaining);
//         let r2 = fee_rate(ln_fee_root, sec_remaining / 2);

//         assert!(r1 > r2);
//         // assert that the rate decays linearly
//         // after 1/2 the time, the rate should be 1/2 the original rate
//         assert!((r1 - 1.0) - ((r2 - 1.0) * 2.0) < 1e-5);
//     }

//     #[test]
//     fn test_fee_at_zero_time_remaining() {
//         let fee_root = 1.10;
//         let ln_fee_root = fee_root.ln();
//         let sec_remaining = 0;

//         let r1 = fee_rate(ln_fee_root, sec_remaining);

//         // the fee rate should be 1.0 at zero time remaining
//         assert!((r1 - 1.0).abs() < 1e-10);
//     }

//     #[test]
//     fn test_zero_fee_rate() {
//         let fee_rate_root = 1.0;
//         let ln_fee_rate_root = fee_rate_root.ln();

//         let r = fee_rate(ln_fee_rate_root, 1000);
//         assert_eq!(r, 1.0);

//         let r = fee_rate(ln_fee_rate_root, 100_000);
//         assert_eq!(r, 1.0);
//     }

//     #[test]
//     fn test_sell_pt() {
//         let market_pt = 200.0;
//         let market_asset = 100.0;
//         let rate_scalar = 5.0;
//         let rate_anchor = 1.01;
//         let fee_rate = 1.01;
//         let net_trader_pt: f64 = -10.0;

//         let trade_result = trade(
//             market_pt as u64,
//             market_asset as u64,
//             rate_scalar,
//             rate_anchor,
//             fee_rate,
//             net_trader_pt,
//             false,
//         );

//         // the trader is selling PT and buying asset
//         // the net trader asset should be positive
//         assert!(trade_result.net_trader_asset > 0.0);

//         // the fee should be positive
//         assert!(trade_result.asset_fee > 0.0);
//     }

//     #[test]
//     fn test_rm_liquidity_even_pool() {
//         let market_total_lp = 1000;
//         let market_total_sy = 1000;
//         let market_total_pt = 1000;

//         let remove_result =
//             rm_liquidity::<f64>(100, market_total_lp, market_total_sy, market_total_pt);

//         assert_eq!(remove_result.sy_out, 100);
//         assert_eq!(remove_result.pt_out, 100);
//     }

//     #[test]
//     fn test_rm_liquidity_uneven_pool() {
//         let market_total_lp = 1000;
//         let market_total_sy = 2000;
//         let market_total_pt = 500;

//         let remove_result =
//             rm_liquidity::<f64>(100, market_total_lp, market_total_sy, market_total_pt);

//         assert_eq!(remove_result.sy_out, 200);
//         assert_eq!(remove_result.pt_out, 50);
//     }

//     #[test]
//     fn test_rm_liquidity_partial() {
//         let market_total_lp = 1000;
//         let market_total_sy = 1000;
//         let market_total_pt = 1000;

//         let remove_result =
//             rm_liquidity::<f64>(250, market_total_lp, market_total_sy, market_total_pt);

//         assert_eq!(remove_result.sy_out, 250);
//         assert_eq!(remove_result.pt_out, 250);
//     }

//     #[test]
//     fn test_rm_liquidity_all() {
//         let market_total_lp = 1000;
//         let market_total_sy = 1000;
//         let market_total_pt = 1000;

//         let remove_result =
//             rm_liquidity::<f64>(1000, market_total_lp, market_total_sy, market_total_pt);

//         assert_eq!(remove_result.sy_out, 1000);
//         assert_eq!(remove_result.pt_out, 1000);
//     }

//     #[test]
//     fn test_rm_liquidity_large_numbers() {
//         let market_total_lp = 1_000_000_000;
//         let market_total_sy = 2_000_000_000;
//         let market_total_pt = 3_000_000_000;

//         let remove_result = rm_liquidity::<f64>(
//             100_000_000,
//             market_total_lp,
//             market_total_sy,
//             market_total_pt,
//         );

//         assert_eq!(remove_result.sy_out, 200_000_000);
//         assert_eq!(remove_result.pt_out, 300_000_000);
//     }

//     #[test]
//     fn test_rm_liquidity_small_amount() {
//         let market_total_lp = 1_000_000;
//         let market_total_sy = 1_000_000;
//         let market_total_pt = 1_000_000;

//         let remove_result =
//             rm_liquidity::<f64>(1, market_total_lp, market_total_sy, market_total_pt);

//         assert_eq!(remove_result.sy_out, 1);
//         assert_eq!(remove_result.pt_out, 1);
//     }

//     #[test]
//     #[should_panic]
//     fn test_rm_liquidity_more_than_total() {
//         let market_total_lp = 1000;
//         let market_total_sy = 1000;
//         let market_total_pt = 1000;

//         rm_liquidity::<f64>(1001, market_total_lp, market_total_sy, market_total_pt);
//     }

//     #[test]
//     fn test_buy_pt_no_fees() {
//         let market_pt = 100;
//         let market_asset = 100;
//         let rate_scalar = 20.0;
//         let rate_anchor = 1.10;
//         let net_trader_pt = 1.0;
//         let fee_rate = 1.0;

//         let tr = trade(
//             market_pt,
//             market_asset,
//             rate_scalar,
//             rate_anchor,
//             fee_rate,
//             net_trader_pt,
//             false,
//         );

//         assert!(tr.asset_fee == 0.0);
//         assert!(tr.net_trader_asset < 0.0);
//         assert!(tr.net_trader_asset < -1.0 / 1.1);

//         // the trader should get a slightly worse rate than the ideal rate
//         let ideal_exchange_rate = exchange_rate(logit(99.0 / 199.0), rate_scalar, rate_anchor);
//         assert!(tr.net_trader_asset <= -1.0 / ideal_exchange_rate);
//         assert!(tr.net_trader_asset > -1.01 / ideal_exchange_rate);
//     }

//     #[test]
//     fn test_sell_pt_no_fee() {
//         let market_pt = 100;
//         let market_asset = 100;
//         let rate_scalar = 20.0;
//         let rate_anchor = 1.10;
//         let net_trader_pt = -1.0;
//         let fee_rate = 1.0;

//         let tr = trade(
//             market_pt,
//             market_asset,
//             rate_scalar,
//             rate_anchor,
//             fee_rate,
//             net_trader_pt,
//             false,
//         );

//         assert!(tr.asset_fee == 0.0);
//         assert!(tr.net_trader_asset > 0.0);
//         assert!(tr.net_trader_asset < 1.0 / 1.1);

//         // the trader should get a slightly worse rate than the ideal rate
//         let ideal_exchange_rate = exchange_rate(logit(101.0 / 201.0), rate_scalar, rate_anchor);
//         assert!(tr.net_trader_asset <= 1.0 / ideal_exchange_rate);
//         assert!(tr.net_trader_asset > 0.99 / ideal_exchange_rate);
//     }

//     #[test]
//     fn test_buy_pt_with_fee() {
//         let market_pt = 1000;
//         let market_asset = 1000;
//         let rate_scalar = 20.0;
//         let rate_anchor = 1.10;
//         let fee_rate = 1.01;
//         let net_trader_pt = 1.0;
//         let trade_result = trade(
//             market_pt,
//             market_asset,
//             rate_scalar,
//             rate_anchor,
//             fee_rate,
//             net_trader_pt,
//             false,
//         );

//         // the trader is buying PT and selling asset
//         // so the trader asset should be negative
//         assert!(trade_result.net_trader_asset < 0.0);

//         // the exchange rate is ideally 1.10, but the trader is paying a 1% fee an suffering price impact
//         assert!(trade_result.net_trader_asset < -1.0 / 1.1);

//         // the fee should be positive (meaning the trader is paying more asset to receive PT)
//         assert!(trade_result.asset_fee > 0.0);

//         // the actual asset fee is less than 1% of the net asset fee
//         let asset_spend_minus_fee = -trade_result.net_trader_asset - trade_result.asset_fee;
//         assert!(asset_spend_minus_fee * 0.01 > trade_result.asset_fee);
//         assert!(asset_spend_minus_fee * 0.009 < trade_result.asset_fee);
//     }

//     #[test]
//     fn test_trade_roundtrip_buy() {
//         // Initial market state
//         let mut market_pt = 100000;
//         let mut market_asset = 100000;
//         let rate_scalar = 20.0;
//         let rate_anchor = 1.10;
//         let fee_rate = 1.01;

//         println!("\nInitial market state:");
//         println!("market_pt: {}", market_pt);
//         println!("market_asset: {}", market_asset);

//         // First trade: Buy PT (positive net_trader_pt means pulling PT out of market)
//         let net_trader_pt = 1000.0;

//         // TODO: DELETE - Verify first trade input
//         assert!(
//             net_trader_pt > 0.0,
//             "First trade: net_trader_pt should be positive when buying PT"
//         );

//         let trade_result_pt = trade(
//             market_pt,
//             market_asset,
//             rate_scalar,
//             rate_anchor,
//             fee_rate,
//             net_trader_pt,
//             false,
//         );

//         println!("\nFirst trade results:");
//         println!("net_trader_pt: {}", net_trader_pt);
//         println!("net_trader_asset: {}", trade_result_pt.net_trader_asset);
//         println!("asset_fee: {}", trade_result_pt.asset_fee);

//         // TODO: DELETE - Verify first trade output
//         assert!(
//             trade_result_pt.net_trader_asset < 0.0,
//             "First trade: net_trader_asset should be negative when buying PT"
//         );
//         assert!(
//             trade_result_pt.asset_fee > 0.0,
//             "First trade: asset_fee should be positive"
//         );

//         // Update market state after first trade
//         // net_trader_pt is positive, so we subtract it from market_pt (pulling PT out)
//         market_pt -= net_trader_pt.to_u64();
//         // net_trader_asset is negative (putting asset in), so we add it to market_asset
//         // Also add the asset fee to market_asset
//         market_asset += (-trade_result_pt.net_trader_asset).to_u64();
//         market_asset += trade_result_pt.asset_fee.to_u64();

//         println!("\nMarket state after first trade:");
//         println!("market_pt: {}", market_pt);
//         println!("market_asset: {}", market_asset);

//         // TODO: DELETE - Verify market state after first trade
//         assert!(
//             market_pt < 100000,
//             "Market PT should decrease after buying PT"
//         );
//         assert!(
//             market_asset > 100000,
//             "Market asset should increase after buying PT"
//         );

//         // Second trade: Sell PT back (positive net_trader_asset means putting asset in)
//         let net_trader_asset = -trade_result_pt.net_trader_asset; // Make it positive to sell PT

//         println!("\nSecond trade inputs:");
//         println!("net_trader_asset: {}", net_trader_asset);

//         // TODO: DELETE - Verify second trade input
//         assert!(
//             net_trader_asset > 0.0,
//             "Second trade: net_trader_asset should be positive when selling PT"
//         );

//         let trade_result_asset = trade_asset(
//             market_pt,
//             market_asset,
//             rate_scalar,
//             rate_anchor,
//             fee_rate,
//             net_trader_asset,
//             false,
//         );

//         println!("\nSecond trade results:");
//         println!("net_trader_pt: {}", trade_result_asset.net_trader_pt);
//         println!("asset_fee: {}", trade_result_asset.asset_fee);
//         println!(
//             "PT difference from first trade: {}",
//             -trade_result_asset.net_trader_pt - net_trader_pt
//         );

//         // TODO: DELETE - Verify second trade output
//         assert!(
//             trade_result_asset.net_trader_pt < 0.0,
//             "Second trade: net_trader_pt should be negative when selling PT"
//         );
//         assert!(
//             trade_result_asset.asset_fee > 0.0,
//             "Second trade: asset_fee should be positive"
//         );

//         // Update market state after second trade
//         // net_trader_pt is negative (from trade_result_asset), so we add its absolute value to market_pt (selling PT back)
//         market_pt += (-trade_result_asset.net_trader_pt).to_u64();
//         // net_trader_asset is positive, so we subtract it from market_asset
//         // Also add the asset fee to market_asset
//         market_asset -= net_trader_asset.to_u64();
//         market_asset += trade_result_asset.asset_fee.to_u64();

//         println!("\nFinal market state:");
//         println!("market_pt: {}", market_pt);
//         println!("market_asset: {}", market_asset);

//         // TODO: DELETE - Verify final market state
//         assert!(
//             market_pt < 100000,
//             "Final market PT should be less than initial due to fees"
//         );
//         assert!(
//             market_asset < 100000,
//             "Final market asset should be less than initial due to fees"
//         );

//         // Calculate total fees (ceiling to match on-chain behavior)
//         let total_fees = (trade_result_pt.asset_fee + trade_result_asset.asset_fee).ceil() as u64;

//         println!("\nFee summary:");
//         println!("First trade fee: {}", trade_result_pt.asset_fee);
//         println!("Second trade fee: {}", trade_result_asset.asset_fee);
//         println!("Total fees: {}", total_fees);

//         // Verify final market state
//         assert_eq!(market_asset, 100000 - total_fees);

//         // PT should be close to initial amount, but not exactly due to fees affecting exchange rate
//         assert!((market_pt as f64 - 100000.0).abs() < 1.0);
//     }

//     #[test]
//     fn test_trade_roundtrip_buy_edge_cases() {
//         // Test with different market sizes
//         let test_cases = vec![
//             (100, 100, 5.0),      // Small market
//             (1000, 1000, 10.0),   // Medium market
//             (10000, 10000, 20.0), // Large market
//         ];

//         for (market_pt, market_asset, net_trader_pt) in test_cases {
//             let rate_scalar = 20.0;
//             let rate_anchor = 1.10;
//             let fee_rate = 1.01;

//             // First trade: Buy PT
//             let trade_result_pt = trade(
//                 market_pt,
//                 market_asset,
//                 rate_scalar,
//                 rate_anchor,
//                 fee_rate,
//                 net_trader_pt,
//                 false,
//             );

//             // Calculate new market state
//             let new_market_pt = market_pt - net_trader_pt.to_u64();
//             let new_market_asset = market_asset
//                 + (-trade_result_pt.net_trader_asset - trade_result_pt.asset_fee).to_u64();

//             // Second trade: Sell PT
//             let inverse_net_trader_asset =
//                 -(trade_result_pt.net_trader_asset + trade_result_pt.asset_fee);
//             let trade_result_asset = trade_asset(
//                 new_market_pt,
//                 new_market_asset,
//                 rate_scalar,
//                 rate_anchor,
//                 fee_rate,
//                 inverse_net_trader_asset,
//                 false,
//             );

//             // Verify roundtrip properties
//             let first_total = trade_result_pt.net_trader_asset + trade_result_pt.asset_fee;
//             let second_total = inverse_net_trader_asset + trade_result_asset.asset_fee;
//             assert!((first_total + second_total).abs() < 1e-10);
//             assert!((trade_result_pt.asset_fee - trade_result_asset.asset_fee).abs() < 1e-10);
//         }
//     }

//     #[test]
//     fn test_trade_roundtrip_buy_trade_only() {
//         // Initial market state
//         let mut market_pt = 100000;
//         let mut market_asset = 100000;
//         let rate_scalar = 20.0;
//         let rate_anchor = 1.10;
//         let fee_rate = 1.01;

//         println!("\nInitial market state:");
//         println!("market_pt: {}", market_pt);
//         println!("market_asset: {}", market_asset);

//         // First trade: Buy PT (positive net_trader_pt means pulling PT out of market)
//         let net_trader_pt = 1000.0;

//         // TODO: DELETE - Verify first trade input
//         assert!(
//             net_trader_pt > 0.0,
//             "First trade: net_trader_pt should be positive when buying PT"
//         );

//         let trade_result_pt = trade(
//             market_pt,
//             market_asset,
//             rate_scalar,
//             rate_anchor,
//             fee_rate,
//             net_trader_pt,
//             false,
//         );

//         println!("\nFirst trade results:");
//         println!("net_trader_pt: {}", net_trader_pt);
//         println!("net_trader_asset: {}", trade_result_pt.net_trader_asset);
//         println!("asset_fee: {}", trade_result_pt.asset_fee);

//         // TODO: DELETE - Verify first trade output
//         assert!(
//             trade_result_pt.net_trader_asset < 0.0,
//             "First trade: net_trader_asset should be negative when buying PT"
//         );
//         assert!(
//             trade_result_pt.asset_fee > 0.0,
//             "First trade: asset_fee should be positive"
//         );

//         // Update market state after first trade
//         // net_trader_pt is positive, so we subtract it from market_pt (pulling PT out)
//         market_pt -= net_trader_pt.to_u64();
//         // net_trader_asset is negative (putting asset in), so we add it to market_asset
//         // Also add the asset fee to market_asset
//         market_asset += (-trade_result_pt.net_trader_asset).to_u64();
//         market_asset += trade_result_pt.asset_fee.to_u64();

//         println!("\nMarket state after first trade:");
//         println!("market_pt: {}", market_pt);
//         println!("market_asset: {}", market_asset);

//         // TODO: DELETE - Verify market state after first trade
//         assert!(
//             market_pt < 100000,
//             "Market PT should decrease after buying PT"
//         );
//         assert!(
//             market_asset > 100000,
//             "Market asset should increase after buying PT"
//         );

//         // Second trade: Sell PT back (negative net_trader_pt means putting PT back in)
//         let net_trader_pt = -net_trader_pt; // Make it negative to sell PT back

//         println!("\nSecond trade inputs:");
//         println!("net_trader_pt: {}", net_trader_pt);

//         // TODO: DELETE - Verify second trade input
//         assert!(
//             net_trader_pt < 0.0,
//             "Second trade: net_trader_pt should be negative when selling PT"
//         );

//         let trade_result_pt2 = trade(
//             market_pt,
//             market_asset,
//             rate_scalar,
//             rate_anchor,
//             fee_rate,
//             net_trader_pt,
//             false,
//         );

//         println!("\nSecond trade results:");
//         println!("net_trader_asset: {}", trade_result_pt2.net_trader_asset);
//         println!("asset_fee: {}", trade_result_pt2.asset_fee);
//         println!(
//             "PT difference from first trade: {}",
//             -net_trader_pt - 1000.0
//         );

//         // TODO: DELETE - Verify second trade output
//         assert!(
//             trade_result_pt2.net_trader_asset > 0.0,
//             "Second trade: net_trader_asset should be positive when selling PT"
//         );
//         assert!(
//             trade_result_pt2.asset_fee > 0.0,
//             "Second trade: asset_fee should be positive"
//         );

//         // Update market state after second trade
//         // net_trader_pt is negative (putting PT back in), so we add its absolute value to market_pt
//         market_pt += (-net_trader_pt).to_u64();
//         // net_trader_asset is positive (getting asset back), so we subtract it from market_asset
//         // Also add the asset fee to market_asset
//         market_asset -= trade_result_pt2.net_trader_asset.to_u64();
//         market_asset += trade_result_pt2.asset_fee.to_u64();

//         println!("\nFinal market state:");
//         println!("market_pt: {}", market_pt);
//         println!("market_asset: {}", market_asset);

//         // TODO: DELETE - Verify final market state
//         assert!(
//             market_pt == 100000,
//             "Final market PT should be equal to initial due to roundtrip"
//         );
//         assert!(
//             market_asset > 100000,
//             "Final market asset should be greater than initial due to fees"
//         );

//         // Calculate total fees (ceiling to match on-chain behavior)
//         let total_fees = (trade_result_pt.asset_fee + trade_result_pt2.asset_fee).ceil() as u64;

//         println!("\nFee summary:");
//         println!("First trade fee: {}", trade_result_pt.asset_fee);
//         println!("Second trade fee: {}", trade_result_pt2.asset_fee);
//         println!("Total fees: {}", total_fees);

//         // Verify final market state
//         assert_eq!(market_asset, 100000 + total_fees);

//         // PT should be close to initial amount, but not exactly due to fees affecting exchange rate
//         assert!((market_pt as f64 - 100000.0).abs() < 1.0);
//     }
// }
