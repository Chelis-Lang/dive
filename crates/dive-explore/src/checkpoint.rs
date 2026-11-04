//! Completed-level native safety search images, owned by their checked Store.
//!
//! Only checkpoint-enabled runs retain discovery rows. Images are trusted local
//! continuation data, not independent reachability or property certificates.

//! ```
//! use dive_explore::checkpoint::Session;
//! let transaction = Session::pause(2)?;
//! assert!(transaction.paused().is_none());
//! assert!(!transaction.recovered());
//! # Ok::<(), dive_core::Error>(())
//! ```
use std::sync::Arc;

use dive_core::{Error, Handle, Heap, Store};

use crate::Stats;

/// Existing bounded native archive discovery limit.
pub const MAX_STATES: usize = 65_536;
/// Existing bounded native archive depth limit.
pub const MAX_LEVEL: u64 = 65_536;
/// Maximum encoded search image, independently of retained Store memory.
pub const MAX_BYTES: usize = 64 << 20;
const MAGIC: &[u8; 8] = b"DIVEEX01";

#[derive(Clone, Debug)]
pub(super) struct Discovery {
    pub(super) key: u64,
    pub(super) canonical: Vec<Handle>,
    pub(super) original: Vec<Handle>,
    pub(super) parent: Option<u64>,
    pub(super) ordinal: u32,
}

/// A validated, Store-owned ordered search image. It is not a model certificate.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub(super) store: Arc<Store>,
    pub(super) variables: usize,
    pub(super) next_level: u64,
    pub(super) stats: Stats,
    pub(super) discoveries: Vec<Discovery>,
    pub(super) frontier: Vec<u64>,
    pub(super) warnings: Vec<(u16, u32)>,
}

fn invalid(message: impl Into<String>) -> Error {
    Error::unsupported(format!("native search snapshot: {}", message.into()))
}

impl Snapshot {
    /// Next source level; no saved source has yet been expanded at this boundary.
    #[must_use]
    pub const fn next_level(&self) -> u64 {
        self.next_level
    }
    /// Saved model counters before the selected recovery reset policy.
    #[must_use]
    pub const fn stats(&self) -> Stats {
        self.stats
    }
    /// Number of saved concrete discoveries, including all expanded states.
    #[must_use]
    pub fn discovered(&self) -> usize {
        self.discoveries.len()
    }
    /// Number of actual ordered unexpanded states.
    #[must_use]
    pub fn queued(&self) -> usize {
        self.frontier.len()
    }
    /// Saved warning expression ids, to validate against the actual source IR.
    pub fn warning_sites(&self) -> impl Iterator<Item = u32> + '_ {
        self.warnings.iter().map(|v| v.1)
    }

    fn validate(&self) -> Result<(), Error> {
        let count = self.discoveries.len();
        if self.variables > 1024
            || count == 0
            || count > MAX_STATES
            || self.next_level == 0
            || self.next_level > MAX_LEVEL
            || self.stats.distinct != count as u64
            || self.stats.generated < self.stats.initial_generated
            || self.stats.generated < self.stats.distinct
            || self.stats.generated > i64::MAX as u64
            || self.stats.depth > MAX_LEVEL
            || self.frontier.is_empty()
            || self.stats.left_on_queue != self.frontier.len() as u64
        {
            return Err(invalid("count/level/counter bounds"));
        }
        let mut depths = Vec::new();
        depths
            .try_reserve_exact(count)
            .map_err(|_| invalid("depth allocation"))?;
        let mut children = false;
        let mut max_depth = 0;
        for (id, d) in self.discoveries.iter().enumerate() {
            if d.canonical.len() != self.variables || d.original.len() != self.variables {
                return Err(invalid("discovery row width"));
            }
            let depth = match d.parent {
                None if !children => 1,
                Some(parent) if parent < (id as u64) => {
                    children = true;
                    depths[usize::try_from(parent).map_err(|_| invalid("parent address space"))?]
                        + 1
                }
                _ => return Err(invalid("ordered predecessor forest")),
            };
            if depth > MAX_LEVEL {
                return Err(invalid("predecessor depth bound"));
            }
            max_depth = max_depth.max(depth);
            depths.push(depth);
        }
        let first =
            usize::try_from(self.frontier[0]).map_err(|_| invalid("frontier address space"))?;
        if first >= count
            || self.frontier.len() != count - first
            || self
                .frontier
                .iter()
                .enumerate()
                .any(|(i, &id)| id != (first + i) as u64 || depths[first + i] != self.next_level)
            || max_depth != self.stats.depth
        {
            return Err(invalid("complete ordered frontier suffix/depth"));
        }
        if self
            .warnings
            .iter()
            .any(|(var, _)| usize::from(*var) >= self.variables)
        {
            return Err(invalid("warning variable"));
        }
        Ok(())
    }

    /// Encodes owned rows and ordered search facts, without reference values.
    ///
    /// # Errors
    /// Structural bounds, allocation, or the actual owner's cancellation.
    pub fn encode(&self, proceed: &(dyn Fn() -> bool + Sync)) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let mut out = Output(Vec::new());
        out.bytes(MAGIC)?;
        out.word(self.variables as u64)?;
        out.word(self.next_level)?;
        for n in [
            self.stats.generated,
            self.stats.distinct,
            self.stats.depth,
            self.stats.left_on_queue,
            self.stats.initial_generated,
        ] {
            out.word(n)?;
        }
        out.word(self.discoveries.len() as u64)?;
        for (i, d) in self.discoveries.iter().enumerate() {
            if i.is_multiple_of(128) && !proceed() {
                return Err(Error::stop());
            }
            out.word(d.key)?;
            out.word(d.parent.unwrap_or(u64::MAX))?;
            out.word(u64::from(d.ordinal))?;
            for h in d.canonical.iter().chain(&d.original) {
                out.word(h.bits())?;
            }
        }
        out.word(self.frontier.len() as u64)?;
        for &id in &self.frontier {
            out.word(id)?;
        }
        out.word(self.warnings.len() as u64)?;
        for &(var, site) in &self.warnings {
            out.word(u64::from(var))?;
            out.word(u64::from(site))?;
        }
        Ok(out.0)
    }

    /// Decodes rows only through this fresh validated Store owner.
    ///
    /// # Errors
    /// Invalid framing, source-independent structure, ownership, bounds or cancellation.
    pub fn decode(
        bytes: &[u8],
        store: Arc<Store>,
        proceed: &(dyn Fn() -> bool + Sync),
    ) -> Result<Self, Error> {
        if bytes.len() > MAX_BYTES {
            return Err(invalid("image byte bound"));
        }
        let mut input = Input { bytes, at: 0 };
        if input.take(8)? != MAGIC {
            return Err(invalid("format/version"));
        }
        let variables =
            usize::try_from(input.word()?).map_err(|_| invalid("variable address space"))?;
        if variables > 1024 {
            return Err(invalid("variable bound"));
        }
        let next_level = input.word()?;
        let stats = Stats {
            generated: input.word()?,
            distinct: input.word()?,
            depth: input.word()?,
            left_on_queue: input.word()?,
            initial_generated: input.word()?,
        };
        let count = input.count(24 + variables * 16, MAX_STATES)?;
        let mut discoveries = Vec::new();
        discoveries
            .try_reserve_exact(count)
            .map_err(|_| invalid("discovery allocation"))?;
        for i in 0..count {
            if i.is_multiple_of(128) && !proceed() {
                return Err(Error::stop());
            }
            let key = input.word()?;
            let parent = input.word()?;
            let ordinal = u32::try_from(input.word()?).map_err(|_| invalid("successor ordinal"))?;
            let mut rows = [Vec::new(), Vec::new()];
            for (phase, row) in rows.iter_mut().enumerate() {
                let mut words = Vec::new();
                words
                    .try_reserve_exact(variables)
                    .map_err(|_| invalid("row allocation"))?;
                for _ in 0..variables {
                    words.push(input.word()?);
                }
                *row = dive_core::store::snapshot::row(&store, &words, phase == 0, proceed)?;
            }
            let [canonical, original] = rows;
            discoveries.push(Discovery {
                key,
                parent: (parent != u64::MAX).then_some(parent),
                ordinal,
                canonical,
                original,
            });
        }
        let n = input.count(8, MAX_STATES)?;
        let mut frontier = Vec::new();
        frontier
            .try_reserve_exact(n)
            .map_err(|_| invalid("frontier allocation"))?;
        for _ in 0..n {
            frontier.push(input.word()?);
        }
        let n = input.count(16, MAX_STATES)?;
        let mut warnings = Vec::new();
        warnings
            .try_reserve_exact(n)
            .map_err(|_| invalid("warning allocation"))?;
        for _ in 0..n {
            warnings.push((
                u16::try_from(input.word()?).map_err(|_| invalid("warning variable"))?,
                u32::try_from(input.word()?).map_err(|_| invalid("warning site"))?,
            ));
        }
        if input.at != bytes.len() {
            return Err(invalid("trailing search image bytes"));
        }
        let snapshot = Self {
            store,
            variables,
            next_level,
            stats,
            discoveries,
            frontier,
            warnings,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
}

struct Output(Vec<u8>);
impl Output {
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let end = self
            .0
            .len()
            .checked_add(bytes.len())
            .filter(|&n| n <= MAX_BYTES)
            .ok_or_else(|| invalid("search image byte limit"))?;
        if self.0.capacity() < end {
            let target = end.max(self.0.capacity().saturating_mul(2)).min(MAX_BYTES);
            self.0
                .try_reserve_exact(target - self.0.len())
                .map_err(|_| invalid("search image allocation"))?;
        }
        self.0.extend_from_slice(bytes);
        Ok(())
    }
    fn word(&mut self, n: u64) -> Result<(), Error> {
        self.bytes(&n.to_le_bytes())
    }
}
struct Input<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Input<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&n| n <= self.bytes.len())
            .ok_or_else(|| invalid("truncated search image"))?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }
    fn word(&mut self) -> Result<u64, Error> {
        let mut a = [0; 8];
        a.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(a))
    }
    fn count(&mut self, unit: usize, max: usize) -> Result<usize, Error> {
        let n = usize::try_from(self.word()?).map_err(|_| invalid("count address space"))?;
        if n > max
            || n.checked_mul(unit)
                .is_none_or(|n| n > self.bytes.len().saturating_sub(self.at))
        {
            return Err(invalid("declared count bound"));
        }
        Ok(n)
    }
}

/// Optional native checkpoint transaction. Ordinary exploration does not create it.
#[derive(Debug)]
pub struct Session {
    pub(super) pause_after: Option<u64>,
    pub(super) imported: Option<Snapshot>,
    pub(super) retain_counters: bool,
    pub(super) discoveries: Vec<Discovery>,
    pub(super) paused: Option<Snapshot>,
    pub(super) retained_bytes: usize,
    pub(super) owner: Option<Arc<Store>>,
    pub(super) started: bool,
    restored: bool,
}
impl Session {
    /// Pauses after `levels` complete source levels; zero pauses after Init.
    ///
    /// # Errors
    /// A level beyond the existing bounded native archive policy.
    pub fn pause(levels: u64) -> Result<Self, Error> {
        if levels >= MAX_LEVEL {
            return Err(invalid("pause level bound"));
        }
        Ok(Self {
            pause_after: Some(levels),
            imported: None,
            retain_counters: true,
            discoveries: Vec::new(),
            paused: None,
            retained_bytes: 0,
            owner: None,
            started: false,
            restored: false,
        })
    }
    /// Continues an actual saved frontier; `retain_counters=false` uses TLC's reset.
    #[must_use]
    pub fn recover(snapshot: Snapshot, retain_counters: bool) -> Self {
        Self {
            pause_after: None,
            imported: Some(snapshot),
            retain_counters,
            discoveries: Vec::new(),
            paused: None,
            retained_bytes: 0,
            owner: None,
            started: false,
            restored: false,
        }
    }
    /// Whether all imported membership, predecessors and frontier were installed.
    #[must_use]
    pub const fn recovered(&self) -> bool {
        self.restored
    }

    /// The actual committed-level pause, never a property verdict.
    #[must_use]
    pub fn paused(&self) -> Option<&Snapshot> {
        self.paused.as_ref()
    }

    pub(super) fn record(
        &mut self,
        heap: &mut Heap,
        values: &[Handle],
        original: &[Handle],
        key: u64,
        parent: Option<u64>,
        ordinal: u32,
    ) -> Result<(), Error> {
        if self.pause_after.is_none() {
            return Ok(());
        }
        let bytes = values
            .len()
            .checked_mul(16)
            .and_then(|n| n.checked_add(48))
            .ok_or_else(|| invalid("discovery address space"))?;
        if self.discoveries.len() >= MAX_STATES
            || self
                .retained_bytes
                .checked_add(bytes)
                .is_none_or(|n| n > MAX_BYTES / 2)
        {
            return Err(invalid("checkpoint discovery admission bound"));
        }
        let mut canonical = Vec::new();
        canonical
            .try_reserve_exact(values.len())
            .map_err(|_| invalid("canonical row allocation"))?;
        for &h in values {
            canonical.push(heap.intern(h)?);
        }
        let mut kept = Vec::new();
        kept.try_reserve_exact(original.len())
            .map_err(|_| invalid("original row allocation"))?;
        for &h in original {
            kept.push(heap.intern_exact(h)?);
        }
        self.discoveries
            .try_reserve(1)
            .map_err(|_| invalid("discovery index allocation"))?;
        self.discoveries.push(Discovery {
            key,
            canonical,
            original: kept,
            parent,
            ordinal,
        });
        self.retained_bytes += bytes;
        Ok(())
    }
}

pub(super) fn restore(
    ex: &mut super::Explorer<'_>,
    m: &mut dive_core::Machine,
) -> Result<bool, Error> {
    let Some(session) = ex.checkpoint.as_deref_mut() else {
        return Ok(false);
    };
    if session.started {
        return Err(invalid("checkpoint session reused"));
    }
    session.started = true;
    let owner = Arc::clone(m.heap.store());
    // Recovery-only sessions retain no expanded payload owner after installation.
    session.owner = session.pause_after.map(|_| Arc::clone(&owner));
    let Some(snapshot) = session.imported.take() else {
        return Ok(false);
    };
    let retain = session.retain_counters;
    if !Arc::ptr_eq(&snapshot.store, &owner) || snapshot.variables != ex.plan.vars.len() {
        return Err(invalid("restored Store owner/plan width"));
    }
    snapshot.validate()?;
    for (id, d) in snapshot.discoveries.iter().enumerate() {
        if id.is_multiple_of(128) && (ex.poll)() {
            return Err(Error::stop());
        }
        m.s1.copy_from_slice(&d.original);
        let mut identity = Vec::new();
        let key = super::state_identity(m, ex.plan, ex.seen.exact.as_ref().map(|_| &mut identity))?;
        if key != d.key
            || d.original
                .iter()
                .zip(&d.canonical)
                .any(|(&a, &b)| !m.heap.eq(a, b))
        {
            return Err(invalid("restored discovery identity"));
        }
        ex.cur_id = d.parent;
        if !ex.seen_insert(
            key,
            &identity,
            &m.heap,
            ("RecoveryInsertion", Some(u64::from(d.ordinal))),
        )? {
            return Err(invalid("duplicate restored discovery key"));
        }
        let stored = match d.parent {
            None => ex.log.push_initial(d.ordinal),
            Some(parent) => ex.log.push(parent, d.ordinal),
        }
        .map_err(|e| Error::internal(format!("predecessor log: {e}")))?;
        if stored != id as u64 {
            return Err(invalid("restored predecessor coordinate"));
        }
        // Imported identities and rows are persistent. Do not retain VIEW or
        // symmetry evaluation scratch across the full discovery stream.
        m.heap.reset();
    }
    for &id in &snapshot.frontier {
        let d = &snapshot.discoveries
            [usize::try_from(id).map_err(|_| invalid("frontier address space"))?];
        if let Some(frontier) = &mut ex.frontier {
            frontier.push(id, &d.canonical, &d.original)?;
        } else {
            super::capture_reserve(&mut ex.next, d.canonical.len())?;
            ex.next.extend_from_slice(&d.canonical);
            super::capture_reserve(&mut ex.next_originals, d.original.len())?;
            ex.next_originals.extend_from_slice(&d.original);
            super::capture_reserve(&mut ex.next_ids, 1)?;
            ex.next_ids.push(id);
        }
    }
    ex.stats = snapshot.stats;
    ex.stats.left_on_queue = 0;
    if !retain {
        ex.stats.generated = ex.stats.distinct;
        ex.stats.initial_generated = ex.stats.distinct;
    }
    ex.level = snapshot.next_level;
    ex.initial = false;
    ex.cur_id = None;
    ex.warnings = snapshot.warnings;
    m.warnings = ex.warnings.clone();
    m.heap.reset();
    if let Some(session) = ex.checkpoint.as_deref_mut() {
        session.restored = true;
    }
    Ok(true)
}

pub(super) fn boundary(ex: &mut super::Explorer<'_>) -> Result<(), Error> {
    let queued = ex.queued_states();
    let Some(session) = ex.checkpoint.as_deref_mut() else {
        return Ok(());
    };
    if queued == 0 || session.pause_after.is_none_or(|n| ex.level <= n) {
        return Ok(());
    }
    if session.discoveries.len() as u64 != ex.stats.distinct || ex.log.len() != ex.stats.distinct {
        return Err(invalid("completed-level discovery/log mismatch"));
    }
    let first = session
        .discoveries
        .len()
        .checked_sub(queued)
        .ok_or_else(|| invalid("completed-level queue count"))?;
    let mut frontier = Vec::new();
    frontier
        .try_reserve_exact(queued)
        .map_err(|_| invalid("boundary frontier allocation"))?;
    frontier.extend((first..session.discoveries.len()).map(|id| id as u64));
    if ex.frontier.is_none() && ex.next_ids != frontier {
        return Err(invalid("completed-level frontier order"));
    }
    ex.stats.left_on_queue = queued as u64;
    let snapshot = Snapshot {
        store: Arc::clone(
            session
                .owner
                .as_ref()
                .ok_or_else(|| invalid("boundary owner missing"))?,
        ),
        variables: ex.plan.vars.len(),
        next_level: ex.level,
        stats: ex.stats,
        discoveries: std::mem::take(&mut session.discoveries),
        frontier,
        warnings: ex.warnings.clone(),
    };
    snapshot.validate()?;
    session.paused = Some(snapshot);
    ex.stop = Some(super::Verdict::CheckpointPaused {
        next_level: ex.level,
    });
    Err(Error::stop())
}
