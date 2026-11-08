//! Native private index/record protocol. No key/offset vector is retained.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

use super::{DiskSeen, InsertOutcome, Key, Operation, SeenError, error};
use crate::StoreMode;

pub(super) const HEADER: u64 = 32;
pub(super) const SLOT: u64 = 16;
const RECORD: u64 = 32;
const DATA: [u8; 8] = *b"DIVESD01";
const INDEX: [u8; 8] = *b"DIVESI01";

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn number(bytes: &[u8]) -> u64 {
    let mut n = [0; 8];
    n.copy_from_slice(bytes);
    u64::from_le_bytes(n)
}

fn mode(mode: StoreMode) -> u64 {
    match mode {
        StoreMode::Fingerprint => 0,
        StoreMode::Exact => 1,
    }
}

fn header(magic: [u8; 8], mode_value: StoreMode, capacity: u64) -> [u8; 32] {
    let mut bytes = [0; 32];
    bytes[..8].copy_from_slice(&magic);
    bytes[8..16].copy_from_slice(&mode(mode_value).to_le_bytes());
    bytes[16..24].copy_from_slice(&capacity.to_le_bytes());
    let sum = checksum(&bytes[..24]);
    bytes[24..].copy_from_slice(&sum.to_le_bytes());
    bytes
}

fn slot(position: u64, offset: u64) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&offset.to_le_bytes());
    bytes[8..].copy_from_slice(&checksum(&(position ^ offset).to_le_bytes()).to_le_bytes());
    bytes
}

struct Record {
    hash: u64,
    key: Vec<u8>,
    next: u64,
}

enum Bytes<'a> {
    Fingerprint([u8; 8]),
    Exact(&'a [u8]),
}
impl AsRef<[u8]> for Bytes<'_> {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Fingerprint(b) => b,
            Self::Exact(b) => b,
        }
    }
}

impl DiskSeen {
    fn parts<'a>(&self, key: Key<'a>) -> Result<(u64, Bytes<'a>), SeenError> {
        let (hash, bytes) = match (self.mode, key) {
            (StoreMode::Fingerprint, Key::Fingerprint(fp)) => {
                (fp, Bytes::Fingerprint(fp.to_le_bytes()))
            }
            (StoreMode::Exact, Key::Exact(bytes)) => (self.hash(bytes), Bytes::Exact(bytes)),
            _ => {
                return Err(error(
                    Operation::Admission,
                    "key mode differs from store mode",
                ));
            }
        };
        if bytes.as_ref().len() > self.limits.key_bytes {
            return Err(error(
                Operation::Admission,
                "key exceeds record workspace bound",
            ));
        }
        Ok((hash, bytes))
    }

    #[allow(
        clippy::unused_self,
        reason = "test-only forced placement hash belongs to this store"
    )]
    fn hash(&self, bytes: &[u8]) -> u64 {
        #[cfg(test)]
        if let Some(hash) = self.forced_hash {
            return hash;
        }
        key_hash(bytes)
    }

    fn read(&mut self, file: &mut File, buffer: &mut [u8]) -> Result<(), SeenError> {
        self.check_fault(Operation::Read)?;
        file.read_exact(buffer)
            .map_err(|e| error(Operation::Read, e))
    }
    fn write(&mut self, file: &mut File, buffer: &[u8]) -> Result<(), SeenError> {
        self.check_fault(Operation::Write)?;
        file.write_all(buffer)
            .map_err(|e| error(Operation::Write, e))
    }
    fn seek(&mut self, file: &mut File, at: u64) -> Result<(), SeenError> {
        self.check_fault(Operation::Seek)?;
        file.seek(SeekFrom::Start(at))
            .map(|_| ())
            .map_err(|e| error(Operation::Seek, e))
    }
    fn allocate(&mut self, length: usize) -> Result<Vec<u8>, SeenError> {
        if length > self.limits.key_bytes {
            return Err(error(
                Operation::Corruption,
                "record exceeds workspace bound",
            ));
        }
        self.check_fault(Operation::Allocate)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|e| error(Operation::Allocate, e))?;
        bytes.resize(length, 0);
        Ok(bytes)
    }

    pub(super) fn initialize(&mut self) -> Result<(), SeenError> {
        let mut data = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.directory.join("data"))
            .map_err(|e| error(Operation::Create, e))?;
        self.write(&mut data, &header(DATA, self.mode, 0))?;
        self.new_index("index", self.capacity)?;
        Ok(())
    }

    fn new_index(&mut self, name: &str, capacity: u64) -> Result<File, SeenError> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(self.directory.join(name))
            .map_err(|e| error(Operation::Create, e))?;
        self.write(&mut file, &header(INDEX, self.mode, capacity))?;
        for position in 0..capacity {
            self.write(&mut file, &slot(position, 0))?;
        }
        Ok(file)
    }

    fn open(
        &mut self,
        name: &str,
        magic: [u8; 8],
        capacity: u64,
        expected: u64,
    ) -> Result<File, SeenError> {
        self.check_fault(Operation::Read)?;
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.directory.join(name))
            .map_err(|e| error(Operation::Read, e))?;
        if file
            .metadata()
            .map_err(|e| error(Operation::Read, e))?
            .len()
            != expected
        {
            return Err(error(Operation::Corruption, "short or trailing file bytes"));
        }
        let mut bytes = [0; 32];
        self.read(&mut file, &mut bytes)?;
        if bytes != header(magic, self.mode, capacity) {
            return Err(error(
                Operation::Corruption,
                "unsupported or corrupt seen-state header",
            ));
        }
        Ok(file)
    }

    fn index_offset(&mut self, file: &mut File, position: u64) -> Result<u64, SeenError> {
        let at = position
            .checked_mul(SLOT)
            .and_then(|n| n.checked_add(HEADER))
            .ok_or_else(|| error(Operation::Admission, "index arithmetic overflow"))?;
        self.seek(file, at)?;
        let mut bytes = [0; 16];
        self.read(file, &mut bytes)?;
        let offset = number(&bytes[..8]);
        if bytes != slot(position, offset) {
            return Err(error(Operation::Corruption, "index slot checksum mismatch"));
        }
        Ok(offset)
    }

    fn write_slot(&mut self, file: &mut File, position: u64, offset: u64) -> Result<(), SeenError> {
        self.seek(file, HEADER + position * SLOT)?;
        self.write(file, &slot(position, offset))
    }

    fn record(&mut self, data: &mut File, offset: u64) -> Result<Record, SeenError> {
        if offset < HEADER
            || offset
                .checked_add(RECORD)
                .is_none_or(|end| end > self.data_end)
        {
            return Err(error(
                Operation::Corruption,
                "record offset outside committed data",
            ));
        }
        self.seek(data, offset)?;
        let mut bytes = [0; 32];
        self.read(data, &mut bytes)?;
        let length = number(&bytes[..8]);
        let hash = number(&bytes[8..16]);
        let ordinal = number(&bytes[16..24]);
        let next = offset
            .checked_add(RECORD)
            .and_then(|n| n.checked_add(length))
            .ok_or_else(|| error(Operation::Corruption, "record length overflow"))?;
        if next > self.data_end
            || ordinal >= self.count
            || (self.mode == StoreMode::Fingerprint && length != 8)
        {
            return Err(error(
                Operation::Corruption,
                "record length or ordinal invalid",
            ));
        }
        let length = usize::try_from(length)
            .map_err(|_| error(Operation::Corruption, "record length not representable"))?;
        let mut key = self.allocate(length)?;
        self.read(data, &mut key)?;
        let sum = checksum(&bytes[..24]) ^ checksum(&key);
        let actual = if self.mode == StoreMode::Fingerprint {
            number(&key)
        } else {
            self.hash(&key)
        };
        if number(&bytes[24..]) != sum || hash != actual {
            return Err(error(
                Operation::Corruption,
                "record checksum or key hash mismatch",
            ));
        }
        Ok(Record { hash, key, next })
    }

    fn probe(
        &mut self,
        index: &mut File,
        data: &mut File,
        capacity: u64,
        hash: u64,
        key: &[u8],
    ) -> Result<(u64, bool), SeenError> {
        let mut position = hash & (capacity - 1);
        for _ in 0..capacity.min(self.limits.probe_steps) {
            let offset = self.index_offset(index, position)?;
            if offset == 0 {
                return Ok((position, false));
            }
            let record = self.record(data, offset)?;
            if record.hash == hash && record.key == key {
                return Ok((position, true));
            }
            position = (position + 1) & (capacity - 1);
        }
        Err(error(
            Operation::Admission,
            "index probe work limit exhausted",
        ))
    }

    pub(super) fn find_key(&mut self, key: Key<'_>) -> Result<(u64, bool), SeenError> {
        let (hash, bytes) = self.parts(key)?;
        let mut data = self.open("data", DATA, 0, self.data_end)?;
        let mut index = self.open("index", INDEX, self.capacity, HEADER + self.capacity * SLOT)?;
        self.probe(&mut index, &mut data, self.capacity, hash, bytes.as_ref())
    }

    pub(super) fn insert_inner(&mut self, key: Key<'_>) -> Result<InsertOutcome, SeenError> {
        let (_, found) = self.find_key(key)?;
        if found {
            return Ok(InsertOutcome::Present);
        }
        if self.count >= self.limits.entries {
            return Err(error(
                Operation::Admission,
                "physical entry limit exhausted",
            ));
        }
        if self.count + 1 > self.capacity / 2 {
            self.rehash()?;
        }
        let (position, found) = self.find_key(key)?;
        if found {
            return Err(error(
                Operation::Corruption,
                "key appeared during exclusive insert",
            ));
        }
        let (hash, bytes) = self.parts(key)?;
        let key = bytes.as_ref();
        let length = u64::try_from(key.len())
            .map_err(|_| error(Operation::Admission, "key length overflow"))?;
        let end = self
            .data_end
            .checked_add(RECORD)
            .and_then(|n| n.checked_add(length))
            .ok_or_else(|| error(Operation::Admission, "data length overflow"))?;
        let mut record = [0; 32];
        record[..8].copy_from_slice(&length.to_le_bytes());
        record[8..16].copy_from_slice(&hash.to_le_bytes());
        record[16..24].copy_from_slice(&self.count.to_le_bytes());
        let sum = checksum(&record[..24]) ^ checksum(key);
        record[24..].copy_from_slice(&sum.to_le_bytes());
        let mut data = self.open("data", DATA, 0, self.data_end)?;
        let mut index = self.open("index", INDEX, self.capacity, HEADER + self.capacity * SLOT)?;
        self.seek(&mut data, self.data_end)?;
        self.write(&mut data, &record)?;
        self.write(&mut data, key)?;
        self.write_slot(&mut index, position, self.data_end)?;
        self.count += 1;
        self.data_end = end;
        Ok(InsertOutcome::Inserted)
    }

    fn rehash(&mut self) -> Result<(), SeenError> {
        self.check_fault(Operation::Rehash)?;
        self.audit_inner(&|| true)?;
        let capacity = self
            .capacity
            .checked_mul(2)
            .ok_or_else(|| error(Operation::Admission, "index capacity overflow"))?;
        let mut data = self.open("data", DATA, 0, self.data_end)?;
        let mut next = self.new_index("index-next", capacity)?;
        let mut offset = HEADER;
        for ordinal in 0..self.count {
            let record = self.record(&mut data, offset)?;
            self.seek(&mut data, offset + 16)?;
            let mut id = [0; 8];
            self.read(&mut data, &mut id)?;
            if number(&id) != ordinal {
                return Err(error(
                    Operation::Corruption,
                    "data discovery ordinal mismatch",
                ));
            }
            let (position, found) =
                self.probe(&mut next, &mut data, capacity, record.hash, &record.key)?;
            if found {
                return Err(error(
                    Operation::Corruption,
                    "duplicate physical key during rehash",
                ));
            }
            self.write_slot(&mut next, position, offset)?;
            offset = record.next;
        }
        if offset != self.data_end {
            return Err(error(
                Operation::Corruption,
                "data has unindexed trailing records",
            ));
        }
        self.check_fault(Operation::Write)?;
        next.sync_all().map_err(|e| error(Operation::Write, e))?;
        self.check_fault(Operation::Rehash)?;
        fs::rename(
            self.directory.join("index-next"),
            self.directory.join("index"),
        )
        .map_err(|e| error(Operation::Rehash, e))?;
        self.capacity = capacity;
        Ok(())
    }

    pub(super) fn gap_inner(&mut self, fp_bits: u32) -> Result<u64, SeenError> {
        if self.mode != StoreMode::Fingerprint {
            return Err(error(
                Operation::Summary,
                "exact keys do not use fingerprint collision estimates",
            ));
        }
        let pairs = self
            .count
            .checked_mul(self.count.saturating_sub(1))
            .and_then(|n| n.checked_div(2))
            .ok_or_else(|| error(Operation::Summary, "summary pair arithmetic overflow"))?;
        if pairs > self.limits.summary_pairs {
            return Err(error(
                Operation::Summary,
                "collision summary work limit exceeded",
            ));
        }
        self.audit_inner(&|| true)?;
        let mut data = self.open("data", DATA, 0, self.data_end)?;
        let mut gap = crate::collision::NO_GAP;
        let mut left = HEADER;
        for _ in 0..self.count {
            let first = self.record(&mut data, left)?;
            let x = number(&first.key);
            let mut right = first.next;
            while right < self.data_end {
                let second = self.record(&mut data, right)?;
                let y = number(&second.key);
                gap = crate::collision::closer(gap, x.min(y), x.max(y), fp_bits);
                right = second.next;
            }
            left = first.next;
        }
        if left != self.data_end {
            return Err(error(Operation::Corruption, "summary data count mismatch"));
        }
        Ok(gap)
    }

    pub(super) fn audit_inner(&mut self, proceed: &dyn Fn() -> bool) -> Result<bool, SeenError> {
        if !proceed() {
            return Ok(false);
        }
        let mut data = self.open("data", DATA, 0, self.data_end)?;
        let mut index = self.open("index", INDEX, self.capacity, HEADER + self.capacity * SLOT)?;
        let mut occupied = 0;
        for position in 0..self.capacity {
            if position.is_multiple_of(128) && !proceed() {
                return Ok(false);
            }
            let offset = self.index_offset(&mut index, position)?;
            if offset != 0 {
                let _ = self.record(&mut data, offset)?;
                occupied += 1;
            }
        }
        if occupied != self.count {
            return Err(error(
                Operation::Corruption,
                "index occupancy differs from physical entries",
            ));
        }
        let mut offset = HEADER;
        for ordinal in 0..self.count {
            if ordinal.is_multiple_of(128) && !proceed() {
                return Ok(false);
            }
            let record = self.record(&mut data, offset)?;
            self.seek(&mut data, offset + 16)?;
            let mut id = [0; 8];
            self.read(&mut data, &mut id)?;
            if number(&id) != ordinal {
                return Err(error(
                    Operation::Corruption,
                    "data discovery ordinal mismatch",
                ));
            }
            let (position, found) = self.probe(
                &mut index,
                &mut data,
                self.capacity,
                record.hash,
                &record.key,
            )?;
            if !found || self.index_offset(&mut index, position)? != offset {
                return Err(error(
                    Operation::Corruption,
                    "physical record is not uniquely reachable from index",
                ));
            }
            offset = record.next;
        }
        if offset != self.data_end {
            return Err(error(
                Operation::Corruption,
                "data count or trailing records mismatch",
            ));
        }
        Ok(true)
    }
}

/// A deterministic 64-bit hash of a key's bytes, used only to place keys in shards
/// and slots. Equality is always decided by comparing the bytes, so the quality of
/// this hash affects speed, never results.
fn key_hash(bytes: &[u8]) -> u64 {
    const K: u64 = 0x51_7c_c1_b7_27_22_0a_95;
    let mut h = 0x243f_6a88_85a3_08d3 ^ (bytes.len() as u64);
    let (words, rest) = bytes.as_chunks::<8>();
    for w in words {
        h = (h.rotate_left(5) ^ u64::from_le_bytes(*w)).wrapping_mul(K);
    }
    if !rest.is_empty() {
        let mut w = [0u8; 8];
        w[..rest.len()].copy_from_slice(rest);
        h = (h.rotate_left(5) ^ u64::from_le_bytes(w)).wrapping_mul(K);
    }
    // The `splitmix64` finalizer spreads every input bit over the top bits, which
    // choose the shard and the slot.
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^ (h >> 31)
}
