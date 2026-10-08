//! Interned names: strings, model values and record field names.
//!
//! TLC keeps every name in one global table (`util.UniqueString`) and orders names by
//! their position in it (`UniqueString.compareTo` returns `this.tok - other.tok`).
//! dive mirrors that with an [`Interner`] that hands out [`Atom`]s numbered in
//! insertion order. Fed in SANY/TLC's encounter order, the numbering reproduces TLC's
//! order of strings, model values and record fields; see ADR 0011.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, PoisonError};

use crate::ctx::ValueOrder;

/// An interned name. Cloning is a reference-count increment.
///
/// Equality is by text, so atoms from different interners still compare correctly for
/// equality; ordering by token is only meaningful within one interner.
#[derive(Clone)]
pub struct Atom(Arc<AtomInner>);

struct AtomInner {
    text: Box<str>,
    tok: u32,
    hash: u64,
    utf16_len: u32,
    /// TLC's model-value type: the first UTF-16 unit when the name has more than two
    /// units and its second unit is `_` (`ModelValue` constructor). 0 means untyped.
    mv_type: u16,
}

impl Atom {
    fn new(text: &str, tok: u32) -> Self {
        let units: Vec<u16> = text.encode_utf16().collect();
        let mv_type = if units.len() > 2 && units[1] == u16::from(b'_') {
            units[0]
        } else {
            0
        };
        Self(Arc::new(AtomInner {
            text: text.into(),
            tok,
            hash: text_hash(text),
            utf16_len: u32::try_from(units.len()).unwrap_or(u32::MAX),
            mv_type,
        }))
    }

    /// The name's text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0.text
    }

    /// The intern token: the position at which the name was first interned.
    #[must_use]
    pub fn tok(&self) -> u32 {
        self.0.tok
    }

    /// The length in UTF-16 code units (Java's `String.length()`).
    #[must_use]
    pub fn utf16_len(&self) -> u32 {
        self.0.utf16_len
    }

    /// TLC's model-value type character, as a UTF-16 unit, if the name is typed
    /// (`x_name`: more than two units, the second being `_`).
    #[must_use]
    pub fn mv_type(&self) -> Option<u16> {
        (self.0.mv_type != 0).then_some(self.0.mv_type)
    }

    /// A precomputed, deterministic 64-bit hash of the text.
    #[must_use]
    pub fn text_hash(&self) -> u64 {
        self.0.hash
    }

    /// Orders two atoms under the given string order.
    #[must_use]
    pub fn cmp_with(&self, other: &Self, order: ValueOrder) -> Ordering {
        if Arc::ptr_eq(&self.0, &other.0) {
            return Ordering::Equal;
        }
        match order {
            ValueOrder::Tlc => self
                .0
                .tok
                .cmp(&other.0.tok)
                .then_with(|| self.0.text.cmp(&other.0.text)),
            ValueOrder::Canonical => self.0.text.cmp(&other.0.text),
        }
    }
}

impl PartialEq for Atom {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
            || (self.0.hash == other.0.hash && self.0.text == other.0.text)
    }
}

impl Eq for Atom {}

impl Hash for Atom {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.hash);
    }
}

impl fmt::Debug for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}#{}", self.as_str(), self.tok())
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// FNV-1a over the bytes, then a 64-bit finalizer. Deterministic across runs.
pub(crate) fn text_hash(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    mix64(h)
}

/// The `splitmix64` finalizer.
pub(crate) const fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The table of names for one run. Thread-safe; share it with an [`Arc`].
///
/// Tokens are assigned in insertion order, starting at 0. To reproduce TLC's order of
/// strings, model values and record fields, intern names in the order SANY and TLC
/// first meet them (ADR 0011).
///
/// ```
/// use dive_value::Interner;
/// let names = Interner::new();
/// let b = names.intern("b");
/// let a = names.intern("a");
/// assert!(b.tok() < a.tok());
/// assert_eq!(names.intern("b"), b);
/// ```
#[derive(Default)]
pub struct Interner {
    table: Mutex<HashMap<Box<str>, Atom>>,
}

impl Interner {
    /// An empty table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the atom for `text`, interning it with the next token if it is new.
    pub fn intern(&self, text: &str) -> Atom {
        let mut table = self.table.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(atom) = table.get(text) {
            return atom.clone();
        }
        let tok = u32::try_from(table.len()).unwrap_or(u32::MAX);
        let atom = Atom::new(text, tok);
        table.insert(text.into(), atom.clone());
        atom
    }

    /// Returns the atom for `text` if it has been interned.
    #[must_use]
    pub fn get(&self, text: &str) -> Option<Atom> {
        let table = self.table.lock().unwrap_or_else(PoisonError::into_inner);
        table.get(text).cloned()
    }

    /// The number of interned names.
    #[must_use]
    pub fn len(&self) -> usize {
        self.table
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Whether no name has been interned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl fmt::Debug for Interner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Interner")
            .field("len", &self.len())
            .finish()
    }
}
