# Fallible disk seen-state library

`dive-store` has an additive private-file library for fingerprint or exact
canonical keys. An explicit [ordinary safety route](disk-seen-safety.md) now
admits it with reported inline fallback. The default checker retains its
seen-state sets in memory. Frontier and predecessor budgets keep separate scope.

Run the Cargo example:

```sh
cargo run -p dive-store --example disk_seen --locked
```

The library API is fallible:

```rust
use dive_store::{StoreMode, seen::{DiskSeen, InsertOutcome, Key, Limits}};

let mut seen = DiskSeen::create(
    StoreMode::Exact, &std::env::temp_dir(), Limits::default(),
)?;
assert_eq!(seen.insert(Key::Exact(b"canonical-state"))?, InsertOutcome::Inserted);
assert_eq!(seen.insert(Key::Exact(b"canonical-state"))?, InsertOutcome::Present);
assert!(seen.contains(Key::Exact(b"canonical-state"))?);
let closed = seen.close()?;
assert!(closed.cleanup_completed);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Exact keys are existing canonical bytes: storage does not decode, normalize or
evaluate them. Fingerprint keys compare all 64 bits. Duplicate keys remain
duplicates even when the physical entry limit is reached. A read, write,
allocation, resource or integrity error returns `SeenError` (`DIVE-S0700`),
poisons the store and is returned by subsequent operations. A failed write never
returns `Inserted` or `Present`.

Successful insertion confirms complete in-process record/index writes. There is
no per-insert data fsync, crash-durability guarantee or restart/import API. These
are owned transient files, not checkpoints.

Drivers with an actual run owner may call `close_controlled(validate, proceed)`.
It returns the cleanup facts and whether the retained-record audit completed.
Cancellation or `validate = false` still removes owned files, but the returned
audit flag is false and cannot certify their contents. A later `close()` cannot
upgrade a discarded store into an audited one. Original storage failures remain
authoritative and retain any later cleanup failure.

`stats().physical_entries` counts fully written record/index insertions. It must
not be used as a failed model run's committed discovery count. `disk_bytes` is
the expected current data/index length, not cumulative rehash traffic; a failed
write/rehash may leave extra partial or staging bytes. `resident_key_bytes` is zero because no key
or offset collection is retained in RAM. OS page cache, paths, handles, caller
keys and at most two record-sized workspaces are outside that statement. It is
not a total RAM budget.

Default `Limits` allow one key of at most 64 MiB, 16,777,216 physical entries,
33,554,432 slots per probe and 1,000,000 pair comparisons for the collision
summary. Key bounds and arithmetic are checked before allocation/growth. The
first fingerprint-gap implementation uses a bounded quadratic scan, matching
the existing TLC subset/minimum-gap rule. Too many pairs return a typed summary
error instead of silently dropping the collision estimate. It is meaningful
against TLC when keys are TLC fingerprints; exact mode refuses that summary.

Always call `close()` before declaring completion. It audits record/index
reachability, then removes only owned files and the owned private directory.
Unknown caller-added files are retained and cause cleanup to fail. A cleanup
error prevents a successful close; an earlier storage failure remains the
primary cause, with cleanup failure attached. Drop cleanup is only a fallback.
Cleanup can remove some owned files before failing; the poisoned store cannot
be reused afterward. Retain the failure and cleanup flag before discarding it.
Checksums detect accidental corruption and provide no archive authenticity or
independent search proof.

The first library permits coordinated exclusive calls. Parallel read views, direct disk-file
recovery and graph routing remain pending. Core safety checkpoints separately
rebuild membership from saved concrete discoveries through the existing fallible
insertions; they do not import old disk files. Ordinary inline safety admission
and separately budgeted seen/frontier/predecessor combinations use the explicit
[engine contract](disk-seen-safety.md). See
[ADR 0063](../decisions/0083-storage-and-checkpoint.md).

The source-bound control study
records full key identity, rehash, poisoned-error and owned-cleanup controls, the
actual Cargo example and current engine regression gates. It supplies library
evidence; the later engine increment supplies its own evidence and option contract. It
does not establish checkpoint support.
