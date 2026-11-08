//! Runnable additive disk membership/cleanup example; no checker admission claim.
use std::collections::BTreeSet;

use dive_store::StoreMode;
use dive_store::seen::{DiskSeen, InsertOutcome, Key, Limits};
use dive_value::{Ctx, Value, codec::encode_canonical};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for ctx in [Ctx::tlc(), Ctx::default()] {
        let mut disk =
            DiskSeen::create(StoreMode::Exact, &std::env::temp_dir(), Limits::default())?;
        let mut memory = BTreeSet::new();
        for n in [0, 1, 2, 1, 0, 3] {
            let value = Value::tuple(vec![Value::Int(n), Value::Int(n + 1)]);
            let mut key = Vec::new();
            encode_canonical(&value, ctx, &mut key)?;
            assert_eq!(disk.contains(Key::Exact(&key))?, memory.contains(&key));
            let expected = if memory.insert(key.clone()) {
                InsertOutcome::Inserted
            } else {
                InsertOutcome::Present
            };
            assert_eq!(disk.insert(Key::Exact(&key))?, expected);
        }
        assert_eq!(disk.stats().physical_entries, memory.len() as u64);
        assert_eq!(disk.stats().resident_key_bytes, 0);
        let stats = disk.close()?;
        assert!(stats.cleanup_completed);
        assert_eq!(stats.disk_bytes, 0);
        println!(
            "{} exact keys, explicit owned cleanup complete",
            stats.physical_entries
        );
    }
    Ok(())
}
