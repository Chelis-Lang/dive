//! Same-process frontier rows tied to one retained, append-only [`Store`].
//!
//! This transports opaque handles, including original receiver/history views.
//! It is not a value encoding or checkpoint format. The enclosing frontier
//! supplies record lengths, order and checksums; this codec checks its private
//! lifetime token, row width, canonical identity and actual Store coordinates.
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//! use dive_core::{Handle, Store, frontier_codec::FrontierCodec};
//!
//! let codec = FrontierCodec::new(Arc::new(Store::new(true)), 1)?;
//! let bytes = codec.encode(&[Handle::TRUE], &[Handle::TRUE])?;
//! let row = codec.decode(&bytes)?;
//! assert_eq!(row.identity, [Handle::TRUE]);
//! # Ok::<(), dive_core::Error>(())
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::node::flags;
use crate::{Error, Handle, Store};

const MAGIC: &[u8; 8] = b"DIVEHR01";
const HEADER: usize = 24;
static NEXT: AtomicU64 = AtomicU64::new(1);

/// Two rows with different roles: mathematical identity and original evaluation.
#[derive(Debug)]
pub struct Rows {
    /// Canonical mathematical state values used for frontier identity and replay.
    pub identity: Vec<Handle>,
    /// Actual original values, retaining receiver class and construction history.
    pub original: Vec<Handle>,
}

/// A narrow frontier handle codec which keeps its live Store allocated.
///
/// Within a process, only the issuing instance decodes its unchanged rows.
/// Another codec over the same Store does not recover them. There is no file
/// import, restart or Store-compaction interface.
#[derive(Debug)]
pub struct FrontierCodec {
    store: Arc<Store>,
    token: u64,
    variables: usize,
}

impl FrontierCodec {
    /// Retains the Store and allocates a private, nonreused process lifetime token.
    ///
    /// # Errors
    /// The process exhausted its codec tokens or a row cannot fit address space.
    pub fn new(store: Arc<Store>, variables: usize) -> Result<Self, Error> {
        variables
            .checked_mul(16)
            .and_then(|n| n.checked_add(HEADER))
            .ok_or_else(|| invalid("row size overflow"))?;
        let token = NEXT
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| invalid("process lifetime tokens exhausted"))?;
        Ok(Self {
            store,
            token,
            variables,
        })
    }

    /// Encodes complete canonical and original rows without converting values.
    ///
    /// # Errors
    /// Wrong widths, transient/unknown/lazy handles, a noncanonical identity,
    /// poisoned Store, or allocation failure. No normalization is performed.
    pub fn encode(&self, identity: &[Handle], original: &[Handle]) -> Result<Vec<u8>, Error> {
        if self.store.poisoned() {
            return Err(invalid("owning Store is poisoned"));
        }
        if identity.len() != self.variables || original.len() != self.variables {
            return Err(invalid("row width differs from the owning plan"));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(HEADER + self.variables * 16)
            .map_err(|_| invalid("row allocation"))?;
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&self.token.to_le_bytes());
        bytes.extend_from_slice(&(self.variables as u64).to_le_bytes());
        for (canonical, row) in [(true, identity), (false, original)] {
            for &handle in row {
                self.checked(handle.bits(), canonical)?;
                bytes.extend_from_slice(&handle.bits().to_le_bytes());
            }
        }
        Ok(bytes)
    }

    /// Decodes a checked enclosing frontier record from this exact live owner.
    ///
    /// # Errors
    /// Invalid version/token/width/length, unknown coordinates, transient/lazy
    /// values, a noncanonical identity, poisoned Store or allocation failure.
    pub fn decode(&self, bytes: &[u8]) -> Result<Rows, Error> {
        if self.store.poisoned() {
            return Err(invalid("owning Store is poisoned"));
        }
        if bytes.len() != HEADER + self.variables * 16
            || bytes.get(..8) != Some(MAGIC.as_slice())
            || word(&bytes[8..16]) != self.token
            || word(&bytes[16..24]) != self.variables as u64
        {
            return Err(invalid("invalid format, owner, width or length"));
        }
        let mut rows = Rows {
            identity: Vec::new(),
            original: Vec::new(),
        };
        for (canonical, out, offset) in [
            (true, &mut rows.identity, HEADER),
            (false, &mut rows.original, HEADER + self.variables * 8),
        ] {
            out.try_reserve_exact(self.variables)
                .map_err(|_| invalid("decoded row allocation"))?;
            for chunk in bytes[offset..offset + self.variables * 8]
                .as_chunks::<8>()
                .0
            {
                out.push(self.checked(word(chunk), canonical)?);
            }
        }
        Ok(rows)
    }

    fn checked(&self, bits: u64, canonical: bool) -> Result<Handle, Error> {
        if self.store.poisoned() {
            return Err(invalid("owning Store is poisoned"));
        }
        let handle = Handle(bits);
        match bits & 7 {
            0 => {}
            1 if handle == Handle::FALSE || handle == Handle::TRUE => {}
            2 | 3 => {
                let id = u32::try_from(bits >> 3).map_err(|_| invalid("name id"))?;
                if self.store.strings().text(id).is_none() {
                    return Err(invalid("unknown name"));
                }
            }
            4 => {
                let view = self
                    .store
                    .retained_view(handle)
                    .ok_or_else(|| invalid("unknown persistent coordinate"))?;
                if view.is_lazy() || (canonical && view.flags & flags::VIEW != 0) {
                    return Err(invalid("lazy value or noncanonical identity"));
                }
            }
            _ => return Err(invalid("invalid, empty or transient handle")),
        }
        Ok(handle)
    }
}

fn word(bytes: &[u8]) -> u64 {
    let mut array = [0; 8];
    array.copy_from_slice(bytes);
    u64::from_le_bytes(array)
}

fn invalid(message: &str) -> Error {
    Error::internal(format!("native frontier handle row: {message}"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "live-Store codec controls")]
mod tests {
    use crate::Heap;

    use super::*;

    #[test]
    fn rows_keep_genuine_tuple_views_across_arena_reset() {
        let store = Arc::new(Store::new(true));
        let mut heap = Heap::new(Arc::clone(&store), true, false);
        let one = Handle::int(1).unwrap();
        let two = Handle::int(2).unwrap();
        let tuple = heap.tuple_literal(&[one, two]);
        let raw = heap.tuple_literal(&[tuple]);
        let identity = heap.intern(raw).unwrap();
        let original = heap.intern_exact(raw).unwrap();
        let codec = FrontierCodec::new(Arc::clone(&store), 1).unwrap();
        let bytes = codec.encode(&[identity], &[original]).unwrap();
        heap.reset();
        let rows = codec.decode(&bytes).unwrap();
        assert_eq!(rows.identity, [identity]);
        assert_eq!(rows.original, [original]);
        assert!(heap.view(rows.original[0]).is_tuple_literal());
        assert!(!heap.view(rows.identity[0]).is_tuple_literal());
        let weak = Arc::downgrade(&store);
        drop(heap);
        drop(store);
        assert!(weak.upgrade().is_some(), "codec retains the actual Store");
        assert!(codec.decode(&bytes).is_ok());
    }

    #[test]
    fn wrong_owner_width_transients_and_corrupt_coordinates_refuse() {
        let store = Arc::new(Store::new(true));
        let codec = FrontierCodec::new(Arc::clone(&store), 1).unwrap();
        let other = FrontierCodec::new(Arc::clone(&store), 1).unwrap();
        let one = Handle::int(1).unwrap();
        let bytes = codec.encode(&[one], &[one]).unwrap();
        assert!(other.decode(&bytes).is_err());
        for length in [0, 7, 8, 23, 24, bytes.len() - 1] {
            assert!(codec.decode(&bytes[..length]).is_err());
        }
        for bits in [
            Handle::EMPTY.bits(),
            6,
            17,
            Handle::transient(0, 0).bits(),
            Handle::persistent(0, 2, 1234).bits(),
            Handle::string(44).bits(),
        ] {
            let mut bad = bytes.clone();
            bad[HEADER..HEADER + 8].copy_from_slice(&bits.to_le_bytes());
            assert!(codec.decode(&bad).is_err(), "{bits}");
        }
        let mut heap = Heap::new(store, true, false);
        let tuple = heap.tuple_literal(&[one]);
        assert!(codec.encode(&[tuple], &[tuple]).is_err());
        let original = heap.intern_exact(tuple).unwrap();
        assert!(codec.encode(&[original], &[original]).is_err());
        assert!(codec.encode(&[], &[one]).is_err());
        assert!(
            !heap.store().poisoned(),
            "bad file coordinates do not read a poison view"
        );
        let empty = FrontierCodec::new(Arc::clone(heap.store()), 0).unwrap();
        let empty_bytes = empty.encode(&[], &[]).unwrap();
        let _ = heap.view(Handle::persistent(0, 2, crate::handle::MAX_CLASS_INDEX));
        assert!(empty.decode(&empty_bytes).is_err());
        assert!(empty.encode(&[], &[]).is_err());
    }
}
