//! Bounded quiescent Store images for completed-level native recovery.
//!
//! These trusted local images preserve issued coordinates, names and explicit
//! construction authority. They are not value interchange or reachability proofs.
//! A fresh startup must match before a restored owner can be constructed.
//!
//! ```
//! use std::sync::Arc;
//! use dive_core::{Store, store::snapshot};
//! let startup = Store::new(true);
//! let prefix = snapshot::capture(&startup, 1 << 20, &|| true)?;
//! let fresh = Store::new(true);
//! let restored: Arc<Store> = snapshot::restore(&fresh, &prefix, &prefix, 1 << 20, &|| true)?;
//! assert!(snapshot::row(&restored, &[], true, &|| true)?.is_empty());
//! # Ok::<(), dive_core::Error>(())
//! ```

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, PoisonError};

use super::{
    CLASSES, Head, Index, LOCAL_CLASS_SHIFT, MAX_CLASS_INDEX, SHARDS, Store, class_of, empty_table,
    interning_hash,
};
use crate::Error;
use crate::handle::{Handle, Tag};
use crate::node::{Kind, flags};

const MAGIC: &[u8; 8] = b"DIVEST01";
const RECORD: usize = 42;

fn invalid(message: impl Into<String>) -> Error {
    Error::unsupported(format!("native Store snapshot: {}", message.into()))
}

struct Write {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write {
    fn put(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let end = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|&n| n <= self.limit)
            .ok_or_else(|| invalid("byte admission bound"))?;
        if self.bytes.capacity() < end {
            let target = end
                .max(self.bytes.capacity().saturating_mul(2))
                .min(self.limit);
            self.bytes
                .try_reserve_exact(target - self.bytes.len())
                .map_err(|_| invalid("image allocation"))?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn word(&mut self, n: u64) -> Result<(), Error> {
        self.put(&n.to_le_bytes())
    }
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.word(bytes.len() as u64)?;
        self.put(bytes)
    }
}

struct Read<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Read<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&end| end <= self.bytes.len())
            .ok_or_else(|| invalid("truncated image"))?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }
    fn word(&mut self) -> Result<u64, Error> {
        let mut a = [0; 8];
        a.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(a))
    }
    fn count(&mut self, unit: usize) -> Result<usize, Error> {
        let n = usize::try_from(self.word()?).map_err(|_| invalid("count address space"))?;
        if n.checked_mul(unit)
            .is_none_or(|n| n > self.bytes.len().saturating_sub(self.at))
        {
            return Err(invalid("declared count exceeds image"));
        }
        Ok(n)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Node {
    pub(super) handle: u64,
    pub(super) kind: Kind,
    pub(super) flags: u8,
    pub(super) lo: i64,
    pub(super) hash: u64,
    pub(super) sum: u64,
    pub(super) data: Vec<Handle>,
}
pub(super) struct Image {
    pub(super) semantics: [u8; 6],
    pub(super) sealed: bool,
    pub(super) names: Vec<String>,
    pub(super) lens: Vec<[u64; CLASSES]>,
    pub(super) nodes: Vec<Node>,
    pub(super) history: Vec<[Handle; 2]>,
}

pub(super) fn semantics(store: &Store) -> [u8; 6] {
    [
        u8::from(store.hashcons),
        u8::from(store.semantics.string_order == crate::strings::StringOrder::Token),
        u8::from(store.semantics.int_mode == crate::int::IntMode::Tlc),
        u8::from(store.semantics.tlc_comparisons),
        u8::from(store.semantics.tlc_exploration),
        u8::from(store.lazy_sets()),
    ]
}

fn lengths(store: &Store) -> Vec<[u64; CLASSES]> {
    store
        .shards
        .iter()
        .map(|shard| {
            shard
                .index
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .lens
        })
        .collect()
}

/// Captures an owned Store at a quiescent boundary without normalizing values.
///
/// The caller must have joined all evaluating workers. Issued coordinates and
/// exact raw/normalized origin registrations are retained; runtime hosts are not.
///
/// # Errors
/// Cancellation, a byte bound, foreign hosts, invalid ownership or concurrent growth.
#[allow(
    clippy::too_many_lines,
    reason = "one quiescent topological image transaction"
)]
pub fn capture(
    store: &Store,
    max_bytes: usize,
    proceed: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<u8>, Error> {
    if !proceed() {
        return Err(Error::stop());
    }
    if store.poisoned() || store.draws().is_some() || store.registers().is_some() {
        return Err(invalid("poisoned or effectful owner"));
    }
    let lens = lengths(store);
    let count = lens
        .iter()
        .flatten()
        .try_fold(0usize, |n, &v| {
            usize::try_from(v).ok().and_then(|v| n.checked_add(v))
        })
        .ok_or_else(|| invalid("node count overflow"))?;
    if count > max_bytes / RECORD {
        return Err(invalid("node admission bound"));
    }
    if store.strings.memory_usage().strings > 65_536 {
        return Err(invalid("name admission bound"));
    }
    let names = store.strings.texts_from(0);
    let authority = store
        .history_views
        .read()
        .unwrap_or_else(PoisonError::into_inner);
    if authority.len() > max_bytes / 16 {
        return Err(invalid("origin admission bound"));
    }
    let history = authority.clone();
    drop(authority);
    let mut out = Write {
        bytes: Vec::new(),
        limit: max_bytes,
    };
    out.put(MAGIC)?;
    out.put(&semantics(store))?;
    out.put(&[u8::from(store.strings.is_sealed())])?;
    out.word(names.len() as u64)?;
    for name in &names {
        out.bytes(name.as_bytes())?;
    }
    for col in &lens {
        for &n in col {
            out.word(n)?;
        }
    }
    out.word(count as u64)?;
    let mut done = HashSet::new();
    done.try_reserve(count)
        .map_err(|_| invalid("node index allocation"))?;
    let mut active = HashSet::new();
    let mut stack = Vec::new();
    for (shard, cols) in lens.iter().enumerate() {
        for (class, &len) in cols.iter().enumerate() {
            for index in 0..len {
                let root = Handle::persistent(
                    u8::try_from(shard).map_err(|_| invalid("shard"))?,
                    u8::try_from(class).map_err(|_| invalid("class"))?,
                    index,
                );
                stack.push((root, false));
                while let Some((h, leaving)) = stack.pop() {
                    if done.len().is_multiple_of(128) && !proceed() {
                        return Err(Error::stop());
                    }
                    if done.contains(&h.bits()) {
                        continue;
                    }
                    let v = store
                        .retained_view(h)
                        .ok_or_else(|| invalid("unissued node"))?;
                    if leaving {
                        active.remove(&h.bits());
                        done.insert(h.bits());
                        out.word(h.bits())?;
                        out.put(&[v.kind as u8, v.flags])?;
                        out.word(v.lo.cast_unsigned())?;
                        out.word(v.hash)?;
                        out.word(v.sum)?;
                        out.word(v.data.len() as u64)?;
                        for child in v.data {
                            out.word(child.bits())?;
                        }
                    } else {
                        if !active.insert(h.bits()) {
                            return Err(invalid("cyclic value nodes"));
                        }
                        stack
                            .try_reserve(v.data.len() + 1)
                            .map_err(|_| invalid("traversal allocation"))?;
                        stack.push((h, true));
                        for &child in v.data.iter().rev() {
                            if child.is_persistent() && !done.contains(&child.bits()) {
                                stack.push((child, false));
                            }
                        }
                    }
                }
            }
        }
    }
    out.word(history.len() as u64)?;
    for pair in &history {
        out.word(pair[0].bits())?;
        out.word(pair[1].bits())?;
    }
    if lengths(store) != lens
        || store.strings.texts_from(0) != names
        || *store
            .history_views
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            != history
    {
        return Err(invalid("owner changed during snapshot"));
    }
    Ok(out.bytes)
}

fn kind(byte: u8) -> Result<Kind, Error> {
    Ok(match byte {
        0 => Kind::Set,
        1 => Kind::IntFcn,
        2 => Kind::Fcn,
        3 => Kind::Interval,
        4 => Kind::Subset,
        5 => Kind::FcnSet,
        6 => Kind::RecSet,
        7 => Kind::SeqSet,
        8 => Kind::Nat,
        9 => Kind::Int,
        10 => Kind::Product,
        11 => Kind::Union,
        12 => Kind::Lambda,
        13 => Kind::SetPred,
        14 => Kind::BigInt,
        15 => Kind::SetEnum,
        _ => return Err(invalid("unknown node kind")),
    })
}

fn scalar(h: Handle, names: usize, seen: &HashSet<u64>) -> bool {
    match h.tag() {
        Tag::Int => true,
        Tag::Bool => h == Handle::TRUE || h == Handle::FALSE,
        Tag::Str | Tag::Model => {
            usize::try_from(h.bits() >> 3).is_ok_and(|id| id < names && u32::try_from(id).is_ok())
        }
        Tag::Persistent => seen.contains(&h.bits()),
        _ => false,
    }
}

#[allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "one bounded binary parser; short names denote checked node coordinates"
)]
pub(super) fn parse(bytes: &[u8], proceed: &(dyn Fn() -> bool + Sync)) -> Result<Image, Error> {
    let mut input = Read { bytes, at: 0 };
    if input.take(8)? != MAGIC {
        return Err(invalid("format/version"));
    }
    let mut sem = [0; 6];
    sem.copy_from_slice(input.take(6)?);
    let sealed = input.take(1)?[0];
    if sem.iter().any(|&v| v > 1) || sealed > 1 {
        return Err(invalid("semantics flags"));
    }
    let count = input.count(8)?;
    if count > 65_536 {
        return Err(invalid("name admission bound"));
    }
    let mut names = Vec::new();
    names
        .try_reserve_exact(count)
        .map_err(|_| invalid("name table allocation"))?;
    let mut unique = HashSet::new();
    for _ in 0..count {
        let len = input.count(1)?;
        let name = std::str::from_utf8(input.take(len)?).map_err(|_| invalid("name UTF-8"))?;
        if !unique.insert(name.to_owned()) {
            return Err(invalid("duplicate name token"));
        }
        names.push(name.to_owned());
    }
    let mut lens = Vec::new();
    for _ in 0..SHARDS {
        let mut cols = [0; CLASSES];
        for n in &mut cols {
            *n = input.word()?;
            if *n > MAX_CLASS_INDEX {
                return Err(invalid("node coordinate bound"));
            }
        }
        lens.push(cols);
    }
    let count = input.count(RECORD)?;
    let declared = lens
        .iter()
        .flatten()
        .try_fold(0u64, |n, &v| n.checked_add(v))
        .ok_or_else(|| invalid("node count overflow"))?;
    if declared != count as u64 {
        return Err(invalid("column/node counts differ"));
    }
    let mut nodes = Vec::new();
    nodes
        .try_reserve_exact(count)
        .map_err(|_| invalid("node allocation"))?;
    let mut seen = HashSet::new();
    seen.try_reserve(count)
        .map_err(|_| invalid("node index allocation"))?;
    for ordinal in 0..count {
        if ordinal.is_multiple_of(128) && !proceed() {
            return Err(Error::stop());
        }
        let handle = input.word()?;
        let k = kind(input.take(1)?[0])?;
        let fl = input.take(1)?[0];
        let lo = input.word()?.cast_signed();
        let hash = input.word()?;
        let sum = input.word()?;
        let len = input.count(8)?;
        let mut data = Vec::new();
        data.try_reserve_exact(len)
            .map_err(|_| invalid("child allocation"))?;
        for _ in 0..len {
            let h = Handle(input.word()?);
            if !scalar(h, names.len(), &seen) {
                return Err(invalid("foreign, forward or transient child"));
            }
            data.push(h);
        }
        let h = Handle(handle);
        if !h.is_persistent() || !seen.insert(handle) {
            return Err(invalid("duplicate/nonpersistent coordinate"));
        }
        let (s, c, i) = h.persistent_parts();
        let index_hash = interning_hash(k, fl, lo, hash, &data);
        if fl & !(flags::VIEW | flags::LAZY) != 0
            || c != class_of(data.len(), fl)
            || i >= lens[usize::from(s)][usize::from(c)]
            || index_hash >> 56 != u64::from(s)
        {
            return Err(invalid("node coordinate/header mismatch"));
        }
        if matches!(k, Kind::Fcn | Kind::RecSet) && !len.is_multiple_of(2)
            || matches!(k, Kind::Interval | Kind::Subset | Kind::SeqSet) && len != 1
            || matches!(k, Kind::FcnSet | Kind::Union) && len != 2
            || matches!(k, Kind::Nat | Kind::Int) && len != 0
            || k.is_lazy() && len == 0
        {
            return Err(invalid("node shape"));
        }
        if k == Kind::SetEnum && fl & flags::UNNORM == 0 {
            return Err(invalid("raw set phase"));
        }
        if k == Kind::BigInt
            && (len == 0
                || !matches!(lo, -1 | 1)
                || data
                    .iter()
                    .any(|h| h.as_int().is_none_or(|n| u32::try_from(n).is_err()))
                || data.last().and_then(|h| h.as_int()) == Some(0))
        {
            return Err(invalid("integer magnitude"));
        }
        if fl & flags::TUPLE != 0 && (k != Kind::IntFcn || lo != 1)
            || fl & flags::RECORD != 0 && k != Kind::Fcn
            || fl & flags::UNNORM != 0 && k != Kind::SetEnum
            || fl & flags::HISTORY != 0 && !matches!(k, Kind::Set | Kind::SetEnum)
        {
            return Err(invalid("receiver/history flags"));
        }
        nodes.push(Node {
            handle,
            kind: k,
            flags: fl,
            lo,
            hash,
            sum,
            data,
        });
    }
    let count = input.count(16)?;
    let mut history = Vec::new();
    history
        .try_reserve_exact(count)
        .map_err(|_| invalid("origin allocation"))?;
    let by_handle: HashMap<_, _> = nodes.iter().map(|n| (n.handle, n)).collect();
    for origin in 0..count {
        let mut pair = [Handle::EMPTY; 2];
        for (phase, h) in pair.iter_mut().enumerate() {
            *h = Handle(input.word()?);
            if !h.is_empty() {
                let node = by_handle
                    .get(&h.bits())
                    .ok_or_else(|| invalid("unknown owned-history node"))?;
                if node.flags & flags::HISTORY == 0
                    || node.lo != i64::try_from(origin + 1).map_err(|_| invalid("origin bound"))?
                    || (node.kind == Kind::SetEnum) != (phase == 0)
                {
                    return Err(invalid("origin/phase authority mismatch"));
                }
            }
        }
        history.push(pair);
    }
    for node in &nodes {
        if node.flags & flags::HISTORY != 0 {
            let origin = usize::try_from(node.lo)
                .ok()
                .and_then(|n| n.checked_sub(1))
                .and_then(|i| history.get(i));
            if !origin.is_some_and(|pair| pair.iter().any(|h| h.bits() == node.handle)) {
                return Err(invalid("unregistered history authority"));
            }
        }
    }
    if input.at != bytes.len() {
        return Err(invalid("trailing image bytes"));
    }
    Ok(Image {
        semantics: sem,
        sealed: sealed != 0,
        names,
        lens,
        nodes,
        history,
    })
}

/// Validates that a completed image extends its recorded startup without opaque new providers.
/// Call before committing an archive, as well as before constructing a restored owner.
///
/// # Errors
/// Invalid framing, prefix, ownership, unknown provider, bounds or cancellation.
pub fn validate_extension(
    startup: &[u8],
    image: &[u8],
    proceed: &(dyn Fn() -> bool + Sync),
) -> Result<(), Error> {
    extension(startup, image, proceed).map(|_| ())
}

fn extension(
    expected_startup: &[u8],
    image: &[u8],
    proceed: &(dyn Fn() -> bool + Sync),
) -> Result<Image, Error> {
    let before = parse(expected_startup, proceed)?;
    let after = parse(image, proceed)?;
    if before.semantics != after.semantics
        || before.sealed != after.sealed
        || after.names.get(..before.names.len()) != Some(before.names.as_slice())
    {
        return Err(invalid("startup semantics/name prefix mismatch"));
    }
    let old: HashMap<_, _> = before.nodes.iter().map(|n| (n.handle, n)).collect();
    for node in &after.nodes {
        if old.get(&node.handle).is_some_and(|old| *old != node) {
            return Err(invalid("startup value prefix changed"));
        }
    }
    if before
        .lens
        .iter()
        .zip(&after.lens)
        .any(|(a, b)| a.iter().zip(b).any(|(a, b)| a > b))
        || before.history.len() > after.history.len()
    {
        return Err(invalid("startup prefix shrank"));
    }
    if before
        .history
        .iter()
        .zip(&after.history)
        .any(|(a, b)| a.iter().zip(b).any(|(a, b)| !a.is_empty() && a != b))
    {
        return Err(invalid("startup history authority changed"));
    }
    for node in &after.nodes {
        if !old.contains_key(&node.handle) && (node.kind.is_lazy() || node.flags & flags::LAZY != 0)
        {
            return Err(invalid("deferred provider added after startup"));
        }
    }
    Ok(after)
}

/// Reconstructs a fresh owner only after its startup and image validate.
///
/// The compiled startup's issued handles remain valid because their complete
/// prefix is checked byte-for-byte and all stored coordinates are preserved.
/// Raw/normalized ownership registrations are restored explicitly. Providers
/// added after startup refuse rather than being reconstructed from flags.
///
/// # Errors
/// Invalid structure/identity, bounds, cancellation, or unrepresented providers.
#[allow(
    clippy::too_many_lines,
    reason = "validate one startup/image pair before installing any restored owner"
)]
pub fn restore(
    startup: &Store,
    expected_startup: &[u8],
    image: &[u8],
    max_bytes: usize,
    proceed: &(dyn Fn() -> bool + Sync),
) -> Result<Arc<Store>, Error> {
    if image.len() > max_bytes || expected_startup.len() > max_bytes {
        return Err(invalid("image byte bound"));
    }
    if capture(startup, max_bytes, proceed)? != expected_startup {
        return Err(invalid("compiled startup prefix mismatch"));
    }
    let after = extension(expected_startup, image, proceed)?;
    install(startup, after, proceed)
}

pub(super) fn install(
    startup: &Store,
    after: Image,
    proceed: &(dyn Fn() -> bool + Sync),
) -> Result<Arc<Store>, Error> {
    let store = Arc::new(Store::with_semantics(startup.hashcons, startup.semantics));
    store
        .strings
        .preload(after.names.iter().map(String::as_str))?;
    if after.sealed {
        store.strings.seal();
    }
    if after.semantics[5] != 0 {
        store.note_lazy_sets();
    }
    if let Some(printer) = startup.printer.get() {
        store.set_printer(printer.clone());
    }
    for (ordinal, node) in after.nodes.iter().enumerate() {
        if ordinal.is_multiple_of(128) && !proceed() {
            return Err(Error::stop());
        }
        let (s, c, i) = Handle(node.handle).persistent_parts();
        if !store.cols[usize::from(s)].put(
            c,
            i,
            Head {
                kind: node.kind,
                flags: node.flags,
                lo: node.lo,
                hash: node.hash,
                sum: node.sum,
            },
            &node.data,
        ) {
            return Err(invalid("duplicate restored slot"));
        }
    }
    let mut buckets: [Vec<&Node>; SHARDS] = std::array::from_fn(|_| Vec::new());
    for node in &after.nodes {
        let s = usize::from(Handle(node.handle).persistent_parts().0);
        buckets[s]
            .try_reserve(1)
            .map_err(|_| invalid("index bucket allocation"))?;
        buckets[s].push(node);
    }
    for (s, cols) in after.lens.iter().enumerate() {
        let mut index = Index {
            lens: *cols,
            ..Index::default()
        };
        let entries = &buckets[s];
        let count = cols.iter().sum::<u64>();
        index.misses = count;
        if startup.hashcons && count > 0 {
            let count = usize::try_from(count).map_err(|_| invalid("index address space"))?;
            let cap = count
                .checked_mul(2)
                .and_then(usize::checked_next_power_of_two)
                .ok_or_else(|| invalid("index capacity"))?
                .max(64);
            index.table = empty_table(cap);
        }
        for (ordinal, node) in entries.iter().enumerate() {
            if ordinal.is_multiple_of(128) && !proceed() {
                return Err(Error::stop());
            }
            index.handles += node.data.len() as u64;
            let (_, c, i) = Handle(node.handle).persistent_parts();
            index.boxed_payload_bytes +=
                usize::from(c >= 2) * std::mem::size_of_val(node.data.as_slice());
            if startup.hashcons {
                let hash = interning_hash(node.kind, node.flags, node.lo, node.hash, &node.data);
                let h32 = u32::try_from(hash & u64::from(u32::MAX))
                    .map_err(|_| invalid("intern hash low word"))?;
                let mut pos = h32 as usize & (index.table.len() - 1);
                while index.table[pos].load(std::sync::atomic::Ordering::Relaxed) != 0 {
                    pos = (pos + 1) & (index.table.len() - 1);
                }
                let local = (u32::from(c) << LOCAL_CLASS_SHIFT)
                    | u32::try_from(i).map_err(|_| invalid("local index"))?;
                index.table[pos].store(
                    (u64::from(h32) << 32) | u64::from(local + 1),
                    std::sync::atomic::Ordering::Relaxed,
                );
                index.used += 1;
            }
        }
        *store.shards[s]
            .index
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = index;
    }
    *store
        .history_views
        .write()
        .unwrap_or_else(PoisonError::into_inner) = after.history;
    Ok(store)
}

/// Resolves a saved row through the newly validated owner without forging handles.
///
/// # Errors
/// Unknown/scalar-invalid coordinates, deferred values or unowned history.
pub fn row(
    store: &Store,
    bits: &[u64],
    canonical: bool,
    proceed: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<Handle>, Error> {
    let mut out = Vec::new();
    out.try_reserve_exact(bits.len())
        .map_err(|_| invalid("row allocation"))?;
    let names = store.strings.texts_from(0).len();
    let mut seen = HashSet::new();
    let mut steps = 0usize;
    for &bits in bits {
        let h = Handle(bits);
        let mut pending = vec![h];
        while let Some(value) = pending.pop() {
            if steps.is_multiple_of(128) && !proceed() {
                return Err(Error::stop());
            }
            steps = steps.saturating_add(1);
            if value.is_persistent() {
                if !seen.insert(value.bits()) {
                    continue;
                }
                let v = store
                    .retained_view(value)
                    .ok_or_else(|| invalid("unknown row coordinate"))?;
                if v.is_lazy()
                    || v.kind.is_lazy()
                    || canonical && v.flags & flags::VIEW != 0
                    || v.flags & flags::HISTORY != 0 && !store.owns_history(value)
                {
                    return Err(invalid("row provider/canonical/ownership mismatch"));
                }
                pending.extend_from_slice(v.data);
            } else if !scalar(value, names, &HashSet::new()) {
                return Err(invalid("invalid scalar row value"));
            }
        }
        out.push(h);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "owned snapshot corruption and authority controls"
    )]
    use super::*;
    use crate::Heap;
    fn owner() -> Arc<Store> {
        Arc::new(Store::with_semantics(true, crate::Semantics::tlc()))
    }
    #[test]
    fn restores_coordinates_aliases_raw_normalized_authority_and_fresh_frontier_owner() {
        let startup = owner();
        let prefix = capture(&startup, 1 << 20, &|| true).expect("prefix");
        let mut heap = Heap::new(Arc::clone(&startup), true, false);
        let one = Handle::int(1).expect("int");
        let two = Handle::int(2).expect("int");
        let local = heap.local_literal(&[two, one, two]).expect("literal");
        let raw = heap.intern_exact(local).expect("owned raw");
        let alias = heap.tuple_literal(&[raw, raw]);
        let alias = heap.intern_exact(alias).expect("nested aliases");
        let normalized = heap.normalize_construction(local).expect("normalization");
        let normalized = heap.intern_exact(normalized).expect("owned normalized");
        assert_ne!(raw, normalized);
        assert!(startup.owns_history(raw));
        assert!(startup.owns_history(normalized));
        let identity = heap.intern(normalized).expect("canonical");
        let image = capture(&startup, 1 << 20, &|| true).expect("image");
        let fresh = owner();
        let mut corrupt_phase = image.clone();
        let at = corrupt_phase.len() - 8;
        corrupt_phase[at..].copy_from_slice(&raw.bits().to_le_bytes());
        assert!(
            restore(&fresh, &prefix, &corrupt_phase, 1 << 20, &|| true).is_err(),
            "origin phase cannot be fabricated from a HISTORY flag"
        );
        let restored = restore(&fresh, &prefix, &image, 1 << 20, &|| true).expect("restore");
        assert!(!Arc::ptr_eq(&restored, &startup));
        assert!(restored.owns_history(raw));
        assert!(restored.owns_history(normalized));
        assert_eq!(
            restored.history_views.read().expect("authority").as_slice(),
            startup.history_views.read().expect("authority").as_slice()
        );
        assert_eq!(
            row(
                &restored,
                &[alias.bits(), raw.bits(), normalized.bits()],
                false,
                &|| true
            )
            .expect("row"),
            vec![alias, raw, normalized]
        );
        assert!(
            row(&restored, &[raw.bits()], true, &|| true).is_err(),
            "raw authority must not masquerade as canonical identity"
        );
        let mut new_heap = Heap::new(Arc::clone(&restored), true, false);
        assert!(
            new_heap.normalize_construction(raw).is_err(),
            "retained raw ownership must not fabricate a normalization provider"
        );
        assert_eq!(
            new_heap
                .normalize_construction(normalized)
                .expect("normalized authority"),
            normalized
        );
        let old_codec =
            crate::frontier_codec::FrontierCodec::new(Arc::clone(&startup), 1).expect("old codec");
        let new_codec = crate::frontier_codec::FrontierCodec::new(Arc::clone(&restored), 1)
            .expect("fresh codec");
        let old_bytes = old_codec.encode(&[identity], &[raw]).expect("old frontier");
        assert!(
            new_codec.decode(&old_bytes).is_err(),
            "persisted Store coordinates confer no old process-local codec authority"
        );
        let new_bytes = new_codec.encode(&[identity], &[raw]).expect("new frontier");
        assert!(new_codec.decode(&new_bytes).is_ok());
    }
    #[test]
    fn unknown_post_startup_provider_refuses_before_archive_commit() {
        let startup = owner();
        let prefix = capture(&startup, 1 << 20, &|| true).expect("prefix");
        let mut heap = Heap::new(Arc::clone(&startup), true, false);
        let domain = heap.interval(0, 1);
        let provider = heap.lambda(17, domain, &[]);
        heap.intern_const(provider)
            .expect("startup-capable provider node");
        let image = capture(&startup, 1 << 20, &|| true).expect("image");
        assert!(validate_extension(&prefix, &image, &|| true).is_err());
        assert!(restore(&owner(), &prefix, &image, 1 << 20, &|| true).is_err());
    }

    #[test]
    fn malformed_prefix_bounds_names_coordinates_and_cancel_refuse() {
        let startup = owner();
        let prefix = capture(&startup, 1 << 20, &|| true).expect("prefix");
        assert!(capture(&startup, 8, &|| true).is_err());
        assert!(capture(&startup, 1 << 20, &|| false).is_err());
        let mut heap = Heap::new(Arc::clone(&startup), true, false);
        let value = heap.tuple_literal(&[Handle::TRUE]);
        let value = heap.intern_exact(value).expect("value");
        let image = capture(&startup, 1 << 20, &|| true).expect("image");
        let fresh = owner();
        for cut in [0, 7, image.len() - 1] {
            assert!(restore(&fresh, &prefix, &image[..cut], 1 << 20, &|| true).is_err());
        }
        let mut trailing = image.clone();
        trailing.push(0);
        assert!(restore(&fresh, &prefix, &trailing, 1 << 20, &|| true).is_err());
        let mut bad = image.clone();
        bad[8] = 255;
        assert!(restore(&fresh, &prefix, &bad, 1 << 20, &|| true).is_err());
        assert!(
            restore(&fresh, &image, &image, 1 << 20, &|| true).is_err(),
            "startup cannot absorb later discoveries"
        );
        assert!(
            row(&fresh, &[value.bits()], false, &|| true).is_err(),
            "coordinates need an actual issuing owner"
        );
        assert!(row(&fresh, &[6], false, &|| true).is_err());
    }
}
