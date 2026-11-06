//! Bounded owned DAG exchange for identity-checked trusted worker sessions.
//!
//! Persistent coordinates never cross this boundary. A reply may reference only
//! nodes/origins explicitly issued by its matching request. The transport must
//! authenticate the complete request/reply and bind compiled startup identity.

use std::collections::HashMap;
use std::sync::Arc;

use crate::handle::Tag;
use crate::node::{Kind, flags};
use crate::{Error, Handle, Heap, Store};

const MAGIC: &[u8; 8] = b"DIVEOW01";
const HEADER: usize = 33;
const NODE: usize = 51;
const DEPTH: usize = 128;

fn invalid(message: impl std::fmt::Display) -> Error {
    Error::internal(format!("DIVE-S0501: owned worker exchange: {message}"))
}

/// One request's native input-node and constructor-origin authority.
///
/// This owner is process-local and is never serialized. Wire references are
/// indexes into the matching authenticated request, not raw handles or tokens.
pub struct Exchange {
    store: Arc<Store>,
    names: usize,
    nodes: Vec<Handle>,
    origins: Vec<i64>,
    by_node: HashMap<Handle, u64>,
    by_origin: HashMap<i64, u64>,
    startup: Option<Arc<Exchange>>,
}

/// Local ordinal authority for a fixed compiled startup. A transport must bind
/// the complete startup identity before allowing references into this index.
#[derive(Clone)]
pub struct Startup(Arc<Exchange>);
impl Startup {
    /// Captures issued startup nodes and constructor owners without mutation.
    ///
    /// # Errors
    /// Invalid owned nodes, the private index limit or interruption.
    pub fn new(
        heap: &Heap,
        limit: usize,
        proceed: &(dyn Fn() -> bool + Sync),
    ) -> Result<Self, Error> {
        let nodes = crate::store::snapshot::issued(heap.store(), limit / NODE, proceed)?;
        let mut origins = Vec::new();
        let mut by_origin = HashMap::new();
        let mut by_node = HashMap::new();
        by_node.try_reserve(nodes.len()).map_err(invalid)?;
        for (index, &handle) in nodes.iter().enumerate() {
            let node = heap
                .store()
                .retained_view(handle)
                .ok_or_else(|| invalid("unissued startup node"))?;
            by_node.insert(handle, index as u64);
            if node.flags & flags::HISTORY != 0 && !by_origin.contains_key(&node.lo) {
                if !heap.store().owns_history(handle) {
                    return Err(invalid("unowned startup history"));
                }
                heap.store().history_snapshots(node.lo)?;
                by_origin.try_reserve(1).map_err(invalid)?;
                origins.try_reserve(1).map_err(invalid)?;
                by_origin.insert(node.lo, origins.len() as u64);
                origins.push(node.lo);
            }
        }
        Ok(Self(Arc::new(Exchange {
            store: Arc::clone(heap.store()),
            names: heap.store().strings().len(),
            nodes,
            origins,
            by_node,
            by_origin,
            startup: None,
        })))
    }
}

impl Exchange {
    /// Encodes actual owned input roots without reference-value conversion.
    ///
    /// # Errors
    /// Foreign, lazy, unowned, oversized or interrupted values.
    pub fn request(
        heap: &Heap,
        roots: &[Handle],
        limit: usize,
        proceed: &(dyn Fn() -> bool + Sync),
    ) -> Result<(Vec<u8>, Self), Error> {
        Self::request_inner(heap, roots, limit, proceed, None)
    }

    /// Encodes a source while retaining aliases to an authenticated startup.
    ///
    /// # Errors
    /// Foreign startup authority or the same admission errors as [`Self::request`].
    pub fn request_with_startup(
        heap: &Heap,
        roots: &[Handle],
        limit: usize,
        proceed: &(dyn Fn() -> bool + Sync),
        startup: &Startup,
    ) -> Result<(Vec<u8>, Self), Error> {
        startup.0.owner(heap)?;
        Self::request_inner(heap, roots, limit, proceed, Some(Arc::clone(&startup.0)))
    }

    fn request_inner(
        heap: &Heap,
        roots: &[Handle],
        limit: usize,
        proceed: &(dyn Fn() -> bool + Sync),
        startup: Option<Arc<Exchange>>,
    ) -> Result<(Vec<u8>, Self), Error> {
        let mut writer = Writer::new(heap, limit, None, startup.as_deref(), proceed)?;
        let bytes = writer.encode(roots)?;
        let exchange = Self {
            store: Arc::clone(heap.store()),
            names: heap.store().strings().len(),
            nodes: writer.nodes,
            origins: writer.origins,
            by_node: writer.by_node,
            by_origin: writer.by_origin,
            startup,
        };
        Ok((bytes, exchange))
    }

    /// Imports a request only into an independently identity-checked startup.
    ///
    /// # Errors
    /// Invalid framing, ownership, names, shape, limits or interruption.
    pub fn receive_request(
        heap: &mut Heap,
        bytes: &[u8],
        limit: usize,
        proceed: &(dyn Fn() -> bool + Sync),
    ) -> Result<(Vec<Handle>, Self), Error> {
        Self::receive_inner(heap, bytes, limit, proceed, None)
    }

    /// Imports against an independently authenticated compiled startup index.
    ///
    /// # Errors
    /// Foreign startup or invalid request values and authority.
    pub fn receive_request_with_startup(
        heap: &mut Heap,
        bytes: &[u8],
        limit: usize,
        proceed: &(dyn Fn() -> bool + Sync),
        startup: &Startup,
    ) -> Result<(Vec<Handle>, Self), Error> {
        startup.0.owner(heap)?;
        Self::receive_inner(heap, bytes, limit, proceed, Some(Arc::clone(&startup.0)))
    }

    fn receive_inner(
        heap: &mut Heap,
        bytes: &[u8],
        limit: usize,
        proceed: &(dyn Fn() -> bool + Sync),
        startup: Option<Arc<Exchange>>,
    ) -> Result<(Vec<Handle>, Self), Error> {
        let (roots, nodes, origins) =
            decode(heap, bytes, limit, None, startup.as_deref(), proceed)?;
        let mut by_node = HashMap::new();
        let mut by_origin = HashMap::new();
        by_node.try_reserve(nodes.len()).map_err(invalid)?;
        by_origin.try_reserve(origins.len()).map_err(invalid)?;
        by_node.extend(nodes.iter().enumerate().map(|(i, &h)| (h, i as u64)));
        by_origin.extend(origins.iter().enumerate().map(|(i, &o)| (o, i as u64)));
        Ok((
            roots,
            Self {
                store: Arc::clone(heap.store()),
                names: heap.store().strings().len(),
                nodes,
                origins,
                by_node,
                by_origin,
                startup,
            },
        ))
    }

    /// Encodes a worker result while retaining aliases to actual input owners.
    ///
    /// # Errors
    /// Wrong owner, invalid values, record limits or interruption.
    pub fn reply(
        &self,
        heap: &Heap,
        roots: &[Handle],
        limit: usize,
        proceed: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<u8>, Error> {
        self.owner(heap)?;
        Writer::new(heap, limit, Some(self), self.startup.as_deref(), proceed)?.encode(roots)
    }

    /// Imports a reply through this request's actual issuing owner.
    ///
    /// # Errors
    /// Unknown input indexes, malformed values, limits or interruption.
    pub fn receive_reply(
        &self,
        heap: &mut Heap,
        bytes: &[u8],
        limit: usize,
        proceed: &(dyn Fn() -> bool + Sync),
    ) -> Result<Vec<Handle>, Error> {
        self.owner(heap)?;
        decode(
            heap,
            bytes,
            limit,
            Some(self),
            self.startup.as_deref(),
            proceed,
        )
        .map(|(roots, _, _)| roots)
    }

    fn owner(&self, heap: &Heap) -> Result<(), Error> {
        if !Arc::ptr_eq(&self.store, heap.store())
            || heap.store().strings().len() != self.names
            || heap.poisoned()
        {
            return Err(invalid("foreign, changed-name or poisoned owner"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Ref {
    Scalar(Handle),
    Local(u64),
    Input(u64),
    Startup(u64),
}

struct Writer<'a> {
    heap: &'a Heap,
    limit: usize,
    input: Option<&'a Exchange>,
    startup: Option<&'a Exchange>,
    proceed: &'a (dyn Fn() -> bool + Sync),
    bytes: Vec<u8>,
    scratch: usize,
    nodes: Vec<Handle>,
    origins: Vec<i64>,
    by_node: HashMap<Handle, u64>,
    by_origin: HashMap<i64, u64>,
}

impl<'a> Writer<'a> {
    fn new(
        heap: &'a Heap,
        limit: usize,
        input: Option<&'a Exchange>,
        startup: Option<&'a Exchange>,
        proceed: &'a (dyn Fn() -> bool + Sync),
    ) -> Result<Self, Error> {
        if !proceed() {
            return Err(Error::stop());
        }
        if heap.poisoned() {
            return Err(invalid("poisoned source"));
        }
        Ok(Self {
            heap,
            limit,
            input,
            startup,
            proceed,
            bytes: Vec::new(),
            scratch: 0,
            nodes: Vec::new(),
            origins: Vec::new(),
            by_node: HashMap::new(),
            by_origin: HashMap::new(),
        })
    }
    fn put(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let end = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|&n| n <= self.limit)
            .ok_or_else(|| invalid("record byte limit"))?;
        self.bytes
            .try_reserve(end - self.bytes.len())
            .map_err(invalid)?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn byte(&mut self, n: u8) -> Result<(), Error> {
        self.put(&[n])
    }
    fn word(&mut self, n: u64) -> Result<(), Error> {
        self.put(&n.to_le_bytes())
    }
    fn encode(&mut self, roots: &[Handle]) -> Result<Vec<u8>, Error> {
        self.put(MAGIC)?;
        self.byte(u8::from(self.input.is_some()) | (u8::from(self.startup.is_some()) * 2))?;
        self.word(0)?;
        self.word(0)?;
        self.word(roots.len() as u64)?;
        let mut refs = Vec::new();
        refs.try_reserve_exact(roots.len()).map_err(invalid)?;
        for &root in roots {
            refs.push(self.visit(root, 0)?);
        }
        // Constructor companions carry actual registered phase authority, even
        // when only the other phase occurs in the root row.
        if let Some(input) = self.input {
            for &origin in &input.origins {
                for h in self.heap.store().history_snapshots(origin)? {
                    if !h.is_empty() {
                        self.visit(h, 0)?;
                    }
                }
            }
        }
        if let Some(startup) = self.startup {
            for &origin in &startup.origins {
                for h in self.heap.store().history_snapshots(origin)? {
                    if !h.is_empty() {
                        self.visit(h, 0)?;
                    }
                }
            }
        }
        let mut index = 0;
        while index < self.origins.len() {
            for h in self.heap.store().history_snapshots(self.origins[index])? {
                if !h.is_empty() {
                    self.visit(h, 0)?;
                }
            }
            index += 1;
        }
        for reference in refs {
            self.reference(reference)?;
        }
        self.bytes[9..17].copy_from_slice(&(self.nodes.len() as u64).to_le_bytes());
        self.bytes[17..25].copy_from_slice(&(self.origins.len() as u64).to_le_bytes());
        Ok(std::mem::take(&mut self.bytes))
    }
    fn origin(&mut self, origin: i64) -> Result<(u8, u64), Error> {
        if let Some(id) = self
            .startup
            .and_then(|startup| startup.by_origin.get(&origin))
        {
            return Ok((3, *id));
        }
        if let Some(id) = self.input.and_then(|input| input.by_origin.get(&origin)) {
            return Ok((2, *id));
        }
        if let Some(&id) = self.by_origin.get(&origin) {
            return Ok((1, id));
        }
        if self.origins.len() >= self.limit / NODE {
            return Err(invalid("origin limit"));
        }
        let id = self.origins.len() as u64;
        self.by_origin.try_reserve(1).map_err(invalid)?;
        self.origins.try_reserve(1).map_err(invalid)?;
        self.origins.push(origin);
        self.by_origin.insert(origin, id);
        Ok((1, id))
    }
    fn visit(&mut self, value: Handle, depth: usize) -> Result<Ref, Error> {
        if !(self.proceed)() {
            return Err(Error::stop());
        }
        if !value.is_persistent() {
            if value.is_transient() {
                return Err(invalid("transient root"));
            }
            return Ok(Ref::Scalar(value));
        }
        if let Some(id) = self.input.and_then(|input| input.by_node.get(&value)) {
            return Ok(Ref::Input(*id));
        }
        if let Some(id) = self.startup.and_then(|startup| startup.by_node.get(&value)) {
            return Ok(Ref::Startup(*id));
        }
        if let Some(&id) = self.by_node.get(&value) {
            return Ok(Ref::Local(id));
        }
        if depth >= DEPTH || self.nodes.len() >= self.limit / NODE {
            return Err(invalid("DAG depth/node limit"));
        }
        let view = self
            .heap
            .store()
            .retained_view(value)
            .ok_or_else(|| invalid("unissued source"))?;
        if view.is_lazy() || view.kind.is_lazy() {
            return Err(Error::unsupported("distributed deferred value provider"));
        }
        if view.flags & flags::HISTORY != 0 && !self.heap.store().owns_history(value) {
            return Err(invalid("unregistered history"));
        }
        let (kind, fl, lo, hash, sum) = (view.kind, view.flags, view.lo, view.hash, view.sum);
        let scratch = view
            .data
            .len()
            .checked_mul(size_of::<Handle>() + size_of::<Ref>())
            .filter(|&n| n <= self.limit.saturating_sub(self.scratch))
            .ok_or_else(|| invalid("DAG traversal workspace limit"))?;
        self.scratch += scratch;
        if view
            .data
            .len()
            .checked_mul(9)
            .and_then(|n| n.checked_add(NODE))
            .is_none_or(|n| n > self.limit.saturating_sub(self.bytes.len()))
        {
            return Err(invalid("node byte limit"));
        }
        let mut children = Vec::new();
        children
            .try_reserve_exact(view.data.len())
            .map_err(invalid)?;
        children.extend_from_slice(view.data);
        let mut refs = Vec::new();
        refs.try_reserve_exact(children.len()).map_err(invalid)?;
        for child in children {
            refs.push(self.visit(child, depth + 1)?);
        }
        self.scratch -= scratch;
        let origin = if fl & flags::HISTORY != 0 {
            self.origin(lo)?
        } else {
            (0, 0)
        };
        self.byte(kind as u8)?;
        self.byte(fl)?;
        self.word(if origin.0 == 0 { lo.cast_unsigned() } else { 0 })?;
        self.word(hash)?;
        self.word(sum)?;
        self.byte(origin.0)?;
        self.word(origin.1)?;
        self.word(refs.len() as u64)?;
        for reference in refs {
            self.reference(reference)?;
        }
        let index = self.nodes.len() as u64;
        self.nodes.try_reserve(1).map_err(invalid)?;
        self.by_node.try_reserve(1).map_err(invalid)?;
        self.nodes.push(value);
        self.by_node.insert(value, index);
        Ok(Ref::Local(index))
    }
    fn reference(&mut self, reference: Ref) -> Result<(), Error> {
        match reference {
            Ref::Local(id) => {
                self.byte(5)?;
                self.word(id)
            }
            Ref::Input(id) => {
                self.byte(6)?;
                self.word(id)
            }
            Ref::Startup(id) => {
                self.byte(7)?;
                self.word(id)
            }
            Ref::Scalar(h) => match h.tag() {
                Tag::Empty if h.is_empty() => self.byte(0),
                Tag::Int => {
                    self.byte(1)?;
                    self.word(
                        h.as_int()
                            .ok_or_else(|| invalid("integer"))?
                            .cast_unsigned(),
                    )
                }
                Tag::Bool if h.as_bool().is_some() => {
                    self.byte(2)?;
                    self.byte(u8::from(h == Handle::TRUE))
                }
                Tag::Str | Tag::Model => {
                    let id = h
                        .as_str_id()
                        .or_else(|| h.as_model_id())
                        .ok_or_else(|| invalid("name token"))?;
                    let text = self
                        .heap
                        .store()
                        .strings()
                        .text(id)
                        .ok_or_else(|| invalid("unissued name"))?;
                    self.byte(if h.tag() == Tag::Str { 3 } else { 4 })?;
                    self.word(u64::from(id))?;
                    self.word(text.len() as u64)?;
                    self.put(text.as_bytes())
                }
                _ => Err(invalid("invalid scalar")),
            },
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&n| n <= self.bytes.len())
            .ok_or_else(|| invalid("truncated packet"))?;
        let bytes = &self.bytes[self.at..end];
        self.at = end;
        Ok(bytes)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn word(&mut self) -> Result<u64, Error> {
        let mut b = [0; 8];
        b.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(b))
    }
    fn count(&mut self, unit: usize) -> Result<usize, Error> {
        let n = usize::try_from(self.word()?).map_err(invalid)?;
        if n.checked_mul(unit)
            .is_none_or(|n| n > self.bytes.len().saturating_sub(self.at))
        {
            return Err(invalid("declared count"));
        }
        Ok(n)
    }
    fn reference(
        &mut self,
        heap: &Heap,
        nodes: &[Handle],
        input: Option<&Exchange>,
        startup: Option<&Exchange>,
    ) -> Result<Handle, Error> {
        match self.byte()? {
            0 => Ok(Handle::EMPTY),
            1 => Handle::int(self.word()?.cast_signed()).ok_or_else(|| invalid("inline integer")),
            2 => match self.byte()? {
                0 => Ok(Handle::FALSE),
                1 => Ok(Handle::TRUE),
                _ => Err(invalid("Boolean")),
            },
            tag @ (3 | 4) => {
                let id = u32::try_from(self.word()?).map_err(invalid)?;
                let len = self.count(1)?;
                let text = std::str::from_utf8(self.take(len)?).map_err(invalid)?;
                if heap.store().strings().text(id).as_deref() != Some(text) {
                    return Err(invalid("startup name authority mismatch"));
                }
                Ok(if tag == 3 {
                    Handle::string(id)
                } else {
                    Handle::model(id)
                })
            }
            5 => nodes
                .get(usize::try_from(self.word()?).map_err(invalid)?)
                .copied()
                .ok_or_else(|| invalid("forward local node")),
            6 => input
                .and_then(|i| {
                    usize::try_from(self.word().ok()?)
                        .ok()
                        .and_then(|n| i.nodes.get(n))
                })
                .copied()
                .ok_or_else(|| invalid("unknown input node")),
            7 => startup
                .and_then(|i| {
                    usize::try_from(self.word().ok()?)
                        .ok()
                        .and_then(|n| i.nodes.get(n))
                })
                .copied()
                .ok_or_else(|| invalid("unknown startup node")),
            _ => Err(invalid("reference tag")),
        }
    }
}

type Decoded = (Vec<Handle>, Vec<Handle>, Vec<i64>);
#[allow(
    clippy::too_many_lines,
    reason = "one bounded authenticated owned DAG import"
)]
fn decode(
    heap: &mut Heap,
    bytes: &[u8],
    limit: usize,
    input: Option<&Exchange>,
    startup: Option<&Exchange>,
    proceed: &(dyn Fn() -> bool + Sync),
) -> Result<Decoded, Error> {
    if bytes.len() > limit || bytes.len() < HEADER {
        return Err(invalid("packet size"));
    }
    let mut read = Reader { bytes, at: 0 };
    if read.take(8)? != MAGIC
        || read.byte() != Ok(u8::from(input.is_some()) | (u8::from(startup.is_some()) * 2))
    {
        return Err(invalid("version/direction"));
    }
    let count = usize::try_from(read.word()?).map_err(invalid)?;
    let origin_count = usize::try_from(read.word()?).map_err(invalid)?;
    let roots = usize::try_from(read.word()?).map_err(invalid)?;
    if count > limit / NODE
        || origin_count > count
        || roots > limit / size_of::<Handle>()
        || count
            .checked_mul(NODE)
            .and_then(|n| n.checked_add(roots))
            .is_none_or(|n| n > bytes.len() - HEADER)
    {
        return Err(invalid("packet admission"));
    }
    let mut origins = Vec::new();
    origins.try_reserve_exact(origin_count).map_err(invalid)?;
    for _ in 0..origin_count {
        origins.push(heap.store().history_origin()?);
    }
    let mut nodes = Vec::new();
    nodes.try_reserve_exact(count).map_err(invalid)?;
    for _ in 0..count {
        if !proceed() {
            return Err(Error::stop());
        }
        let kind = crate::candidate_codec::kind(read.byte()?)?;
        let fl = read.byte()?;
        let mut lo = read.word()?.cast_signed();
        let hash = read.word()?;
        let sum = read.word()?;
        let origin_tag = read.byte()?;
        let origin_id = usize::try_from(read.word()?).map_err(invalid)?;
        let origin = match (origin_tag, origin_id) {
            (0, 0) => None,
            (1, n) => Some(*origins.get(n).ok_or_else(|| invalid("local origin"))?),
            (2, n) => Some(
                *input
                    .and_then(|i| i.origins.get(n))
                    .ok_or_else(|| invalid("input origin"))?,
            ),
            (3, n) => Some(
                *startup
                    .and_then(|i| i.origins.get(n))
                    .ok_or_else(|| invalid("startup origin"))?,
            ),
            _ => return Err(invalid("origin tag")),
        };
        if origin_tag != 0 && lo != 0 {
            return Err(invalid("origin authority"));
        }
        let len = read.count(1)?;
        if len > limit / size_of::<Handle>() {
            return Err(invalid("child workspace limit"));
        }
        let mut children = Vec::new();
        children.try_reserve_exact(len).map_err(invalid)?;
        for _ in 0..len {
            if !proceed() {
                return Err(Error::stop());
            }
            children.push(read.reference(heap, &nodes, input, startup)?);
        }
        crate::candidate_codec::shape(kind, fl, lo, u64::from(origin.is_some()), &children)?;
        if let Some(origin) = origin {
            lo = origin;
        }
        let existing = origin
            .map(|origin| heap.store().history_snapshots(origin))
            .transpose()?
            .map(|pair| pair[usize::from(kind == Kind::Set)])
            .filter(|h| !h.is_empty());
        let h = if let Some(existing) = existing {
            let view = heap
                .store()
                .retained_view(existing)
                .ok_or_else(|| invalid("unissued existing phase"))?;
            if (view.kind, view.flags, view.lo, view.hash, view.sum) != (kind, fl, lo, hash, sum)
                || view.data != children
            {
                return Err(invalid(
                    "attempted replacement of an actual constructor phase",
                ));
            }
            existing
        } else {
            heap.store
                .intern(&mut heap.cache, kind, fl, lo, hash, sum, &children)
        };
        if let Some(origin) = origin {
            heap.store().retain_history(origin, kind == Kind::Set, h)?;
        }
        nodes.push(h);
    }
    for &origin in &origins {
        if heap
            .store()
            .history_snapshots(origin)?
            .iter()
            .all(|h| h.is_empty())
        {
            return Err(invalid("unused origin authority"));
        }
    }
    let mut values = Vec::new();
    values.try_reserve_exact(roots).map_err(invalid)?;
    for _ in 0..roots {
        if !proceed() {
            return Err(Error::stop());
        }
        values.push(read.reference(heap, &nodes, input, startup)?);
    }
    if read.at != bytes.len() || heap.poisoned() {
        return Err(invalid("trailing data or poisoned import"));
    }
    Ok((values, nodes, origins))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "owned cross-process DAG positive and malformed controls"
    )]
    use super::*;

    fn heap(tlc: bool, hashcons: bool) -> Heap {
        let store = Arc::new(Store::with_semantics(
            hashcons,
            if tlc {
                crate::Semantics::tlc()
            } else {
                crate::Semantics::default()
            },
        ));
        store.strings().preload(["a", "b"]).expect("startup names");
        store.strings().seal();
        Heap::new(store, true, false)
    }

    #[test]
    fn request_reply_preserves_classes_aliases_and_canonical_sets() {
        for tlc in [false, true] {
            for hashcons in [false, true] {
                let mut sender = heap(tlc, hashcons);
                let one = Handle::int(1).expect("one");
                let two = Handle::int(2).expect("two");
                let set = sender
                    .canonical_test_set(&[two, one, two])
                    .expect("literal");
                let raw = sender.intern_exact(set).expect("raw");
                let normal = sender.norm(set).expect("canonical");
                let normal = sender.intern_exact(normal).expect("normal");
                let tuple = sender.tuple_literal(&[raw, raw, normal, Handle::model(0)]);
                let tuple = sender.intern_exact(tuple).expect("root");
                let (bytes, authority) =
                    Exchange::request(&sender, &[tuple], 1 << 20, &|| true).expect("request");
                let mut worker = heap(tlc, hashcons);
                let (roots, remote) =
                    Exchange::receive_request(&mut worker, &bytes, 1 << 20, &|| true)
                        .expect("import");
                let row = worker.view(roots[0]).data.to_vec();
                assert_eq!(row[0], row[1]);
                assert!(worker.view(roots[0]).is_tuple_literal());
                assert_eq!(worker.view(row[0]).data, &[one, two]);
                assert_eq!(worker.view(row[2]).data, &[one, two]);
                assert_eq!(worker.store().memory_usage().history_view_bytes, 0);
                let bytes = remote
                    .reply(&worker, &roots, 1 << 20, &|| true)
                    .expect("unchanged reply");
                assert_eq!(
                    authority
                        .receive_reply(&mut sender, &bytes, 1 << 20, &|| true)
                        .expect("input alias"),
                    vec![tuple]
                );
                let fresh = worker
                    .canonical_test_set(&[two, one, two])
                    .expect("independent constructor");
                let fresh = worker.intern_exact(fresh).expect("fresh history");
                let out = worker.tuple_literal(&[roots[0], fresh, row[0]]);
                let out = worker.intern_exact(out).expect("reply root");
                let bytes = remote
                    .reply(&worker, &[out], 1 << 20, &|| true)
                    .expect("reply");
                let result = authority
                    .receive_reply(&mut sender, &bytes, 1 << 20, &|| true)
                    .expect("result");
                let values = sender.view(result[0]).data;
                assert_eq!(values[0], tuple);
                assert_eq!(values[2], raw);
                assert_eq!(sender.view(values[1]).data, &[one, two]);
                assert_eq!(sender.store().memory_usage().history_view_bytes, 0);
            }
        }
    }

    #[test]
    fn malformed_names_origins_references_limits_and_wrong_owner_refuse() {
        let mut sender = heap(false, true);
        let one = Handle::int(1).expect("one");
        let tuple = sender.tuple_literal(&[one, Handle::string(1)]);
        let tuple = sender.intern_exact(tuple).expect("tuple");
        let (bytes, authority) =
            Exchange::request(&sender, &[tuple], 1 << 20, &|| true).expect("request");
        assert!(Exchange::request(&sender, &[tuple], 16, &|| true).is_err());
        assert!(Exchange::request(&sender, &[tuple], 1 << 20, &|| false).is_err());
        for at in [0, 9, 17, 25, 34, 59, 60] {
            let mut malformed = bytes.clone();
            malformed[at] = 255;
            assert!(
                Exchange::receive_request(&mut heap(false, true), &malformed, 1 << 20, &|| true)
                    .is_err(),
                "byte {at}"
            );
        }
        let mut name = bytes.clone();
        let name_byte = name.iter().rposition(|&b| b == b'b').expect("text");
        name[name_byte] = b'z';
        assert!(
            Exchange::receive_request(&mut heap(false, true), &name, 1 << 20, &|| true).is_err()
        );
        assert!(
            Exchange::receive_request(
                &mut heap(false, true),
                &bytes[..bytes.len() - 1],
                1 << 20,
                &|| true
            )
            .is_err()
        );
        assert!(
            authority
                .receive_reply(&mut heap(false, true), &bytes, 1 << 20, &|| true)
                .is_err()
        );
        assert!(
            authority
                .receive_reply(&mut sender, &bytes, 1 << 20, &|| true)
                .is_err()
        );
    }

    #[test]
    fn compiled_startup_aliases_keep_independent_store_ownership() {
        for tlc in [false, true] {
            for hashcons in [false, true] {
                let mut sender = heap(tlc, hashcons);
                let mut worker = heap(tlc, hashcons);
                let one = Handle::int(1).expect("one");
                let two = Handle::int(2).expect("two");
                let raw = sender
                    .canonical_test_set(&[two, one, two])
                    .expect("startup constructor");
                let raw = sender.intern_exact(raw).expect("startup raw");
                let remote_literal = worker
                    .canonical_test_set(&[two, one, two])
                    .expect("independent startup constructor");
                let remote_raw = worker
                    .intern_exact(remote_literal)
                    .expect("independent startup raw");
                let authority = Startup::new(&sender, 1 << 20, &|| true).expect("startup index");
                let remote_authority =
                    Startup::new(&worker, 1 << 20, &|| true).expect("independent startup index");
                let (bytes, context) =
                    Exchange::request_with_startup(&sender, &[raw], 1 << 20, &|| true, &authority)
                        .expect("request");
                assert_eq!(
                    u64::from_le_bytes(bytes[9..17].try_into().expect("count")),
                    0,
                    "startup is an ordinal reference"
                );
                assert!(
                    Exchange::receive_request(&mut worker, &bytes, 1 << 20, &|| true).is_err(),
                    "unbound startup refused"
                );
                let (roots, remote) = Exchange::receive_request_with_startup(
                    &mut worker,
                    &bytes,
                    1 << 20,
                    &|| true,
                    &remote_authority,
                )
                .expect("import");
                assert_eq!(
                    roots,
                    [remote_raw],
                    "actual independently compiled constant alias"
                );
                let normal = worker.norm(remote_literal).expect("canonical");
                let normal = worker.intern_exact(normal).expect("owned new phase");
                let bytes = remote
                    .reply(&worker, &[remote_raw, normal], 1 << 20, &|| true)
                    .expect("reply");
                let roots = context
                    .receive_reply(&mut sender, &bytes, 1 << 20, &|| true)
                    .expect("reply import");
                assert_eq!(roots[0], raw);
                assert_eq!(sender.view(roots[1]).lo, sender.view(raw).lo);
                assert_eq!(sender.store().memory_usage().history_view_bytes, 0);
                assert_eq!(sender.view(roots[1]).data, &[one, two]);
            }
        }
    }
}
