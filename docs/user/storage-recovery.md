# Search storage and native recovery

Explicit [distributed safety](distributed-safety.md) admits coordinator-owned
frontier/predecessor spill, but currently refuses disk Seen, candidate payload
spill, Store reclamation/remapping and all distributed checkpoint/pause/recovery
combinations.
A worker process can restart and retry uncommitted work in a live coordinator;
coordinator loss does not leave a durable distributed continuation.

This guide covers the opt-in disk storage for safety search and native
checkpoint/recovery. The design decision is
[ADR 0083](../decisions/0083-storage-and-checkpoint.md).

Core (the default engine) safety BFS supports `--store exact` and the frontier and
predecessor budget/directory options. Exact membership resolves hash collisions by comparing
complete canonical mathematical state identities; representation and receiver
history stay attached to the concrete states used for evaluation and replay.
Parallel exact search retains candidate collisions until ordered commit.
Predecessor read, write, corruption and explicit-cleanup failures produce a
resource stop with storage evidence and no certified counterexample.
Core safety BFS also supports opt-in disk membership with `--seen-disk-entries`,
using one actual worker and complete canonical keys for exact membership.
Core never falls back. Core also supports completed-level safety checkpoint
and recovery with these storage choices. Temporal exact/spill/disk combinations
remain unsupported.

The core frontier stores paired canonical and original opaque handle rows under
one retained live Store. Original receiver classes are preserved; values are not re-encoded. Its
private files cannot be recovered after restart or decoded by another run.
Each decoded chunk contains at most 1,024 state pairs and is outside the encoded
queue budgets. Retained Store/history allocations, seen sets, candidates, arenas
and decoded chunks remain outside those component budgets. The report's value
codec node limit is therefore null.

Before certifying a result, the core validates all retained frontier records,
including unvisited rows, with the actual run control checked every 128 rows.
Budget stops close and clean the remaining files without scanning unprocessed
records or asserting completeness. Cancellation during decoding or validation
keeps its budget-stop meaning; read, write, corruption and cleanup failures
invalidate certification and retain explicit storage diagnostics.

## Reclaiming expanded Core values

Explicit pure Core safety BFS accepts `--store-compact-bytes N`:

```sh
dive check docs/user/examples/Counter.tla --store-compact-bytes 0 --frontier-memory-bytes 0 --predecessor-memory-bytes 0 --output json
```

The matching `CheckRequest` and `CheckOptions` field is
`store_compact_bytes: Option<usize>`. Zero forces reclamation after Init and each
completed source level. A positive number triggers it when the Store's measured
retained-byte lower bound exceeds that number.

When the option is omitted, Core uses a default trigger of 256 MiB (268435456
bytes) in both profiles for pure fingerprint safety BFS: no exact membership, no
spill or disk-membership budget, no runtime effects, temporal properties,
sampling, checkpoint, debugger or distributed workers. Reclamation preserves every
result: verdict, counts, depth, counterexample and warnings are unchanged. Below
the trigger it never runs, so most searches pay nothing. Under the default, a
search that reclamation cannot serve keeps its resident Store instead of refusing,
and the report says why in `core.metrics.store_reclamation.skipped`; an explicit
`--store-compact-bytes` keeps the named refusals listed below. The option is a
trigger, **not a hard memory cap**: all live values remain mandatory, and values
can grow during a source level before its workers join.

At those boundaries dive replaces the accepted-value Store with its compiled
startup and live next frontier. Expanded payloads without a live root are
released. Canonical state identities and original receiver/history rows stay
separate. Raw/normalized origins, aliases, tuple/function classes, dynamic name
tokens and warning order survive; evaluator caches and worker Arenas are cleared.
Disk frontiers receive fresh owner-bound files, streamed one paired row at a
time. Concrete witnesses use the existing predecessor/ordinal replay and still
undergo independent validation.

Both profiles, fingerprint/exact membership, VIEW, safety symmetry, requested
worker widths and existing candidate/frontier/predecessor/disk storage policies
are supported. Exact membership on this route stores full closed mathematical
key bytes independently of the Store; symbolic/deferred keys outside that
encoding refuse by name. Disk seen still uses one actual worker. Core recovery
may enable or change this trigger after restoring a checked archive:

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
among completed collection transactions; they are not a total-RSS peak. The
ordinary final `core.metrics.store.memory` snapshot can include values rebuilt
for the selected word after checking stops.
Allocation/address-space failures in reclamation or detached-key work report
`DIVE-S0203` resource-limit with explicit `failures`; they certify no word.

The live frontier, permanent startup and its fixed image, encountered names,
seen keys, predecessor records, selected trace/replay values, mapping/codec
workspaces, temporary old/new Store overlap and allocator/OS overhead remain
outside the trigger. Combine the independent storage budgets and whole-run
time/RSS controls as needed. Filesystem/corruption/cleanup failures prevent
certification; cancellation remains an incomplete resource stop. See
[ADR 0149](../decisions/0149-completed-level-store-reclamation.md).

## Candidate payload spill

Core pure safety BFS also accepts
`--candidate-memory-bytes N` and `--candidate-directory DIR`:

```sh
dive check docs/user/examples/Counter.tla --workers 8 --candidate-memory-bytes 0 --output json
```

Zero forces disk; omission retains resident execution in either profile. The
matching `CheckRequest` and `CheckOptions` fields have the same names. A candidate
directory without a budget is rejected before opening the model. Runtime
effects and temporal capture refuse this policy by name.

The budget counts each worker's pending encoded occurrences and compound DAG
payloads, including the queue index. A chunk retains at most one source per
worker. Values are encoded while still owned by the worker's Arena and imported
into the shared Store only during ordered consumption. Raw/normalized constructor
origins and original tuple/function classes are retained. Spilling a queue does
not first intern all its speculative payloads into the Store.

This works with both profiles, fingerprint/exact membership, VIEW, safety
symmetry, disk seen, frontier/predecessor spill and Core safety checkpoint/recovery.
One-record codec workspaces have a separate 64 MiB record bound and a 128-level
nesting bound. Identity closure preserves DAG sharing, reuses closed roots and
shares unchanged immutable payloads when only a representation header changes;
its per-operation visited map and child-copy workspace are bounded. The separate
constructor-origin index retains at most 1 MiB per pending source, preserving
aliases across that source's records. Exhaustion stops with named `DIVE-S0202`
resource-limit; this index is never charged as accepted Store payload. Active generator roots/Arena, dynamic names, accepted Store/history,
other caches and total process memory are outside the queue budget. Generator
roots are never reset merely to make a queue budget fit.

JSON `core.metrics.candidate_storage` reports the per-worker budget, aggregate
peak encoded capacity, committed spill bytes, remaining bytes and failures.
Separate `peak_origin_index_bytes` / `origin_index_bytes` fields report actual
retained codec index capacity; the per-worker index limit is explicit.
Read, write, checksum and cleanup failures produce a resource stop without a
certified word. Completed queues are removed; they cannot be recovered after
restart or used as trace evidence. See [ADR 0143](../decisions/0143-owned-candidate-payload-spill.md).

## Transient frontier and predecessor storage

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
recovery. Canonical state keys and fingerprints still use mathematical equality.
The regression suite covers application, independently validated invariant
witnesses, and tuple/function comparison across both profiles, both stores,
workers 1/4/8, forced frontier/predecessor spill and level-zero recovery. Comparison
alone does not distinguish the stored representation. Application detects the
representation-dependent diagnostic; the witness controls independently validate
the mathematical trace and cannot detect the lost representation bit. Version-1
trace JSON remains a separate transport format and does not establish storage
correctness.

## Native checkpoints

Checkpoints are a separate, bounded, safety-only feature. An archive in an
older pre-release format is rejected with `DIVE-S0400` before evaluation.
Core recovery supports both profiles, fingerprint/exact, VIEW, safety symmetry,
changed requested worker widths, transient spill and disk membership (one actual
worker). It resumes the saved frontier without re-enumerating Init as search work.

```sh
dive check docs/user/examples/CheckpointCounter.tla --compat=tlc --seed 7 --fp 0 --workers 4 --checkpoint counter.core.cp --checkpoint-after-levels 2 --checkpoint-counters retain --output json
dive check docs/user/examples/CheckpointCounter.tla --compat=tlc --workers 8 --recover counter.core.cp --checkpoint-counters retain --output json
```

The additive Rust API is `check_file_checkpoint_engine(request, checkpoint,
Engine::Core)`; `check_file_checkpoint` uses the default engine, Core. Core's
`DIVECC01` archive retains the compiled startup prefix, owned Store DAG,
complete original/canonical discovery rows, keys, parents/ordinals and ordered
frontier. Its Store image keeps the retired construction-history table as an
empty count, so the byte layout is unchanged; an archive whose image records
construction history is rejected with `DIVE-S0400` ("legacy construction
history") rather than restored. New transient files
receive fresh run ownership; old live frontier bytes cannot be reused. It does
not re-encode state values. Source, executable, config
bytes, semantic policy, seed/fingerprint and Core strategy switches must match.
Workers and resource/storage budgets may change. Unknown post-startup deferred
providers, runtime effects and temporal graphs refuse explicitly.

Only checkpoint-enabled Core runs retain the extra discovery rows. The aggregate
64 MiB archive and 65,536-state/level admission bounds do not bound live Store,
codec workspaces or process memory. There is no separate per-state image
limit (`max_state_bytes` is null). Recovery-only runs release
expanded discovery rows after rebuilding keys and may continue under ordinary
search budgets; they do not automatically save a new archive. Disk files remain
transient: native checkpoints reconstruct membership from concrete discovery
rows. [ADR 0141](../decisions/0141-owned-core-safety-recovery.md) records this
persistent owned-history contract.

Archive source identity
includes the module name and exact bytes of adjacent local Java override classes.
The supported Hanoi `Bits.class` can pause and recover with unchanged class
bytes. Worker choices do not alter
checkpoint semantic identity. Recovery first-error order and corrupt-bootstrap
diagnostics remain unchanged.
No-class TLA+ fallback keeps the existing recovery worker policy. Removing or modifying a class rejects recovery before using saved
states. Unknown local classes remain unsupported at binding.

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
- **Format.** Archives use format `DIVECC01`. Archives in older pre-release
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
Ordinary seen sets stay resident by default; the frontier and predecessor flags do
not imply a seen-set budget.

[The opt-in safety route](disk-seen-safety.md) uses it for transient disk
seen-state membership, with inline fallback and independent concrete witnesses.
It can be combined with the frontier and predecessor budgets. It is not a total
memory budget. Behavior graphs and temporal combinations refuse before work.
Recovery reconstructs disk membership from its saved
concrete discoveries; the transient disk files themselves are not imported.
