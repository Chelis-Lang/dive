//! Integers of any size (`03-value-model.md` §9, §11.1; D-004).
//!
//! An integer is an inline handle while it fits 61 bits, and otherwise a
//! compound node of kind [`Kind::BigInt`]: `lo` is the sign (`1` or `-1`)
//! and `data` the magnitude in 32-bit digits, least significant first, each
//! an inline integer handle (so the generic walks of the heap never mistake
//! a digit for a child). The form is canonical, so equal integers have equal
//! node contents and an inline and a big handle are never equal.
//!
//! Arithmetic follows the reference engine's `dive_value::Int` (a port of
//! `tlc2.module.Naturals` and `tlc2.module.Integers`, tlaplus `94d0c50`):
//! in [`IntMode::Big`] (the default profile) results are exact; in
//! [`IntMode::Tlc`] (`--int=tlc`) operands and results are Java `int`s, and
//! leaving `[-2^31, 2^31 - 1]` is TLC's overflow error, with TLC's message.

use std::cmp::Ordering;

use num_bigint::{BigInt, Sign};
use num_integer::Integer as _;
use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::Error;
use crate::handle::Handle;
use crate::heap::Heap;
use crate::node::Kind;

/// The integer semantics of a run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IntMode {
    /// Mathematical integers (the default profile, D-004).
    #[default]
    Big,
    /// TLC's 32-bit integers (`--int=tlc`, part of `--compat=tlc`).
    Tlc,
}

/// A binary integer operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntOp {
    /// `a + b`.
    Add,
    /// `a - b`.
    Sub,
    /// `a * b`.
    Mul,
    /// `a \div b`, floored (`Integers.Divide`).
    Div,
    /// `a \div b` as `Naturals.Divide` computes it, for a specification
    /// that loads `Naturals` but not `Integers`. Floored in big mode.
    DivNaturals,
    /// `a % b`, for `b > 0` (`Integers.Mod`).
    Mod,
    /// `a ^ b`, for `b >= 0` (`Integers.Expt`).
    Pow,
}

/// The largest result of `^` computed in big mode, in bits (as the
/// reference engine).
const MAX_POW_BITS: u64 = 1 << 26;

fn overflow(what: &str) -> Error {
    Error::eval(format!("Overflow when computing {what}"))
}

impl IntOp {
    const fn symbol(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div | Self::DivNaturals => " \\div ",
            Self::Mod => "%",
            Self::Pow => "^",
        }
    }
}

impl Heap {
    /// The integer semantics of this heap's arithmetic.
    #[must_use]
    pub fn int_mode(&self) -> IntMode {
        self.int_mode
    }

    /// Sets the integer semantics (from the run's profile).
    pub fn set_int_mode(&mut self, mode: IntMode) {
        self.int_mode = mode;
    }

    /// An integer, inline when it fits 61 bits.
    pub fn int_from_i64(&mut self, v: i64) -> Handle {
        Handle::int(v).unwrap_or_else(|| self.big_node(&BigInt::from(v)))
    }

    /// An integer, inline when it fits 61 bits.
    pub fn int_from_big(&mut self, v: &BigInt) -> Handle {
        match v.to_i64().and_then(Handle::int) {
            Some(h) => h,
            None => self.big_node(v),
        }
    }

    fn big_node(&mut self, v: &BigInt) -> Handle {
        let (sign, digits) = v.to_u32_digits();
        let data: Vec<Handle> = digits
            .into_iter()
            .map(|d| Handle::int(i64::from(d)).unwrap_or(Handle::EMPTY))
            .collect();
        let sign = if sign == Sign::Minus { -1 } else { 1 };
        self.symbolic(Kind::BigInt, sign, &data)
    }

    /// Whether `h` is an integer of any size.
    #[must_use]
    pub fn is_int(&self, h: Handle) -> bool {
        h.as_int().is_some() || (h.is_compound() && self.view(h).kind == Kind::BigInt)
    }

    /// The value of an integer handle of any size.
    #[must_use]
    pub fn big(&self, h: Handle) -> Option<BigInt> {
        if let Some(v) = h.as_int() {
            return Some(BigInt::from(v));
        }
        if !h.is_compound() {
            return None;
        }
        let v = self.view(h);
        if v.kind != Kind::BigInt {
            return None;
        }
        let digits: Vec<u32> = v
            .data
            .iter()
            .map(|d| d.as_int().and_then(|x| u32::try_from(x).ok()).unwrap_or(0))
            .collect();
        let sign = if v.lo < 0 { Sign::Minus } else { Sign::Plus };
        Some(BigInt::from_slice(sign, &digits))
    }

    /// The numeric order of two integers of any size; `None` unless both are
    /// integers.
    #[must_use]
    pub fn int_cmp(&self, a: Handle, b: Handle) -> Option<Ordering> {
        if let (Some(x), Some(y)) = (a.as_int(), b.as_int()) {
            return Some(x.cmp(&y));
        }
        Some(self.big(a)?.cmp(&self.big(b)?))
    }

    /// `op(a, b)` under the heap's [`IntMode`].
    ///
    /// # Errors
    /// A non-integer operand; division by zero, `%` by a non-positive
    /// number, a negative exponent, `0^0`; in TLC mode TLC's overflow; in
    /// big mode a power above 2^26 bits (unsupported, as the reference).
    pub fn int_arith(&mut self, op: IntOp, lhs: Handle, rhs: Handle) -> Result<Handle, Error> {
        if self.int_mode == IntMode::Big
            && let (Some(x), Some(y)) = (lhs.as_int(), rhs.as_int())
            && let Some(small) = small_arith(op, x, y)
        {
            return small.map(|v| self.int_from_i64(v));
        }
        let (Some(x), Some(y)) = (self.big(lhs), self.big(rhs)) else {
            return Err(Error::eval("expected an integer"));
        };
        let value = match self.int_mode {
            IntMode::Big => big_arith(op, &x, &y)?,
            IntMode::Tlc => BigInt::from(tlc_arith(op, tlc_operand(&x)?, tlc_operand(&y)?)?),
        };
        Ok(self.int_from_big(&value))
    }

    /// `-a` under the heap's [`IntMode`].
    ///
    /// # Errors
    /// A non-integer operand; in TLC mode `-(-2^31)`.
    pub fn int_neg(&mut self, a: Handle) -> Result<Handle, Error> {
        let Some(x) = self.big(a) else {
            return Err(Error::eval("expected an integer"));
        };
        if self.int_mode == IntMode::Tlc {
            let v = tlc_operand(&x)?;
            return v
                .checked_neg()
                .map(|r| Handle::int(i64::from(r)).unwrap_or(Handle::EMPTY))
                .ok_or_else(|| overflow("--2147483648"));
        }
        Ok(self.int_from_big(&-x))
    }
}

/// An operand in TLC mode: a Java `int`.
fn tlc_operand(v: &BigInt) -> Result<i32, Error> {
    v.to_i32().ok_or_else(|| {
        Error::eval(format!(
            "Overflow when computing {v} (not a 32-bit TLC integer)"
        ))
    })
}

/// The errors that do not depend on the mode, from the operands' signs.
fn domain_errors(
    op: IntOp,
    x_sign: Ordering,
    y_sign: Ordering,
    y: &dyn std::fmt::Display,
) -> Result<(), Error> {
    match (op, y_sign) {
        (IntOp::Div | IntOp::DivNaturals, Ordering::Equal) => {
            Err(Error::eval("The second argument of \\div is 0."))
        }
        (IntOp::Mod, Ordering::Equal | Ordering::Less) => Err(Error::eval(format!(
            "The second argument of % should be a positive number, but instead it is:\n{y}"
        ))),
        (IntOp::Pow, Ordering::Less) => Err(Error::eval(format!(
            "The second argument of ^ should be a natural number, but instead it is:\n{y}"
        ))),
        (IntOp::Pow, Ordering::Equal) if x_sign == Ordering::Equal => {
            Err(Error::eval("0^0 is undefined."))
        }
        _ => Ok(()),
    }
}

fn big_sign(v: &BigInt) -> Ordering {
    match v.sign() {
        Sign::Minus => Ordering::Less,
        Sign::NoSign => Ordering::Equal,
        Sign::Plus => Ordering::Greater,
    }
}

/// Big-mode arithmetic on two `i64`s: `None` when the result needs a big
/// integer (the slow path then computes it).
fn small_arith(op: IntOp, x: i64, y: i64) -> Option<Result<i64, Error>> {
    if let Err(e) = domain_errors(op, x.cmp(&0), y.cmp(&0), &y) {
        return Some(Err(e));
    }
    let r = match op {
        IntOp::Add => x.checked_add(y),
        IntOp::Sub => x.checked_sub(y),
        IntOp::Mul => x.checked_mul(y),
        IntOp::Div | IntOp::DivNaturals => {
            if x == i64::MIN && y == -1 {
                None
            } else {
                Some(num_integer::Integer::div_floor(&x, &y))
            }
        }
        IntOp::Mod => Some(x.mod_floor(&y)),
        IntOp::Pow => {
            if y == 0 {
                Some(1)
            } else {
                u32::try_from(y).ok().and_then(|e| x.checked_pow(e))
            }
        }
    };
    r.map(Ok)
}

fn big_arith(op: IntOp, x: &BigInt, y: &BigInt) -> Result<BigInt, Error> {
    domain_errors(op, big_sign(x), big_sign(y), y)?;
    Ok(match op {
        IntOp::Add => x + y,
        IntOp::Sub => x - y,
        IntOp::Mul => x * y,
        IntOp::Div | IntOp::DivNaturals => x.div_floor(y),
        IntOp::Mod => x.mod_floor(y),
        IntOp::Pow => {
            if y.is_zero() {
                return Ok(BigInt::one());
            }
            if x.abs() <= BigInt::one() {
                return Ok(if x.is_negative() && y.is_even() {
                    BigInt::one()
                } else {
                    x.clone()
                });
            }
            let e = y
                .to_u64()
                .filter(|e| e.saturating_mul(x.bits()) <= MAX_POW_BITS);
            let Some(e) = e.and_then(|e| u32::try_from(e).ok()) else {
                return Err(Error::unsupported(format!(
                    "The value of {x}^{y} is too large: it exceeds dive's limit of {MAX_POW_BITS} bits."
                )));
            };
            num_traits::Pow::pow(x, e)
        }
    })
}

/// TLC-mode arithmetic on Java `int`s (`Naturals`, `Integers`).
fn tlc_arith(op: IntOp, a: i32, b: i32) -> Result<i32, Error> {
    domain_errors(op, a.cmp(&0), b.cmp(&0), &b)?;
    let what = || format!("{a}{}{b}", op.symbol());
    match op {
        IntOp::Add => a.checked_add(b).ok_or_else(|| overflow(&what())),
        IntOp::Sub => a.checked_sub(b).ok_or_else(|| overflow(&what())),
        IntOp::Mul => a.checked_mul(b).ok_or_else(|| overflow(&what())),
        IntOp::Div => {
            if a == i32::MIN && b == -1 {
                return Err(overflow("-2147483648 \\div -1"));
            }
            Ok(num_integer::Integer::div_floor(&a, &b))
        }
        IntOp::DivNaturals => {
            // `Naturals.Divide`: Java's truncating division, decremented
            // only when the quotient is negative and inexact; wraps on
            // `-2^31 \div -1`.
            let mut q = a.wrapping_div(b);
            if q < 0 && q.wrapping_mul(b) != a {
                q -= 1;
            }
            Ok(q)
        }
        IntOp::Mod => Ok(a.mod_floor(&b)),
        IntOp::Pow => {
            if b == 0 {
                return Ok(1);
            }
            if a.unsigned_abs() <= 1 {
                return Ok(if a == -1 && b % 2 == 0 { 1 } else { a });
            }
            let mut res = i64::from(a);
            for _ in 1..b {
                res *= i64::from(a);
                if res < i64::from(i32::MIN) || res > i64::from(i32::MAX) {
                    return Err(overflow(&format!("{a}^{b}")));
                }
            }
            i32::try_from(res).map_err(|_| overflow(&format!("{a}^{b}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::store::Store;

    fn heap(mode: IntMode) -> Heap {
        let mut h = Heap::new(Arc::new(Store::default()), false, false);
        h.set_int_mode(mode);
        h
    }

    fn int(v: i64) -> Handle {
        Handle::int(v).unwrap_or(Handle::EMPTY)
    }

    #[test]
    fn big_mode_spills_and_returns_canonically() {
        let mut h = heap(IntMode::Big);
        let max = int(crate::handle::INT_MAX);
        let big = h
            .int_arith(IntOp::Add, max, int(1))
            .unwrap_or(Handle::EMPTY);
        assert!(big.is_compound());
        assert_eq!(h.kind(big), Some(Kind::BigInt));
        assert_eq!(h.big(big), Some(BigInt::from(crate::handle::INT_MAX) + 1));
        // Back into range: inline again.
        let back = h
            .int_arith(IntOp::Sub, big, int(1))
            .unwrap_or(Handle::EMPTY);
        assert_eq!(back, max);
        // 2^100, interned and compared.
        let p = h
            .int_arith(IntOp::Pow, int(2), int(100))
            .unwrap_or(Handle::EMPTY);
        let q = h.int_arith(IntOp::Mul, p, int(1)).unwrap_or(Handle::EMPTY);
        assert!(h.eq(p, q));
        assert_eq!(h.hash(p), h.hash(q));
        let (pp, pq) = (
            h.intern(p).unwrap_or(Handle::EMPTY),
            h.intern(q).unwrap_or(Handle::EMPTY),
        );
        assert_eq!(pp, pq);
        let n = h.int_neg(p).unwrap_or(Handle::EMPTY);
        assert_eq!(h.int_cmp(n, int(0)), Some(Ordering::Less));
        assert_eq!(
            h.int_cmp(p, int(i64::from(i32::MAX))),
            Some(Ordering::Greater)
        );
        assert!(!h.poisoned());
    }

    #[test]
    fn big_integers_order_among_integers() {
        let mut hp = heap(IntMode::Big);
        let big = hp
            .int_arith(IntOp::Pow, int(10), int(30))
            .unwrap_or(Handle::EMPTY);
        let neg = hp.int_neg(big).unwrap_or(Handle::EMPTY);
        let text = hp.string("a").unwrap_or(Handle::EMPTY);
        let empty = hp.empty_set();
        let mut v = vec![text, big, int(3), empty, Handle::TRUE, neg];
        let set = hp.set_from(&mut v);
        let got = hp.view(set).data.to_vec();
        assert_eq!(got, vec![Handle::TRUE, neg, int(3), big, text, empty]);
        assert!(!hp.poisoned());
    }

    #[test]
    fn floored_division_and_modulus() {
        let mut h = heap(IntMode::Big);
        let q = |h: &mut Heap, op, a, b| {
            h.int_arith(op, int(a), int(b))
                .ok()
                .and_then(Handle::as_int)
        };
        assert_eq!(q(&mut h, IntOp::Div, -7, 2), Some(-4));
        assert_eq!(q(&mut h, IntOp::Mod, -7, 2), Some(1));
        assert_eq!(
            q(&mut h, IntOp::DivNaturals, -1, 2),
            Some(-1),
            "floored in big mode"
        );
        assert!(h.int_arith(IntOp::Div, int(1), int(0)).is_err());
        assert!(h.int_arith(IntOp::Mod, int(1), int(0)).is_err());
        assert!(h.int_arith(IntOp::Pow, int(0), int(0)).is_err());
        assert!(h.int_arith(IntOp::Pow, int(2), int(-1)).is_err());
        assert_eq!(q(&mut h, IntOp::Pow, -1, 7), Some(-1));
    }

    #[test]
    fn tlc_mode_reproduces_tlc_overflow() {
        let mut h = heap(IntMode::Tlc);
        let msg = |r: Result<Handle, Error>| r.err().map(|e| e.message().to_owned());
        assert_eq!(
            msg(h.int_arith(IntOp::Add, int(2_147_483_647), int(1))),
            Some("Overflow when computing 2147483647+1".to_owned())
        );
        assert_eq!(
            msg(h.int_arith(IntOp::Mul, int(65536), int(32768))),
            Some("Overflow when computing 65536*32768".to_owned())
        );
        assert_eq!(
            msg(h.int_arith(IntOp::Pow, int(2), int(31))),
            Some("Overflow when computing 2^31".to_owned())
        );
        assert_eq!(
            msg(h.int_neg(int(-2_147_483_648))),
            Some("Overflow when computing --2147483648".to_owned())
        );
        assert_eq!(
            msg(h.int_arith(IntOp::Div, int(-2_147_483_648), int(-1))),
            Some("Overflow when computing -2147483648 \\div -1".to_owned())
        );
        // `Naturals.Divide` truncates toward zero when the quotient is 0,
        // and wraps instead of overflowing.
        let q = |h: &mut Heap, a, b| {
            h.int_arith(IntOp::DivNaturals, int(a), int(b))
                .ok()
                .and_then(Handle::as_int)
        };
        assert_eq!(q(&mut h, -1, 2), Some(0));
        assert_eq!(q(&mut h, -7, 2), Some(-4));
        assert_eq!(q(&mut h, -2_147_483_648, -1), Some(-2_147_483_648));
        assert_eq!(
            h.int_arith(IntOp::Pow, int(2), int(30))
                .ok()
                .and_then(Handle::as_int),
            Some(1 << 30)
        );
    }
}
