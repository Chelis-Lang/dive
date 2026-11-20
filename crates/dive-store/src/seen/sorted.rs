//! A bounded mutable fingerprint set backed by sorted, page-indexed immutable runs.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use super::{InsertOutcome, NEXT, Operation, SeenError, Stats, error};

const PAGE: usize = 512;
const PAGE_BYTES: usize = PAGE * 8;
/// Maximum speculative lookups in one window. Only ordered inserts change membership.
pub const WINDOW: usize = 32_768;
const DELTA: usize = 1 << 18;
type Result<T> = std::result::Result<T, SeenError>;

#[derive(Debug)]
struct Page {
    first: u64,
    checksum: u64,
}

#[derive(Debug)]
struct Run {
    path: PathBuf,
    pages: Vec<Page>,
    count: u64,
}

fn checksum(bytes: &[u8]) -> u64 {
    // Each page consists of whole little-endian words. Include their order.
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for word in bytes.as_chunks::<8>().0 {
        hash = (hash ^ u64::from_le_bytes(*word)).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
    hash
}

struct Reader<'a> {
    run: &'a Run,
    file: File,
    block: [u8; PAGE_BYTES],
    page: Option<usize>,
    offset: usize,
    words: usize,
}

impl Run {
    fn reader(&self) -> Result<Reader<'_>> {
        let file = File::open(&self.path).map_err(|e| error(Operation::Read, e))?;
        if file
            .metadata()
            .map_err(|e| error(Operation::Read, e))?
            .len()
            != self.count * 8
        {
            return Err(error(
                Operation::Corruption,
                "sorted fingerprint run length",
            ));
        }
        Ok(Reader {
            run: self,
            file,
            block: [0; PAGE_BYTES],
            page: None,
            offset: 0,
            words: 0,
        })
    }
}

impl Reader<'_> {
    fn load(&mut self, index: usize) -> Result<()> {
        let page = &self.run.pages[index];
        let start = index as u64 * PAGE as u64;
        self.words = usize::try_from((self.run.count - start).min(PAGE as u64))
            .map_err(|e| error(Operation::Read, e))?;
        let block = &mut self.block[..8 * self.words];
        if self
            .page
            .map_or(index != 0, |previous| previous + 1 != index)
        {
            self.file
                .seek(SeekFrom::Start(start * 8))
                .map_err(|e| error(Operation::Seek, e))?;
        }
        self.file
            .read_exact(block)
            .map_err(|e| error(Operation::Read, e))?;
        if checksum(block) != page.checksum || self.word(0) != page.first {
            return Err(error(
                Operation::Corruption,
                "sorted fingerprint page checksum",
            ));
        }
        self.page = Some(index);
        self.offset = 0;
        Ok(())
    }

    fn word(&self, index: usize) -> u64 {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(&self.block[index * 8..index * 8 + 8]);
        u64::from_le_bytes(bytes)
    }

    fn contains(&mut self, key: u64) -> Result<bool> {
        let Some(page) = self
            .run
            .pages
            .partition_point(|p| p.first <= key)
            .checked_sub(1)
        else {
            return Ok(false);
        };
        if self.page != Some(page) {
            self.load(page)?;
        }
        let (mut lo, mut hi) = (0, self.words);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.word(mid) < key {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        Ok(lo < self.words && self.word(lo) == key)
    }

    fn next(&mut self) -> Result<Option<u64>> {
        if self.offset == self.words {
            let index = self.page.map_or(0, |p| p + 1);
            if index == self.run.pages.len() {
                return Ok(None);
            }
            self.load(index)?;
        }
        let word = self.word(self.offset);
        self.offset += 1;
        Ok(Some(word))
    }
}

struct Writer {
    run: Run,
    file: BufWriter<File>,
    block: Vec<u8>,
    previous: Option<u64>,
}

impl Writer {
    fn new(path: PathBuf, file: File) -> Self {
        Self {
            run: Run {
                path,
                pages: Vec::new(),
                count: 0,
            },
            file: BufWriter::new(file),
            block: Vec::with_capacity(PAGE_BYTES),
            previous: None,
        }
    }

    fn append(&mut self, key: u64) -> Result<()> {
        if self.previous.is_some_and(|prev| prev >= key) {
            return Err(error(
                Operation::Corruption,
                "unordered or duplicate fingerprint in run",
            ));
        }
        self.previous = Some(key);
        self.run.count += 1;
        self.block.extend_from_slice(&key.to_le_bytes());
        if self.block.len() == PAGE_BYTES {
            self.flush_page()?;
        }
        Ok(())
    }

    fn flush_page(&mut self) -> Result<()> {
        if self.block.is_empty() {
            return Ok(());
        }
        let mut first = [0; 8];
        first.copy_from_slice(&self.block[..8]);
        self.run
            .pages
            .try_reserve(1)
            .map_err(|e| error(Operation::Allocate, e))?;
        self.file
            .write_all(&self.block)
            .map_err(|e| error(Operation::Write, e))?;
        self.run.pages.push(Page {
            first: u64::from_le_bytes(first),
            checksum: checksum(&self.block),
        });
        self.block.clear();
        Ok(())
    }

    fn finish(mut self) -> Result<Run> {
        self.flush_page()?;
        self.file.flush().map_err(|e| error(Operation::Write, e))?;
        Ok(self.run)
    }
}

fn ordered(
    runs: &[&Run],
    mut visit: impl FnMut(u64) -> Result<()>,
    proceed: &dyn Fn() -> bool,
) -> Result<bool> {
    let mut readers: Vec<_> = runs.iter().map(|r| r.reader()).collect::<Result<_>>()?;
    let mut heads = BinaryHeap::new();
    for (i, reader) in readers.iter_mut().enumerate() {
        if let Some(key) = reader.next()? {
            heads.push(Reverse((key, i)));
        }
    }
    let mut previous = None;
    let mut count = 0_u64;
    while let Some(Reverse((key, i))) = heads.pop() {
        if count.is_multiple_of(PAGE as u64) && !proceed() {
            return Ok(false);
        }
        if previous.is_some_and(|p| p >= key) {
            return Err(error(
                Operation::Corruption,
                "unordered or duplicate committed fingerprint",
            ));
        }
        visit(key)?;
        count += 1;
        previous = Some(key);
        if let Some(next) = readers[i].next()? {
            heads.push(Reverse((next, i)));
        }
    }
    Ok(true)
}

/// Validates sorted-run entry counts. Key, probe and pair budgets belong to the indexed store.
///
/// # Errors
/// Invalid global capacity or nondefault indexed-store bounds.
pub fn validate(limits: super::Limits) -> Result<()> {
    let default = super::Limits::default();
    if !(1..=i64::MAX as u64 / 8).contains(&limits.entries)
        || limits.key_bytes != default.key_bytes
        || limits.probe_steps != default.probe_steps
        || limits.summary_pairs != default.summary_pairs
    {
        return Err(error(
            Operation::Admission,
            "invalid sorted fingerprint limits",
        ));
    }
    Ok(())
}

/// Transient, single-owner external fingerprint membership. A run stores each complete
/// 64-bit key once and keeps two index words per 512 keys in RAM. In addition to indexes,
/// memory includes a bounded hot set, one query window, and page-sized merge buffers.
///
/// ```
/// use dive_store::seen::{InsertOutcome, sorted::SortedFingerprints};
///
/// let mut seen = SortedFingerprints::create(&std::env::temp_dir(), 1_u64 << 36)?;
/// assert!(seen.prefetch([7, 9, 7], &|| true));
/// assert_eq!(seen.insert(7, &|| true)?, Some(InsertOutcome::Inserted));
/// assert_eq!(seen.insert(7, &|| true)?, Some(InsertOutcome::Present));
/// assert!(!seen.contains(9)?);
/// assert!(seen.close_controlled(true, &|| true)?);
/// # Ok::<(), dive_store::seen::SeenError>(())
/// ```
#[derive(Debug)]
pub struct SortedFingerprints {
    directory: PathBuf,
    owned: HashSet<PathBuf>,
    serial: u64,
    limit: u64,
    count: u64,
    threshold: usize,
    hot: HashSet<u64>,
    runs: Vec<Option<Run>>,
    window: Vec<(u64, bool)>,
    first: Option<SeenError>,
    closed: bool,
    audited: bool,
    gap: Option<u64>,
}

impl SortedFingerprints {
    /// Opens a new private run directory under an existing parent.
    ///
    /// # Errors
    /// Invalid limit, duplicate private directory name, or filesystem failure.
    pub fn create(parent: &Path, limit: u64) -> Result<Self> {
        if !(1..=i64::MAX as u64 / 8).contains(&limit) {
            return Err(error(
                Operation::Admission,
                "sorted fingerprint entry limit outside file offsets",
            ));
        }
        let directory = parent.join(format!(
            "dive-sorted-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).map_err(|e| error(Operation::Create, e))?;
        Ok(Self {
            directory,
            owned: HashSet::new(),
            serial: 0,
            limit,
            count: 0,
            threshold: DELTA,
            hot: HashSet::new(),
            runs: Vec::new(),
            window: Vec::new(),
            first: None,
            closed: false,
            audited: false,
            gap: None,
        })
    }

    fn available(&self) -> Result<()> {
        if let Some(e) = &self.first {
            return Err(e.clone());
        }
        if self.closed {
            return Err(error(
                Operation::Admission,
                "sorted fingerprint set is closed",
            ));
        }
        Ok(())
    }

    fn remember<T>(&mut self, result: Result<T>) -> Result<T> {
        if let Err(e) = &result {
            self.first.get_or_insert_with(|| e.clone());
        }
        result
    }

    /// Prefetches membership of keys in already committed sorted runs. A prefetch fault
    /// drops the hint. Ordered lookup retries and attributes faults to the deciding key.
    /// Ordered insertions update the hint before any later flush can remove a hot key.
    /// Returns false when cancelled. In that case no partial hints survive.
    pub fn prefetch(
        &mut self,
        keys: impl IntoIterator<Item = u64>,
        proceed: &dyn Fn() -> bool,
    ) -> bool {
        self.window.clear();
        if self.runs.is_empty() || self.available().is_err() {
            return true;
        }
        if !proceed() {
            return false;
        }
        self.window.extend(
            keys.into_iter()
                .take(WINDOW)
                .map(|k| (k, self.hot.contains(&k))),
        );
        self.window.sort_unstable_by_key(|&(k, _)| k);
        self.window.dedup_by_key(|entry| entry.0);
        for run in self.runs.iter().flatten() {
            let result = run.reader().and_then(|mut reader| {
                for (i, (key, present)) in self.window.iter_mut().enumerate() {
                    if i.is_multiple_of(128) && !proceed() {
                        return Ok(false);
                    }
                    if !*present {
                        *present = reader.contains(*key)?;
                    }
                }
                Ok(true)
            });
            if !matches!(result, Ok(true)) {
                self.window.clear();
                return result.is_err(); // A speculative read failure waits for ordered lookup.
            }
        }
        true
    }

    fn find(&self, key: u64) -> Result<bool> {
        if self.hot.contains(&key) {
            return Ok(true);
        }
        if let Ok(i) = self.window.binary_search_by_key(&key, |&(k, _)| k) {
            return Ok(self.window[i].1);
        }
        for run in self.runs.iter().flatten() {
            if run.reader()?.contains(key)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Looks up a key at its ordered position.
    ///
    /// # Errors
    /// A read, integrity, or lifecycle failure. A deciding failure poisons subsequent calls.
    pub fn contains(&mut self, key: u64) -> Result<bool> {
        self.available()?;
        let found = self.find(key);
        self.remember(found)
    }

    fn writer(&mut self) -> Result<Writer> {
        let path = self.directory.join(self.serial.to_string());
        self.serial += 1;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| error(Operation::Create, e))?;
        self.owned.insert(path.clone());
        Ok(Writer::new(path, file))
    }

    fn remove(&mut self, path: &Path) -> Result<()> {
        fs::remove_file(path).map_err(|e| error(Operation::Cleanup, e))?;
        self.owned.remove(path);
        Ok(())
    }

    fn flush(&mut self, proceed: &dyn Fn() -> bool) -> Result<bool> {
        if self.hot.is_empty() {
            return Ok(true);
        }
        let first_staging = self.serial;
        let complete = self.flush_hot(proceed)?;
        if !complete {
            // No replacement has been published. Discard only files from this attempt.
            for id in first_staging..self.serial {
                let path = self.directory.join(id.to_string());
                if self.owned.contains(&path) {
                    self.remove(&path)?;
                }
            }
        }
        Ok(complete)
    }

    fn flush_hot(&mut self, proceed: &dyn Fn() -> bool) -> Result<bool> {
        let mut keys: Vec<_> = self.hot.iter().copied().collect();
        keys.sort_unstable();
        let mut writer = self.writer()?;
        for chunk in keys.chunks(PAGE) {
            if !proceed() {
                return Ok(false);
            }
            for &k in chunk {
                writer.append(k)?;
            }
        }
        let mut merged = writer.finish()?;
        let mut rank = 0;
        while rank < self.runs.len() && self.runs[rank].is_some() {
            let mut writer = self.writer()?;
            let old = self.runs[rank]
                .as_ref()
                .ok_or_else(|| error(Operation::Corruption, "missing merge input"))?;
            if !ordered(&[old, &merged], |k| writer.append(k), proceed)? {
                return Ok(false);
            }
            let next = writer.finish()?;
            self.remove(&merged.path)?;
            merged = next;
            rank += 1;
        }
        // Keep the active runs intact until all replacement writes finish.
        for i in 0..rank {
            if let Some(old) = self.runs[i].take() {
                self.remove(&old.path)?;
            }
        }
        if rank == self.runs.len() {
            self.runs.push(None);
        }
        self.runs[rank] = Some(merged);
        self.hot.clear();
        Ok(true)
    }

    /// Inserts at the original discovery position, checking duplicates before the cap.
    /// Returns `None` on cancellation. The caller then closes the transient run.
    ///
    /// # Errors
    /// Reaching the global entry limit or failure of a required read, write or merge.
    pub fn insert(
        &mut self,
        key: u64,
        proceed: &dyn Fn() -> bool,
    ) -> Result<Option<InsertOutcome>> {
        self.available()?;
        let result = (|| {
            if self.find(key)? {
                return Ok(Some(InsertOutcome::Present));
            }
            if self.count == self.limit {
                return Err(error(
                    Operation::Admission,
                    "sorted fingerprint entry limit",
                ));
            }
            if self.hot.len() >= self.threshold && !self.flush(proceed)? {
                return Ok(None);
            }
            self.hot
                .try_reserve(1)
                .map_err(|e| error(Operation::Allocate, e))?;
            self.hot.insert(key);
            if let Ok(i) = self.window.binary_search_by_key(&key, |&(k, _)| k) {
                self.window[i].1 = true;
            }
            self.count += 1;
            Ok(Some(InsertOutcome::Inserted))
        })();
        self.remember(result)
    }

    /// Number of committed keys, expected active file lengths and retained collection capacities.
    #[must_use]
    pub fn stats(&self) -> Stats {
        let runs = self.runs.iter().flatten();
        Stats {
            physical_entries: self.count,
            disk_bytes: runs.clone().map(|r| r.count * 8).sum(),
            resident_key_bytes: self.hot.capacity() as u64 * 8
                + self.window.capacity() as u64 * size_of::<(u64, bool)>() as u64
                + runs
                    .map(|r| r.pages.capacity() as u64 * size_of::<Page>() as u64)
                    .sum::<u64>(),
            poisoned: self.first.is_some(),
            cleanup_completed: self.closed,
        }
    }

    /// Minimum adjacent gap within the two high-bit subsets, available after a complete audit.
    /// The input keys are the checker’s fingerprints, which can differ from TLC’s.
    #[must_use]
    pub fn audited_minimum_gap(&self) -> Option<u64> {
        self.gap.filter(|_| self.audited && self.first.is_none())
    }

    /// Audits retained keys in numeric order when requested and removes owned files even
    /// after cancellation or an earlier failure. An interrupted audit returns `false`.
    ///
    /// # Errors
    /// The first integrity, storage or cleanup failure.
    pub fn close_controlled(&mut self, validate: bool, proceed: &dyn Fn() -> bool) -> Result<bool> {
        if self.closed {
            return self.first.clone().map_or(Ok(self.audited), Err);
        }
        let audit = if validate && self.first.is_none() {
            (|| {
                if !self.flush(proceed)? {
                    return Ok(false);
                }
                let mut audited = 0;
                let mut previous = None;
                let mut gap = crate::collision::NO_GAP;
                let complete = ordered(
                    &self.runs.iter().flatten().collect::<Vec<_>>(),
                    |key| {
                        if let Some(prev) = previous {
                            gap = crate::collision::closer(gap, prev, key, 1);
                        }
                        previous = Some(key);
                        audited += 1;
                        Ok(())
                    },
                    proceed,
                )?;
                if complete && audited != self.count {
                    return Err(error(Operation::Corruption, "sorted fingerprint count"));
                }
                if complete {
                    self.gap = Some(gap);
                }
                Ok(complete)
            })()
        } else {
            Ok(false)
        };
        if let Err(e) = &audit {
            self.first.get_or_insert_with(|| e.clone());
        }
        self.audited = matches!(audit, Ok(true));
        let cleanup = self.clean();
        if let Err(e) = cleanup {
            if let Some(first) = &mut self.first {
                first.cleanup_failure = Some(e.message);
            } else {
                self.first = Some(e);
            }
        }
        if let Some(e) = &self.first {
            return Err(e.clone());
        }
        audit
    }

    fn clean(&mut self) -> Result<()> {
        let mut first = None;
        for path in self.owned.clone() {
            match self.remove(&path) {
                Ok(()) => {}
                Err(e) => {
                    first.get_or_insert(e);
                }
            }
        }
        if let Err(e) = fs::remove_dir(&self.directory) {
            first.get_or_insert_with(|| error(Operation::Cleanup, e));
        }
        if let Some(e) = first {
            return Err(e);
        }
        self.runs.clear();
        self.hot = HashSet::new();
        self.window = Vec::new();
        self.closed = true;
        Ok(())
    }
}

impl Drop for SortedFingerprints {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.clean();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn sorted_queries_follow_order_across_flush_and_close() -> Result<()> {
        let mut set = SortedFingerprints::create(&std::env::temp_dir(), 1_u64 << 40)?;
        set.threshold = 23;
        let mut reference = BTreeSet::new();
        let edge = (1_u64 << 56) - 1;
        for key in [0, 1, u64::MAX, edge, edge + 1] {
            assert_eq!(set.insert(key, &|| true)?, Some(InsertOutcome::Inserted));
            reference.insert(key);
        }
        for wave in 0..32_u64 {
            let keys: Vec<_> = (0..64_u64)
                .map(|i| ((wave * 23 + i * 17) % 911).wrapping_mul(0x9e37_79b9_7f4a_7c15))
                .collect();
            assert!(set.prefetch(keys.iter().copied(), &|| true));
            for key in keys {
                assert_eq!(set.contains(key)?, reference.contains(&key));
                let new = reference.insert(key);
                assert_eq!(
                    set.insert(key, &|| true)? == Some(InsertOutcome::Inserted),
                    new
                );
                assert!(set.contains(key)?);
            }
        }
        assert_eq!(set.stats().physical_entries, reference.len() as u64);
        assert!(set.close_controlled(true, &|| true)?);
        assert!(set.close_controlled(true, &|| true)?);
        assert_eq!(
            set.audited_minimum_gap(),
            Some(crate::collision::tlc_min_gap(reference, 1))
        );
        assert!(set.stats().cleanup_completed);
        Ok(())
    }

    #[test]
    fn interrupted_prefetch_and_merge_leave_previous_membership_available() -> Result<()> {
        use std::cell::Cell;

        let mut set = SortedFingerprints::create(&std::env::temp_dir(), 2000)?;
        set.threshold = 512;
        for key in 0..1024 {
            set.insert(key, &|| true)?;
        }
        let polls = Cell::new(0);
        let proceed = || {
            polls.set(polls.get() + 1);
            polls.get() <= 2
        };
        assert!(!set.prefetch(0..1024, &proceed));
        assert_eq!(set.window, []);
        assert!(set.contains(45)?);
        assert!(!set.contains(1024)?);
        let files = set.owned.clone();
        polls.set(0);
        assert_eq!(set.insert(1024, &proceed)?, None);
        assert_eq!(set.owned, files);
        assert_eq!(set.stats().physical_entries, 1024);
        assert_eq!(set.insert(1024, &|| true)?, Some(InsertOutcome::Inserted));
        assert!(set.close_controlled(true, &|| true)?);
        Ok(())
    }

    #[test]
    fn actual_fault_is_ordered_and_audit_cannot_certify_corrupt_pages() -> Result<()> {
        let mut set = SortedFingerprints::create(&std::env::temp_dir(), 100)?;
        set.threshold = 2;
        for key in [5, 15, 25] {
            set.insert(key, &|| true)?;
        }
        let run = set
            .runs
            .iter()
            .flatten()
            .next()
            .ok_or_else(|| error(Operation::Read, "test run"))?;
        let mut file = OpenOptions::new()
            .write(true)
            .open(&run.path)
            .map_err(|e| error(Operation::Write, e))?;
        file.write_all(&17_u64.to_le_bytes())
            .map_err(|e| error(Operation::Write, e))?;
        drop(file);
        assert!(set.prefetch([25, 15], &|| true));
        assert!(set.contains(25)?);
        assert!(matches!(
            set.contains(15),
            Err(SeenError {
                operation: Operation::Corruption,
                ..
            })
        ));
        assert!(set.close_controlled(true, &|| true).is_err());
        assert_eq!(set.audited_minimum_gap(), None);
        assert!(set.stats().cleanup_completed);
        Ok(())
    }

    #[test]
    fn cap_and_cancel_keep_discovery_counts_and_owned_cleanup() -> Result<()> {
        let mut set = SortedFingerprints::create(&std::env::temp_dir(), u64::from(u32::MAX) + 1)?;
        set.count = u64::from(u32::MAX);
        assert_eq!(set.insert(0, &|| true)?, Some(InsertOutcome::Inserted));
        assert_eq!(set.insert(0, &|| true)?, Some(InsertOutcome::Present));
        assert_eq!(set.count, 1_u64 << 32);
        assert!(matches!(
            set.insert(1, &|| true),
            Err(SeenError {
                operation: Operation::Admission,
                ..
            })
        ));
        assert!(set.close_controlled(false, &|| true).is_err());
        let mut set = SortedFingerprints::create(&std::env::temp_dir(), 32)?;
        set.threshold = 1;
        set.insert(23, &|| true)?;
        assert_eq!(set.insert(24, &|| false)?, None);
        assert_eq!(set.stats().physical_entries, 1);
        assert!(!set.close_controlled(true, &|| false)?);
        assert_eq!(set.audited_minimum_gap(), None);
        assert!(set.stats().cleanup_completed);
        Ok(())
    }
}
