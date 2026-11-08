//! Opt-in native safety storage. Exact identities are separate from receiver history.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use dive_core::frontier_codec::FrontierCodec;
use dive_core::{Error, Handle, Heap};
use dive_store::spill::{MAX_RECORD_BYTES, OrderedFrontier, SpillError};

/// Storage policies for one native safety search. These do not bound total memory.
#[derive(Clone, Debug, Default)]
pub struct Storage {
    /// Resolve state-key collisions using complete mathematical state identities.
    pub exact: bool,
    /// Resident predecessor-log capacity before transient spill.
    pub predecessor_memory_bytes: Option<usize>,
    /// Parent directory for private predecessor files.
    pub predecessor_directory: Option<PathBuf>,
    /// Resident encoded capacity, independently for current and next frontiers.
    pub frontier_memory_bytes: Option<usize>,
    /// Existing parent directory for private frontier files.
    pub frontier_directory: Option<PathBuf>,
    /// Resident encoded candidate bytes per worker; zero immediately spills.
    /// Accepted Store values and active evaluator roots are outside this budget.
    pub candidate_memory_bytes: Option<usize>,
    /// Parent directory for private transient candidate files.
    pub candidate_directory: Option<PathBuf>,
    /// Completed-level trigger for rebuilding the Store from live roots.
    /// Zero forces every boundary. This is not a hard memory limit.
    pub store_compact_bytes: Option<usize>,
    /// The trigger is the caller's default rather than an explicit request.
    /// Reclamation that cannot run on this search is then skipped and recorded
    /// instead of refusing the search; cancellation still stops it.
    pub store_compact_default: bool,
    /// Opt-in fallible disk membership; runs inline and does not bound values.
    pub seen_disk: Option<DiskOptions>,
}

/// Bounds and owned directory for native disk membership.
#[derive(Clone, Debug, Default)]
pub struct DiskOptions {
    /// Existing disk-key workspace, entry and probe bounds.
    pub limits: dive_store::seen::Limits,
    /// Existing parent for private transient files; defaults to system temporary storage.
    pub directory: Option<PathBuf>,
}

/// Ordered position at which disk membership first failed.
#[derive(Clone, Debug)]
pub struct DiskFailure {
    /// Membership, insertion, replay or cleanup phase.
    pub phase: &'static str,
    /// First library failure, including a later cleanup failure.
    pub error: dive_store::seen::SeenError,
    /// Generated count at the original operation.
    pub generated: u64,
    /// Concrete source discovery ID, absent for Init.
    pub predecessor: Option<u64>,
    /// Emission ordinal within the source's ordered successors.
    pub successor_ordinal: u64,
}

/// Actual disk-key storage facts. None of these measure retained state values.
#[derive(Clone, Debug)]
pub struct DiskUsage {
    /// Resolved key and index bounds.
    pub limits: dive_store::seen::Limits,
    /// Successfully committed physical keys, including error-replay admissions.
    pub admitted_keys: u64,
    /// Facts before explicit cleanup.
    pub before_cleanup: Option<dive_store::seen::Stats>,
    /// Facts after cleanup, even when an original cause remains authoritative.
    pub after_cleanup: Option<dive_store::seen::Stats>,
    /// First failed operation and its original ordered position.
    pub first_failure: Option<DiskFailure>,
    /// Search event reached before a later audit or cleanup failure.
    pub deciding_event_before_failure: Option<crate::Verdict>,
    /// A complete retained-record audit finished under the actual run owner.
    pub audit_completed: bool,
}

pub(crate) struct Disk {
    set: dive_store::seen::DiskSeen,
    pub(crate) usage: DiskUsage,
}

impl Disk {
    pub(crate) fn new(
        exact: bool,
        options: &DiskOptions,
    ) -> Result<Self, dive_store::seen::SeenError> {
        let mode = if exact {
            dive_store::StoreMode::Exact
        } else {
            dive_store::StoreMode::Fingerprint
        };
        let parent = options.directory.clone().unwrap_or_else(std::env::temp_dir);
        Ok(Self {
            set: dive_store::seen::DiskSeen::create(mode, &parent, options.limits)?,
            usage: DiskUsage {
                limits: options.limits,
                admitted_keys: 0,
                before_cleanup: None,
                after_cleanup: None,
                first_failure: None,
                deciding_event_before_failure: None,
                audit_completed: false,
            },
        })
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "one positioned fallible membership operation over an actual canonical row"
    )]
    pub(crate) fn operation(
        &mut self,
        exact: bool,
        key: u64,
        row: &[Handle],
        heap: &Heap,
        insert: bool,
        position: (&'static str, u64, Option<u64>, u64),
        poll: &(dyn Fn() -> bool + Sync),
    ) -> Result<bool, Error> {
        use dive_core::key::KeyError;
        use dive_store::seen::{InsertOutcome, Key, Operation, SeenError};
        let bytes = if exact {
            match dive_core::key::canonical(heap, row, self.usage.limits.key_bytes, &|| !poll()) {
                Ok(bytes) => bytes,
                Err(KeyError::Interrupted) => return Err(Error::stop()),
                Err(KeyError::Unsupported(feature)) => return Err(Error::unsupported(feature)),
                Err(KeyError::Internal(message)) => return Err(Error::internal(message)),
                Err(KeyError::Limit(message)) => {
                    let error = SeenError {
                        operation: Operation::Admission,
                        message,
                        cleanup_failure: None,
                    };
                    self.record(position, error.clone());
                    return Err(Error::internal(error.to_string()));
                }
            }
        } else {
            Vec::new()
        };
        let key = if exact {
            Key::Exact(&bytes)
        } else {
            Key::Fingerprint(super::Seen::fix(key))
        };
        let result = if insert {
            self.set
                .insert(key)
                .map(|outcome| outcome == InsertOutcome::Inserted)
        } else {
            self.set.contains(key)
        };
        self.usage.admitted_keys = self.set.stats().physical_entries;
        result.map_err(|error| {
            self.record(position, error.clone());
            Error::internal(error.to_string())
        })
    }

    fn record(
        &mut self,
        position: (&'static str, u64, Option<u64>, u64),
        error: dive_store::seen::SeenError,
    ) {
        self.usage.first_failure.get_or_insert(DiskFailure {
            phase: position.0,
            error,
            generated: position.1,
            predecessor: position.2,
            successor_ordinal: position.3,
        });
    }

    pub(crate) fn finish(
        &mut self,
        validate: bool,
        poll: &(dyn Fn() -> bool + Sync),
        deciding: crate::Verdict,
    ) -> bool {
        self.usage.before_cleanup = Some(self.set.stats());
        let result = self.set.close_controlled(validate, &|| !poll());
        self.usage.after_cleanup = Some(self.set.stats());
        match result {
            Ok((_, complete)) => {
                self.usage.audit_completed = complete;
                !validate || complete
            }
            Err(error) => {
                self.usage.deciding_event_before_failure = Some(deciding);
                if let Some(first) = &mut self.usage.first_failure {
                    if first.error.operation == error.operation
                        && first.error.message == error.message
                    {
                        first.error.cleanup_failure = first
                            .error
                            .cleanup_failure
                            .clone()
                            .or(error.cleanup_failure);
                    } else {
                        first
                            .error
                            .cleanup_failure
                            .get_or_insert_with(|| error.to_string());
                    }
                } else {
                    self.record(("Cleanup", 0, None, 0), error);
                }
                false
            }
        }
    }
}

/// Actual native safety storage facts. These describe components, not RSS.
#[derive(Clone, Debug, Default)]
pub struct Usage {
    /// Completed-level persistent-value reclamation, independently of RSS.
    pub reclamation: ReclamationUsage,
    /// Encoded candidate payload and occurrence storage, not total memory.
    pub candidates: CandidateUsage,
    /// Opt-in disk membership and its first positioned failure.
    pub seen: Option<DiskUsage>,
    /// Exact mathematical identities retained by the collision index.
    pub exact_states: usize,
    /// Predecessor storage before explicit cleanup.
    pub predecessor: dive_store::predecessor::Usage,
    /// Actual predecessor storage failures.
    pub predecessor_failures: Vec<String>,
    /// Search event reached before predecessor storage prevented certification.
    pub predecessor_deciding_event: Option<String>,
    /// Explicit predecessor cleanup finished successfully.
    pub predecessor_cleanup_completed: bool,
    /// Actual encoded frontier component facts; decoded chunks and Store excluded.
    pub frontier: FrontierUsage,
}

/// Persistent-value reclamation facts; live roots and fixed startup remain mandatory.
#[derive(Clone, Debug, Default)]
pub struct ReclamationUsage {
    /// Actual reclamation/key workspace allocation failures.
    pub failures: Vec<String>,
    /// Requested measured-Store-byte trigger; zero forces every boundary.
    pub trigger_bytes: Option<usize>,
    /// The trigger came from the default policy rather than a request.
    pub default_trigger: bool,
    /// Why a default trigger stopped reclaiming; the search continued with the
    /// resident Store unchanged.
    pub skipped: Option<String>,
    /// Completed replacement transactions.
    pub collections: u64,
    /// Cumulative values released from replaced Stores.
    pub reclaimed_nodes: u64,
    /// Largest measured Store snapshot before a completed transaction.
    pub peak_before_bytes: u64,
    /// Largest measured Store snapshot after a completed transaction.
    pub peak_after_bytes: u64,
    /// Largest number of nodes retained after a completed transaction.
    pub peak_after_nodes: u64,
}

/// Actual candidate queue accounting, including occurrence metadata.
#[derive(Clone, Debug, Default)]
pub struct CandidateUsage {
    /// Requested encoded resident bytes per worker's pending source queue.
    pub memory_bytes_per_worker: Option<usize>,
    /// Peak aggregate resident encoded queue capacity across workers.
    pub peak_retained_bytes: u64,
    /// Current resident encoded queue capacity after cleanup.
    pub retained_bytes: u64,
    /// Peak aggregate retained codec constructor-origin index capacity.
    pub peak_origin_index_bytes: u64,
    /// Current retained codec origin-index capacity after cleanup.
    pub origin_index_bytes: u64,
    /// Confirmed complete transient data and index bytes written.
    pub spilled_bytes: u64,
    /// Actual encoding, filesystem, decoding and cleanup failures.
    pub failures: Vec<String>,
    /// Search result reached before a storage failure prevented certification.
    pub deciding_event_before_failure: Option<String>,
}

/// Encoded frontier accounting. This is neither RSS nor a total memory budget.
#[derive(Clone, Debug, Default)]
pub struct FrontierUsage {
    /// Requested resident encoded bytes per queue.
    pub memory_bytes_per_queue: Option<usize>,
    /// Peak combined current/next resident encoded capacities.
    pub peak_retained_bytes: u64,
    /// Resident encoded bytes after explicit cleanup.
    pub retained_bytes: u64,
    /// Confirmed complete writes; failed writes can leave additional partial bytes.
    pub spilled_bytes: u64,
    /// Actual codec, read, write, validation or cleanup failures.
    pub failures: Vec<String>,
    /// Search event reached before storage prevented certification.
    pub deciding_event_before_failure: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct Chunk {
    pub(crate) identity: Vec<Handle>,
    pub(crate) original: Vec<Handle>,
    pub(crate) ids: Vec<u64>,
}

/// Two ordered encoded queues sharing a fixed live Store/codec owner.
pub(crate) struct Frontiers {
    codec: FrontierCodec,
    variables: usize,
    memory_bytes: usize,
    directory: PathBuf,
    next: OrderedFrontier,
    current: Option<OrderedFrontier>,
    next_first: Option<u64>,
    current_first: Option<u64>,
    pub(crate) usage: FrontierUsage,
}

impl Frontiers {
    pub(crate) fn new(
        store: &Arc<dive_core::Store>,
        variables: usize,
        options: &Storage,
    ) -> Result<Self, Error> {
        let memory_bytes = options.frontier_memory_bytes.unwrap_or(0);
        if variables
            .checked_mul(16)
            .and_then(|n| n.checked_add(24))
            .is_none_or(|n| n > MAX_RECORD_BYTES)
        {
            return Err(Error::unsupported(
                "native frontier row exceeds the encoded record limit",
            ));
        }
        let directory = options
            .frontier_directory
            .clone()
            .unwrap_or_else(std::env::temp_dir);
        Ok(Self {
            codec: FrontierCodec::new(Arc::clone(store), variables)?,
            variables,
            memory_bytes,
            next: OrderedFrontier::new(memory_bytes, &directory),
            directory,
            current: None,
            next_first: None,
            current_first: None,
            usage: FrontierUsage {
                memory_bytes_per_queue: Some(memory_bytes),
                ..FrontierUsage::default()
            },
        })
    }

    pub(crate) fn next_len(&self) -> usize {
        self.next.len()
    }

    pub(crate) fn current_len(&self) -> usize {
        self.current.as_ref().map_or(0, OrderedFrontier::len)
    }

    /// Streams the live next queue into a fresh Store-bound codec. Old sources
    /// are audited and closed before their owner is released; readers remain
    /// bounded to one paired row independently of the full frontier width.
    pub(crate) fn remap(
        &mut self,
        remap: &mut dive_core::store::compact::Remap,
        poll: &(dyn Fn() -> bool + Sync),
    ) -> Result<Self, Error> {
        let mut fresh = Self::new(
            remap.store(),
            self.variables,
            &Storage {
                frontier_memory_bytes: Some(self.memory_bytes),
                frontier_directory: Some(self.directory.clone()),
                ..Storage::default()
            },
        )?;
        let result = (|| {
            for i in 0..self.next.len() {
                if i.is_multiple_of(128) && poll() {
                    return Err(Error::stop());
                }
                let read = self.next.read(i);
                let (id, bytes) = self.observe(read)?;
                if self.next_first.and_then(|n| n.checked_add(i as u64)) != Some(id) {
                    return self.observe(Err(frontier_error("remap discovery position")));
                }
                let read = self.codec.decode(&bytes).map_err(frontier_error);
                let mut rows = self.observe(read)?;
                for h in rows.identity.iter_mut().chain(&mut rows.original) {
                    *h = remap.copy(*h, &|| !poll())?;
                }
                fresh.push(id, &rows.identity, &rows.original)?;
            }
            if !self.finish(true, poll) {
                if self.usage.failures.is_empty() {
                    return Err(Error::stop());
                }
                return Err(Error::internal(self.usage.failures.join("; ")));
            }
            Ok(())
        })();
        if let Err(error) = result {
            fresh.finish(false, poll);
            self.usage.spilled_bytes += fresh.usage.spilled_bytes;
            self.usage.peak_retained_bytes = self
                .usage
                .peak_retained_bytes
                .max(fresh.usage.peak_retained_bytes);
            self.usage.failures.extend(fresh.usage.failures);
            return Err(error);
        }
        fresh.usage.spilled_bytes += self.usage.spilled_bytes;
        fresh.usage.peak_retained_bytes = fresh
            .usage
            .peak_retained_bytes
            .max(self.usage.peak_retained_bytes);
        fresh.usage.failures.append(&mut self.usage.failures);
        Ok(fresh)
    }

    fn observe<T>(&mut self, result: Result<T, SpillError>) -> Result<T, Error> {
        result.map_err(|error| {
            self.usage.failures.push(error.to_string());
            Error::internal(error.to_string())
        })
    }

    fn refresh(&mut self) {
        let bytes = self.next.stats().retained_bytes.saturating_add(
            self.current
                .as_ref()
                .map_or(0, |q| q.stats().retained_bytes),
        );
        self.usage.retained_bytes = bytes;
        self.usage.peak_retained_bytes = self.usage.peak_retained_bytes.max(bytes);
    }

    pub(crate) fn push(
        &mut self,
        id: u64,
        identity: &[Handle],
        original: &[Handle],
    ) -> Result<(), Error> {
        let before = self.next.stats().spilled_bytes;
        let result = (|| {
            if self
                .next_first
                .is_some_and(|first| first.checked_add(self.next.len() as u64) != Some(id))
            {
                return Err(frontier_error("discovery IDs are not consecutive"));
            }
            let bytes = self
                .codec
                .encode(identity, original)
                .map_err(frontier_error)?;
            self.next.push(id, &bytes)
        })();
        self.next_first.get_or_insert(id);
        self.usage.spilled_bytes = self
            .usage
            .spilled_bytes
            .saturating_add(self.next.stats().spilled_bytes.saturating_sub(before));
        self.refresh();
        self.observe(result)
    }

    fn validate(
        &self,
        queue: &OrderedFrontier,
        first: Option<u64>,
        poll: &(dyn Fn() -> bool + Sync),
    ) -> Result<bool, SpillError> {
        for index in 0..queue.len() {
            if index % 128 == 0 && poll() {
                return Ok(false);
            }
            let (id, bytes) = queue.read(index)?;
            if first.and_then(|n| n.checked_add(index as u64)) != Some(id) {
                return Err(frontier_error(
                    "record discovery ID differs from its ordered position",
                ));
            }
            self.codec.decode(&bytes).map_err(frontier_error)?;
        }
        Ok(true)
    }

    fn close_queue(
        &mut self,
        queue: &mut OrderedFrontier,
        first: Option<u64>,
        validate: bool,
        poll: &(dyn Fn() -> bool + Sync),
    ) -> Result<bool, Error> {
        let checked = if validate {
            self.validate(queue, first, poll)
        } else {
            Ok(true)
        };
        let closed = queue.close();
        let check = self.observe(checked);
        let close = self.observe(closed);
        check.and_then(|complete| close.map(|()| complete))
    }

    pub(crate) fn advance(&mut self, poll: &(dyn Fn() -> bool + Sync)) -> Result<bool, Error> {
        if let Some(mut old) = self.current.take()
            && !self.close_queue(&mut old, self.current_first, true, poll)?
        {
            self.refresh();
            return Ok(false);
        }
        let fresh = OrderedFrontier::new(self.memory_bytes, &self.directory);
        self.current = Some(std::mem::replace(&mut self.next, fresh));
        self.current_first = self.next_first.take();
        self.refresh();
        Ok(true)
    }

    pub(crate) fn chunk(
        &mut self,
        start: usize,
        end: usize,
        poll: &(dyn Fn() -> bool + Sync),
    ) -> Result<Option<Chunk>, Error> {
        let result = (|| {
            let queue = self
                .current
                .as_ref()
                .ok_or_else(|| frontier_error("missing current queue"))?;
            let mut chunk = Chunk::default();
            let handles = end
                .checked_sub(start)
                .and_then(|n| n.checked_mul(self.variables))
                .ok_or_else(|| frontier_error("decoded chunk size overflow"))?;
            chunk
                .identity
                .try_reserve_exact(handles)
                .map_err(frontier_error)?;
            chunk
                .original
                .try_reserve_exact(handles)
                .map_err(frontier_error)?;
            chunk
                .ids
                .try_reserve_exact(end - start)
                .map_err(frontier_error)?;
            for index in start..end {
                if (index - start).is_multiple_of(128) && poll() {
                    return Ok(None);
                }
                let (id, bytes) = queue.read(index)?;
                if self.current_first.and_then(|n| n.checked_add(index as u64)) != Some(id) {
                    return Err(frontier_error(
                        "record discovery ID differs from its ordered position",
                    ));
                }
                let rows = self.codec.decode(&bytes).map_err(frontier_error)?;
                chunk.identity.extend(rows.identity);
                chunk.original.extend(rows.original);
                chunk.ids.push(id);
            }
            Ok(Some(chunk))
        })();
        self.observe(result)
    }

    pub(crate) fn finish(&mut self, validate: bool, poll: &(dyn Fn() -> bool + Sync)) -> bool {
        let mut complete = true;
        if let Some(mut current) = self.current.take() {
            complete &= self
                .close_queue(&mut current, self.current_first, validate, poll)
                .unwrap_or(false);
        }
        let mut next = std::mem::replace(
            &mut self.next,
            OrderedFrontier::new(self.memory_bytes, &self.directory),
        );
        complete &= self
            .close_queue(&mut next, self.next_first, validate && complete, poll)
            .unwrap_or(false);
        self.refresh();
        complete
    }
}

fn frontier_error(cause: impl std::fmt::Display) -> SpillError {
    SpillError {
        code: "DIVE-S0200",
        message: format!("native frontier: {cause}"),
    }
}

/// Retained canonical rows, bucketed by the existing state hash. Hashes select
/// candidates only; even a complete collision cannot discard a distinct state.
#[derive(Debug, Default)]
pub(crate) struct Exact {
    rows: HashMap<u64, Vec<Vec<Handle>>>,
    encoded: Option<HashMap<u64, Vec<Vec<u8>>>>,
    len: usize,
}

impl Exact {
    pub(crate) fn detached() -> Self {
        Self {
            encoded: Some(HashMap::new()),
            ..Self::default()
        }
    }

    pub(crate) const fn is_detached(&self) -> bool {
        self.encoded.is_some()
    }

    pub(crate) fn contains_controlled(
        &self,
        key: u64,
        row: &[Handle],
        heap: &Heap,
        poll: &(dyn Fn() -> bool + Sync),
    ) -> Result<bool, Error> {
        let Some(encoded) = &self.encoded else {
            return Ok(self.contains(key, row, heap));
        };
        let bytes = detached_key(heap, row, poll)?;
        Ok(encoded
            .get(&key)
            .is_some_and(|bucket| bucket.contains(&bytes)))
    }

    pub(crate) fn insert_controlled(
        &mut self,
        key: u64,
        row: &[Handle],
        heap: &Heap,
        poll: &(dyn Fn() -> bool + Sync),
    ) -> Result<bool, Error> {
        let Some(encoded) = &mut self.encoded else {
            return self.insert(key, row, heap);
        };
        let bytes = detached_key(heap, row, poll)?;
        if encoded
            .get(&key)
            .is_some_and(|bucket| bucket.contains(&bytes))
        {
            return Ok(false);
        }
        encoded
            .try_reserve(1)
            .map_err(|_| Error::internal("DIVE-S0203: detached exact state index allocation"))?;
        let bucket = encoded.entry(key).or_default();
        bucket.try_reserve(1).map_err(|_| {
            Error::internal("DIVE-S0203: detached exact collision bucket allocation")
        })?;
        bucket.push(bytes);
        self.len += 1;
        Ok(true)
    }

    pub(crate) fn contains_encoded(
        &self,
        key: u64,
        bytes: &[u8],
        heap: &Heap,
        limit: usize,
        poll: &(dyn Fn() -> bool + Sync),
    ) -> Result<bool, Error> {
        let Some(rows) = self.rows.get(&key) else {
            return Ok(false);
        };
        for row in rows {
            let encoded =
                dive_core::key::canonical(heap, row, limit, &|| !poll()).map_err(|error| {
                    if error == dive_core::key::KeyError::Interrupted {
                        Error::stop()
                    } else {
                        Error::internal(format!("DIVE-S0500: distributed exact key: {error:?}"))
                    }
                })?;
            if encoded == bytes {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub(crate) const fn len(&self) -> usize {
        self.len
    }
    pub(crate) fn contains(&self, key: u64, row: &[Handle], heap: &Heap) -> bool {
        self.rows.get(&key).is_some_and(|bucket| {
            bucket.iter().any(|kept| {
                kept.len() == row.len() && kept.iter().zip(row).all(|(&a, &b)| heap.eq(a, b))
            })
        })
    }

    pub(crate) fn insert(&mut self, key: u64, row: &[Handle], heap: &Heap) -> Result<bool, Error> {
        if self.contains(key, row, heap) {
            return Ok(false);
        }
        self.rows
            .try_reserve(1)
            .map_err(|_| Error::unsupported("exact state index allocation"))?;
        let mut kept = Vec::new();
        kept.try_reserve_exact(row.len())
            .map_err(|_| Error::unsupported("exact state row allocation"))?;
        kept.extend_from_slice(row);
        let bucket = self.rows.entry(key).or_default();
        bucket
            .try_reserve(1)
            .map_err(|_| Error::unsupported("exact state collision bucket allocation"))?;
        bucket.push(kept);
        self.len += 1;
        Ok(true)
    }
}

fn detached_key(
    heap: &Heap,
    row: &[Handle],
    poll: &(dyn Fn() -> bool + Sync),
) -> Result<Vec<u8>, Error> {
    use dive_core::key::KeyError;
    dive_core::key::canonical(heap, row, usize::MAX, &|| !poll()).map_err(|error| match error {
        KeyError::Interrupted => Error::stop(),
        KeyError::Internal(message) => Error::internal(message),
        KeyError::Unsupported(message) => Error::unsupported(message),
        KeyError::Limit(message) => Error::internal(format!("DIVE-S0203: {message}")),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "exact collision controls")]
mod tests {
    use std::fs;
    use std::io::{Seek, SeekFrom, Write};
    use std::sync::Arc;

    use dive_core::Store;

    use super::*;

    fn directory() -> PathBuf {
        static NEXT_DIRECTORY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "dive-native-frontier-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    fn queues(path: &std::path::Path) -> Frontiers {
        let mut queues = Frontiers::new(
            &Arc::new(Store::new(true)),
            1,
            &Storage {
                frontier_memory_bytes: Some(0),
                frontier_directory: Some(path.to_path_buf()),
                ..Storage::default()
            },
        )
        .unwrap();
        for id in 0_u32..3 {
            let value = Handle::int(i64::from(id)).unwrap();
            queues.push(u64::from(id), &[value], &[value]).unwrap();
        }
        queues
    }

    #[test]
    fn remapping_cancels_and_reports_corruption_write_and_cleanup_failures() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        for fault in ["cancel", "corrupt", "write", "cleanup"] {
            let path = directory();
            let store = Arc::new(Store::new(true));
            let permanent =
                dive_core::store::compact::Permanent::capture(&store, &|| true).unwrap();
            let mut remap = permanent.begin(&store, &|| true).unwrap();
            let mut queues = Frontiers::new(
                &store,
                1,
                &Storage {
                    frontier_memory_bytes: Some(0),
                    frontier_directory: Some(path.clone()),
                    ..Storage::default()
                },
            )
            .unwrap();
            for id in 0_u32..4 {
                let h = Handle::int(i64::from(id)).unwrap();
                queues.push(u64::from(id), &[h], &[h]).unwrap();
            }
            let owned = queues.next.disk_directory().unwrap().to_path_buf();
            if fault == "write" {
                let blocked = path.join("file");
                fs::write(&blocked, b"not a directory").unwrap();
                queues.directory = blocked;
            }
            let calls = AtomicUsize::new(0);
            let poll = || {
                // The first row has already been copied and written when the
                // next root polls. Mutations therefore exercise partial output.
                if calls.fetch_add(1, Ordering::Relaxed) == 3 {
                    match fault {
                        "cancel" => return true,
                        "corrupt" => {
                            let mut file = fs::OpenOptions::new()
                                .write(true)
                                .open(owned.join("states"))
                                .unwrap();
                            file.seek(SeekFrom::End(-1)).unwrap();
                            file.write_all(&[255]).unwrap();
                        }
                        "cleanup" => {
                            fs::rename(&owned, path.join("moved")).unwrap();
                            fs::write(&owned, b"blocks cleanup").unwrap();
                        }
                        _ => {}
                    }
                }
                false
            };
            let result = queues.remap(&mut remap, &poll);
            assert!(result.is_err(), "{fault}");
            if fault == "cancel" {
                assert_eq!(result.err().unwrap().kind(), dive_core::ErrorKind::Stop);
                assert_eq!(queues.usage.failures, [] as [String; 0]);
            } else {
                assert!(!queues.usage.failures.is_empty(), "{fault}");
            }
            queues.finish(false, &|| false);
            if fault == "cleanup" {
                assert!(queues.usage.failures.iter().any(|s| s.contains("cleanup")));
            }
            assert_eq!(queues.usage.retained_bytes, 0);
            fs::remove_dir_all(path).unwrap();
        }
    }

    #[test]
    fn detached_exact_keys_survive_replacement_and_resolve_complete_collisions() {
        let store = Arc::new(Store::new(true));
        let permanent = dive_core::store::compact::Permanent::capture(&store, &|| true).unwrap();
        let mut heap = Heap::new(Arc::clone(&store), true, false);
        let one = Handle::int(1).unwrap();
        let two = Handle::int(2).unwrap();
        let a = heap.tuple_literal(&[one, two]);
        let a = heap.intern(a).unwrap();
        let b = heap.tuple_literal(&[two, one]);
        let b = heap.intern(b).unwrap();
        let mut exact = Exact::detached();
        assert!(exact.insert_controlled(0, &[a], &heap, &|| false).unwrap());
        assert!(exact.insert_controlled(0, &[b], &heap, &|| false).unwrap());
        let mut remap = permanent.begin(&store, &|| true).unwrap();
        let mapped = remap.copy(b, &|| true).unwrap();
        let replacement = Heap::new(Arc::clone(remap.store()), true, false);
        assert!(
            exact
                .contains_controlled(0, &[mapped], &replacement, &|| false)
                .unwrap()
        );
        assert!(
            !exact
                .insert_controlled(0, &[mapped], &replacement, &|| false)
                .unwrap()
        );
        assert!(
            exact.rows.is_empty(),
            "detached keys must retain no old Store handles"
        );
        assert_eq!(exact.len(), 2);
    }

    #[test]
    fn missing_current_file_and_unvisited_corruption_invalidate_storage() {
        for missing in [false, true] {
            let path = directory();
            let mut queues = queues(&path);
            queues.advance(&|| false).unwrap();
            assert!(queues.chunk(0, 1, &|| false).unwrap().is_some());
            let owned = queues
                .current
                .as_ref()
                .unwrap()
                .disk_directory()
                .unwrap()
                .to_path_buf();
            if missing {
                fs::remove_file(owned.join("states")).unwrap();
                assert!(queues.chunk(1, 2, &|| false).is_err());
            } else {
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .open(owned.join("states"))
                    .unwrap();
                file.seek(SeekFrom::End(-1)).unwrap();
                file.write_all(&[255]).unwrap();
            }
            queues.finish(true, &|| false);
            assert_ne!(queues.usage.failures, [] as [String; 0]);
            assert!(!owned.exists());
            assert_eq!(queues.usage.retained_bytes, 0);
            fs::remove_dir(path).unwrap();
        }
    }

    #[test]
    fn already_stopped_cleanup_does_not_read_or_poll_unprocessed_records() {
        let path = directory();
        let mut queues = queues(&path);
        queues.advance(&|| false).unwrap();
        let owned = queues
            .current
            .as_ref()
            .unwrap()
            .disk_directory()
            .unwrap()
            .to_path_buf();
        fs::remove_file(owned.join("states")).unwrap();
        queues.finish(false, &|| true);
        assert_eq!(queues.usage.failures, [] as [String; 0]);
        assert!(!owned.exists());
        fs::remove_dir(path).unwrap();
    }

    #[test]
    fn cleanup_failure_is_explicit_even_without_a_certifiable_ending() {
        let path = directory();
        let mut queues = queues(&path);
        let owned = queues.next.disk_directory().unwrap().to_path_buf();
        let moved = path.join("moved");
        fs::rename(&owned, &moved).unwrap();
        fs::write(&owned, b"file blocks directory cleanup").unwrap();
        queues.finish(false, &|| false);
        assert!(
            queues
                .usage
                .failures
                .iter()
                .any(|error| error.contains("cleanup"))
        );
        fs::remove_file(owned).unwrap();
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn complete_hash_collision_preserves_distinct_rows_and_zero_key() {
        let mut heap = Heap::new(Arc::new(Store::new(true)), true, false);
        let mut seen = Exact::default();
        let one = Handle::int(1).unwrap();
        let two = Handle::int(2).unwrap();
        let a = heap.tuple(&[one, two]);
        let a = heap.intern(a).unwrap();
        let b = heap.tuple(&[two, one]);
        let b = heap.intern(b).unwrap();
        assert!(seen.insert(0, &[a], &heap).unwrap());
        assert!(seen.insert(0, &[b], &heap).unwrap());
        assert!(!seen.insert(0, &[a], &heap).unwrap());
        assert!(!seen.contains(1, &[a], &heap));
        assert_eq!(seen.len, 2);
    }

    #[test]
    fn exact_equality_ignores_literal_class_and_hashconsing_without_erasing_views() {
        for hashcons in [false, true] {
            let mut heap = Heap::new(Arc::new(Store::new(hashcons)), true, false);
            let one = Handle::int(1).unwrap();
            let two = Handle::int(2).unwrap();
            let tuple = heap.tuple_literal(&[one, two]);
            let original = heap.intern_exact(tuple).unwrap();
            let canonical_tuple = heap.intern(original).unwrap();
            let function = heap.fcn_sorted(&[one, two], &[one, two]);
            let canonical_function = heap.intern(function).unwrap();
            let mut seen = Exact::default();
            assert!(seen.insert(7, &[canonical_tuple], &heap).unwrap());
            assert!(!seen.insert(7, &[canonical_function], &heap).unwrap());
            assert!(heap.view(original).is_tuple_literal());
            assert!(!heap.view(canonical_tuple).is_tuple_literal());
            assert!(!heap.view(canonical_function).is_tuple_literal());
        }
    }
}
