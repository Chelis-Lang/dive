# Disk seen-state safety checking

`dive check --seen-disk-entries N` keeps the set of seen states on disk instead
of in memory, as fingerprints or, with `--store exact`, exact keys. It is off by
default. The indexed disk search runs on one worker. For multi-worker fingerprint
searches, add `--partitioned-fingerprints`. The option bounds the disk-membership workspaces, not total
search or process memory.

```sh
dive check docs/user/examples/CheckpointCounter.tla --compat=tlc --seed 7 --fp 0 --workers 8 --store exact --seen-disk-entries 1000 --output json
```

The example completes with five distinct and six generated states.

dive runs disk checking in either semantic profile and either store mode. It
works with pure VIEW, safety symmetry, constraints, and transient frontier and
predecessor spill. Exact keys encode closed mathematical values directly; the
concrete states used for evaluation and the trace stay in memory with the
live Store. Rust callers use `check_file`. Temporal properties and opaque or
deferred keys refuse by name.

dive's completed-level checkpoints rebuild disk membership from the saved
states; use `check_file_checkpoint` or the CLI as described in
[storage and recovery](storage-recovery.md). A read, write, key-limit, audit or
cleanup failure stops the run without a certified result. A canceled audit of
the retained records still cleans up, and does not vouch for the records it did
not reach.

## Multi-worker fingerprint membership

For a large breadth-first safety search, create a scratch directory on the
machine running the checker. Set the entry limit to the maximum number of
distinct fingerprints that may be admitted across the entire run.

```sh
mkdir -p scratch
dive check Model.tla --config Model.cfg --workers 32 \
  --partitioned-fingerprints --seen-disk-entries 50000000000 \
  --seen-directory scratch --output json
```

Workers expand states concurrently. The coordinator looks up fingerprints in
batches and admits each discovery in source and successor order. It keeps up
to 262,144 recent keys, a window of up to 32,768 queries, and a small page
index in memory. Sealed runs use eight bytes per key and two resident index
words per 512 keys before allocator overhead. Smaller runs also keep a one-byte-per-key membership filter in RAM. Filters are limited to runs with at most 33,554,432 keys. Active filters use less than 64 MiB at the normal flush size. A merge can briefly retain the old filters, an intermediate and the replacement together. Files contain the complete keys, and the closing audit reads every page. Compaction temporarily needs
space for both inputs and their replacement. The OS can also cache file pages
in memory. Frontier rows and the parent log still need their own capacity.

Use this option when resident fingerprint tables limit the size of the search.
At five million SlateDB discoveries it saved 80–100 MiB of peak process memory
and increased elapsed time. The [scale measurements](../developer/large-models.md#external-fingerprint-membership)
give the individual runs. A checker using fingerprints still has a collision
risk. Use `--store exact` on a smaller control to check full state identities.

The global entry limit ranges from 1 through 1,152,921,504,606,846,975. This
route requires fingerprint safety and resident candidate payloads. Use the
indexed store for exact keys or checkpoint and recovery. Temporal checking,
distributed workers and an independent compressed frontier refuse this
combination. Leave the indexed key, probe and pair limits at their defaults.
Specify `--sequential` to expand inline. Rust callers set
`SeenOptions::partitioned` and `limits.entries`.

A completed audit streams the keys in order and checks file lengths, page
checksums, uniqueness and the entry count. The report then includes
`seen_storage.native_fingerprint_min_gap`, the smallest gap between adjacent
native fingerprints in the same high-bit half. The value is 9,223,372,036,854,775,807
when neither half contains a pair with a smaller gap. It describes the actual
stored keys, including any replay insertions. It cannot certify collision-free
state identities. Read `resident_key_collection_bytes` as the capacity of the
key, query and page-index collections. Use process RSS and cgroup memory for
host sizing.

## Run an example

```sh
cargo run -p dive-cli --release -- check \
  crates/dive-api/examples/seen-safety/Counter.tla \
  --compat=tlc --seed 7 --fp 0 --workers 8 \
  --seen-disk-entries 1000 --output json
```

Add `--store exact` for full canonical keys, or drop `--compat=tlc` for the
default profile. Without the disk flags the same command gives the same keys,
safety outcome, concrete states and counts. The report keeps eight requested
workers, sets `options.workers.used` to one, and gives the reason: concurrent
disk reads are not supported.

`--seen-directory DIR` names a parent directory, which must already exist. The
checker creates a private child directory there and audits and removes the
files it owns. It never overwrites or recursively removes your files, including
files you add to its private directory.

A directory without `--seen-disk-entries`, malformed limits, and combinations
with temporal checking or recovery are rejected before the model loads.
Binding and ASSUMEs run before the disk store is created, so if creation fails,
their output can still appear, with zero Init and search workers reported.

The Rust request is the same checker request:

```rust
use dive_api::{CheckRequest, SeenOptions, Store, check_file};

let request = CheckRequest {
    file: "crates/dive-api/examples/seen-safety/Counter.tla".into(),
    compat_tlc: true,
    store: Store::Exact,
    seed: Some(7), fp: Some(0), workers: Some(8),
    seen_disk: Some(SeenOptions::default()),
    ..CheckRequest::default()
};
let report = check_file(&request)?;
let output = report.to_json();
assert_eq!(output["options"]["workers"]["used"], 1);
assert_eq!(output["seen_storage"]["after_cleanup"]["cleanup_completed"], true);
# Ok::<(), dive_api::ApiError>(())
```

To run it in all four profile and store combinations:

```sh
cargo run -p dive-api --example disk_seen_safety --locked
```

## Indexed store limits

`SeenOptions::limits` and `SeenLimits` set the same limits as the CLI:

| Option | Default | Meaning |
| --- | ---: | --- |
| `--seen-disk-entries N` | off | Maximum key records written; 1 through 4,294,967,295 |
| `--seen-key-bytes N` | 67,108,864 | Workspace for one record's key, from 8 bytes through 64 MiB |
| `--seen-probe-steps N` | 33,554,432 | Maximum index slots examined per lookup or rehash insert; positive |
| `--seen-summary-pairs N` | 1,000,000 | Pair comparisons for the fingerprint-gap summary; zero is valid |

A duplicate is still recognized when the entry limit is full. A new state
beyond the limit returns `resource-limit`, never absent, duplicate or success.
Zero entries or probe steps, or key bytes out of range, make a malformed
request. Exact keys keep all the canonical bytes and fingerprints keep all 64
bits: putting keys on disk changes neither the key scheme nor how keys compare.

Nothing about the key set is kept in RAM. The keys and states passed in,
frontiers, predecessors, candidates, trace reconstruction, graphs, file
handles and the OS page cache are outside these limits. You can add
`--frontier-memory-bytes` and `--predecessor-memory-bytes`, including zero, but
no combination of these flags is a total memory budget. Inserts do not fsync,
and the files are not crash-durable and cannot be reused after a restart.

## Reading the report

`seen_storage` reports, separately, the keys admitted, the physical entries
and the expected disk lengths before cleanup; what cleanup did; and the first
typed failure, its phase and its generated, predecessor and ordinal position.
If an audit or cleanup fails later, `deciding_event_before_failure` keeps the
event that decided the run. A key written to disk does not mean the matching
predecessor or frontier row was written. After a failure, partial writes and
staging files can exceed the expected byte counts, so that field does not
measure disk usage.

The indexed store’s fingerprint summary uses TLC's optimistic collision estimate when supplied by the engine.
When TLC's threshold calls for the actual-gap estimate and the summary limit is
exceeded, the run refuses to complete rather than leaving the field out. Exact
mode reports no fingerprint collision estimate, as in memory.

A cleanup failure prevents a completed safety result, even when the search had
already found an outcome. A counterexample kept under `resource-limit` is
diagnostic only. First errors are typed `DIVE-S0700` storage failures, and a
later cleanup failure does not replace the original cause.

## Model roles

`PROPERTY` and `SPECIFICATION` roles bind as they do in memory, through
the same binder, with the same verdict, counts, output, warnings and
counterexample. That includes effectful roles, quantified `PROPERTY` domains,
strict INSTANCE substitutions and fairness-only specifications. As in TLC, a
state-level formula used directly as a `PROPERTY` is checked only on initial
states and emits warning 2255; use `PROPERTY []P` or `INVARIANT P` to check
every state.

Missing or invalid INIT and NEXT roles are handled in the binder's usual order
and can print startup output before the error.

These refuse by name with disk seen states: temporal checking, graph and
source capture, and DFID. Simulation has no disk option. Pure safety VIEW and
symmetry keep their concrete states and independently validated
counterexamples; their combinations with randomness or liveness refuse.

```sh
cargo run -p dive-cli --release -- check \
  crates/dive-api/examples/seen-roles/Counter.tla \
  --compat=tlc --seed 7 --fp 0 --workers 8 \
  --store exact --seen-disk-entries 1000 --output json
cargo run -p dive-api --example disk_seen_roles --locked
```

This example has four states and completes cleanup. Its configuration holds
both `SPECIFICATION Spec` and `PROPERTY Bound`, and the Rust `CheckRequest`
above works with it too. The report still shows eight requested workers and one
used.

## Errors found while replaying

When an invariant raises an evaluation error, dive replays the error's
successors through the disk store, as TLC does. For example, start at `x=0`,
generate `x=1`, `x=2` and `x=3`, and divide by zero only when `x=1`. Pinned TLC
stops generating at 2 states, but its replay brings the totals to 4 distinct
and 2 queued (exit 76, invariant message 2111). Disk checking follows the same
continuation through key lookups and inserts; replay is not counted as
generated work.

```sh
cargo run -p dive-cli --release -- check \
  crates/dive-api/examples/seen-replay/ReplayNew.tla \
  --compat=tlc --seed 7 --fp 0 --workers 8 \
  --seen-disk-entries 1000 --output json
cargo run -p dive-api --example disk_seen_replay --locked
```

The checker exits 76 under TLC compatibility; the report is an error, not a
successful check. `seen_storage.replay` keeps the original error, the concrete
predecessor and the ordinal of the successor where it stopped, the frozen
`generated` count, the extra `admitted_keys`, the `queued` contribution,
`unproved_print_output` and any `halted_by` error. Keys admitted during replay
are not new predecessor or frontier rows. If replay hits an I/O or entry limit,
the result is `resource-limit`, keeping the original error and the completed
prefix. The trace is diagnostic and is not independently validated. An error at a site dive does
not recognize reports an unverified error class.

For the smallest case, where the successor was already seen, pinned TLC and
in-memory checking report 3 generated, 3 distinct and 1 queued. The
[storage and checkpoint design](../decisions/0083-storage-and-checkpoint.md)
describes the implementation and its validation. Per-action replay successor
buffers are outside the disk-key budget.

If `Print` or `PrintT` ran before or during the replay, the result is
`unsupported`, naming the repeated output. For example, adding
`PrintT("expand") /\` to `ReplayNew.Next` makes TLC print `"expand"` three
times. Dive does not claim to match TLC's error class or output here: `tlc`
and `counterexample` are null, and the original error, counts, output and
`seen_storage.replay.unproved_print_output=true` are reported as diagnostics.
Any `Print` event counts, including empty text, startup and Init output,
wrappers, configured aliases and quiet reconstruction; warnings and unused
definitions do not. Draining the output does not erase the event. This rule
can also refuse a cached or Init-only print that would not repeat. An earlier
storage or resource failure stays the first cause. In-memory checking prints
as usual.

The replay controls include real `VIEW` and symmetry counterexamples, checked
by the independent validator in both profiles and stores at 1, 4 and 8
requested workers: 144 original counterexamples validate and all 864 mutants
are rejected. The `VIEW` case has its own configured `VIEW`. The 576 disk runs
match every concrete result field of the 384 in-memory runs, and the replay
tests add 432 pure disk comparisons and 504 `Print` refusals against 104 inline
references.

A pure action `PROPERTY` uses the same replay. The pinned `ErInvReplayProp`
model reports exit 76 (error class 2111), four generated and four distinct
states, one queued state, depth 3, and the diagnostic prefix
`(0,0) -> (1,0) -> (2,0)`, the same in memory and on disk. This trace is not
independently validated. Temporal or effectful roles and error replay that
depends on `Print` refuse by name.

See the [disk seen-state library](disk-seen-library.md) and the
[storage and checkpoint design](../decisions/0083-storage-and-checkpoint.md).
