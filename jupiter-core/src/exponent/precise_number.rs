use anchor_lang::{AnchorDeserialize, AnchorSerialize};
use spl_math::{
    precise_number::{self, *},
    uint::U256, 
};
use std::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};

/// High precision number, stored as 4 u64 words in little endian
#[derive(Default, Clone, Debug, Copy, PartialEq, Eq, AnchorSerialize, AnchorDeserialize)]
pub struct Number([u64; 4]);

impl core::fmt::Display for Number {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let n = U256(self.0);
        write!(f, "{}", n)
    }
}

impl From<u64> for Number {
    fn from(value: u64) -> Self {
        let pn = PreciseNumber::new(value.into()).unwrap();
        Number(pn.value.0)
    }
}

impl From<u128> for Number {
    fn from(value: u128) -> Self {
        let pn = PreciseNumber::new(value).unwrap();
        Number(pn.value.0)
    }
}

/// Wrapper around PreciseNumber (a U256) that has 1e12 precision
impl Number {
    // The byte size of Number
    pub const SIZEOF: usize = 32;

    pub const ZERO: Self = Self(U256::zero().0);

    // It just so happens that 1 is 10e12, which is smaller than a u64
    pub const ONE: Self = Self([precise_number::ONE as u64, 0, 0, 0]);

    pub const DENOM: u128 = precise_number::ONE;

    pub fn from_bytes_le(slice: &[u8]) -> Self {
        Self(U256::from_little_endian(slice).0)
    }

    pub fn from_natural_u64(value: u64) -> Self {
        value.into()
    }

    pub fn from_ratio(num: u128, den: u128) -> Self {
        let num = PreciseNumber::new(num).unwrap();
        let den = PreciseNumber::new(den).unwrap();

        PreciseNumber::checked_div(&num, &den)
            .unwrap_or(PreciseNumber::new(0).unwrap())
            .into()
    }

    /// Convert BPS into Number
    pub fn from_bps(bps: u16) -> Self {
        Self::from_natural_u64(bps as u64) / Self::from_natural_u64(10_000)
    }

    pub fn checked_add(&self, x: &Self) -> Option<Self> {
        self.to_pn()
            .checked_add(&x.to_pn())
            .map(|pn| Self(pn.value.0))
    }

    pub fn checked_sub(&self, x: &Self) -> Option<Self> {
        self.to_pn()
            .checked_sub(&x.to_pn())
            .map(|pn| Self(pn.value.0))
    }

    pub fn checked_mul(&self, x: &Self) -> Option<Self> {
        self.to_pn()
            .checked_mul(&x.to_pn())
            .map(|pn| Self(pn.value.0))
    }

    pub fn checked_div(&self, x: &Self) -> Option<Self> {
        self.to_pn()
            .checked_div(&x.to_pn())
            .map(|pn| Self(pn.value.0))
    }

    pub fn to_pn(&self) -> PreciseNumber {
        PreciseNumber {
            value: U256(self.0),
        }
    }

    pub fn min(numbers: &[Self]) -> Self {
        *numbers.iter().min().unwrap()
    }

    pub fn floor_u64(&self) -> u64 {
        self.to_pn()
            .floor()
            .unwrap()
            .to_imprecise()
            .unwrap()
            .try_into()
            .unwrap()
    }

    pub fn ceil(&self) -> u128 {
        self.to_pn().ceiling().unwrap().to_imprecise().unwrap()
    }

    pub fn ceil_u64(&self) -> u64 {
        self.ceil().try_into().unwrap()
    }

    pub fn floor_u128(&self) -> u128 {
        self.to_pn().floor().unwrap().to_imprecise().unwrap()
    }

    pub fn to_f64(&self) -> Option<f64> {
        // scale up by 1e12 to avoid precision loss
        let n = self
            .checked_mul(&Number::from(Self::DENOM))
            .unwrap()
            .to_pn()
            .to_imprecise()
            .unwrap();
        // denominator is always 1e12
        let d = precise_number::ONE;
        u128_to_f64_checked(n, d)
    }
}

/// Convert a u128 ratio to f64, checking for overflow
fn u128_to_f64_checked(numerator: u128, denominator: u128) -> Option<f64> {
    // Constants for maximum values f64 can represent exactly
    const MAX_EXACT_U64: u128 = (1u128 << 53) - 1;

    // 2^64
    const U64_MAX_PLUS_ONE: f64 = 18446744073709551616.0;
    if denominator == 0 {
        return None; // Division by zero is invalid
    }

    // Split the u128 into high and low 64-bit parts for both numerator and denominator
    let num_high = (numerator >> 64) as u64;
    let num_low = numerator as u64;

    let denom_high = (denominator >> 64) as u64;
    let denom_low = denominator as u64;

    // Check if the high part of numerator or denominator exceeds the safe f64 range
    if numerator > MAX_EXACT_U64 || denominator > MAX_EXACT_U64 {
        return None; // Overflow: numbers are too large to represent as f64
    }

    // Convert the high and low parts to f64
    let num_f64 = (num_high as f64) * U64_MAX_PLUS_ONE + num_low as f64;
    let denom_f64 = (denom_high as f64) * U64_MAX_PLUS_ONE + denom_low as f64;

    // Perform the division
    Some(num_f64 / denom_f64)
}

impl From<PreciseNumber> for Number {
    fn from(pn: PreciseNumber) -> Self {
        Self(pn.value.0)
    }
}

impl Add<Number> for Number {
    type Output = Self;

    fn add(self, rhs: Number) -> Self::Output {
        Self(self.to_pn().checked_add(&rhs.to_pn()).unwrap().value.0)
    }
}

impl Mul<Number> for Number {
    type Output = Self;

    fn mul(self, x: Number) -> Self::Output {
        Self(self.checked_mul(&x).unwrap().0)
    }
}

impl Div<Number> for Number {
    type Output = Self;

    fn div(self, x: Number) -> Self {
        Self(self.checked_div(&x).unwrap().0)
    }
}

impl AddAssign<Number> for Number {
    fn add_assign(&mut self, rhs: Number) {
        self.0 = self.checked_add(&rhs).unwrap().0;
    }
}

impl SubAssign<Number> for Number {
    fn sub_assign(&mut self, rhs: Number) {
        self.0 = self.checked_sub(&rhs).unwrap().0;
    }
}

impl Sub<Number> for Number {
    type Output = Self;

    fn sub(self, x: Number) -> Self {
        self.checked_sub(&x).unwrap()
    }
}

impl PartialOrd for Number {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Number {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        U256(self.0).cmp(&U256(other.0))
    }
}

// #[cfg(test)]
// mod test {
//     use super::*;

//     #[test]
//     fn test_to_f64() {
//         let n = Number::from_ratio(1, 2);
//         assert_eq!(n.to_f64(), Some(0.5));

//         let n = Number::from_ratio(123456789, 1_000_000_000);
//         assert_eq!(n.to_f64(), Some(0.123456789));

//         let n = Number::from_ratio(0, 1_000_000_000);
//         assert_eq!(n.to_f64(), Some(0.0));

//         let n = Number::from_ratio(1_000_000_000, 1_000_000_000);
//         assert_eq!(n.to_f64(), Some(1.0));

//         let n = Number::from_ratio(1_000_000_000_000, 1_000_000_000);
//         assert_eq!(n.to_f64(), Some(1000.0));

//         let n = Number::from_ratio(123456789123, 100_000_000_000);
//         assert_eq!(n.to_f64(), Some(1.23456789123));
//     }

//     #[test]
//     fn test_serialization() {
//         let n = Number([ONE as u64, 0, 0, 0]);
//         assert_eq!(n.floor_u64(), 1);

//         let n = Number::from_natural_u64(1_000_000_000);
//         let w0 = n.0[0] as u128;
//         let w1 = n.0[1] as u128;
//         let target: u128 = 1_000_000_000 * ONE;
//         assert_eq!(w0 + (w1 << 64), target);
//     }

//     #[test]
//     fn test_add() {
//         let num1 = Number::from(100u64);
//         let num2 = Number::from(200u64);
//         let ans = Number::from(300u64);
//         let num3 = num1 + num2;
//         assert_eq!(num3, ans);
//     }

//     #[test]
//     fn test_sub() {
//         let num1 = Number::from(100u64);
//         let num2 = Number::from(200u64);
//         let ans = Number::from(100u64);
//         let num3 = num2 - num1;
//         assert_eq!(num3, ans);
//     }

//     #[test]
//     fn test_mul() {
//         let num1 = Number::from(100u64);
//         let num2 = Number::from(200u64);
//         let ans = Number::from(20000u64);
//         let num3 = num1 * num2;
//         assert_eq!(num3, ans);
//     }

//     #[test]
//     fn test_div() {
//         let num1 = Number::from(100u64);
//         let num2 = Number::from(20u64);
//         let ans = Number::from(5u64);
//         let num3 = num1 / num2;
//         assert_eq!(num3, ans);
//     }

//     #[test]
//     fn test_add_assign() {
//         let mut num1 = Number::from(100u64);
//         let num2 = Number::from(200u64);
//         let ans = Number::from(300u64);
//         num1 += num2;
//         assert_eq!(num1, ans);
//     }

//     #[test]
//     fn test_sub_assign() {
//         let mut num1 = Number::from(300u64);
//         let num2 = Number::from(200u64);
//         let ans = Number::from(100u64);
//         num1 -= num2;
//         assert_eq!(num1, ans);
//     }

//     #[test]
//     fn test_min() {
//         let numbers = [
//             Number::from(300u64),
//             Number::from(100u64),
//             Number::from(200u64),
//             Number::from(400u64),
//         ];
//         let min_value = Number::min(&numbers);
//         assert_eq!(min_value, Number::from(100u64));
//     }

//     #[test]
//     fn test_from_ratio() {
//         let num1 = Number::from_ratio(1, 2);
//         assert!(num1 > Number::from(0u64));
//         assert!(num1 < Number::from(1u64));
//     }

//     #[test]
//     fn test_mul_div() {
//         // test multiplying and dividing
//         let e = Number::from_ratio(22346643, 234216);

//         let a = Number::from_natural_u64(100);

//         let e_a = e * a;

//         let e_i = Number::ONE / e;

//         assert_eq!(a / e_a, e_i);
//     }

//     #[test]
//     fn test_floor_u64() {
//         let e = Number::from_ratio(10, 11);
//         assert_eq!(e.floor_u64(), 0);

//         let e = Number::from_ratio(11, 10);
//         assert_eq!(e.floor_u64(), 1);
//     }

//     #[test]
//     fn test_floor_u128() {
//         assert_eq!(Number::from_ratio(19, 10).floor_u128(), 1);
//         assert_eq!(Number::from_ratio(21, 10).floor_u128(), 2);
//         assert_eq!(Number::from_ratio(29, 10).floor_u128(), 2);
//         assert_eq!(Number::from_ratio(20, 10).floor_u128(), 2);
//         assert_eq!(Number::from_ratio(300, 100).floor_u128(), 3);
//         assert_eq!(Number::from_ratio(299, 10).floor_u128(), 29);
//         assert_eq!(Number::from_ratio(9, 10).floor_u128(), 0);
//     }
// }
