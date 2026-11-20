//! Bounded quiescent Store images for completed-level native recovery.
//!
//! These trusted local images preserve issued coordinates and names. They are
//! not value interchange or reachability proofs. Format `DIVEST02` carries no
//! construction-history table; a `DIVEST01` image, which did, is refused by name.
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

const MAGIC: &[u8; 8] = b"DIVEST02";
/// The previous format, which ended with a retired construction-history table.
const LEGACY_MAGIC: &[u8; 8] = b"DIVEST01";
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
}

fn copied<T: Copy>(values: &[T]) -> Result<Vec<T>, Error> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(values.len())
        .map_err(|_| invalid("owned image allocation"))?;
    copy.extend_from_slice(values);
    Ok(copy)
}

fn copy_names<'a>(
    names: impl ExactSizeIterator<Item = &'a str>,
    proceed: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<String>, Error> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(names.len())
        .map_err(|_| invalid("owned name table allocation"))?;
    for (i, name) in names.enumerate() {
        if i.is_multiple_of(128) && !proceed() {
            return Err(Error::stop());
        }
        let mut text = String::new();
        text.try_reserve_exact(name.len())
            .map_err(|_| invalid("owned name allocation"))?;
        text.push_str(name);
        copy.push(text);
    }
    Ok(copy)
}

impl Image {
    /// Duplicates a typed live image without a checkpoint encoder or decoder.
    pub(super) fn duplicate(&self, proceed: &(dyn Fn() -> bool + Sync)) -> Result<Self, Error> {
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(self.nodes.len())
            .map_err(|_| invalid("owned node allocation"))?;
        for (i, node) in self.nodes.iter().enumerate() {
            if i.is_multiple_of(128) && !proceed() {
                return Err(Error::stop());
            }
            nodes.push(Node {
                handle: node.handle,
                kind: node.kind,
                flags: node.flags & !flags::CONCRETE,
                lo: node.lo,
                hash: node.hash,
                sum: node.sum,
                data: copied(&node.data)?,
            });
        }
        Ok(Self {
            semantics: self.semantics,
            sealed: self.sealed,
            names: copy_names(self.names.iter().map(String::as_str), proceed)?,
            lens: copied(&self.lens)?,
            nodes,
        })
    }
}

/// Copies trusted quiescent live roots into typed owned storage. Archive byte,
/// name-count and framing admission apply only to the wire entrypoints below.
#[allow(
    clippy::too_many_lines,
    reason = "one quiescent typed-image transaction without archive admission"
)]
pub(super) fn memory_image(
    store: &Store,
    proceed: &(dyn Fn() -> bool + Sync),
) -> Result<Image, Error> {
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
    let texts = store.strings.texts_from(0);
    let names = copy_names(texts.iter().map(AsRef::as_ref), proceed)?;
    let mut nodes = Vec::new();
    nodes
        .try_reserve_exact(count)
        .map_err(|_| invalid("owned node allocation"))?;
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
                stack
                    .try_reserve(1)
                    .map_err(|_| invalid("traversal allocation"))?;
                stack.push((root, false));
                while let Some((h, leaving)) = stack.pop() {
                    if nodes.len().is_multiple_of(128) && !proceed() {
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
                        let node = Node {
                            handle: h.bits(),
                            kind: v.kind,
                            flags: v.flags & !flags::CONCRETE,
                            lo: v.lo,
                            hash: v.hash,
                            sum: v.sum,
                            data: copied(v.data)?,
                        };
                        validate_node(&node, &lens, names.len(), &mut done)?;
                        nodes.push(node);
                    } else {
                        active
                            .try_reserve(1)
                            .map_err(|_| invalid("traversal allocation"))?;
                        if !active.insert(h.bits()) {
                            return Err(invalid("cyclic value nodes"));
                        }
                        stack
                            .try_reserve(v.data.len() + 1)
                            .map_err(|_| invalid("traversal allocation"))?;
                        stack.push((h, true));
                        stack.extend(
                            v.data
                                .iter()
                                .rev()
                                .filter(|h| h.is_persistent())
                                .map(|&h| (h, false)),
                        );
                    }
                }
            }
        }
    }
    if nodes.len() != count || lengths(store) != lens || store.strings.texts_from(0) != texts {
        return Err(invalid("owner changed during owned capture"));
    }
    Ok(Image {
        semantics: semantics(store),
        sealed: store.strings.is_sealed(),
        names,
        lens,
        nodes,
    })
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

// A local read-only index for a fixed, identity-checked worker startup. Its
// ordinal indexes are the only startup references carried by the owned DAG;
// received coordinates never acquire Store authority.
pub(crate) fn issued(
    store: &Store,
    max_nodes: usize,
    proceed: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<Handle>, Error> {
    let lens = lengths(store);
    let count = lens
        .iter()
        .flatten()
        .try_fold(0usize, |sum, &n| {
            usize::try_from(n).ok().and_then(|n| sum.checked_add(n))
        })
        .filter(|&n| n <= max_nodes)
        .ok_or_else(|| invalid("startup index admission"))?;
    let mut nodes = Vec::new();
    nodes
        .try_reserve_exact(count)
        .map_err(|_| invalid("startup index allocation"))?;
    for (shard, classes) in lens.iter().enumerate() {
        for (class, &len) in classes.iter().enumerate() {
            for index in 0..len {
                if !proceed() {
                    return Err(Error::stop());
                }
                let handle = Handle::persistent(
                    u8::try_from(shard).map_err(|_| invalid("startup shard"))?,
                    u8::try_from(class).map_err(|_| invalid("startup class"))?,
                    index,
                );
                if store.retained_view(handle).is_none() {
                    return Err(invalid("unissued startup node"));
                }
                nodes.push(handle);
            }
        }
    }
    Ok(nodes)
}

/// Captures an owned Store at a quiescent boundary without normalizing values.
///
/// The caller must have joined all evaluating workers. Issued coordinates are
/// retained; runtime hosts are not.
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
                        out.put(&[v.kind as u8, v.flags & !flags::CONCRETE])?;
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
    if lengths(store) != lens || store.strings.texts_from(0) != names {
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

fn validate_node(
    node: &Node,
    lens: &[[u64; CLASSES]],
    names: usize,
    seen: &mut HashSet<u64>,
) -> Result<(), Error> {
    let (handle, kind, bits, low, hash, children) = (
        node.handle,
        node.kind,
        node.flags,
        node.lo,
        node.hash,
        node.data.as_slice(),
    );
    let len = children.len();
    if children
        .iter()
        .any(|&handle_value| !scalar(handle_value, names, seen))
    {
        return Err(invalid("foreign, forward or transient child"));
    }
    let handle_value = Handle(handle);
    if !handle_value.is_persistent() || !seen.insert(handle) {
        return Err(invalid("duplicate/nonpersistent coordinate"));
    }
    let (shard, class, index) = handle_value.persistent_parts();
    let index_hash = interning_hash(kind, bits, low, hash, children);
    if lens.len() != SHARDS
        || bits & !(flags::VIEW | flags::LAZY) != 0
        || class != class_of(children.len(), bits)
        || index >= lens[usize::from(shard)][usize::from(class)]
        || index_hash >> 56 != u64::from(shard)
    {
        return Err(invalid("node coordinate/header mismatch"));
    }
    if matches!(kind, Kind::Fcn | Kind::RecSet) && !len.is_multiple_of(2)
        || matches!(kind, Kind::Interval | Kind::Subset | Kind::SeqSet) && len != 1
        || matches!(kind, Kind::FcnSet | Kind::Union) && len != 2
        || matches!(kind, Kind::Nat | Kind::Int) && len != 0
        || kind.is_lazy() && len == 0
    {
        return Err(invalid("node shape"));
    }
    if kind == Kind::SetEnum && bits & flags::UNNORM == 0 {
        return Err(invalid("raw set phase"));
    }
    if kind == Kind::BigInt
        && (len == 0
            || !matches!(low, -1 | 1)
            || children.iter().any(|handle_value| {
                handle_value
                    .as_int()
                    .is_none_or(|n| u32::try_from(n).is_err())
            })
            || children
                .last()
                .and_then(|handle_value| handle_value.as_int())
                == Some(0))
    {
        return Err(invalid("integer magnitude"));
    }
    if bits & flags::TUPLE != 0 && (kind != Kind::IntFcn || low != 1)
        || bits & flags::RECORD != 0 && kind != Kind::Fcn
        || bits & flags::UNNORM != 0 && kind != Kind::SetEnum
    {
        return Err(invalid("receiver flags"));
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "one bounded binary parser; short names denote checked node coordinates"
)]
pub(super) fn parse(bytes: &[u8], proceed: &(dyn Fn() -> bool + Sync)) -> Result<Image, Error> {
    let mut input = Read { bytes, at: 0 };
    match input.take(8)? {
        magic if magic == MAGIC => {}
        magic if magic == LEGACY_MAGIC => {
            return Err(invalid(
                "format DIVEST01 (legacy construction history) is no longer read; recreate the archive",
            ));
        }
        _ => return Err(invalid("format/version")),
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
        let node = Node {
            handle,
            kind: k,
            flags: fl,
            lo,
            hash,
            sum,
            data,
        };
        validate_node(&node, &lens, names.len(), &mut seen)?;
        nodes.push(node);
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
    {
        return Err(invalid("startup prefix shrank"));
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
/// Providers added after startup refuse rather than being reconstructed from flags.
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
    install(startup, &after, proceed)
}

pub(super) fn install(
    startup: &Store,
    after: &Image,
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
    Ok(store)
}

/// Resolves a saved row through the newly validated owner without forging handles.
///
/// # Errors
/// Unknown/scalar-invalid coordinates or deferred values.
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
                if v.is_lazy() || v.kind.is_lazy() || canonical && v.flags & flags::VIEW != 0 {
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
    fn wire_decoder_keeps_the_checkpoint_name_admission_bound() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&[0; 7]);
        bytes.extend_from_slice(&65_537u64.to_le_bytes());
        bytes.resize(bytes.len() + 65_537 * 8, 0);
        let error = parse(&bytes, &|| true)
            .err()
            .expect("oversized wire name count");
        assert!(error.message().contains("name admission bound"));
    }
    #[test]
    fn restores_canonical_coordinates_aliases_and_fresh_frontier_owner() {
        let startup = owner();
        let prefix = capture(&startup, 1 << 20, &|| true).expect("prefix");
        let mut heap = Heap::new(Arc::clone(&startup), true, false);
        let one = Handle::int(1).expect("int");
        let two = Handle::int(2).expect("int");
        let local = heap.canonical_test_set(&[two, one, two]).expect("literal");
        let raw = heap.intern_exact(local).expect("owned raw");
        let alias = heap.tuple_literal(&[raw, raw]);
        let alias = heap.intern_exact(alias).expect("nested aliases");
        let normalized = heap.norm(local).expect("canonical");
        let normalized = heap.intern_exact(normalized).expect("owned normalized");
        assert_eq!(startup.view(raw).data, &[one, two]);
        let identity = heap.intern(normalized).expect("canonical");
        let image = capture(&startup, 1 << 20, &|| true).expect("image");
        let fresh = owner();
        let restored = restore(&fresh, &prefix, &image, 1 << 20, &|| true).expect("restore");
        assert!(!Arc::ptr_eq(&restored, &startup));
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
        assert_eq!(
            row(&restored, &[identity.bits()], true, &|| true).expect("identity"),
            vec![identity]
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
    fn legacy_construction_history_is_refused_by_name() {
        let startup = owner();
        let mut heap = Heap::new(Arc::clone(&startup), true, false);
        let one = Handle::int(1).expect("one");
        let set = heap.canonical_test_set(&[one]).expect("set");
        heap.intern_exact(set).expect("persistent set");
        let image = capture(&startup, 1 << 20, &|| true).expect("image");
        assert_eq!(&image[..8], b"DIVEST02");
        // A DIVEST01 image: the same body followed by its empty history table.
        let mut legacy = image.clone();
        legacy[..8].copy_from_slice(LEGACY_MAGIC);
        legacy.extend_from_slice(&[0; 8]);
        let error = parse(&legacy, &|| true).err().expect("legacy format");
        assert!(
            error.message().contains("format DIVEST01"),
            "{}",
            error.message()
        );
        assert!(error.message().contains("legacy construction history"));
        let error = validate_extension(&legacy, &legacy, &|| true).expect_err("legacy archive");
        assert!(error.message().contains("format DIVEST01"));
        // The retired history bit is now an unknown flag.
        let mut parsed = parse(&image, &|| true).expect("image");
        let node = parsed.nodes.last_mut().expect("node");
        node.flags |= 32;
        let mut seen = HashSet::new();
        assert!(validate_node(node, &parsed.lens, parsed.names.len(), &mut seen).is_err());
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
