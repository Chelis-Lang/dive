# Disk seen-state library

`dive-store` has a library that keeps a set of fingerprint or exact canonical
keys in private files on disk. `dive check` uses it when you opt in to
[disk seen-state checking](disk-seen-safety.md); by default the checker keeps
seen states in memory. Frontier and predecessor spill have budgets of their
own.

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

A successful insert means the record and index writes completed in this
process. Inserts do not fsync, and there is no crash durability and no API to
restart from or import the files. They are transient files owned by the store,
not checkpoints.

A driver that owns the run may call `close_controlled(validate, proceed)`.
It returns what cleanup did and whether the audit of the retained records
completed. Cancelling, or passing `validate = false`, still removes the owned
files, but the audit flag is false and says nothing about their contents. A
later `close()` cannot turn a discarded store into an audited one. The first
storage failure stays the reported cause, with any later cleanup failure
attached.

`stats().physical_entries` counts fully written record and index inserts. Do
not use it as the number of states a failed model run discovered. `disk_bytes`
is the expected current length of the data and index files, not the total
written by rehashing; a failed write or rehash may leave extra partial or
staging bytes. `resident_key_bytes` is zero because no keys or offsets are kept
in RAM. That figure leaves out the OS page cache, paths, handles, the caller's
keys and at most two record-sized workspaces, so it is not a RAM budget.

Default `Limits` allow one key of at most 64 MiB, 16,777,216 physical entries,
33,554,432 slots per probe and 1,000,000 pair comparisons for the collision
summary. Key sizes and size arithmetic are checked before anything is
allocated or grown. The fingerprint-gap summary uses a bounded quadratic scan
that follows TLC's minimum-gap rule. Too many pairs return a typed summary
error; the collision estimate is never dropped silently. The summary is
comparable with TLC's when the keys are TLC fingerprints, and exact mode
refuses it.

Call `close()` before you treat the run as complete. It audits that every
record and index entry is reachable, then removes only the files and private
directory the store owns. Files the caller added are left in place and make
cleanup fail. A cleanup error means the close did not succeed; an earlier
storage failure stays the main cause, with the cleanup failure attached.
Cleanup on drop is only a fallback. Cleanup can remove some owned files before
it fails, and a poisoned store cannot be reused, so keep the failure and the
cleanup flag before discarding it. Checksums detect accidental corruption only;
they do not authenticate the files.

Calls must be exclusive: the library has no parallel read views, no recovery
from its disk files and no graph routing. Core safety checkpoints rebuild
membership from the saved discovered states through the same fallible inserts;
they do not import old disk files. For how `dive check` combines disk seen
states with frontier and predecessor budgets, see
[disk seen-state checking](disk-seen-safety.md) and the
[storage and checkpoint design](../decisions/0083-storage-and-checkpoint.md).
