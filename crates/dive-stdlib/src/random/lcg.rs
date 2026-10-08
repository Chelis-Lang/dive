//! TLC's pseudo-random index sequences for `RandomSubset`
//! (`EnumerableValue.SubsetEnumerator`, `SetOfFcnsOrRcdsValue.BigIntegerSubsetEnumerator`).

use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};

use super::source::RandomSource;

/// `EnumerableValue.SubsetEnumerator`: `k` indices in `[0, n)` from a full-period
/// linear congruential generator `X' = (a X + c) mod m` with `m >= n`, skipping values
/// `>= n`. The start `X` is `nextInt(n)` and the increment `c` a random prime
/// (`RandomGenerator.nextPrime`), drawn in that order when the enumerator is built.
/// The first `n` indices are distinct; later ones repeat.
#[derive(Clone, Debug)]
pub(crate) struct SubsetIndices {
    n: i64,
    k: i64,
    i: i64,
    index: i64,
    a: i64,
    m: i64,
    c: i64,
}

impl SubsetIndices {
    /// Builds the enumerator, drawing from `src` unless `n <= 0` (TLC draws nothing for
    /// an empty set).
    pub(crate) fn new(count: i32, size: i32, src: &mut dyn RandomSource) -> Self {
        if size <= 0 {
            return Self {
                n: 0,
                k: 0,
                i: 0,
                index: 0,
                a: 0,
                m: 1,
                c: 0,
            };
        }
        let (modulus, multiplier) = optimal_m_and_a(size);
        let index = i64::from(src.next_int_bounded(size).unwrap_or(0));
        let increment = i64::from(src.next_prime());
        Self {
            n: i64::from(size),
            k: i64::from(count),
            i: 0,
            index,
            a: multiplier,
            m: modulus,
            c: increment,
        }
    }

    /// `hasNext` and `nextIndex`.
    pub(crate) fn next_index(&mut self) -> Option<usize> {
        if self.i >= self.k {
            return None;
        }
        self.i += 1;
        loop {
            self.index = (self.a * self.index + self.c) % self.m;
            if self.index < self.n {
                break;
            }
        }
        usize::try_from(self.index).ok()
    }
}

/// `EnumerableValue.computeOptimalMandA(size)`: `m` is the smallest number
/// `>= max(size, 9)` that is neither divisible by 4 nor square-free, and `a - 1` is
/// the product of `m`'s distinct prime factors. Then, with `c` coprime to `m`, the
/// generator has full period `m` (Hull–Dobell).
///
/// TLC factors with `TLCPrimes.primeFactors`, a trial division whose last step
/// (`boundedTrialDivision`) stops after one factor above 3671 and appends the rest as
/// a "factor". For `m < 3673^3` (all `m` here: `size` is an `int`) the rest has at
/// most two prime factors, so that list is the true factorization, which is what
/// this computes.
fn optimal_m_and_a(size: i32) -> (i64, i64) {
    let mut n = i64::from(size.max(9));
    let mut factors = prime_factors(n);
    while n % 4 == 0 || is_square_free(&factors) {
        n += 1;
        factors = prime_factors(n);
    }
    let mut distinct = factors.clone();
    distinct.dedup();
    let a = distinct.iter().product::<i64>() + 1;
    (n, a)
}

/// The prime factors of `n >= 2` with multiplicity, ascending.
fn prime_factors(mut n: i64) -> Vec<i64> {
    let mut out = Vec::new();
    let mut p = 2;
    while p * p <= n {
        while n % p == 0 {
            out.push(p);
            n /= p;
        }
        p += if p == 2 { 1 } else { 2 };
    }
    if n > 1 {
        out.push(n);
    }
    out
}

fn is_square_free(sorted_factors: &[i64]) -> bool {
    sorted_factors.windows(2).all(|w| w[0] != w[1])
}

/// `SetOfFcnsOrRcdsValue.BigIntegerSubsetEnumerator`: for sets whose size does not fit
/// an `int`, `k` indices `(x * i + a) mod size` for `i = 0, 1, ...`, with
/// `x = 2^63 - 25` and `a = |nextLong()|` (Java's `Math.abs`, so `Long.MIN_VALUE` stays
/// negative; `BigInteger.mod` is non-negative either way).
#[derive(Clone, Debug)]
pub(crate) struct BigIndices {
    a: BigInt,
    size: BigInt,
    k: i64,
    i: i64,
}

impl BigIndices {
    pub(crate) fn new(k: i32, size: BigInt, src: &mut dyn RandomSource) -> Self {
        let r = src.next_long();
        // Java's Math.abs(long): wrapping at Long.MIN_VALUE.
        let a = BigInt::from(r.wrapping_abs());
        Self {
            a,
            size,
            k: i64::from(k),
            i: 0,
        }
    }

    pub(crate) fn next_index(&mut self) -> Option<BigInt> {
        if self.i >= self.k || self.size.is_zero() {
            return None;
        }
        let x = BigInt::from(i64::MAX - 24);
        let v = x * BigInt::from(self.i) + &self.a;
        self.i += 1;
        let mut r = v % &self.size;
        if r.is_negative() {
            r += &self.size;
        }
        Some(r)
    }
}

/// The digit of `idx` at `place` in the mixed radix whose lower places have the
/// product `rescale`, for a constituent of `radix` elements:
/// `(idx / rescale) mod radix`.
pub(crate) fn big_digit(idx: &BigInt, rescale: &BigInt, radix: usize) -> usize {
    let r = BigInt::from(radix);
    let mut d = (idx / rescale) % &r;
    if d.is_negative() {
        d += r;
    }
    d.to_usize().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m_and_a_follow_tlc() {
        // n = 9 = 3^2: odd, not square-free; a = 3 + 1.
        assert_eq!(optimal_m_and_a(1), (9, 4));
        assert_eq!(optimal_m_and_a(9), (9, 4));
        // 10 = 2*5 and 11 are square-free, 12 is divisible by 4, 13, 14, 15 are
        // square-free, 16 divisible by 4, 17 square-free, 18 = 2*3^2: a = 7.
        assert_eq!(optimal_m_and_a(10), (18, 7));
        // `EnumerableValue.computeOptimalMandA`, printed by the pinned TLC.
        for (size, m, a) in [
            (1, 9, 4),
            (2, 9, 4),
            (8, 9, 4),
            (11, 18, 7),
            (100, 117, 40),
            (1000, 1014, 79),
            (65_536, 65_538, 21_847),
            (1_000_000, 1_000_017, 333_340),
            (1_000_003, 1_000_017, 333_340),
            (999_999_937, 999_999_938, 142_857_135),
            (2_147_483_646, 2_147_483_646, 715_827_883),
            (i32::MAX, 2_147_483_650, 429_496_731),
        ] {
            assert_eq!(optimal_m_and_a(size), (m, a), "size {size}");
        }
    }

    #[test]
    fn first_n_indices_are_a_permutation() {
        let mut src = super::super::source::JavaRandom::new(3);
        for n in [1, 2, 5, 9, 10, 37, 100] {
            let mut e = SubsetIndices::new(n, n, &mut src);
            let mut seen: Vec<usize> = std::iter::from_fn(|| e.next_index()).collect();
            seen.sort_unstable();
            assert_eq!(
                seen,
                (0..usize::try_from(n).unwrap_or(0)).collect::<Vec<_>>()
            );
        }
    }
}
