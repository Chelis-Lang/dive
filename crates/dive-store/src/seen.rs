//! Fallible, private-file seen-state storage (ADR 0063).
//!
//! This additive library does not implement [`crate::StateSet`]: an I/O error
//! cannot pass through that trait's Boolean membership/insertion interface.
//! No engine currently admits this storage. Entries are physical library commits,
//! not a model checker's ordered discovery-prefix count.
//!
//! Keys and offsets remain on disk. At most two key-sized workspaces, scalar index
//! probes and sequential rehash workspaces are bounded by [`Limits`]. OS page
//! cache, handles, paths and caller-owned keys are outside these bounds.

mod files;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::StoreMode;

static NEXT: AtomicU64 = AtomicU64::new(0);

/// Maximum encoded key admitted by the default library policy: 64 MiB.
pub const MAX_KEY_BYTES: usize = 64 * 1024 * 1024;

/// Deterministic resource bounds, checked before record allocation or index growth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Maximum one-record key buffer. Fingerprint keys always use eight bytes.
    pub key_bytes: usize,
    /// Maximum number of physically committed keys.
    pub entries: u64,
    /// Maximum slots examined by one index lookup, including rehash insertion.
    pub probe_steps: u64,
    /// Maximum pair comparisons for the bounded, quadratic collision summary.
    /// Exceeding this refuses the summary; it never silently omits metadata.
    pub summary_pairs: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            key_bytes: MAX_KEY_BYTES,
            entries: 16 * 1024 * 1024,
            probe_steps: 32 * 1024 * 1024,
            summary_pairs: 1_000_000,
        }
    }
}

/// Borrowed storage identity; exact keys are existing canonical bytes, unchanged.
#[derive(Clone, Copy, Debug)]
pub enum Key<'a> {
    /// Complete 64-bit fingerprint, including the high bit and zero.
    Fingerprint(u64),
    /// Full canonical state encoding. No normalization or provider is invoked.
    Exact(&'a [u8]),
}

/// A successful, fully committed insertion.
/// This is an in-process write commit, not crash durability or recovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertOutcome {
    /// The complete key already exists.
    Present,
    /// Record and index entry were both committed.
    Inserted,
}

/// Typed operation at which the first failure occurred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    /// Invalid key mode, resource bound or checked arithmetic.
    Admission,
    /// Creating private owned files.
    Create,
    /// Reading file bytes or metadata.
    Read,
    /// Seeking to a bounded offset.
    Seek,
    /// Writing or flushing a record/index entry.
    Write,
    /// Allocating the bounded record workspace.
    Allocate,
    /// Constructing and committing a larger on-disk index.
    Rehash,
    /// Invalid header, offset, length, identity or checksum.
    Corruption,
    /// Computing the fingerprint-gap summary.
    Summary,
    /// Removing only owned files and their private directory.
    Cleanup,
}

/// A storage failure retained by the poisoned store. Checksums detect accidental
/// corruption; neither these files nor this error are an authenticity certificate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeenError {
    /// First failing operation; subsequent operations return this same cause.
    pub operation: Operation,
    /// Bounded diagnostic context, without a copy of the key.
    pub message: String,
    /// Additional cleanup failure, without replacing the original cause.
    pub cleanup_failure: Option<String>,
}

impl SeenError {
    /// Stable code for disk seen-state failures.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        "DIVE-S0700"
    }
}

impl std::fmt::Display for SeenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {:?}: {}", self.code(), self.operation, self.message)?;
        if let Some(cleanup) = &self.cleanup_failure {
            write!(f, "; cleanup also failed: {cleanup}")?;
        }
        Ok(())
    }
}
impl std::error::Error for SeenError {}

fn error(operation: Operation, cause: impl std::fmt::Display) -> SeenError {
    SeenError {
        operation,
        message: cause.to_string(),
        cleanup_failure: None,
    }
}

/// Library storage facts, independent of semantic search-prefix statistics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    /// Record/index insertions whose full in-process writes completed. Inserts
    /// do not fsync data and this library has no restart/import guarantee.
    pub physical_entries: u64,
    /// Current expected data/index lengths, not cumulative rehash traffic.
    pub disk_bytes: u64,
    /// Zero: there is no resident key/offset collection.
    pub resident_key_bytes: u64,
    /// Whether an operation failed and accounting may exclude partial writes.
    pub poisoned: bool,
    /// Whether explicit removal of all owned files and directory completed.
    pub cleanup_completed: bool,
}

/// Owned transient disk set. Calls are coordinated through exclusive borrowing;
/// this first library slice does not claim parallel engine integration.
///
/// ```
/// use dive_store::{StoreMode, seen::{DiskSeen, InsertOutcome, Key, Limits}};
/// let mut set = DiskSeen::create(StoreMode::Exact, &std::env::temp_dir(), Limits::default())?;
/// assert_eq!(set.insert(Key::Exact(b"canonical-state"))?, InsertOutcome::Inserted);
/// assert_eq!(set.insert(Key::Exact(b"canonical-state"))?, InsertOutcome::Present);
/// assert!(set.contains(Key::Exact(b"canonical-state"))?);
/// assert!(set.close()?.cleanup_completed);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct DiskSeen {
    directory: PathBuf,
    mode: StoreMode,
    limits: Limits,
    capacity: u64,
    count: u64,
    data_end: u64,
    first: Option<SeenError>,
    closed: bool,
    #[cfg(test)]
    fault: Option<(Operation, u64)>,
    #[cfg(test)]
    forced_hash: Option<u64>,
}

impl DiskSeen {
    /// Creates a unique child directory and versioned files, without clobbering
    /// or adopting caller files. The parent must already exist.
    ///
    /// # Errors
    /// Invalid bounds or filesystem creation/write/cleanup failures.
    pub fn create(mode: StoreMode, parent: &Path, limits: Limits) -> Result<Self, SeenError> {
        if limits.key_bytes > MAX_KEY_BYTES
            || limits.key_bytes < 8
            || limits.entries == 0
            || limits.entries > u64::from(u32::MAX)
            || limits.probe_steps == 0
        {
            return Err(error(
                Operation::Admission,
                "invalid disk seen-state limits",
            ));
        }
        let directory = parent.join(format!(
            "dive-seen-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).map_err(|e| error(Operation::Create, e))?;
        let mut set = Self {
            directory,
            mode,
            limits,
            capacity: 8,
            count: 0,
            data_end: files::HEADER,
            first: None,
            closed: false,
            #[cfg(test)]
            fault: None,
            #[cfg(test)]
            forced_hash: None,
        };
        if let Err(mut failure) = set.initialize() {
            if let Err(cleanup) = set.remove_owned() {
                failure.cleanup_failure = Some(cleanup.message);
            }
            return Err(failure);
        }
        Ok(set)
    }

    /// Current physical facts, also available after a failure.
    #[must_use]
    pub fn stats(&self) -> Stats {
        Stats {
            physical_entries: self.count,
            disk_bytes: if self.closed {
                0
            } else {
                self.data_end + files::HEADER + self.capacity * files::SLOT
            },
            resident_key_bytes: 0,
            poisoned: self.first.is_some(),
            cleanup_completed: self.closed,
        }
    }

    /// Tests full identity, never treating an error as absence.
    ///
    /// # Errors
    /// Mode/bounds, file integrity or I/O failure; the first failure poisons future calls.
    pub fn contains(&mut self, key: Key<'_>) -> Result<bool, SeenError> {
        self.available()?;
        let result = self.find_key(key).map(|(_, found)| found);
        self.retain(result)
    }

    /// Commits one complete record and index slot. A failed partial write poisons
    /// the store, never yielding `Inserted` or `Present`.
    /// These are transient in-process writes, without per-insert data fsync or
    /// a crash-recovery contract.
    ///
    /// # Errors
    /// Membership, resource, allocation, write or rehash failure.
    pub fn insert(&mut self, key: Key<'_>) -> Result<InsertOutcome, SeenError> {
        self.available()?;
        let result = self.insert_inner(key);
        self.retain(result)
    }

    /// Fallible bounded-workspace TLC minimum-gap counterpart. This first slice
    /// scans pairs quadratically and refuses beyond `summary_pairs`; it does not
    /// allocate a fingerprint vector or silently remove collision metadata.
    ///
    /// # Errors
    /// Exact mode, policy work limit, corruption or I/O failure.
    pub fn tlc_min_gap(&mut self, fp_bits: u32) -> Result<u64, SeenError> {
        self.available()?;
        let result = self.gap_inner(fp_bits);
        self.retain(result)
    }

    /// Explicitly removes owned files before a caller reports completion.
    /// Unknown caller-added files are never deleted. A prior failure remains
    /// authoritative even when cleanup succeeds, with any cleanup failure attached.
    /// Cleanup may remove some owned files before failing; the poisoned store
    /// cannot be reused, even if later cleanup succeeds.
    ///
    /// # Errors
    /// The first retained failure, or explicit cleanup failure.
    pub fn close(&mut self) -> Result<Stats, SeenError> {
        if !self.closed {
            if self.first.is_none()
                && let Err(failure) = self.audit_inner()
            {
                self.first = Some(failure);
            }
            if let Err(cleanup) = self.remove_owned() {
                if let Some(first) = &mut self.first {
                    first.cleanup_failure.get_or_insert(cleanup.message);
                } else {
                    self.first = Some(cleanup);
                }
            } else {
                self.closed = true;
            }
        }
        self.first.clone().map_or_else(|| Ok(self.stats()), Err)
    }

    fn remove_owned(&mut self) -> Result<(), SeenError> {
        self.check_fault(Operation::Cleanup)?;
        for name in ["data", "index", "index-next"] {
            match fs::remove_file(self.directory.join(name)) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(error(Operation::Cleanup, e)),
            }
        }
        fs::remove_dir(&self.directory).map_err(|e| error(Operation::Cleanup, e))
    }

    fn available(&self) -> Result<(), SeenError> {
        if let Some(first) = &self.first {
            return Err(first.clone());
        }
        if self.closed {
            return Err(error(Operation::Admission, "store is closed"));
        }
        Ok(())
    }

    fn retain<T>(&mut self, result: Result<T, SeenError>) -> Result<T, SeenError> {
        if let Err(failure) = &result {
            self.first = Some(failure.clone());
        }
        result
    }

    #[allow(
        clippy::unused_self,
        clippy::unnecessary_wraps,
        reason = "test-only fault injection uses the production I/O boundaries"
    )]
    fn check_fault(&mut self, operation: Operation) -> Result<(), SeenError> {
        #[cfg(test)]
        if let Some((wanted, remaining)) = &mut self.fault
            && *wanted == operation
        {
            if *remaining == 0 {
                self.fault = None;
                return Err(error(
                    operation,
                    if operation == Operation::Write {
                        std::io::Error::new(
                            std::io::ErrorKind::StorageFull,
                            "injected storage exhaustion",
                        )
                        .to_string()
                    } else {
                        "injected storage failure".to_owned()
                    },
                ));
            }
            *remaining -= 1;
        }
        let _ = operation;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

impl Drop for DiskSeen {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.remove_owned();
        }
    }
}
