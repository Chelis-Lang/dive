# Search storage and recovery

Safety checking keeps its search in memory by default. The options here move
parts of it to disk, reclaim memory between levels, or pause a search and
resume it later. All of them are opt-in. The
[storage and checkpoint design](../decisions/0083-storage-and-checkpoint.md)
has the details.

Ordinary in-memory safety search packs complete blocks of 256 frontier rows by variable. It restores the full handles when expanding a row. Models with fewer than 256 rows in a level keep their original layout. Temporal capture, checkpoints, distributed workers and explicit frontier or candidate spill use their own formats. [Measurements on four original configurations](../developer/large-models.md#resident-frontier-rows).

dive's safety BFS supports `--store exact` and the frontier and predecessor
budget and directory options. Exact membership resolves hash collisions by
comparing the complete canonical state; the concrete states used for
evaluation and replay keep their own representation. Parallel exact search
keeps colliding candidates until the ordered commit. A predecessor read,
write, corruption or cleanup failure stops the run as a resource stop, with
storage diagnostics and no certified counterexample.

dive's safety BFS also supports disk membership with `--seen-disk-entries`,
using one worker and complete canonical keys for exact membership. The indexed
store supports completed-level checkpoints and recovery. Adding
`--partitioned-fingerprints` uses sorted runs with parallel expansion and a
64-bit global entry limit for ordinary safety searches. See the
[multi-worker fingerprint guide](disk-seen-safety.md#multi-worker-fingerprint-membership).
dive never falls back to another engine. Exact store, spill and disk options
combined with temporal checking refuse.

[Distributed safety](distributed-safety.md) accepts frontier and predecessor
spill on the coordinator. It refuses disk seen states, candidate payload spill,
Store reclamation and every checkpoint, pause and recovery option. A worker
process can restart and retry uncommitted work while the coordinator lives,
but if the coordinator is lost the run cannot be resumed.

dive's frontier stores each state as a pair of canonical and original handles
into one live Store, so values are not re-encoded. Its private files cannot be
recovered after a restart or read by another run. Each decoded chunk holds at
most 1,024 state pairs and does not count against the encoded queue budgets.
The Store and its history, seen sets, candidates, arenas and decoded chunks are
outside those budgets, so the report's value codec node limit is null.

Before reporting a result, dive validates every retained frontier record,
including unvisited ones, checking the run budget every 128 rows. A budget stop
closes and cleans up the remaining files without scanning them and claims no
completeness. Cancelling during decoding or validation is a budget stop; a read,
write, corruption or cleanup failure means no certified result, with storage
diagnostics in the report.

## Reclaiming expanded values

Pure safety BFS accepts `--store-compact-bytes N`:

```sh
dive check docs/user/examples/Counter.tla --store-compact-bytes 0 --frontier-memory-bytes 0 --predecessor-memory-bytes 0 --output json
```

The matching `CheckRequest` and `CheckOptions` field is
`store_compact_bytes: Option<usize>`. Zero forces reclamation after Init and each
completed source level. A positive number triggers it when the Store's measured
retained-byte lower bound exceeds that number.

When the option is omitted, dive uses a default trigger of 256 MiB (268435456
bytes) in both profiles for pure fingerprint safety BFS. That excludes runs with
any of:

- exact membership;
- a spill or disk-membership budget;
- runtime effects or temporal properties;
- sampling, checkpoints, the debugger or distributed workers.
 Reclamation preserves every
result: verdict, counts, depth, counterexample and warnings are unchanged. Below
the trigger it never runs, so most searches pay nothing. Under the default, a
search that reclamation cannot serve keeps its resident Store instead of refusing,
and the report says why in `core.metrics.store_reclamation.skipped`; an explicit
`--store-compact-bytes` refuses those searches by name instead. The option is a
trigger, **not a hard memory cap**: all live values remain mandatory, and values
can grow during a source level before its workers join.

At those boundaries dive rebuilds the Store from the compiled startup and
the live next frontier, and releases expanded values that nothing live refers
to. Canonical state identities and the original values stay separate. How
values were built (raw or normalized, aliases, tuple or function), dynamic name
tokens and warning order are all kept; evaluator caches and worker arenas are
cleared. A disk frontier gets new files for the run, streamed one row pair at a
time. Counterexamples are rebuilt from predecessors and ordinals as before and
are still validated independently.

Both profiles, fingerprint/exact membership, VIEW, safety symmetry, requested
worker widths and existing candidate/frontier/predecessor/disk storage policies
are supported. Exact membership on this route stores full closed mathematical
key bytes independently of the Store; symbolic/deferred keys outside that
encoding refuse by name. Disk seen states still use one worker. Recovery
can turn on or change this trigger after restoring a checked archive:

```sh
dive check docs/user/examples/CheckpointCounter.tla --seed 7 --fp 0 --checkpoint counter.core.cp --checkpoint-after-levels 2 --checkpoint-counters retain --output json
dive check docs/user/examples/CheckpointCounter.tla --recover counter.core.cp --store-compact-bytes 0 --checkpoint-counters retain --output json
```

Checkpoint pause with reclamation, temporal capture, runtime effects and
sampling refuse this policy by name. Checkpoint discovery residency and paging
of temporal graphs are not covered by it.

JSON `core.metrics.store_reclamation` reports `trigger_bytes`, `trigger_source`
(`"requested"`, `"default"` or `null`), `skipped`, `collections`,
`reclaimed_nodes`, `peak_before_bytes_lower_bound`,
`peak_after_bytes_lower_bound` and `peak_after_nodes`. These are observations
across completed collections, not a peak of total RSS. The final
`core.metrics.store.memory` snapshot can include values rebuilt for the
reported trace after checking stops. An allocation or address-space failure
during reclamation or key work reports `DIVE-S0203` resource-limit, lists
`failures`, and certifies no trace.

The trigger does not count:

- the live frontier, the permanent startup and its fixed image;
- encountered names, seen keys and predecessor records;
- values kept for the reported trace and its replay;
- mapping and codec workspaces;
- the overlap of the old and new Store while one replaces the other;
- allocator and OS overhead.

Combine it with the storage budgets and the whole-run time and memory limits
as needed. A filesystem, corruption or cleanup failure means no certified
result; canceling is an incomplete resource stop. See the
[completed-level store reclamation design](../decisions/0149-completed-level-store-reclamation.md).

## Candidate payload spill

Pure safety BFS also accepts
`--candidate-memory-bytes N` and `--candidate-directory DIR`:

```sh
dive check docs/user/examples/Counter.tla --workers 8 --candidate-memory-bytes 0 --output json
```

Zero forces disk; without the option, candidates stay in memory in either
profile. The
matching `CheckRequest` and `CheckOptions` fields have the same names. A candidate
directory without a budget is rejected before opening the model. Runtime
effects and temporal capture refuse this policy by name.

The budget counts each worker's pending encoded occurrences and compound DAG
payloads, including the queue index. A chunk retains at most one source per
worker. Values are encoded while still owned by the worker's Arena and imported
into the shared Store only when the ordered commit consumes them. How each
value was built (raw or normalized, tuple or function) is kept. Spilling a
queue does not first copy its speculative values into the Store.

Candidate spill works with both profiles, fingerprint and exact membership,
VIEW, safety symmetry, disk seen states, frontier and predecessor spill, and
Safety checkpoints and recovery. Encoding one record uses a workspace
limited to 64 MiB and 128 levels of nesting; shared subvalues stay shared, and
the per-operation workspaces are bounded. The queue budget does not cover the
workers' active generators and arenas, dynamic names, the Store, other caches or
total process memory.

JSON `core.metrics.candidate_storage` reports the per-worker budget, aggregate
peak encoded capacity, committed spill bytes, remaining bytes and failures.
Read, write, checksum and cleanup failures produce a resource stop without a certified counterexample. Completed queues are removed; they cannot be recovered after
restart or used as trace evidence. See the [owned candidate payload spill design](../decisions/0143-owned-candidate-payload-spill.md).

## Transient frontier and predecessor storage

### Independently stored frontier values

For a wide frontier of compound values, use `--independent-frontier`. Core writes the identity and original values of up to 64 consecutive rows in one self-contained chunk. This allows the value store to release fully written rows between committed worker batches. The default reclamation trigger for this option is 64 MiB of measured store storage. Set `--store-compact-bytes N` to change it. A partial chunk, worker arenas, fingerprint table, parents and file cache still use memory. Allow sufficient local disk for both active breadth-first levels.

```sh
mkdir -p spill
dive check BPConProof.tla --config BPConProof.cfg --workers 8 --independent-frontier --frontier-directory spill --output json
```

This option applies to ordinary fingerprint safety checking with resident candidates and membership. Both profiles retain the concrete original values for trace replay. Each chunk uses LZ4 when doing so saves disk space. Both the stored record and decoded DAG have a 64 MiB limit, with DAG nesting limited to 128. Files are transient. Invalid records, storage failures or a budget stop prevent certification. JSON reports `frontier_storage.mode = "independent-chunks"`, committed spill bytes and Store collections. The [matched prefixes](../developer/large-models.md#high-fanout-parallel-safety) show the RAM and wall-time tradeoff against the ordinary disk frontier.

### Frontier handles and predecessor records

Safety BFS has independent opt-in frontier and predecessor budgets:

```sh
dive check docs/user/examples/Counter.tla --compat=tlc --seed 7 --fp 0 --workers 8 --frontier-memory-bytes 0 --output json
dive check docs/user/examples/Counter.tla --compat=tlc --seed 7 --fp 0 --workers 8 --predecessor-memory-bytes 0 --output json
```

Zero forces disk storage from the start. Omit the flags to keep the frontier
and predecessor records in memory. `--frontier-directory DIR` and `--predecessor-directory DIR`
choose an existing parent directory; without them dive uses a private
subdirectory of the system temporary directory. A directory without its matching
budget is rejected (`DIVE-A0100`) before the model is opened. The Rust
counterparts are the same-named fields on `CheckRequest` and `CheckOptions`.

- The **frontier budget** counts the encoded current and next queues. Spilled
  queues keep a fixed-size index on disk and no growing RAM offsets.
- The **predecessor budget** counts resident parent/ordinal capacity. Spilled
  records are fixed 16-byte entries. Reconstructed paths have a separately
  reported 1,000,000-step limit.

Neither budget limits total process memory, the seen set, candidate vectors,
decoded values, temporal graphs or reconstructed paths. Both work with both
profiles and with `--store fp` and `--store exact`, and they preserve ordered
search, partial-stop counts and concrete counterexamples. Simulation, DFID and
temporal graph capture refuse these flags by name.

JSON reports `frontier_storage` and `predecessor_storage`: budgets, peak
resident capacity, committed bytes and cleanup status.
`write_accounting_complete: false` means a filesystem failure may have left
partial bytes. Read, write, corruption and cleanup failures are explicit
`resource-limit` results. They clear certified counterexamples and never report
success. These files are transient search storage, not checkpoints. The
transient queue files use structural format 2 (`DIVEFQ02`/`DIVEFI02`); version-1
files refuse.

Structural format 2 preserves whether a value was built as a literal tuple or
as a function over consecutive integers. This matters for diagnostics: applying
such a function to a string must keep its function-domain error after spill or
recovery. Canonical state keys and fingerprints still use mathematical equality, so
comparing values cannot tell the two representations apart; only the error
message from applying the value can. Version-1 trace JSON is a separate
transport format.

## Native checkpoints

Checkpoints are a separate, bounded, safety-only feature. An archive in an
older pre-release format is rejected with `DIVE-S0400` before evaluation.
Recovery supports both profiles, fingerprint and exact stores, VIEW,
safety symmetry, a changed worker count, transient spill and disk membership
(one worker). It resumes the saved frontier without re-enumerating Init as search work.

```sh
dive check docs/user/examples/CheckpointCounter.tla --compat=tlc --seed 7 --fp 0 --workers 4 --checkpoint counter.core.cp --checkpoint-after-levels 2 --checkpoint-counters retain --output json
dive check docs/user/examples/CheckpointCounter.tla --compat=tlc --workers 8 --recover counter.core.cp --checkpoint-counters retain --output json
```

The Rust API is `check_file_checkpoint(request, checkpoint)`. dive's
`DIVECC02` archive retains the compiled startup prefix, owned Store DAG,
complete original/canonical discovery rows, keys, parents/ordinals and ordered
frontier. A `DIVECC01` archive, whose Store images carried a retired
construction-history table, is rejected with `DIVE-S0400` ("archive format
DIVECC01 (legacy construction history) is no longer read"). A recovered run
creates new transient files and never reuses the old run's frontier files.
State values are not re-encoded. Source, executable, config
bytes, semantic policy, seed/fingerprint and strategy switches (`DIVE_CORE_OPT`) must match.
Workers and resource or storage budgets may change. Unknown deferred
providers after startup, runtime effects and temporal graphs refuse by name.

Only checkpoint-enabled runs retain the extra discovery rows. The aggregate
64 MiB archive and 65,536-state/level admission bounds do not bound live Store,
codec workspaces or process memory. There is no separate per-state image
limit (`max_state_bytes` is null). Recovery-only runs release
expanded discovery rows after rebuilding keys and may continue under the usual search budgets; they do not automatically save a new archive. Disk files remain
transient: native checkpoints reconstruct membership from concrete discovery
rows. See the [core safety recovery design](../decisions/0141-owned-core-safety-recovery.md).

The archive's source identity includes the module name and the exact bytes of
any local Java override classes beside it. The supported Hanoi `Bits.class` can
pause and recover with unchanged class bytes; removing or changing a class
rejects recovery before any saved state is used. Unknown local classes refuse at
binding. Worker choices are not part of the archive identity.

```sh
dive check docs/user/examples/CheckpointCounter.tla --compat=tlc --seed 7 --fp 0 --workers 4 --checkpoint counter.cp --checkpoint-after-levels 2 --output json
dive check docs/user/examples/CheckpointCounter.tla --compat=tlc --workers 8 --recover counter.cp --output json
```

The first command pauses after two completed levels, with three discovered states
and one queued state. The second recovers to five distinct and six generated
states. `--checkpoint-after-levels 0` pauses after Init. A pause has
`checkpoint.status: paused-incomplete` and an incomplete resource-stop result; it
is not a safety proof. If the search finishes before the boundary, the normal
verdict is returned with a note that no checkpoint was committed.

Recovery loads the saved states, visited keys, predecessor records and frontier
order; it does not rerun Init. Each saved key is recomputed through the configured
path (symmetry, then VIEW, then the store), so pure VIEW and safety symmetry are
supported. Archives keep the original concrete states. A key mismatch is an
archive refusal, not a model error. Counterexamples are reconstructed concretely
and can be independently validated.

- **Counters.** TLC resets generated and initial counters on recovery, and both
  profiles do so by default. `--checkpoint-counters retain` keeps the
  uninterrupted totals. It is a named result-changing option, so TLC-profile runs
  report `tlc+overrides`. Verdict, trace, distinct count and depth are unchanged.
- **Workers.** Worker width and `--sequential` may change on recovery without
  changing the ordered result. Seed and fingerprint defaults come
  from the archive. Runtime string constructors are supported; TLC mode uses an
  inline one-worker fallback for their encounter order.
- **Identity.** The executable, model, exact configuration bytes, profile, store
  and evaluation settings must match. Even a changed configuration comment needs
  a new snapshot.
- **Bounds.** The archive is limited to 64 MiB, 65,536 discovered states and
  depth 65,536.
  These are archive admission bounds, not a RAM budget. Transient frontier and predecessor
  budgets can be combined with checkpoints and may change on recovery.
- **Commits.** Archives are written to a private staging file, synced and
  atomically committed without replacing an existing file. Failures are reported;
  partial staging files are not checkpoints.
- **Format.** Archives use format `DIVECC02`. Archives in older pre-release
  formats refuse with `DIVE-S0400` rather than guess; recreate them with the
  current executable.
- **Refusals.** Liveness, random or effectful execution, checker reads,
  effectful configuration aliases and evaluator-driven deferred values refuse by
  name in both profiles. Behavior-graph recovery is unsupported.

Archives are trusted local native files, not Java-interoperable archives or model
certificates. Checksums detect corruption; they are not signatures.

## Disk membership

[`dive_store::seen::DiskSeen`](disk-seen-library.md) keeps full fingerprint or
canonical exact keys on disk, with fallible operations and explicit cleanup.
Seen sets stay in memory by default, and the frontier and predecessor flags do
not set a seen-set budget.

[Disk seen-state checking](disk-seen-safety.md) uses it to keep seen states on
disk for one run, on one worker, with independently validated counterexamples.
It combines with the frontier and predecessor budgets and is not a total memory
budget. Behavior graphs and temporal combinations refuse before work.
Recovery reconstructs disk membership from its saved
concrete discoveries; the transient disk files themselves are not imported.
