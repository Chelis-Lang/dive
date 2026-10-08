//! TLA+ integers in both modes: TLC's 32-bit semantics and arbitrary precision.
//!
//! Port of the integer semantics of `tlc2.module.Naturals` and `tlc2.module.Integers`
//! (tlaplus `94d0c50`): which operations overflow, the overflow messages, floored
//! `\div`, non-negative `%`, and the rejection of literals above `2^31 - 1`
//! (`tlc2.tool.impl.SpecProcessor`, `NumeralKind`). TLC uses `Integers.Divide` even
//! for specs that extend only `Naturals` (verified against the oracle, fixture
//! `NatDiv` in the survey notes).

use std::cmp::Ordering;
use std::fmt;
use std::sync::Arc;

use num_bigint::BigInt;
use num_integer::Integer as _;
use num_traits::{One, Signed, ToPrimitive};

use crate::ctx::IntMode;
use crate::error::{ErrorKind, Result, ValueError};

/// An integer: inline when it fits in an `i64`, otherwise a shared big integer.
///
/// Invariant: `Big` never holds a value that fits in an `i64`, so the representation is
/// canonical and structural equality is numeric equality.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Int {
    /// A value in `i64` range.
    Small(i64),
    /// A value outside `i64` range.
    Big(Arc<BigInt>),
}

/// The largest exponent bit-size dive will compute in big mode: results above
/// 2^26 bits (8 MiB) are refused with [`ErrorKind::DiveLimit`].
const MAX_POW_BITS: u64 = 1 << 26;

fn overflow(what: &str) -> ValueError {
    ValueError::new(
        ErrorKind::Overflow,
        format!("Overflow when computing {what}"),
    )
}

impl Int {
    /// Zero.
    pub const ZERO: Self = Self::Small(0);
    /// One.
    pub const ONE: Self = Self::Small(1);

    /// Converts a big integer, keeping the representation canonical.
    #[must_use]
    pub fn from_big(b: BigInt) -> Self {
        match b.to_i64() {
            Some(v) => Self::Small(v),
            None => Self::Big(Arc::new(b)),
        }
    }

    /// The value as an `i64`, if it fits.
    #[must_use]
    pub fn to_i64(&self) -> Option<i64> {
        match self {
            Self::Small(v) => Some(*v),
            Self::Big(_) => None,
        }
    }

    /// The value as an `i32` (a TLC `IntValue`), if it fits.
    #[must_use]
    pub fn to_i32(&self) -> Option<i32> {
        self.to_i64().and_then(|v| i32::try_from(v).ok())
    }

    /// The value as a big integer.
    #[must_use]
    pub fn to_big(&self) -> BigInt {
        match self {
            Self::Small(v) => BigInt::from(*v),
            Self::Big(b) => (**b).clone(),
        }
    }

    /// Whether the value is negative.
    #[must_use]
    pub fn is_negative(&self) -> bool {
        match self {
            Self::Small(v) => *v < 0,
            Self::Big(b) => b.is_negative(),
        }
    }

    /// Parses a decimal numeral as written in a spec.
    ///
    /// In [`IntMode::Tlc`] a numeral above `2^31 - 1` is `EC.TLC_INTEGER_TOO_BIG`, even
    /// when it is the operand of unary minus (`-2147483648` is rejected, as in TLC).
    ///
    /// # Errors
    /// [`ErrorKind::IntegerTooBig`] in TLC mode; [`ErrorKind::Argument`] if `digits` is
    /// not a non-empty string of ASCII digits.
    pub fn from_literal(digits: &str, mode: IntMode) -> Result<Self> {
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(ValueError::new(
                ErrorKind::Argument,
                format!("Not a decimal numeral: {digits}"),
            ));
        }
        let big: BigInt = digits.parse().map_err(|_| {
            ValueError::new(
                ErrorKind::Argument,
                format!("Not a decimal numeral: {digits}"),
            )
        })?;
        Self::from_literal_value(big, digits, mode)
    }

    /// Accepts a numeral already converted to a big integer (for `\b`, `\o`, `\h`
    /// numerals). `text` is the numeral as SANY prints it, used in the error message.
    ///
    /// # Errors
    /// [`ErrorKind::IntegerTooBig`] in TLC mode when the value exceeds `2^31 - 1`.
    pub fn from_literal_value(value: BigInt, text: &str, mode: IntMode) -> Result<Self> {
        let v = Self::from_big(value);
        if mode == IntMode::Tlc && v.to_i32().is_none() {
            return Err(ValueError::new(
                ErrorKind::IntegerTooBig,
                format!("TLC can't handle a number this big.\n{text}"),
            ));
        }
        Ok(v)
    }

    fn tlc_operand(&self) -> Result<i32> {
        self.to_i32().ok_or_else(|| {
            ValueError::new(
                ErrorKind::Overflow,
                format!("Overflow when computing {self} (not a 32-bit TLC integer)"),
            )
        })
    }

    /// `a + b`.
    ///
    /// # Errors
    /// [`ErrorKind::Overflow`] in TLC mode (`Overflow when computing a+b`).
    pub fn add(&self, other: &Self, mode: IntMode) -> Result<Self> {
        match mode {
            IntMode::Tlc => {
                let (a, b) = (self.tlc_operand()?, other.tlc_operand()?);
                a.checked_add(b)
                    .map(|r| Self::Small(i64::from(r)))
                    .ok_or_else(|| overflow(&format!("{a}+{b}")))
            }
            IntMode::Big => Ok(match (self, other) {
                (Self::Small(a), Self::Small(b)) => match a.checked_add(*b) {
                    Some(r) => Self::Small(r),
                    None => Self::from_big(BigInt::from(*a) + b),
                },
                _ => Self::from_big(self.to_big() + other.to_big()),
            }),
        }
    }

    /// `a - b`.
    ///
    /// # Errors
    /// [`ErrorKind::Overflow`] in TLC mode (`Overflow when computing a-b`).
    pub fn sub(&self, other: &Self, mode: IntMode) -> Result<Self> {
        match mode {
            IntMode::Tlc => {
                let (a, b) = (self.tlc_operand()?, other.tlc_operand()?);
                a.checked_sub(b)
                    .map(|r| Self::Small(i64::from(r)))
                    .ok_or_else(|| overflow(&format!("{a}-{b}")))
            }
            IntMode::Big => Ok(match (self, other) {
                (Self::Small(a), Self::Small(b)) => match a.checked_sub(*b) {
                    Some(r) => Self::Small(r),
                    None => Self::from_big(BigInt::from(*a) - b),
                },
                _ => Self::from_big(self.to_big() - other.to_big()),
            }),
        }
    }

    /// `a * b`.
    ///
    /// # Errors
    /// [`ErrorKind::Overflow`] in TLC mode (`Overflow when computing a*b`).
    pub fn mul(&self, other: &Self, mode: IntMode) -> Result<Self> {
        match mode {
            IntMode::Tlc => {
                let (a, b) = (self.tlc_operand()?, other.tlc_operand()?);
                a.checked_mul(b)
                    .map(|r| Self::Small(i64::from(r)))
                    .ok_or_else(|| overflow(&format!("{a}*{b}")))
            }
            IntMode::Big => Ok(match (self, other) {
                (Self::Small(a), Self::Small(b)) => match a.checked_mul(*b) {
                    Some(r) => Self::Small(r),
                    None => Self::from_big(BigInt::from(*a) * b),
                },
                _ => Self::from_big(self.to_big() * other.to_big()),
            }),
        }
    }

    /// Unary minus, `-a` (`Integers.Neg`).
    ///
    /// # Errors
    /// [`ErrorKind::Overflow`] in TLC mode for `-(-2^31)`
    /// (`Overflow when computing --2147483648`).
    pub fn neg(&self, mode: IntMode) -> Result<Self> {
        match mode {
            IntMode::Tlc => {
                let a = self.tlc_operand()?;
                a.checked_neg()
                    .map(|r| Self::Small(i64::from(r)))
                    .ok_or_else(|| overflow("--2147483648"))
            }
            IntMode::Big => Ok(match self {
                Self::Small(a) => match a.checked_neg() {
                    Some(r) => Self::Small(r),
                    None => Self::from_big(-BigInt::from(*a)),
                },
                Self::Big(b) => Self::from_big(-(**b).clone()),
            }),
        }
    }

    /// Floored division, `a \div b` (`Integers.Divide`).
    ///
    /// # Errors
    /// [`ErrorKind::DivisionByZero`] when `b = 0`; [`ErrorKind::Overflow`] in TLC mode
    /// for `-2^31 \div -1`.
    pub fn div(&self, other: &Self, mode: IntMode) -> Result<Self> {
        if other.is_zero() {
            return Err(ValueError::new(
                ErrorKind::DivisionByZero,
                "The second argument of \\div is 0.",
            ));
        }
        match mode {
            IntMode::Tlc => {
                let (a, b) = (self.tlc_operand()?, other.tlc_operand()?);
                if a == i32::MIN && b == -1 {
                    return Err(overflow("-2147483648 \\div -1"));
                }
                Ok(Self::Small(i64::from(num_integer::Integer::div_floor(
                    &a, &b,
                ))))
            }
            IntMode::Big => Ok(match (self, other) {
                (Self::Small(a), Self::Small(b)) if !(*a == i64::MIN && *b == -1) => {
                    Self::Small(num_integer::Integer::div_floor(a, b))
                }
                _ => Self::from_big(self.to_big().div_floor(&other.to_big())),
            }),
        }
    }

    /// `a % b`, defined for `b > 0`, with a result in `0 .. b-1` (`Integers.Mod`).
    ///
    /// # Errors
    /// [`ErrorKind::Argument`] when `b <= 0`.
    pub fn rem(&self, other: &Self, mode: IntMode) -> Result<Self> {
        if !other.is_positive() {
            return Err(ValueError::new(
                ErrorKind::Argument,
                format!(
                    "The second argument of % should be a positive number, but instead it is:\n{other}"
                ),
            ));
        }
        match mode {
            IntMode::Tlc => {
                let (a, b) = (self.tlc_operand()?, other.tlc_operand()?);
                Ok(Self::Small(i64::from(num_integer::Integer::mod_floor(
                    &a, &b,
                ))))
            }
            IntMode::Big => Ok(match (self, other) {
                (Self::Small(a), Self::Small(b)) => {
                    Self::Small(num_integer::Integer::mod_floor(a, b))
                }
                _ => Self::from_big(self.to_big().mod_floor(&other.to_big())),
            }),
        }
    }

    /// `a ^ b` for `b >= 0` (`Integers.Expt`).
    ///
    /// # Errors
    /// [`ErrorKind::Argument`] when `b < 0`; [`ErrorKind::NullPowerNull`] for `0^0`;
    /// [`ErrorKind::Overflow`] in TLC mode (`Overflow when computing a^b`);
    /// [`ErrorKind::DiveLimit`] in big mode when the result would exceed 2^26 bits.
    pub fn pow(&self, other: &Self, mode: IntMode) -> Result<Self> {
        if other.is_negative() {
            return Err(ValueError::new(
                ErrorKind::Argument,
                format!(
                    "The second argument of ^ should be a natural number, but instead it is:\n{other}"
                ),
            ));
        }
        if other.is_zero() {
            if self.is_zero() {
                return Err(ValueError::new(
                    ErrorKind::NullPowerNull,
                    "0^0 is undefined.",
                ));
            }
            return Ok(Self::ONE);
        }
        match mode {
            IntMode::Tlc => {
                let (a, b) = (self.tlc_operand()?, other.tlc_operand()?);
                let mut res = i64::from(a);
                for _ in 1..b {
                    // |res| is non-decreasing for |a| >= 2, and bounded for |a| <= 1,
                    // so stopping early gives TLC's answer without its O(b) loop.
                    if a.unsigned_abs() <= 1 {
                        res = if a == -1 && b % 2 == 0 {
                            1
                        } else {
                            i64::from(a)
                        };
                        break;
                    }
                    res *= i64::from(a);
                    if res < i64::from(i32::MIN) || res > i64::from(i32::MAX) {
                        return Err(overflow(&format!("{a}^{b}")));
                    }
                }
                Ok(Self::Small(res))
            }
            IntMode::Big => {
                let base = self.to_big();
                if base.abs() <= BigInt::one() {
                    let odd = other.to_big().is_odd();
                    return Ok(if base.is_negative() && !odd {
                        Self::ONE
                    } else {
                        self.clone()
                    });
                }
                let exp = other.to_i64().and_then(|e| u64::try_from(e).ok());
                let bits = base.bits();
                match exp {
                    Some(e) if e.saturating_mul(bits) <= MAX_POW_BITS => {
                        let e = u32::try_from(e).unwrap_or(u32::MAX);
                        Ok(Self::from_big(num_traits::Pow::pow(base, e)))
                    }
                    _ => Err(ValueError::new(
                        ErrorKind::DiveLimit,
                        format!(
                            "The value of {self}^{other} is too large: it exceeds dive's limit of {MAX_POW_BITS} bits."
                        ),
                    )),
                }
            }
        }
    }

    fn is_zero(&self) -> bool {
        matches!(self, Self::Small(0))
    }

    fn is_positive(&self) -> bool {
        match self {
            Self::Small(v) => *v > 0,
            Self::Big(b) => b.is_positive(),
        }
    }
}

impl From<i64> for Int {
    fn from(v: i64) -> Self {
        Self::Small(v)
    }
}

impl From<i32> for Int {
    fn from(v: i32) -> Self {
        Self::Small(i64::from(v))
    }
}

impl Ord for Int {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Small(a), Self::Small(b)) => a.cmp(b),
            (Self::Small(_), Self::Big(b)) => {
                if b.is_negative() {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            }
            (Self::Big(a), Self::Small(_)) => {
                if a.is_negative() {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            }
            (Self::Big(a), Self::Big(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for Int {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Int {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Small(v) => write!(f, "{v}"),
            Self::Big(b) => write!(f, "{b}"),
        }
    }
}

impl fmt::Debug for Int {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
