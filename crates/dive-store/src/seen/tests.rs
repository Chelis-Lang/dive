//! Independent memory/disk and fault/corruption controls for the library boundary.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "explicit storage test assertions"
)]

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};

use super::*;

fn make(mode: StoreMode) -> DiskSeen {
    DiskSeen::create(mode, &std::env::temp_dir(), Limits::default()).expect("private store")
}
fn change(set: &DiskSeen, name: &str, offset: u64, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .open(set.directory.join(name))
        .unwrap();
    file.seek(SeekFrom::Start(offset)).unwrap();
    file.write_all(bytes).unwrap();
}
fn check_poison(set: &mut DiskSeen, failure: &SeenError) {
    assert!(set.stats().poisoned);
    assert_eq!(set.contains(Key::Fingerprint(0)).unwrap_err(), *failure);
    assert_eq!(set.insert(Key::Exact(b"anything")).unwrap_err(), *failure);
    assert_eq!(set.close().unwrap_err(), *failure);
    assert!(set.stats().cleanup_completed);
}

#[test]
fn complete_u64_identity_and_rehash_match_memory() {
    let mut disk = make(StoreMode::Fingerprint);
    let mut memory = BTreeSet::new();
    let keys = (0u64..100)
        .map(|n| n.wrapping_mul(0x9e37_79b9_7f4a_7c15))
        .chain([0, u64::MAX, 1 << 63, 0]);
    for key in keys {
        assert_eq!(
            disk.contains(Key::Fingerprint(key)).unwrap(),
            memory.contains(&key)
        );
        let expected = if memory.insert(key) {
            InsertOutcome::Inserted
        } else {
            InsertOutcome::Present
        };
        assert_eq!(disk.insert(Key::Fingerprint(key)).unwrap(), expected);
        assert_eq!(disk.stats().resident_key_bytes, 0);
        assert_eq!(disk.stats().physical_entries, memory.len() as u64);
    }
    for key in &memory {
        assert!(disk.contains(Key::Fingerprint(*key)).unwrap());
    }
    for bits in [1, 2, 16, 63] {
        assert_eq!(
            disk.tlc_min_gap(bits).unwrap(),
            crate::collision::tlc_min_gap(memory.iter().copied(), bits)
        );
    }
    assert!(disk.close().unwrap().cleanup_completed);
}

#[test]
fn exact_canonical_bytes_survive_forced_full_hash_collisions() {
    let mut disk = make(StoreMode::Exact);
    disk.forced_hash = Some(0);
    let mut memory = BTreeSet::new();
    let keys = (0..40)
        .map(|n| vec![n; 19])
        .chain([vec![], vec![0; 19], vec![0]]);
    for key in keys {
        assert_eq!(
            disk.contains(Key::Exact(&key)).unwrap(),
            memory.contains(&key)
        );
        let expected = if memory.insert(key.clone()) {
            InsertOutcome::Inserted
        } else {
            InsertOutcome::Present
        };
        assert_eq!(disk.insert(Key::Exact(&key)).unwrap(), expected);
    }
    for key in memory {
        assert!(disk.contains(Key::Exact(&key)).unwrap());
    }
    assert!(!disk.contains(Key::Exact(b"absent")).unwrap());
    assert!(disk.close().unwrap().cleanup_completed);
}

#[test]
fn faults_never_become_absence_or_success_and_first_error_survives_cleanup() {
    for operation in [
        Operation::Read,
        Operation::Seek,
        Operation::Write,
        Operation::Allocate,
        Operation::Rehash,
    ] {
        let mut disk = make(StoreMode::Fingerprint);
        for key in 0..4 {
            disk.insert(Key::Fingerprint(key)).unwrap();
        }
        disk.fault = Some((operation, 0));
        let failure = disk.insert(Key::Fingerprint(8)).unwrap_err();
        assert_eq!(failure.operation, operation);
        assert_eq!(disk.stats().physical_entries, 4);
        check_poison(&mut disk, &failure);
    }
    let mut disk = make(StoreMode::Fingerprint);
    disk.fault = Some((Operation::Write, 1)); // record header written, key payload fails
    let original = disk.insert(Key::Fingerprint(1)).unwrap_err();
    assert_eq!(original.operation, Operation::Write);
    assert_eq!(disk.stats().physical_entries, 0);
    disk.fault = Some((Operation::Cleanup, 0));
    let closed = disk.close().unwrap_err();
    assert_eq!(closed.operation, original.operation);
    assert_eq!(closed.message, original.message);
    assert!(closed.cleanup_failure.is_some());
    assert!(!disk.stats().cleanup_completed);
    assert!(disk.close().is_err());
    assert!(disk.stats().cleanup_completed);
}

#[test]
fn malformed_files_offsets_and_records_refuse_explicitly() {
    for (file, offset, replacement) in [
        ("data", 0, b"DIVESD00".as_slice()),
        ("index", 0, b"DIVESI00".as_slice()),
        ("data", 8, &7u64.to_le_bytes()),
        ("index", 32, &u64::MAX.to_le_bytes()),
        ("data", 32, &u64::MAX.to_le_bytes()),
        ("data", 48, &u64::MAX.to_le_bytes()),
        ("data", 64, &9u64.to_le_bytes()),
    ] {
        let mut disk = make(StoreMode::Fingerprint);
        disk.insert(Key::Fingerprint(0)).unwrap();
        change(&disk, file, offset, replacement);
        let failure = disk.contains(Key::Fingerprint(0)).unwrap_err();
        assert_eq!(failure.operation, Operation::Corruption);
        check_poison(&mut disk, &failure);
    }
    for file in ["data", "index"] {
        for delta in [-1i64, 1] {
            let mut disk = make(StoreMode::Fingerprint);
            disk.insert(Key::Fingerprint(0)).unwrap();
            let path = disk.directory.join(file);
            let length = fs::metadata(&path)
                .unwrap()
                .len()
                .checked_add_signed(delta)
                .unwrap();
            OpenOptions::new()
                .write(true)
                .open(path)
                .unwrap()
                .set_len(length)
                .unwrap();
            let failure = disk.contains(Key::Fingerprint(0)).unwrap_err();
            assert_eq!(failure.operation, Operation::Corruption);
            check_poison(&mut disk, &failure);
        }
    }
}

#[test]
fn coherent_bad_offset_and_duplicate_index_entries_fail_completion_audit() {
    for offset in [1, u64::MAX, 40] {
        let mut disk = make(StoreMode::Fingerprint);
        disk.insert(Key::Fingerprint(0)).unwrap();
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&offset.to_le_bytes());
        let checksum = offset
            .to_le_bytes()
            .iter()
            .fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
                (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
            });
        bytes[8..].copy_from_slice(&checksum.to_le_bytes());
        change(&disk, "index", 32, &bytes);
        let failure = disk.contains(Key::Fingerprint(0)).unwrap_err();
        assert_eq!(failure.operation, Operation::Corruption);
        check_poison(&mut disk, &failure);
    }
    let mut disk = make(StoreMode::Fingerprint);
    disk.insert(Key::Fingerprint(0)).unwrap();
    let bytes = fs::read(disk.directory.join("index")).unwrap();
    // Duplicate a coherent offset under its new slot identity.
    let offset = u64::from_le_bytes(bytes[32..40].try_into().unwrap());
    let mut duplicate = [0; 16];
    duplicate[..8].copy_from_slice(&offset.to_le_bytes());
    let checksum = (1 ^ offset)
        .to_le_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    duplicate[8..].copy_from_slice(&checksum.to_le_bytes());
    change(&disk, "index", 48, &duplicate);
    assert_eq!(disk.close().unwrap_err().operation, Operation::Corruption);
    assert!(disk.stats().cleanup_completed);
}

#[test]
fn resource_limits_are_deterministic_and_precede_allocation() {
    let mut disk = DiskSeen::create(
        StoreMode::Exact,
        &std::env::temp_dir(),
        Limits {
            key_bytes: 8,
            ..Limits::default()
        },
    )
    .unwrap();
    disk.fault = Some((Operation::Allocate, 0));
    let failure = disk.insert(Key::Exact(b"ninebytes")).unwrap_err();
    assert_eq!(failure.operation, Operation::Admission);
    assert_eq!(disk.stats().physical_entries, 0);
    check_poison(&mut disk, &failure);
    let mut disk = DiskSeen::create(
        StoreMode::Fingerprint,
        &std::env::temp_dir(),
        Limits {
            entries: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    disk.insert(Key::Fingerprint(1)).unwrap();
    assert_eq!(
        disk.insert(Key::Fingerprint(1)).unwrap(),
        InsertOutcome::Present
    );
    assert_eq!(
        disk.insert(Key::Fingerprint(2)).unwrap_err().operation,
        Operation::Admission
    );
    assert!(disk.close().is_err());
    let mut disk = DiskSeen::create(
        StoreMode::Fingerprint,
        &std::env::temp_dir(),
        Limits {
            summary_pairs: 0,
            ..Limits::default()
        },
    )
    .unwrap();
    disk.insert(Key::Fingerprint(1)).unwrap();
    disk.insert(Key::Fingerprint(2)).unwrap();
    assert_eq!(
        disk.tlc_min_gap(1).unwrap_err().operation,
        Operation::Summary
    );
    assert!(disk.close().is_err());
}

#[test]
fn cleanup_never_removes_caller_added_files() {
    let mut disk = make(StoreMode::Exact);
    let directory = disk.directory.clone();
    fs::write(directory.join("caller-owned"), b"retain").unwrap();
    assert_eq!(disk.close().unwrap_err().operation, Operation::Cleanup);
    assert_eq!(fs::read(directory.join("caller-owned")).unwrap(), b"retain");
    assert!(!disk.stats().cleanup_completed);
    fs::remove_file(directory.join("caller-owned")).unwrap();
    assert!(disk.close().is_err()); // first cleanup failure stays authoritative
    assert!(disk.stats().cleanup_completed);
    assert!(!directory.exists());
}

#[test]
fn committed_data_before_failed_index_is_not_an_inserted_entry() {
    let mut disk = make(StoreMode::Fingerprint);
    disk.fault = Some((Operation::Write, 2));
    let failure = disk.insert(Key::Fingerprint(7)).unwrap_err();
    assert_eq!(failure.operation, Operation::Write);
    assert!(failure.message.contains("storage exhaustion"));
    assert_eq!(disk.stats().physical_entries, 0);
    assert!(fs::metadata(disk.directory.join("data")).unwrap().len() > disk.data_end);
    check_poison(&mut disk, &failure);
}

#[test]
fn failed_rehash_commit_keeps_old_capacity_and_first_failure() {
    let mut disk = make(StoreMode::Fingerprint);
    for key in 0..4 {
        disk.insert(Key::Fingerprint(key)).unwrap();
    }
    disk.fault = Some((Operation::Rehash, 1));
    let failure = disk.insert(Key::Fingerprint(4)).unwrap_err();
    assert_eq!(failure.operation, Operation::Rehash);
    assert_eq!(disk.stats().physical_entries, 4);
    assert_eq!(disk.capacity, 8);
    assert!(disk.directory.join("index-next").exists());
    check_poison(&mut disk, &failure);
}

#[test]
fn probe_bound_refuses_instead_of_claiming_absence() {
    let mut disk = DiskSeen::create(
        StoreMode::Exact,
        &std::env::temp_dir(),
        Limits {
            probe_steps: 1,
            ..Limits::default()
        },
    )
    .unwrap();
    disk.forced_hash = Some(0);
    disk.insert(Key::Exact(b"one")).unwrap();
    let failure = disk.contains(Key::Exact(b"two")).unwrap_err();
    assert_eq!(failure.operation, Operation::Admission);
    check_poison(&mut disk, &failure);
}

#[test]
fn unavailable_files_are_io_errors_and_closed_stores_refuse_work() {
    let mut disk = make(StoreMode::Fingerprint);
    fs::remove_file(disk.directory.join("data")).unwrap();
    let failure = disk.contains(Key::Fingerprint(0)).unwrap_err();
    assert_eq!(failure.operation, Operation::Read);
    check_poison(&mut disk, &failure);
    let mut disk = make(StoreMode::Fingerprint);
    let stats = disk.close().unwrap();
    assert_eq!(stats.disk_bytes, 0);
    assert_eq!(
        disk.insert(Key::Fingerprint(0)).unwrap_err().operation,
        Operation::Admission
    );
    assert_eq!(
        disk.contains(Key::Fingerprint(0)).unwrap_err().operation,
        Operation::Admission
    );
}

#[test]
fn malformed_key_mode_and_constructor_bounds_do_no_file_work() {
    let mut disk = make(StoreMode::Fingerprint);
    disk.fault = Some((Operation::Read, 0));
    let failure = disk.contains(Key::Exact(b"wrong mode")).unwrap_err();
    assert_eq!(failure.operation, Operation::Admission);
    assert_eq!(disk.fault, Some((Operation::Read, 0)));
    assert!(disk.close().is_err());
    assert!(disk.stats().cleanup_completed);
    // A nonexistent parent distinguishes input admission from attempted creation.
    let absent = std::env::temp_dir().join(format!(
        "dive-seen-absent-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    for limits in [
        Limits {
            key_bytes: MAX_KEY_BYTES + 1,
            ..Limits::default()
        },
        Limits {
            entries: 0,
            ..Limits::default()
        },
        Limits {
            probe_steps: 0,
            ..Limits::default()
        },
    ] {
        let failure = DiskSeen::create(StoreMode::Exact, &absent, limits).unwrap_err();
        assert_eq!(failure.operation, Operation::Admission);
        assert!(!absent.exists());
    }
}
