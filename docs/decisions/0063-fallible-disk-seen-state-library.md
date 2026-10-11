# 0063: Fallible disk seen-state library

- **Status:** accepted
- **Date:** 2026-10-11

## Context

Frontier and predecessor spill do not bound the resident `FpSet`/`ExactSet`.
Their `StateSet` interface returns Boolean membership/insertion answers. An I/O
failure cannot be represented as absence, a duplicate or a successful insertion.
The pinned `tlc2.tool.fp.FPSet.put`, `contains` and `checkFPs` at `94d0c50`
declare `IOException`; `DiskFPSet.diskLookup` and `checkFPs` retain that boundary.
TLC's disk set retains a page index in memory. This native format does not copy
the Java layout or promise checkpoint interoperability.

## Decision

Add `dive_store::seen::DiskSeen` with explicit fallible membership/insertion,
typed operations, first-failure poisoning and explicit owned cleanup. It is not
an implementation of the infallible `StateSet` trait.
`InsertOutcome::Present` corresponds to TLC `put` returning true (already seen);
`Inserted` corresponds to false. The enum prevents reversing that convention. Full fingerprint identity
includes zero and the high bit. Exact identity is the complete existing canonical
byte string, independent of placement-hash collisions. No constructor, provider,
normalization or spec callback is invoked by the store.

Private `DIVESD01` records and `DIVESI01` slots carry bounded lengths, ordinal
identity and corruption checksums. A power-of-two open-addressed index lives
entirely on disk. Growth sequentially rebuilds a private replacement index and
commits it after validation; no input-sized offset collection is allocated.
At most two key-sized workspaces are live during rehash/audit. The default
one-key limit is 64 MiB, physical entries 16,777,216, one-probe steps 33,554,432,
and collision-summary pairs 1,000,000. Bounds are deterministic admission
policies, not exact elapsed-time/CPU or total-process-memory limits.

The first collision summary is a fallible quadratic pair scan with bounded
workspace. Above its pair policy it refuses explicitly, instead of omitting
collision metadata. Exact mode does not acquire fingerprint guarantees from
this method. Explicit close validates full record/index reachability before
cleanup. Cleanup removes only known owned files and their private directory;
caller-added files cause an error and are retained. Drop is a best-effort fallback
and cannot certify completion.

Record/index commits are physical library facts: complete in-process writes,
without per-insert data fsync or any crash-durability, restart or checkpoint
import contract. They are not the model checker's ordered discovery-prefix count: staged keys can precede semantic publication.
Native engine admission is a separate increment using the ordered source commit
authority of issue #204, after the fallible protocol is integrated. Existing
memory checking, frontier/predecessor budgets, checkpoint formats and canonical
key/FP bytes are unchanged.

## Consequences

The library offers actual disk-backed key retention with zero resident key/offset
collections. OS page cache, handles, paths, caller keys, read/rehash workspaces,
candidates, traces and graphs remain outside this statement. Index probing and
quadratic summaries can be slower than the in-memory stores; no speed claim is
made. A read/write/allocation/rehash/corruption failure poisons subsequent calls,
and later cleanup errors attach to the first cause rather than replace it.

There is no CLI flag or ordinary checking admission yet, and no total-memory,
checkpoint, parallel-library or feature-family certification claim. See the
[user contract](../user/disk-seen-library.md) for the current supported boundary.
