//! The global, append-only string table (`03-value-model.md` §2, §7).
//!
//! Ids are assigned on first encounter. The default profile orders strings by
//! text, so ids carry no order. Compat intern-token order is M2 work.

use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

/// The string table.
#[derive(Debug, Default)]
pub struct Strings {
    inner: RwLock<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    ids: HashMap<Arc<str>, u32>,
    texts: Vec<Arc<str>>,
}

impl Strings {
    /// The id of `text`, assigning one if it is new.
    ///
    /// # Errors
    /// More than `u32::MAX` strings.
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

    /// Compares two ids by text (the default profile's order).
    #[must_use]
    pub fn cmp(&self, a: u32, b: u32) -> std::cmp::Ordering {
        if a == b {
            return std::cmp::Ordering::Equal;
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
