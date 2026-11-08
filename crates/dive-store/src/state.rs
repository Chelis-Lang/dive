//! State fingerprints and exact-store keys.
//!
//! A state is the tuple of its variables' values in the IR's variable order.
//! [`state_key`] concatenates the variables' canonical value encodings
//! ([`dive_value::codec`]): within one run, two states have equal keys exactly when
//! they are equal.
//!
//! [`StateFingerprinter`] computes the state fingerprint of ADR 0015 §5: dive's
//! two-level fingerprint (FP64 value fingerprints combined by [`combine`]), or
//! TLC's own.

use dive_value::fp64::{Fingerprinter, TlcFingerprint};
use dive_value::{Ctx, Value, codec};

use crate::error::StoreError;

/// Appends the canonical exact-store key of a state.
///
/// # Errors
/// TLC's fingerprinting errors (a variable holding `Nat`, say), as
/// [`StoreError::Value`].
pub fn state_key(vars: &[Value], ctx: Ctx, out: &mut Vec<u8>) -> Result<(), StoreError> {
    for v in vars {
        codec::encode_canonical(v, ctx, out)?;
    }
    Ok(())
}

/// The state fingerprint (ADR 0015 §5), in one of two schemes.
///
/// - **dive's** ([`StateFingerprinter::new`], the default profile): each
///   variable's value is fingerprinted with FP64 under the run's polynomial,
///   and the value fingerprints are combined, in the IR's variable order, by a
///   non-linear keyed chain ([`combine`]): every value fingerprint is mixed
///   with its position's key by a 64-bit bijective mixer ([`mix64`]), and the
///   chain mixes again after each one. Value fingerprints are cached in shared
///   compound values ([`TlcFingerprint::fingerprint_cached`]), so a successor
///   pays only for the variables whose values it changed; combining costs two
///   mixer calls per variable.
/// - **TLC's** ([`StateFingerprinter::tlc`], `--compat=tlc`):
///   `TLCStateMut.fingerPrint` without `VIEW` or symmetry, one FP64 stream over
///   the values in TLC's variable order. It equals TLC's fingerprint bit for
///   bit, so the fingerprint set, and the "actual" collision figure computed
///   from it, are TLC's. It cannot reuse cached value fingerprints.
///
/// Both take a state in the IR's variable order. TLC's order is a permutation
/// of it (`dive_ir::Ir::tlc_var_order`), passed in as IR variable indices:
/// `order[i]` is the IR index of the variable whose TLC `varLoc` is `i`.
///
/// The scheme decides only which fingerprints are produced. Verdicts and
/// counts are the same under both, except through a collision.
///
/// **Why the combination is not FP64.** FP64 is affine over GF(2), so FP64
/// over FP64 value fingerprints is affine in every value's bytes: the same
/// change made to two variables cancels whenever its two positional factors
/// agree modulo the polynomial, and such states collide with certainty
/// (issue #42). The mixed chain has no such structure. For two states that
/// differ in exactly one variable whose value fingerprints differ, the chain
/// cannot collide at all (each step is a bijection); otherwise a collision
/// needs the mixer outputs to agree, which for unrelated inputs happens with
/// probability about `2^-64`, the per-pair figure TLC's estimates assume.
#[derive(Clone, Debug)]
pub struct StateFingerprinter {
    values: TlcFingerprint,
    index: usize,
    /// TLC's variable order, or `None` for the IR's order.
    tlc_order: Option<Box<[usize]>>,
    /// Whether [`StateFingerprinter::fingerprint`] is TLC's fingerprint.
    tlc_scheme: bool,
}

impl StateFingerprinter {
    /// dive's two-level fingerprint with TLC's polynomial `index` (`-fp index`,
    /// `0..131`): the default profile. Its [`StateFingerprinter::tlc_fingerprint`]
    /// takes the variables in the order given.
    #[must_use]
    pub fn new(index: usize) -> Option<Self> {
        TlcFingerprint::new(index).map(|values| Self {
            values,
            index,
            tlc_order: None,
            tlc_scheme: false,
        })
    }

    /// TLC's own state fingerprint with polynomial `index`: the scheme of
    /// `--compat=tlc`. `tlc_order` is TLC's variable order as IR variable
    /// indices (`tlc_order[varLoc]` is an IR index). `None` when `index` is out
    /// of range or `tlc_order` is not a permutation of `0..n`.
    ///
    /// ```
    /// use dive_store::state::StateFingerprinter;
    /// use dive_value::{Ctx, Value};
    /// let ctx = Ctx::tlc();
    /// // The IR's order is (x, y); TLC's is (y, x).
    /// let f = StateFingerprinter::tlc(0, &[1, 0]).ok_or("a permutation")?;
    /// let identity = StateFingerprinter::tlc(0, &[0, 1]).ok_or("a permutation")?;
    /// let x_y = [Value::Int(1), Value::Int(2)];
    /// let y_x = [Value::Int(2), Value::Int(1)];
    /// assert_eq!(f.fingerprint(&x_y, ctx)?, identity.fingerprint(&y_x, ctx)?);
    /// assert_ne!(f.fingerprint(&x_y, ctx)?, identity.fingerprint(&x_y, ctx)?);
    /// assert!(StateFingerprinter::tlc(0, &[0, 0]).is_none());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn tlc(index: usize, tlc_order: &[usize]) -> Option<Self> {
        let mut seen = vec![false; tlc_order.len()];
        for &i in tlc_order {
            let slot = seen.get_mut(i)?;
            if *slot {
                return None;
            }
            *slot = true;
        }
        TlcFingerprint::new(index).map(|values| Self {
            values,
            index,
            tlc_order: Some(tlc_order.into()),
            tlc_scheme: true,
        })
    }

    /// The polynomial index, to report with every fingerprint-mode result.
    #[must_use]
    pub const fn poly_index(&self) -> usize {
        self.index
    }

    /// Whether [`StateFingerprinter::fingerprint`] computes TLC's fingerprint.
    #[must_use]
    pub const fn is_tlc(&self) -> bool {
        self.tlc_scheme
    }

    /// A state's fingerprint in this fingerprinter's scheme; `vars` is in the
    /// IR's variable order.
    ///
    /// ```
    /// use dive_store::state::StateFingerprinter;
    /// use dive_value::{Ctx, Value};
    /// let f = StateFingerprinter::new(0).ok_or("index 0 exists")?;
    /// let ctx = Ctx::tlc();
    /// let s = [Value::Int(1), Value::tuple(vec![Value::TRUE])];
    /// let t = [Value::Int(2), Value::tuple(vec![Value::TRUE])];
    /// assert_ne!(f.fingerprint(&s, ctx)?, f.fingerprint(&t, ctx)?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    /// TLC's fingerprinting errors, such as a variable holding `Nat`.
    pub fn fingerprint(&self, vars: &[Value], ctx: Ctx) -> Result<u64, StoreError> {
        if self.tlc_scheme {
            return self.tlc_fingerprint(vars, ctx);
        }
        let mut chain = Chain::new();
        for v in vars {
            chain.push(self.values.fingerprint_cached(v, ctx)?);
        }
        Ok(chain.finish())
    }

    /// TLC's own state fingerprint (`TLCStateMut.fingerPrint` without `VIEW` or
    /// symmetry), whatever the scheme: one FP64 stream over the values of `vars`,
    /// given in the IR's variable order and taken in TLC's. Without a TLC order
    /// ([`StateFingerprinter::new`]) the values are taken as given.
    ///
    /// # Errors
    /// TLC's fingerprinting errors; [`StoreError::VarCount`] when `vars` does not
    /// have one value per variable of the order.
    pub fn tlc_fingerprint(&self, vars: &[Value], ctx: Ctx) -> Result<u64, StoreError> {
        let Some(order) = &self.tlc_order else {
            return Ok(self.values.fingerprint_state(vars, ctx)?);
        };
        let count = StoreError::VarCount {
            expected: order.len(),
            found: vars.len(),
        };
        if vars.len() != order.len() {
            return Err(count);
        }
        let mut fp = self.values.fp64().empty();
        for &i in order {
            let v = vars.get(i).ok_or_else(|| count.clone())?;
            fp = self.values.extend(fp, v, ctx)?;
        }
        Ok(fp)
    }
}

/// The key of variable position `i`: `(i + 1)` times the 64-bit golden ratio,
/// so every position has a distinct, dense key.
const fn position_key(i: u64) -> u64 {
    0x9E37_79B9_7F4A_7C15_u64.wrapping_mul(i.wrapping_add(1))
}

/// The chain's starting value (the first 64 bits of π's fraction).
const CHAIN_SEED: u64 = 0x243F_6A88_85A3_08D3;

/// `SplitMix64`'s output function (Steele, Lea and Flood, 2014; the constants of
/// David Stafford's "Mix13"), as in Sebastiano Vigna's public-domain
/// `splitmix64.c`: a bijection of `u64` with full avalanche.
///
/// ```
/// use dive_store::state::mix64;
/// assert_eq!(mix64(0), 0);
/// assert_eq!(mix64(1), 0x5692_161D_100B_05E5);
/// // `splitmix64.c`'s first output from seed 0 (state `0x9E3779B97F4A7C15`).
/// assert_eq!(mix64(0x9E37_79B9_7F4A_7C15), 0xE220_A839_7B1D_CDAF);
/// ```
#[must_use]
#[inline]
pub const fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The running state of [`combine`].
#[derive(Clone, Copy, Debug)]
struct Chain {
    h: u64,
    pos: u64,
}

impl Chain {
    const fn new() -> Self {
        Self {
            h: CHAIN_SEED,
            pos: 0,
        }
    }

    #[inline]
    fn push(&mut self, value_fp: u64) {
        let term = mix64(value_fp ^ position_key(self.pos));
        self.h = mix64(self.h ^ term);
        self.pos += 1;
    }

    const fn finish(self) -> u64 {
        self.h
    }
}

/// dive's combination of per-variable value fingerprints into a state
/// fingerprint (ADR 0015 §5), in variable order:
///
/// ```text
/// h₀ = SEED;  hᵢ₊₁ = mix64(hᵢ ⊕ mix64(fpᵢ ⊕ Kᵢ));  result hₙ
/// ```
///
/// where `Kᵢ` is a distinct key per position and [`mix64`] is `SplitMix64`'s
/// bijective output function. It is not affine over GF(2) (nor over the
/// integers modulo `2^64`), so equal changes in two variables do not cancel,
/// and it is a bijection in each `fpᵢ` when the others are fixed.
///
/// ```
/// use dive_store::state::combine;
/// // Position-keyed: swapping two variables' values changes the fingerprint.
/// assert_ne!(combine([1, 2]), combine([2, 1]));
/// assert_ne!(combine([7]), combine([7, 7]));
/// ```
#[must_use]
pub fn combine(value_fps: impl IntoIterator<Item = u64>) -> u64 {
    let mut chain = Chain::new();
    for fp in value_fps {
        chain.push(fp);
    }
    chain.finish()
}
