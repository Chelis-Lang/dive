//! TLC's fingerprint-collision estimates (ADR 0015 §5).
//!
//! When a fingerprint-mode run succeeds, TLC prints two estimates of the probability
//! that it missed states because two distinct states had the same fingerprint:
//!
//! ```text
//! Model checking completed. No error has been found.
//!   Estimates of the probability that TLC did not check all reachable states
//!   because two distinct states had the same fingerprint:
//!   calculated (optimistic):  val = 2.9E-18
//!   based on the actual fingerprints:  val = 8.6E-12
//! ```
//!
//! Ported from tlaplus `94d0c50` (`tlatools/org.lamport.tlatools/src/`):
//!
//! - **Optimistic:** `distinct × ((generated − distinct) / 2^64)`, evaluated in Java
//!   `double` arithmetic (`AbstractChecker.calculateOptimisticProbability`).
//! - **Actual:** `1 / d`, where `d` is the smallest difference between two
//!   fingerprints in TLC's set, rounded to two significant digits half-up
//!   (`AbstractChecker.reportSuccess`, `BigDecimal` with `MathContext(2)`). TLC's
//!   default set is a `MultiFPSet` of `2^fpBits` sub-sets (default `fpBits` = 1)
//!   split by the fingerprints' top bits, and each sub-set's `DiskFPSet.checkFPs`
//!   scans its own sorted fingerprints; the minimum is over sub-sets
//!   (`MultiFPSet.checkFPs`). So `d` ignores pairs that straddle a sub-set
//!   boundary. [`tlc_min_gap`] reproduces this.
//! - **When the actual figure is printed:** only when the optimistic one is at least
//!   `1E-10` (`ModelChecker.reportSuccess(FPSet, long)`); TLC skips the scan
//!   otherwise. Neither is printed at a violation.
//! - **Printing:** `AbstractChecker.ProbabilityToString` with two significant digits,
//!   applied to Java's `Double.toString`.
//!
//! Both are estimates that assume fingerprints behave like independent random 64-bit
//! values; dive says so wherever it reports them. The actual figure equals TLC's only
//! when computed over TLC's own fingerprints (TLC's variable order and value
//! fingerprints; ADR 0015 §5).

/// TLC's `Long.MAX_VALUE`, the minimum gap of a set with no two fingerprints in one
/// sub-set.
pub const NO_GAP: u64 = i64::MAX.unsigned_abs();

/// The optimistic estimate below which TLC prints only the optimistic figure.
pub const TLC_ACTUAL_THRESHOLD: f64 = 1e-10;

/// TLC's default `fpBits`: its fingerprint set has two sub-sets, split by the top bit.
pub const TLC_DEFAULT_FP_BITS: u32 = 1;

/// The optimistic collision probability, exactly as TLC computes it in Java `double`
/// arithmetic (`AbstractChecker.calculateOptimisticProbability`).
///
/// ```
/// use dive_store::collision::{optimistic_probability, probability_to_string};
/// // CigaretteSmokers: 6 distinct of 15 generated states.
/// let p = optimistic_probability(6, 15);
/// assert_eq!(probability_to_string(p, 2), "2.9E-18");
/// ```
#[must_use]
pub fn optimistic_probability(distinct: u64, generated: u64) -> f64 {
    // `numOfDistinctStates * ((numOfGenStates - numOfDistinctStates) / Math.pow(2, 64))`
    // with Java `long`s: the subtraction wraps, and each operand converts to `double`.
    let d = i64::from_ne_bytes(distinct.to_ne_bytes());
    let g = i64::from_ne_bytes(generated.to_ne_bytes());
    #[allow(
        clippy::cast_precision_loss,
        reason = "Java's long-to-double conversion"
    )]
    let (d, diff) = (d as f64, g.wrapping_sub(d) as f64);
    d * (diff / 2f64.powi(64))
}

/// The "actual" collision probability from the minimum fingerprint gap `gap`, exactly
/// as TLC computes it: `1 / gap` rounded half-up to two significant digits, then
/// converted to `double` (`BigDecimal.divide(…, new MathContext(2)).doubleValue()`).
///
/// A gap of 0 (two equal fingerprints, which a set never holds) gives 1.
///
/// ```
/// use dive_store::collision::{actual_probability, probability_to_string};
/// assert_eq!(probability_to_string(actual_probability(115_755_763_337), 2), "8.6E-12");
/// ```
#[must_use]
pub fn actual_probability(gap: u64) -> f64 {
    if gap == 0 {
        return 1.0;
    }
    // Find k with 10·gap ≤ 10^k < 100·gap, so that 10^k / gap has two integer digits.
    let d = u128::from(gap);
    let (mut p, mut k) = (1u128, 0u32);
    while p < 10 * d {
        p *= 10;
        k += 1;
    }
    let mut m = p / d;
    if 2 * (p % d) >= d {
        m += 1; // HALF_UP
    }
    if m == 100 {
        m = 10;
        k -= 1;
    }
    // `m × 10^-k`, correctly rounded, as `BigDecimal.doubleValue` gives.
    format!("{m}e-{k}").parse().unwrap_or(f64::NAN)
}

/// Whether `a` and `b` lie in the same one of TLC's `2^fp_bits` sub-sets.
fn same_subset(a: u64, b: u64, fp_bits: u32) -> bool {
    let shift = 64 - fp_bits.clamp(1, 63);
    a >> shift == b >> shift
}

/// Folds the gap between consecutive sorted fingerprints `a < b` into `gap`.
pub(crate) fn closer(gap: u64, a: u64, b: u64, fp_bits: u32) -> u64 {
    if b > a && same_subset(a, b, fp_bits) {
        gap.min(b - a)
    } else {
        gap
    }
}

/// The minimum gap TLC's `checkFPs` reports for this set of fingerprints, with TLC's
/// fingerprint set split into `2^fp_bits` sub-sets (TLC's `-fpbits`, default
/// [`TLC_DEFAULT_FP_BITS`]; values are clamped to `1..=63`, since TLC never uses 0
/// for its disk-based sets). [`NO_GAP`] if no two fingerprints share a sub-set.
///
/// ```
/// use dive_store::collision::{NO_GAP, tlc_min_gap};
/// assert_eq!(tlc_min_gap([10, 3, 7], 1), 3);
/// // 0x7FFF… and 0x8000… are adjacent but in different sub-sets.
/// assert_eq!(tlc_min_gap([0x7fff_ffff_ffff_ffff, 0x8000_0000_0000_0000], 1), NO_GAP);
/// ```
#[must_use]
pub fn tlc_min_gap(fps: impl IntoIterator<Item = u64>, fp_bits: u32) -> u64 {
    let mut v: Vec<u64> = fps.into_iter().collect();
    v.sort_unstable();
    v.dedup();
    v.windows(2)
        .fold(NO_GAP, |gap, w| closer(gap, w[0], w[1], fp_bits))
}

/// The figures TLC prints on success, as their text after `val = `.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlcSuccessFigures {
    /// "calculated (optimistic)".
    pub optimistic: String,
    /// "based on the actual fingerprints", when TLC prints it.
    pub actual: Option<String>,
}

/// What TLC prints on success for `distinct` and `generated` states, computing the
/// minimum fingerprint gap with `min_gap` only when TLC would
/// (`ModelChecker.reportSuccess(FPSet, long)` and both `AbstractChecker.reportSuccess`
/// overloads).
///
/// ```
/// use dive_store::collision::tlc_success_figures;
/// let small = tlc_success_figures(6, 15, || unreachable!("not scanned"));
/// assert_eq!(small.optimistic, "2.9E-18");
/// assert_eq!(small.actual, None);
/// let big = tlc_success_figures(6962, 318_877, || 115_755_763_337);
/// assert_eq!(big.optimistic, "1.2E-10");
/// assert_eq!(big.actual.as_deref(), Some("8.6E-12"));
/// ```
pub fn tlc_success_figures(
    distinct: u64,
    generated: u64,
    min_gap: impl FnOnce() -> u64,
) -> TlcSuccessFigures {
    let optimistic = optimistic_probability(distinct, generated);
    if optimistic < TLC_ACTUAL_THRESHOLD {
        return TlcSuccessFigures {
            optimistic: probability_to_string(optimistic, 2),
            actual: None,
        };
    }
    if distinct == generated && generated == 0 {
        return TlcSuccessFigures {
            optimistic: "0.0".to_owned(),
            actual: Some("0.0".to_owned()),
        };
    }
    TlcSuccessFigures {
        optimistic: probability_to_string(optimistic, 2),
        actual: Some(probability_to_string(actual_probability(min_gap()), 2)),
    }
}

/// Java's `Double.toString(double)`: the shortest decimal digits that identify `x`,
/// as `d.ddd` for magnitudes in `[1e-3, 1e7)` and as `d.dddE±n` otherwise, always
/// with a digit after the point.
///
/// The digits are Rust's shortest round-trip digits. The pinned JRE (17) uses the
/// older `FloatingDecimal` algorithm, which in rare cases prints more digits than
/// needed; that can only matter to [`probability_to_string`] when an extra digit
/// changes the rounding, and every TLC figure in the conformance goldens is
/// reproduced.
#[must_use]
pub fn java_double_to_string(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_owned();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0.0" } else { "0.0" }.to_owned();
    }
    let sci = format!("{:e}", x.abs());
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let sign = if x < 0.0 { "-" } else { "" };
    if (1e-3..1e7).contains(&x.abs()) {
        let (int, frac) = if exp >= 0 {
            let width = usize::try_from(exp).unwrap_or(0) + 1;
            let mut padded = digits.clone();
            while padded.len() < width {
                padded.push('0');
            }
            let (int, frac) = padded.split_at(width);
            (int.to_owned(), frac.to_owned())
        } else {
            let zeros = usize::try_from(-exp - 1).unwrap_or(0);
            ("0".to_owned(), format!("{}{digits}", "0".repeat(zeros)))
        };
        let frac = if frac.is_empty() {
            "0".to_owned()
        } else {
            frac
        };
        format!("{sign}{int}.{frac}")
    } else {
        let (first, rest) = digits.split_at(1.min(digits.len()));
        let rest = if rest.is_empty() { "0" } else { rest };
        format!("{sign}{first}.{rest}E{exp}")
    }
}

/// TLC's `AbstractChecker.ProbabilityToString(double, int)`: Java's
/// `Double.toString(val)` cut to `significant_digits` significant digits, rounding
/// half-up on the next digit only. A port of the Java method, quirks included.
#[must_use]
pub fn probability_to_string(val: f64, significant_digits: usize) -> String {
    if val == 0.0 {
        return "0.0".to_owned();
    }
    let java = java_double_to_string(val);
    let s: Vec<char> = java.chars().collect();
    let len = s.len();
    let is_digit = |i: usize| s.get(i).is_some_and(char::is_ascii_digit);
    let mut result: Vec<char> = Vec::new();
    let mut next = 0;
    let mut found = 0;
    while next < len && s[next] == '0' {
        next += 1;
    }
    while is_digit(next) {
        result.push(s[next]);
        found += 1;
        next += 1;
    }
    if next == len {
        return result.into_iter().collect();
    } else if s[next] != '.' {
        return java;
    }
    if found >= significant_digits {
        next += 1;
        while is_digit(next) {
            next += 1;
        }
    } else {
        next += 1;
        result.push('.');
        if found == 0 {
            while next < len && s[next] == '0' {
                next += 1;
                result.push('0');
            }
        }
        while is_digit(next) && found < significant_digits {
            result.push(s[next]);
            next += 1;
            found += 1;
        }
        if is_digit(next) && s[next].to_digit(10).is_some_and(|d| d >= 5) {
            carry(&mut result);
        }
        while is_digit(next) {
            next += 1;
        }
    }
    if next >= len {
        return result.into_iter().collect();
    }
    if s[next] == 'E' {
        result.extend(&s[next..]);
        return result.into_iter().collect();
    }
    java
}

/// Propagates a rounding carry backwards over `result`'s digits, as
/// `ProbabilityToString` does (skipping the point; prefixing `1` past the front).
fn carry(result: &mut Vec<char>) {
    let mut prev = result.len();
    loop {
        let Some(p) = prev.checked_sub(1) else {
            result.insert(0, '1');
            return;
        };
        prev = p;
        match result[p] {
            '9' => result[p] = '0',
            c @ '0'..='8' => {
                result[p] = char::from_digit(c.to_digit(10).unwrap_or(0) + 1, 10).unwrap_or(c);
                return;
            }
            _ => {} // the point: keep going
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_double_to_string_layout() {
        for (x, s) in [
            (1.0, "1.0"),
            (0.5, "0.5"),
            (0.001, "0.001"),
            (0.000_99, "9.9E-4"),
            (1e7, "1.0E7"),
            (1_234_567.0, "1234567.0"),
            (2.927_345_865_710_862e-18, "2.927345865710862E-18"),
            (1e-10, "1.0E-10"),
            (100.0, "100.0"),
            (-0.25, "-0.25"),
        ] {
            assert_eq!(java_double_to_string(x), s, "{x:e}");
        }
    }

    #[test]
    fn probability_to_string_rounds_like_tlc() {
        // The inputs of the commented-out test in AbstractChecker, with the outputs
        // of TLC's own method on the pinned runtime (tests/fixtures/tlc-probability).
        for (x, s) in [
            (0.5, ".5"),
            (0.0995, ".100"),
            (0.000_000_01, "1.0E-8"),
            (1.0, "1.0"),
            (0.002_234_1, ".0022"),
            (0.002_235_1, ".0022"),
            (3.141_59e-12, "3.1E-12"),
            (0.999, "1.00"),
            (0.002_351_111, ".0024"),
            (22.8e-14, "2.3E-13"),
            (37.0, "37"),
            (4.85, "4.9"),
            (9.95e-11, "10.0E-11"),
        ] {
            assert_eq!(probability_to_string(x, 2), s, "{x:e}");
        }
    }

    #[test]
    fn actual_probability_rounds_half_up() {
        assert!((actual_probability(1) - 1.0).abs() < f64::EPSILON);
        assert_eq!(probability_to_string(actual_probability(8), 2), ".13");
        assert_eq!(
            probability_to_string(actual_probability(NO_GAP), 2),
            "1.1E-19"
        );
        // 1/40 = 0.025 exactly.
        assert_eq!(probability_to_string(actual_probability(40), 2), ".025");
        // 1/95 = 0.010526… rounds to 0.011.
        assert_eq!(probability_to_string(actual_probability(95), 2), ".011");
        // 1/999 = 0.001001… rounds to 0.0010, which Java prints as 0.001.
        assert_eq!(probability_to_string(actual_probability(999), 2), ".001");
    }
}
