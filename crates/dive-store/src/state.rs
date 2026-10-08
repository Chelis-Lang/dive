//! States as bytes, and state fingerprints.
//!
//! A state is the tuple of its variables' values in the IR's variable order. Its
//! encodings are the concatenation of the variables' value encodings
//! ([`dive_value::codec`]):
//!
//! - [`encode_state`] is lossless: frontier blocks, disk spill and checkpoints use it,
//!   because TLC's behaviour depends on how values are represented (`1..3` prints as
//!   an interval, an unnormalized set keeps TLC's membership rules).
//! - [`state_key`] is the canonical exact-store key: within one run, two states have
//!   equal keys exactly when they are equal.
//!
//! [`StateFingerprinter`] computes the two-level FP64 fingerprint of ADR 0015 §5.

use dive_value::fp64::{Fingerprinter, TlcFingerprint};
use dive_value::{Atom, Ctx, Interner, Value, codec};

use crate::error::StoreError;

/// Appends the lossless encoding of a state (its variables' values in order).
///
/// # Errors
/// Only from enumerating an evaluator-driven lazy set held by a variable.
pub fn encode_state(vars: &[Value], ctx: Ctx, out: &mut Vec<u8>) -> Result<(), StoreError> {
    for v in vars {
        codec::encode(v, ctx, out)?;
    }
    Ok(())
}

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

/// Decodes states for one worker, resolving names through the run's [`Interner`]
/// with a private cache, so hot decoding never takes the interner's lock.
///
/// ```
/// use dive_store::state::{StateDecoder, encode_state};
/// use dive_value::{Ctx, Interner, Value};
///
/// let names = Interner::new();
/// let ctx = Ctx::tlc();
/// let state = vec![Value::Int(3), Value::string(&names, "idle")];
/// let mut bytes = Vec::new();
/// encode_state(&state, ctx, &mut bytes)?;
/// let mut decoder = StateDecoder::new(&names);
/// assert_eq!(decoder.decode(&bytes)?, state);
/// # Ok::<(), dive_store::StoreError>(())
/// ```
#[derive(Debug)]
pub struct StateDecoder<'a> {
    names: &'a Interner,
    atoms: Vec<Option<Atom>>,
}

impl<'a> StateDecoder<'a> {
    /// A decoder for states whose names live in `names`.
    #[must_use]
    pub fn new(names: &'a Interner) -> Self {
        Self {
            names,
            atoms: Vec::new(),
        }
    }

    /// Decodes a whole encoded state.
    ///
    /// # Errors
    /// [`StoreError::Value`] with [`dive_value::ErrorKind::Codec`] when `bytes` is not
    /// an encoded state of this run.
    pub fn decode(&mut self, bytes: &[u8]) -> Result<Vec<Value>, StoreError> {
        let mut out = Vec::new();
        self.decode_into(bytes, &mut out)?;
        Ok(out)
    }

    /// Decodes a whole encoded state into `out`, which is cleared first.
    ///
    /// # Errors
    /// As [`StateDecoder::decode`].
    pub fn decode_into(&mut self, bytes: &[u8], out: &mut Vec<Value>) -> Result<(), StoreError> {
        out.clear();
        let mut input = bytes;
        let names = self.names;
        let atoms = &mut self.atoms;
        let mut resolve = |tok: u32| -> Option<Atom> {
            let i = usize::try_from(tok).ok()?;
            if let Some(Some(a)) = atoms.get(i) {
                return Some(a.clone());
            }
            let a = names.resolve(tok)?;
            if atoms.len() <= i {
                atoms.resize(i + 1, None);
            }
            atoms[i] = Some(a.clone());
            Some(a)
        };
        while !input.is_empty() {
            out.push(codec::decode(&mut input, &mut resolve)?);
        }
        Ok(())
    }
}

/// dive's state fingerprint (ADR 0015 §5): FP64, under the run's polynomial, over the
/// FP64 fingerprints of the variables' values in variable order.
///
/// Value fingerprints are cached in shared compound values
/// ([`TlcFingerprint::fingerprint_cached`]), so a successor pays only for the
/// variables whose values it changed; combining costs eight table steps per
/// variable.
///
/// These are not TLC's state fingerprints, which hash every value's bytes in one
/// stream in TLC's variable order. Fingerprint values never appear in compared
/// results; [`StateFingerprinter::tlc_fingerprint`] computes TLC's own where TLC
/// semantics observe it.
#[derive(Clone, Debug)]
pub struct StateFingerprinter {
    values: TlcFingerprint,
    index: usize,
}

impl StateFingerprinter {
    /// Fingerprints with TLC's polynomial `index` (`-fp index`, `0..131`).
    #[must_use]
    pub fn new(index: usize) -> Option<Self> {
        TlcFingerprint::new(index).map(|values| Self { values, index })
    }

    /// The polynomial index, to report with every fingerprint-mode result.
    #[must_use]
    pub const fn poly_index(&self) -> usize {
        self.index
    }

    /// The two-level fingerprint of a state.
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
        let f = self.values.fp64();
        let mut fp = f.empty();
        for v in vars {
            let vf = self.values.fingerprint_cached(v, ctx)?;
            fp = f.extend_i64(fp, i64::from_le_bytes(vf.to_le_bytes()));
        }
        Ok(fp)
    }

    /// TLC's own state fingerprint (`TLCStateMut.fingerPrint` without `VIEW` or
    /// symmetry): one FP64 stream over the values. `vars` must be in **TLC's**
    /// variable order (WP-A3) for the result to equal TLC's.
    ///
    /// # Errors
    /// TLC's fingerprinting errors.
    pub fn tlc_fingerprint(&self, vars: &[Value], ctx: Ctx) -> Result<u64, StoreError> {
        Ok(self.values.fingerprint_state(vars, ctx)?)
    }
}
