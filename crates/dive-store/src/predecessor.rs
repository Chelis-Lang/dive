//! Bounded predecessor records for concrete counterexample replay.
//!
//! Fixed-width transient records have implicit discovery IDs. Once spilled,
//! no growing offset table or record vector stays resident. Reconstruction
//! vectors are separate from the retained-record budget.

mod packed;

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::StoreError;
use crate::trace::{StateId, TraceLog, TraceStep};

const MAGIC: &[u8; 8] = b"DIVEPR01";
const HEADER: u64 = 24;
const RECORD: u64 = 16;
const ROOT: u64 = u64::MAX;
static SERIAL: AtomicU64 = AtomicU64::new(0);
/// Maximum reconstructed path in opt-in bounded-log mode. Each disk read
/// needs only one 16-byte record; the returned path is outside the log budget.
pub const MAX_PATH_STEPS: usize = 1_000_000;

fn error(operation: &str, cause: impl std::fmt::Display) -> StoreError {
    StoreError::Predecessor {
        operation: operation.into(),
        message: cause.to_string(),
    }
}

/// Retained predecessor capacity and confirmed complete disk writes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    /// Current resident record and block-index capacity, excluding reconstruction.
    pub retained_bytes: u64,
    /// Peak resident capacity for records and their block index.
    pub peak_retained_bytes: u64,
    /// Confirmed header/record writes. Failed writes can leave extra bytes.
    pub written_bytes: u64,
    /// Number of complete records admitted.
    pub records: u64,
    /// Whether the log switched to disk.
    pub spilled: bool,
}

#[derive(Debug)]
struct Disk {
    directory: PathBuf,
    file: Mutex<File>,
}

/// Memory reference log, optionally spilled under a separate capacity limit.
/// Call [`Self::close`] before certifying completion. Files are private and
/// transient; this is not a restart/checkpoint format.
#[derive(Debug)]
pub struct PredecessorLog {
    memory: TraceLog,
    packed: Option<packed::Packed>,
    budget: Option<usize>,
    parent: PathBuf,
    disk: Option<Disk>,
    pending_directory: Option<PathBuf>,
    failure: Mutex<Option<StoreError>>,
    usage: Usage,
    poisoned: bool,
    closed: bool,
}

impl PredecessorLog {
    /// Creates an empty log without filesystem access. With an unspecified
    /// budget the resident log uses indexed parent deltas and ordinals.
    #[must_use]
    pub fn new(budget: Option<usize>, directory: Option<&Path>) -> Self {
        Self {
            memory: TraceLog::new(),
            packed: budget.is_none().then(packed::Packed::default),
            budget,
            parent: directory.map_or_else(std::env::temp_dir, Path::to_path_buf),
            disk: None,
            pending_directory: None,
            failure: Mutex::new(None),
            usage: Usage::default(),
            poisoned: false,
            closed: false,
        }
    }

    /// Committed records, in discovery order.
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.usage.records
    }

    /// Whether the log is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Current/peak resident capacity and confirmed complete writes.
    #[must_use]
    pub fn usage(&self) -> Usage {
        Usage {
            retained_bytes: self
                .packed
                .as_ref()
                .map_or_else(|| self.memory.memory_bytes(), packed::Packed::memory_bytes)
                as u64,
            ..self.usage
        }
    }

    /// First typed native storage failure observed. Caller messages are never
    /// interpreted as storage diagnostics. The slot is bounded to one error.
    #[must_use]
    pub fn failure(&self) -> Option<StoreError> {
        self.failure.lock().map_or_else(
            |_| Some(error("failure evidence", "lock poisoned")),
            |failure| failure.clone(),
        )
    }

    fn observe(&self, error: &StoreError) {
        if matches!(error, StoreError::Predecessor { .. })
            && let Ok(mut failure) = self.failure.lock()
            && failure.is_none()
        {
            *failure = Some(error.clone());
        }
    }

    fn ready(&self) -> Result<(), StoreError> {
        if self.poisoned || self.closed {
            Err(error("admission", "log is failed or closed"))
        } else {
            Ok(())
        }
    }

    /// Appends an initial record without changing its enumeration ordinal.
    ///
    /// # Errors
    /// Allocation, filesystem, record-count or closed-log failure.
    pub fn push_initial(&mut self, ordinal: u32) -> Result<StateId, StoreError> {
        self.append(None, ordinal)
            .inspect_err(|error| self.observe(error))
    }

    /// Appends a successor whose parent is strictly earlier.
    ///
    /// # Errors
    /// Invalid parent, allocation, filesystem or closed-log failure.
    pub fn push(&mut self, parent: StateId, ordinal: u32) -> Result<StateId, StoreError> {
        if parent >= self.len() {
            return Err(StoreError::BadParent {
                id: self.len(),
                parent,
            });
        }
        self.append(Some(parent), ordinal)
            .inspect_err(|error| self.observe(error))
    }

    fn append(&mut self, parent: Option<StateId>, ordinal: u32) -> Result<StateId, StoreError> {
        self.ready()?;
        let id = self.len();
        let next = id
            .checked_add(1)
            .filter(|n| *n != ROOT)
            .ok_or_else(|| error("record count", "overflow"))?;
        if let Some(packed) = &mut self.packed {
            packed
                .append(id, parent, ordinal)
                .map_err(|e| error("allocation", e))?;
            self.usage.records = next;
            self.usage.peak_retained_bytes = self
                .usage
                .peak_retained_bytes
                .max(packed.memory_bytes() as u64);
            return Ok(id);
        }
        if self.disk.is_none() {
            if let Some(budget) = self.budget {
                let n = usize::try_from(next).map_err(|e| error("record count", e))?;
                if self
                    .memory
                    .reserved_bytes_for(n)
                    .is_none_or(|size| size > budget)
                {
                    self.spill()?;
                } else {
                    self.memory
                        .reserve_record(true)
                        .map_err(|e| error("allocation", e))?;
                    if self.memory.memory_bytes() > budget {
                        self.spill()?;
                    }
                }
            } else {
                self.memory
                    .reserve_record(false)
                    .map_err(|e| error("allocation", e))?;
            }
        }
        if let Some(disk) = &mut self.disk {
            let record = record(id, parent.unwrap_or(ROOT), ordinal);
            let offset = offset(id)?;
            let result = disk
                .file
                .get_mut()
                .map_err(|e| error("lock", e))
                .and_then(|file| {
                    file.seek(SeekFrom::Start(offset))
                        .and_then(|_| file.write_all(&record))
                        .map_err(|e| error("write record", e))
                });
            if let Err(e) = result {
                self.poisoned = true;
                return Err(e);
            }
            self.usage.written_bytes += RECORD;
        } else {
            match parent {
                Some(parent) => {
                    self.memory.push(parent, ordinal)?;
                }
                None => {
                    self.memory.push_initial(ordinal);
                }
            }
            self.usage.peak_retained_bytes = self
                .usage
                .peak_retained_bytes
                .max(self.memory.memory_bytes() as u64);
        }
        self.usage.records = next;
        Ok(id)
    }

    fn spill(&mut self) -> Result<(), StoreError> {
        let directory = self.parent.join(format!(
            "dive-predecessors-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(&directory)
            .map_err(|e| error("create directory", e))?;
        self.pending_directory = Some(directory.clone());
        // Retain ownership immediately so any partial creation is cleaned.
        let result = (|| {
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(directory.join("records"))
                .map_err(|e| error("create records", e))?;
            file.write_all(&header())
                .map_err(|e| error("write header", e))?;
            self.usage.written_bytes += HEADER;
            for id in 0..self.len() {
                let step = self.memory.get(id).ok_or(StoreError::UnknownState(id))?;
                file.write_all(&record(id, step.parent.unwrap_or(ROOT), step.ordinal))
                    .map_err(|e| error("copy record", e))?;
                self.usage.written_bytes += RECORD;
            }
            Ok(Disk {
                directory: directory.clone(),
                file: Mutex::new(file),
            })
        })();
        match result {
            Ok(disk) => {
                self.disk = Some(disk);
                self.pending_directory = None;
                self.memory = TraceLog::new();
                self.usage.spilled = true;
                Ok(())
            }
            Err(e) => {
                self.poisoned = true;
                // Surface cleanup failure along with the original failure.
                match fs::remove_dir_all(&directory) {
                    Ok(()) => {
                        self.pending_directory = None;
                        Err(e)
                    }
                    Err(cleanup) => {
                        Err(error("failed creation cleanup", format!("{e}; {cleanup}")))
                    }
                }
            }
        }
    }

    /// Reads a single committed record, checking header, total length,
    /// checksum, and strictly-earlier parent before returning it.
    ///
    /// # Errors
    /// Unknown state, corruption, unavailable file or closed/failed log.
    pub fn get(&self, id: StateId) -> Result<TraceStep, StoreError> {
        self.get_record(id).inspect_err(|error| self.observe(error))
    }

    fn get_record(&self, id: StateId) -> Result<TraceStep, StoreError> {
        self.ready()?;
        if id >= self.len() {
            return Err(StoreError::UnknownState(id));
        }
        if let Some(packed) = &self.packed {
            return packed.get(id).ok_or(StoreError::UnknownState(id));
        }
        let Some(disk) = &self.disk else {
            return self.memory.get(id).ok_or(StoreError::UnknownState(id));
        };
        // Reopening catches deletion/replacement instead of reading an unlinked
        // inode and silently claiming the transient backing file is intact.
        let mut file =
            File::open(disk.directory.join("records")).map_err(|e| error("open reader", e))?;
        self.check_file(&mut file)?;
        file.seek(SeekFrom::Start(offset(id)?))
            .map_err(|e| error("seek record", e))?;
        let mut bytes = [0; 16];
        file.read_exact(&mut bytes)
            .map_err(|e| error("read record", e))?;
        let parent = u64::from_le_bytes(bytes[..8].try_into().map_err(|e| error("parent", e))?);
        let ordinal = u32::from_le_bytes(bytes[8..12].try_into().map_err(|e| error("ordinal", e))?);
        if bytes != record(id, parent, ordinal) {
            return Err(error("checksum", format!("record {id}")));
        }
        if parent != ROOT && parent >= id {
            return Err(error("parent", StoreError::BadParent { id, parent }));
        }
        Ok(TraceStep {
            id,
            parent: (parent != ROOT).then_some(parent),
            ordinal,
        })
    }

    fn check_file(&self, file: &mut File) -> Result<(), StoreError> {
        let expected = offset(self.len())?;
        if file.metadata().map_err(|e| error("metadata", e))?.len() != expected {
            return Err(error("length", format!("expected {expected} bytes")));
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|e| error("seek header", e))?;
        let mut bytes = [0; 24];
        file.read_exact(&mut bytes)
            .map_err(|e| error("read header", e))?;
        if bytes != header() {
            return Err(error("header", "invalid magic/version/record width"));
        }
        Ok(())
    }

    fn validate_records(&self, file: &mut File) -> Result<(), StoreError> {
        self.check_file(file)?;
        for id in 0..self.len() {
            let mut bytes = [0; 16];
            file.read_exact(&mut bytes)
                .map_err(|e| error("final read record", e))?;
            let parent = u64::from_le_bytes(bytes[..8].try_into().map_err(|e| error("parent", e))?);
            let ordinal =
                u32::from_le_bytes(bytes[8..12].try_into().map_err(|e| error("ordinal", e))?);
            if bytes != record(id, parent, ordinal) {
                return Err(error("checksum", format!("record {id}")));
            }
            if parent != ROOT && parent >= id {
                return Err(error("parent", StoreError::BadParent { id, parent }));
            }
        }
        Ok(())
    }

    /// Reconstructs root-first ordinals without evaluating the specification.
    ///
    /// # Errors
    /// Invalid record, allocation, I/O, or more than [`MAX_PATH_STEPS`] in
    /// opt-in bounded mode. The returned vector is outside the record budget.
    pub fn path(&self, id: StateId) -> Result<Vec<TraceStep>, StoreError> {
        self.path_with_limit(
            id,
            if self.budget.is_some() {
                MAX_PATH_STEPS
            } else {
                usize::MAX
            },
        )
        .inspect_err(|error| self.observe(error))
    }

    fn path_with_limit(&self, id: StateId, limit: usize) -> Result<Vec<TraceStep>, StoreError> {
        let mut path = Vec::new();
        let mut current = Some(id);
        while let Some(id) = current {
            if path.len() >= limit {
                return Err(error("reconstruction limit", limit));
            }
            let step = self.get(id)?;
            path.try_reserve(1)
                .map_err(|e| error("path allocation", e))?;
            path.push(step);
            current = step.parent;
        }
        path.reverse();
        Ok(path)
    }

    /// Closes and removes owned transient files. Call before reporting a
    /// completed result; `Drop` is only a best-effort fallback.
    ///
    /// # Errors
    /// Final header/length/readability validation or cleanup failed.
    pub fn close(&mut self) -> Result<(), StoreError> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        let Some(disk) = self.disk.take() else {
            if let Some(directory) = self.pending_directory.take() {
                fs::remove_dir_all(directory).map_err(|e| error("pending cleanup", e))?;
            }
            return Ok(());
        };
        let mut failures = Vec::new();
        match File::open(disk.directory.join("records")) {
            Ok(mut file) => {
                if let Err(e) = self.validate_records(&mut file) {
                    failures.push(e.to_string());
                }
            }
            Err(e) => failures.push(error("final open", e).to_string()),
        }
        let directory = disk.directory.clone();
        drop(disk);
        if let Err(e) = fs::remove_dir_all(directory) {
            failures.push(error("cleanup", e).to_string());
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(error("close", failures.join("; ")))
        }
    }
}

impl Drop for PredecessorLog {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn offset(id: u64) -> Result<u64, StoreError> {
    id.checked_mul(RECORD)
        .and_then(|n| n.checked_add(HEADER))
        .ok_or_else(|| error("offset", "overflow"))
}
fn header() -> [u8; 24] {
    let mut bytes = [0; 24];
    bytes[..8].copy_from_slice(MAGIC);
    bytes[8..12].copy_from_slice(&1_u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&16_u32.to_le_bytes());
    bytes
}
fn record(id: u64, parent: u64, ordinal: u32) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&parent.to_le_bytes());
    bytes[8..12].copy_from_slice(&ordinal.to_le_bytes());
    let checksum = id
        .to_le_bytes()
        .into_iter()
        .chain(bytes[..12].iter().copied())
        .fold(2_166_136_261_u32, |sum, byte| {
            (sum ^ u32::from(byte)).wrapping_mul(16_777_619)
        });
    bytes[12..].copy_from_slice(&checksum.to_le_bytes());
    bytes
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "regression controls")]
mod tests {
    use super::*;
    fn disk_path(log: &PredecessorLog) -> PathBuf {
        log.disk.as_ref().unwrap().directory.join("records")
    }
    fn filled(budget: usize) -> PredecessorLog {
        let mut log = PredecessorLog::new(Some(budget), None);
        assert_eq!(log.push_initial(3).unwrap(), 0);
        assert_eq!(log.push_initial(9).unwrap(), 1);
        assert_eq!(log.push(1, 5).unwrap(), 2);
        assert_eq!(log.push(2, 7).unwrap(), 3);
        log
    }
    #[test]
    fn zero_tiny_and_transition_preserve_ids_and_ordinals() {
        let mut memory = PredecessorLog::new(None, None);
        for ordinal in [3, 9] {
            memory.push_initial(ordinal).unwrap();
        }
        memory.push(1, 5).unwrap();
        memory.push(2, 7).unwrap();
        for budget in [0, 1, 12, 24, 36, 48, 1024] {
            let mut log = filled(budget);
            assert_eq!(log.path(3).unwrap(), memory.path(3).unwrap());
            assert!(log.usage().retained_bytes <= budget as u64);
            assert!(log.usage().peak_retained_bytes <= budget as u64);
            let path = log.disk.as_ref().map(|d| d.directory.clone());
            log.close().unwrap();
            if let Some(path) = path {
                assert!(!path.exists());
            }
        }
    }
    #[test]
    fn reconstruction_bound_and_bad_parent_refuse_explicitly() {
        let mut log = filled(0);
        assert!(log.path_with_limit(3, 2).is_err());
        assert!(log.path_with_limit(3, 3).is_ok());
        assert!(matches!(log.push(4, 0), Err(StoreError::BadParent { .. })));
        assert_eq!(log.len(), 4);
        log.close().unwrap();
    }

    #[test]
    fn all_corrupt_records_and_missing_files_refuse() {
        for kind in [
            "truncated",
            "extra",
            "header",
            "checksum",
            "parent",
            "missing",
        ] {
            let mut log = filled(0);
            let path = disk_path(&log);
            match kind {
                "missing" => fs::remove_file(&path).unwrap(),
                "truncated" => OpenOptions::new()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_len(HEADER + 1)
                    .unwrap(),
                "extra" => OpenOptions::new()
                    .append(true)
                    .open(&path)
                    .unwrap()
                    .write_all(&[0])
                    .unwrap(),
                "header" => {
                    let mut f = OpenOptions::new().write(true).open(&path).unwrap();
                    f.write_all(b"BADMAGIC").unwrap();
                }
                "checksum" => {
                    let mut f = OpenOptions::new().write(true).open(&path).unwrap();
                    f.seek(SeekFrom::Start(offset(2).unwrap() + 12)).unwrap();
                    f.write_all(&[0; 4]).unwrap();
                }
                _ => {
                    let mut f = OpenOptions::new().write(true).open(&path).unwrap();
                    f.seek(SeekFrom::Start(offset(2).unwrap())).unwrap();
                    f.write_all(&record(2, 2, 5)).unwrap();
                }
            }
            assert!(log.path(3).is_err(), "{kind}");
            assert!(log.close().is_err(), "{kind}");
            assert!(!path.exists());
        }
    }
    #[test]
    fn corrupted_unvisited_record_prevents_certification() {
        let mut log = filled(0);
        let mut f = OpenOptions::new()
            .write(true)
            .open(disk_path(&log))
            .unwrap();
        f.seek(SeekFrom::Start(offset(0).unwrap())).unwrap();
        f.write_all(&record(0, 0, 3)).unwrap();
        // The path through root 1 is intact, but completed storage is not.
        assert!(log.path(3).is_ok());
        assert!(log.close().is_err());
    }
    #[test]
    fn failed_write_poisoned_log_and_cleanup_failure_explicit() {
        let mut log = filled(0);
        let path = disk_path(&log);
        *log.disk.as_mut().unwrap().file.get_mut().unwrap() = File::open(&path).unwrap();
        assert!(log.push(3, 1).is_err());
        let first = log.failure();
        assert!(
            matches!(&first, Some(StoreError::Predecessor { operation, .. }) if operation == "write record")
        );
        assert_eq!(log.len(), 4);
        assert!(log.push(3, 2).is_err());
        assert_eq!(log.failure(), first);
        log.close().unwrap();
        assert!(!path.exists());
        let mut log = filled(0);
        let directory = log.disk.as_ref().unwrap().directory.clone();
        fs::remove_dir_all(&directory).unwrap();
        assert!(log.close().is_err());
    }
}
