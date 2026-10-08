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

/// The state fingerprint (ADR 0015 §5), in one of two schemes.
///
/// - **dive's** ([`StateFingerprinter::new`], the default profile): FP64, under
///   the run's polynomial, over the FP64 fingerprints of the variables' values in
///   the IR's variable order. Value fingerprints are cached in shared compound
///   values ([`TlcFingerprint::fingerprint_cached`]), so a successor pays only
///   for the variables whose values it changed; combining costs eight table
///   steps per variable.
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
        let f = self.values.fp64();
        let mut fp = f.empty();
        for v in vars {
            let vf = self.values.fingerprint_cached(v, ctx)?;
            fp = f.extend_i64(fp, i64::from_le_bytes(vf.to_le_bytes()));
        }
        Ok(fp)
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
