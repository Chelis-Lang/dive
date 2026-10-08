//! The bit source every random draw is built on, and `java.util.Random`, bit-exact.

use super::primes::PRIMES;

/// A source of random bits, with the derived draws TLC uses.
///
/// Everything TLC draws reduces to `java.util.Random.next(bits)`: `nextInt(bound)`,
/// `nextLong` and `nextDouble` are fixed functions of successive `next` calls. So a
/// source only implements [`RandomSource::next_bits`]; the provided methods are
/// Java 17's algorithms (`java/util/Random.java`), and give TLC's values exactly when
/// the bits are [`JavaRandom`]'s.
///
/// The trait is object safe. Any `FnMut(u32) -> i32` is a source, so an evaluator
/// that must not name this crate (`dive-eval` sits below `dive-stdlib` in the
/// layering) can carry a `&mut dyn FnMut(u32) -> i32` and the operators accept it:
///
/// ```
/// use dive_stdlib::random::{JavaRandom, RandomSource};
/// let mut rng = JavaRandom::new(7);
/// let mut bits = |n| rng.next_bits(n);
/// let f: &mut dyn FnMut(u32) -> i32 = &mut bits;
/// // RandomElement(1..1000000) with -seed 7, as TLC draws it (probe P9).
/// assert_eq!(1 + (f.next_double() * 1_000_000.0).floor() as i64, 730_700);
/// ```
pub trait RandomSource {
    /// The next `bits` random bits (`1..=32`), as `java.util.Random.next(bits)`:
    /// non-negative for `bits < 32`, any `i32` for 32.
    fn next_bits(&mut self, bits: u32) -> i32;

    /// `Random.nextDouble()`: a double in `[0, 1)` from 53 bits.
    fn next_double(&mut self) -> f64 {
        let hi = i64::from(self.next_bits(26));
        let lo = i64::from(self.next_bits(27));
        #[allow(clippy::cast_precision_loss, reason = "exact: the value has 53 bits")]
        let n = ((hi << 27) + lo) as f64;
        n * DOUBLE_UNIT
    }

    /// `Random.nextInt(bound)`, or `None` when `bound <= 0` (where Java throws
    /// `IllegalArgumentException`). Draws nothing for a bad bound.
    fn next_int_bounded(&mut self, bound: i32) -> Option<i32> {
        if bound <= 0 {
            return None;
        }
        let mut r = self.next_bits(31);
        let m = bound - 1;
        if bound & m == 0 {
            // A power of two: the high bits.
            #[allow(clippy::cast_possible_truncation, reason = "result < bound")]
            return Some(((i64::from(bound) * i64::from(r)) >> 31) as i32);
        }
        let mut u = r;
        loop {
            r = u % bound;
            // Java: `u - r + m < 0` in int arithmetic rejects the biased tail.
            if u.wrapping_sub(r).wrapping_add(m) < 0 {
                u = self.next_bits(31);
            } else {
                return Some(r);
            }
        }
    }

    /// `Random.nextLong()`.
    fn next_long(&mut self) -> i64 {
        let hi = i64::from(self.next_bits(32));
        let lo = i64::from(self.next_bits(32));
        (hi << 32).wrapping_add(lo)
    }

    /// `RandomGenerator.nextPrime(Random)`: one of TLC's 1341 primes near 1.28
    /// million, chosen with one `nextDouble`.
    fn next_prime(&mut self) -> i32 {
        let len = PRIMES.len();
        let mut index = len;
        while index == len {
            // `(int) Math.floor(r.nextDouble() * index)`; `index` is `len` here.
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss,
                reason = "0 <= nextDouble * len < len"
            )]
            {
                index = (self.next_double() * len as f64).floor() as usize;
            }
        }
        PRIMES[index]
    }

    /// `(int) Math.floor(nextDouble() * size)`: TLC's index into a set of `size`
    /// elements (`SetEnumValue.randomElement`, `IntervalValue.randomElement`,
    /// `SimulationWorker.randomState`). `size` 0 gives 0, as in TLC, after drawing.
    fn next_index(&mut self, size: u64) -> u64 {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "0 <= nextDouble * size < size; sizes past 2^53 lose precision \
                      deterministically (dive mode only: TLC sizes are ints)"
        )]
        let i = (self.next_double() * size as f64).floor() as u64;
        i
    }
}

/// `0x1.0p-53`, Java's `DOUBLE_UNIT`.
const DOUBLE_UNIT: f64 = f64::EPSILON / 2.0;

impl<F: FnMut(u32) -> i32 + ?Sized> RandomSource for F {
    fn next_bits(&mut self, bits: u32) -> i32 {
        self(bits)
    }
}

/// `java.util.Random`, bit-exact: the 48-bit linear congruential generator of
/// Knuth, TAOCP vol. 2 §3.2.1, as Java specifies it.
///
/// TLC draws all of its pseudo-random choices from instances of this class (or of its
/// subclass `tlc2.util.RandomGenerator`, which only adds counting and
/// [`RandomSource::next_prime`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct JavaRandom {
    state: u64,
}

const MULTIPLIER: u64 = 0x0005_DEEC_E66D;
const ADDEND: u64 = 0xB;
const MASK: u64 = (1 << 48) - 1;

impl JavaRandom {
    /// `new Random(seed)`.
    #[must_use]
    pub const fn new(seed: i64) -> Self {
        Self {
            state: Self::scramble(seed),
        }
    }

    /// `setSeed(seed)`.
    pub const fn set_seed(&mut self, seed: i64) {
        self.state = Self::scramble(seed);
    }

    #[allow(
        clippy::cast_sign_loss,
        reason = "two's complement bits, as Java's long"
    )]
    const fn scramble(seed: i64) -> u64 {
        (seed as u64 ^ MULTIPLIER) & MASK
    }

    /// `RandomGenerator.setSeed(seed, aril)`: reseed, then skip `aril` doubles (TLC's
    /// `-aril` for simulation).
    pub fn set_seed_aril(&mut self, seed: i64, aril: u64) {
        self.set_seed(seed);
        for _ in 0..aril {
            let _ = self.next_double();
        }
    }
}

impl RandomSource for JavaRandom {
    fn next_bits(&mut self, bits: u32) -> i32 {
        debug_assert!((1..=32).contains(&bits));
        self.state = self.state.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK;
        // `(int)(seed >>> (48 - bits))`: keep the low 32 bits of the shifted value.
        #[allow(clippy::cast_possible_truncation, reason = "Java's (int) cast")]
        let v = (self.state >> (48 - bits)) as u32;
        v.cast_signed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_java_for_seed_7() {
        // `java.util.Random(7)`: floor(nextDouble() * 1000000), from the pinned JRE.
        let mut r = JavaRandom::new(7);
        let got: Vec<u64> = (0..10).map(|_| r.next_index(1_000_000)).collect();
        assert_eq!(
            got,
            [
                730_699, 749_169, 348_309, 897_277, 708_177, 351_914, 120_736, 849_913, 83_219,
                928_748
            ]
        );
    }

    #[test]
    fn bad_bound_draws_nothing() {
        let mut r = JavaRandom::new(1);
        let before = r;
        assert_eq!(r.next_int_bounded(0), None);
        assert_eq!(r.next_int_bounded(-5), None);
        assert_eq!(r, before);
    }
}
