//! Ordered frontier records with bounded resident encoded storage (ADR 0024).
//!
//! This is a native transient format, not a TLC checkpoint format. It bounds
//! retained encoded frontier bytes; a reader and writer each also hold at most
//! one record. Decoded values, seen sets, candidates, traces and graphs are
//! outside this budget. Checksums detect accidental corruption, not tampering.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const DATA_MAGIC: &[u8; 8] = b"DIVEFQ01";
const INDEX_MAGIC: &[u8; 8] = b"DIVEFI01";
const HEADER: usize = 32;
/// Maximum encoded record read into one reader or writer workspace: 64 MiB.
/// Larger states refuse spill mode, rather than allocate an unbounded buffer.
pub const MAX_RECORD_BYTES: usize = 64 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Explicit storage failure; never a successful incomplete frontier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpillError {
    /// Stable storage error code.
    pub code: &'static str,
    /// Operation and underlying failure.
    pub message: String,
}

impl std::fmt::Display for SpillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for SpillError {}

fn error(operation: &str, cause: impl std::fmt::Display) -> SpillError {
    SpillError {
        code: "DIVE-S0200",
        message: format!("frontier {operation}: {cause}"),
    }
}

/// Resident encoded bytes and cumulative bytes written to frontier files.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpillStats {
    /// Resident data/index capacity. Excludes one-record codec workspaces.
    pub retained_bytes: u64,
    /// Confirmed complete writes including headers and index. A failed write
    /// can leave additional partial bytes; callers must flag incomplete accounting.
    pub spilled_bytes: u64,
}

#[derive(Debug)]
struct Disk {
    directory: PathBuf,
    data: File,
    index: File,
    bytes: u64,
}

/// A frontier built and read in insertion order, carrying concrete state IDs.
/// Files are isolated below the supplied parent. Call [`Self::close`] before
/// reporting completion: `Drop` cleanup is only a best-effort fallback.
#[derive(Debug)]
pub struct OrderedFrontier {
    limit: usize,
    parent: PathBuf,
    bytes: Vec<u8>,
    offsets: Vec<u64>,
    disk: Option<Disk>,
    len: usize,
    written: u64,
}

impl OrderedFrontier {
    /// Creates an empty queue without touching the filesystem.
    #[must_use]
    pub fn new(memory_bytes: usize, parent: &Path) -> Self {
        Self {
            limit: memory_bytes,
            parent: parent.into(),
            bytes: Vec::new(),
            offsets: Vec::new(),
            disk: None,
            len: 0,
            written: 0,
        }
    }

    /// Number of committed records.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether no record has been committed.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The current resident capacities and bytes written.
    #[must_use]
    pub fn stats(&self) -> SpillStats {
        SpillStats {
            retained_bytes: (self.bytes.capacity() + self.offsets.capacity() * 8) as u64,
            spilled_bytes: self.written,
        }
    }

    /// Appends an encoded state without dropping its concrete ID.
    ///
    /// # Errors
    /// Allocation, filesystem or record-size failure. `len` increases only
    /// after both the record and its index entry have been written.
    pub fn push(&mut self, id: u64, state: &[u8]) -> Result<(), SpillError> {
        if state.len() > MAX_RECORD_BYTES {
            return Err(error(
                "record size",
                format!("{} exceeds {MAX_RECORD_BYTES} bytes", state.len()),
            ));
        }
        let record = header(self.len as u64, id, state);
        if self.disk.is_none() {
            let needed = self
                .bytes
                .len()
                .checked_add(HEADER)
                .and_then(|n| n.checked_add(state.len()))
                .ok_or_else(|| error("length", "overflow"))?;
            let total = needed
                .checked_add((self.offsets.len() + 1).saturating_mul(8))
                .ok_or_else(|| error("length", "overflow"))?;
            if total <= self.limit {
                self.bytes
                    .try_reserve_exact(needed - self.bytes.len())
                    .map_err(|e| error("allocation", e))?;
                self.offsets
                    .try_reserve_exact(1)
                    .map_err(|e| error("allocation", e))?;
            }
            if total > self.limit || self.stats().retained_bytes > self.limit as u64 {
                self.start_disk()?;
            }
        }
        if let Some(disk) = &mut self.disk {
            let offset = disk.bytes;
            disk.data
                .write_all(&record)
                .and_then(|()| disk.data.write_all(state))
                .map_err(|e| error("write record", e))?;
            disk.index
                .write_all(&offset.to_le_bytes())
                .map_err(|e| error("write index", e))?;
            disk.bytes += (HEADER + state.len()) as u64;
            self.written += (HEADER + state.len() + 8) as u64;
        } else {
            self.offsets.push(self.bytes.len() as u64);
            self.bytes.extend_from_slice(&record);
            self.bytes.extend_from_slice(state);
        }
        self.len += 1;
        Ok(())
    }

    fn start_disk(&mut self) -> Result<(), SpillError> {
        let serial = NEXT.fetch_add(1, Ordering::Relaxed);
        let directory = self
            .parent
            .join(format!("dive-frontier-{}-{serial}", std::process::id()));
        fs::create_dir(&directory).map_err(|e| error("create directory", e))?;
        let created = (|| {
            let mut data = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(directory.join("states"))?;
            let mut index = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(directory.join("index"))?;
            data.write_all(DATA_MAGIC)?;
            index.write_all(INDEX_MAGIC)?;
            data.write_all(&self.bytes)?;
            for &offset in &self.offsets {
                index.write_all(&(offset + 8).to_le_bytes())?;
            }
            Ok::<_, std::io::Error>(Disk {
                directory: directory.clone(),
                data,
                index,
                bytes: 8 + self.bytes.len() as u64,
            })
        })();
        match created {
            Ok(disk) => {
                self.written += 16 + self.bytes.len() as u64 + self.offsets.len() as u64 * 8;
                self.disk = Some(disk);
                self.bytes = Vec::new();
                self.offsets = Vec::new();
                Ok(())
            }
            Err(e) => {
                let cleanup = fs::remove_dir_all(&directory);
                Err(error(
                    "initialize files",
                    match cleanup {
                        Ok(()) => e.to_string(),
                        Err(c) => format!("{e}; cleanup also failed: {c}"),
                    },
                ))
            }
        }
    }

    /// Reads exactly one record with bounded buffering. File handles are
    /// opened separately for each call: seeks are never shared across workers.
    ///
    /// # Errors
    /// Missing files, invalid version/index/length/checksum, or read failure.
    pub fn read(&self, position: usize) -> Result<(u64, Vec<u8>), SpillError> {
        if position >= self.len {
            return Err(error("index", "position outside frontier"));
        }
        if let Some(disk) = &self.disk {
            let mut index =
                File::open(disk.directory.join("index")).map_err(|e| error("open index", e))?;
            let mut data =
                File::open(disk.directory.join("states")).map_err(|e| error("open record", e))?;
            check_magic(&mut index, *INDEX_MAGIC)?;
            check_magic(&mut data, *DATA_MAGIC)?;
            index
                .seek(SeekFrom::Start(8 + position as u64 * 8))
                .map_err(|e| error("seek index", e))?;
            let mut offset = [0; 8];
            index
                .read_exact(&mut offset)
                .map_err(|e| error("read index", e))?;
            let offset = u64::from_le_bytes(offset);
            if offset < 8 {
                return Err(error("index", "record offset overlaps header"));
            }
            data.seek(SeekFrom::Start(offset))
                .map_err(|e| error("seek record", e))?;
            let mut record = [0; HEADER];
            data.read_exact(&mut record)
                .map_err(|e| error("read header", e))?;
            let length = number(&record[16..24]);
            let end = offset
                .checked_add(HEADER as u64)
                .and_then(|n| n.checked_add(length));
            let file_len = data.metadata().map_err(|e| error("metadata", e))?.len();
            if length > MAX_RECORD_BYTES as u64 || end.is_none_or(|n| n > file_len) {
                return Err(error(
                    "length",
                    "truncated, overflowing or oversized record",
                ));
            }
            let length = usize::try_from(length).map_err(|e| error("length", e))?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(length)
                .map_err(|e| error("read allocation", e))?;
            bytes.resize(length, 0);
            data.read_exact(&mut bytes)
                .map_err(|e| error("read payload", e))?;
            validate(position, &record, &bytes)?;
            Ok((number(&record[8..16]), bytes))
        } else {
            let offset = usize::try_from(self.offsets[position]).map_err(|e| error("index", e))?;
            let record = &self.bytes[offset..offset + HEADER];
            let length =
                usize::try_from(number(&record[16..24])).map_err(|e| error("length", e))?;
            let bytes = self
                .bytes
                .get(offset + HEADER..offset + HEADER + length)
                .ok_or_else(|| error("length", "invalid resident record"))?;
            validate(position, record, bytes)?;
            Ok((number(&record[8..16]), bytes.to_vec()))
        }
    }

    /// Transient storage directory, for diagnostics and storage controls.
    /// This is not a portable checkpoint or recovery interface.
    #[must_use]
    pub fn disk_directory(&self) -> Option<&Path> {
        self.disk.as_ref().map(|d| d.directory.as_path())
    }

    /// Removes owned files explicitly so cleanup failures reach the caller.
    ///
    /// # Errors
    /// Filesystem cleanup failure. The queue remains available for retry.
    pub fn close(&mut self) -> Result<(), SpillError> {
        if let Some(disk) = &self.disk {
            fs::remove_dir_all(&disk.directory).map_err(|e| error("cleanup", e))?;
            self.disk = None;
        }
        self.bytes = Vec::new();
        self.offsets = Vec::new();
        self.len = 0;
        Ok(())
    }
}

impl Drop for OrderedFrontier {
    fn drop(&mut self) {
        if let Some(disk) = &self.disk {
            let _ = fs::remove_dir_all(&disk.directory);
        }
    }
}

fn check_magic(file: &mut File, magic: [u8; 8]) -> Result<(), SpillError> {
    let mut found = [0; 8];
    file.read_exact(&mut found)
        .map_err(|e| error("read version", e))?;
    if found != magic {
        return Err(error("version", "unsupported or corrupt frontier format"));
    }
    Ok(())
}

fn number(bytes: &[u8]) -> u64 {
    let mut n = [0; 8];
    n.copy_from_slice(bytes);
    u64::from_le_bytes(n)
}
fn checksum(position: u64, id: u64, bytes: &[u8]) -> u64 {
    position
        .to_le_bytes()
        .into_iter()
        .chain(id.to_le_bytes())
        .chain(bytes.iter().copied())
        .fold(0xcbf2_9ce4_8422_2325, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
        })
}
fn header(position: u64, id: u64, bytes: &[u8]) -> [u8; HEADER] {
    let mut out = [0; HEADER];
    for (i, n) in [
        position,
        id,
        bytes.len() as u64,
        checksum(position, id, bytes),
    ]
    .into_iter()
    .enumerate()
    {
        out[i * 8..i * 8 + 8].copy_from_slice(&n.to_le_bytes());
    }
    out
}
fn validate(position: usize, record: &[u8], bytes: &[u8]) -> Result<(), SpillError> {
    if number(&record[..8]) != position as u64
        || number(&record[24..32]) != checksum(position as u64, number(&record[8..16]), bytes)
    {
        return Err(error("checksum", "record identity or payload mismatch"));
    }
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "storage failure controls"
)]
mod tests {
    use super::*;
    fn parent() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "dive-frontier-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).expect("test directory");
        p
    }
    fn queue() -> (PathBuf, OrderedFrontier) {
        let p = parent();
        let mut q = OrderedFrontier::new(0, &p);
        q.push(37, b"abc").expect("record");
        (p, q)
    }
    #[test]
    fn memory_and_disk_keep_ids_order_and_capacity_bound() {
        for budget in [0, 1, 64, 1024] {
            let p = parent();
            let mut q = OrderedFrontier::new(budget, &p);
            for i in 0..20 {
                q.push(100 + i, &[u8::try_from(i).expect("small index"); 19])
                    .expect("push");
                assert!(q.stats().retained_bytes <= budget as u64);
            }
            for i in 0..20 {
                assert_eq!(
                    q.read(i).unwrap(),
                    (
                        100 + i as u64,
                        vec![u8::try_from(i).expect("small index"); 19]
                    )
                );
            }
            assert!(q.read(20).is_err());
            q.close().unwrap();
            assert_eq!(fs::read_dir(&p).unwrap().count(), 0);
            fs::remove_dir(p).unwrap();
        }
    }
    #[test]
    fn corrupt_truncated_wrong_index_and_oversized_records_refuse() {
        for case in 0..6 {
            let (p, mut q) = queue();
            let d = q.disk.as_ref().unwrap();
            let file = d.directory.join("states");
            let mut bytes = fs::read(&file).unwrap();
            match case {
                0 => bytes[0] ^= 1,
                1 => bytes.truncate(8 + HEADER - 1),
                2 => *bytes.last_mut().unwrap() ^= 1,
                3 => bytes[8 + 16..8 + 24].copy_from_slice(&u64::MAX.to_le_bytes()),
                4 => bytes[8..16].copy_from_slice(&1u64.to_le_bytes()),
                _ => fs::write(
                    d.directory.join("index"),
                    [INDEX_MAGIC.as_slice(), &0u64.to_le_bytes()].concat(),
                )
                .unwrap(),
            }
            fs::write(file, bytes).unwrap();
            assert!(q.read(0).is_err(), "case{case}");
            q.close().unwrap();
            fs::remove_dir(p).unwrap();
        }
    }
    #[test]
    fn missing_read_write_and_cleanup_failures_are_explicit() {
        let (p, mut q) = queue();
        let disk = q.disk.as_mut().unwrap();
        let path = disk.directory.join("states");
        disk.data = File::open(&path).unwrap();
        assert!(q.push(38, b"def").is_err());
        assert_eq!(q.len(), 1);
        fs::remove_file(path).unwrap();
        assert!(q.read(0).is_err());
        let actual = q.disk.as_ref().unwrap().directory.clone();
        q.disk.as_mut().unwrap().directory = actual.join("index");
        assert!(q.close().is_err());
        q.disk.as_mut().unwrap().directory = actual;
        q.close().unwrap();
        fs::remove_dir(p).unwrap();
        let p = parent();
        let mut q = OrderedFrontier::new(0, &p.join("missing"));
        assert!(q.push(1, b"x").is_err());
        assert_eq!(q.len(), 0);
        fs::remove_dir(p).unwrap();
    }
}
