//! The global, append-only string table (`03-value-model.md` §2, §7).
//!
//! Ids are assigned on first encounter. The default profile orders strings by
//! their UTF-8 bytes ([`StringOrder::Text`]), so ids carry no order there.
//! Under `--compat=tlc` strings compare by intern token
//! ([`StringOrder::Token`]), which reproduces `UniqueString.compareTo`: the
//! id is the token, so the table must first be fed TLC's startup encounter
//! order with [`Strings::preload`] (`03-value-model.md` §7).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Arc, PoisonError, RwLock};

/// How strings compare (`02-semantic-contract.md` D1, D2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StringOrder {
    /// By UTF-8 bytes: the default profile's canonical order (D-010).
    #[default]
    Text,
    /// By intern token, in encounter order: TLC's order.
    Token,
}

/// The string table.
#[derive(Debug, Default)]
pub struct Strings {
    inner: RwLock<Inner>,
    order: StringOrder,
    sealed: AtomicBool,
}

#[derive(Debug, Default)]
struct Inner {
    ids: HashMap<Arc<str>, u32>,
    texts: Vec<Arc<str>>,
}

impl Strings {
    /// An empty table with the given order.
    #[must_use]
    pub fn new(order: StringOrder) -> Self {
        Self {
            inner: RwLock::default(),
            order,
            sealed: AtomicBool::new(false),
        }
    }

    /// The order strings compare in.
    #[must_use]
    pub fn order(&self) -> StringOrder {
        self.order
    }

    /// Interns `texts` in the given order, so that under
    /// [`StringOrder::Token`] they compare in that order. Call it before
    /// anything else is interned: TLC's tokens are assigned in its startup
    /// encounter order (configuration, built-ins, lexer tokens, semantic
    /// names), and the reference binder records that order.
    ///
    /// # Errors
    /// `texts` repeats a string, or the table already holds strings that are
    /// not a prefix of `texts`, so the tokens would not be TLC's; or more
    /// than `u32::MAX` strings.
    pub fn preload<'a>(
        &self,
        texts: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), crate::Error> {
        for (n, t) in texts.into_iter().enumerate() {
            if self.intern(t)? as usize != n {
                return Err(crate::Error::internal(
                    "string tokens preloaded out of order: the table must be a prefix of distinct texts",
                ));
            }
        }
        Ok(())
    }

    /// Refuses every later new string: under [`StringOrder::Token`] a string
    /// TLC first meets during the search gets a token that depends on what
    /// TLC evaluated before it, so after [`Strings::preload`] the run seals
    /// the table rather than guess the order.
    pub fn seal(&self) {
        self.sealed.store(true, AtomicOrdering::Release);
    }

    /// The id of `text`, assigning one if it is new.
    ///
    /// # Errors
    /// More than `u32::MAX` strings; a new string after [`Strings::seal`]
    /// (unsupported).
    pub fn intern(&self, text: &str) -> Result<u32, crate::Error> {
        if let Some(&id) = self
            .inner
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .ids
            .get(text)
        {
            return Ok(id);
        }
        let mut inner = self.inner.write().unwrap_or_else(PoisonError::into_inner);
        if let Some(&id) = inner.ids.get(text) {
            return Ok(id);
        }
        if self.sealed.load(AtomicOrdering::Acquire) {
            return Err(crate::Error::unsupported(format!(
                "the string or name {text:?}, which TLC first meets after start-up: its place in TLC's string order depends on evaluation order (--compat=tlc)"
            )));
        }
        let id = u32::try_from(inner.texts.len())
            .map_err(|_| crate::Error::unsupported("more than 2^32 distinct strings"))?;
        let text: Arc<str> = text.into();
        inner.texts.push(text.clone());
        inner.ids.insert(text, id);
        Ok(id)
    }

    /// The text of an id.
    #[must_use]
    pub fn text(&self, id: u32) -> Option<Arc<str>> {
        self.inner
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .texts
            .get(id as usize)
            .cloned()
    }

    /// The texts of the ids from `start` on, in id order, under one read
    /// lock: a worker's heap copies them once instead of locking the table
    /// on every comparison ([`crate::heap::Heap::cmp`]).
    #[must_use]
    pub fn texts_from(&self, start: usize) -> Vec<Arc<str>> {
        self.inner
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .texts
            .get(start..)
            .map(<[Arc<str>]>::to_vec)
            .unwrap_or_default()
    }

    /// Compares two ids in the table's order: by text, or by token.
    #[must_use]
    pub fn cmp(&self, a: u32, b: u32) -> std::cmp::Ordering {
        if a == b {
            return std::cmp::Ordering::Equal;
        }
        if self.order == StringOrder::Token {
            return a.cmp(&b);
        }
        let inner = self.inner.read().unwrap_or_else(PoisonError::into_inner);
        match (inner.texts.get(a as usize), inner.texts.get(b as usize)) {
            (Some(x), Some(y)) => x.cmp(y),
            _ => a.cmp(&b),
        }
    }

    /// The number of strings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .texts
            .len()
    }

    /// Whether the table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::*;

    #[test]
    fn a_sealed_table_refuses_new_strings_only() {
        let t = Strings::new(StringOrder::Token);
        assert!(t.preload(["zeta", "alpha"]).is_ok());
        t.seal();
        assert_eq!(t.intern("alpha").ok(), Some(1));
        assert!(t.intern("beta").is_err());
    }

    #[test]
    fn text_order_ignores_encounter_order() {
        let t = Strings::new(StringOrder::Text);
        let (b, a) = (t.intern("b").unwrap_or(0), t.intern("a").unwrap_or(0));
        assert_eq!(t.cmp(a, b), Ordering::Less);
    }

    #[test]
    fn token_order_follows_preloaded_encounter_order() {
        let t = Strings::new(StringOrder::Token);
        assert!(t.preload(["zeta", "alpha", "mid"]).is_ok());
        let ids: Vec<u32> = ["alpha", "zeta", "mid", "new"]
            .iter()
            .map(|s| t.intern(s).unwrap_or(u32::MAX))
            .collect();
        assert_eq!(t.cmp(ids[1], ids[0]), Ordering::Less, "zeta was first");
        assert_eq!(t.cmp(ids[0], ids[2]), Ordering::Less);
        assert_eq!(t.cmp(ids[2], ids[3]), Ordering::Less, "runtime names last");
        // Preloading again with the same prefix is fine; after other names
        // it is refused.
        assert!(t.preload(["zeta", "alpha"]).is_ok());
        let late = Strings::new(StringOrder::Token);
        let _ = late.intern("x");
        assert!(late.preload(["y"]).is_err());
    }
}
