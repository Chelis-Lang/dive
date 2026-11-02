# Search storage and native recovery

This guide covers the opt-in disk storage for safety search and native
checkpoint/recovery. The design decision is
[ADR 0083](../decisions/0083-storage-and-checkpoint.md).

`--engine core` safety BFS supports `--store exact` and the frontier and
predecessor budget/directory options. Exact membership resolves hash collisions by comparing
complete canonical mathematical state identities; representation and receiver
history stay attached to the concrete states used for evaluation and replay.
Parallel exact search retains candidate collisions until ordered commit.
Predecessor read, write, corruption and explicit-cleanup failures produce a
resource stop with storage evidence and no certified counterexample.
The core still refuses disk membership, checkpoint/recovery
and temporal exact/spill combinations. `--engine auto` can use the reference
engine for these remaining combinations and reports that fallback.

The core frontier stores paired canonical and original opaque handle rows under
one retained live Store. Original receiver classes and construction history are
preserved; values are not reconstructed through the reference value codec. Its
private files cannot be recovered after restart or decoded by another run.
Each decoded chunk contains at most 1,024 state pairs and is outside the encoded
queue budgets. Retained Store/history allocations, seen sets, candidates, arenas
and decoded chunks remain outside those component budgets. The report therefore
leaves the reference value codec's node limit null for actual core runs.

Before certifying a result, the core validates all retained frontier records,
including unvisited rows, with the actual run control checked every 128 rows.
Budget stops close and clean the remaining files without scanning unprocessed
records or asserting completeness. Cancellation during decoding or validation
keeps its budget-stop meaning; read, write, corruption and cleanup failures
invalidate certification and retain explicit storage diagnostics.

## Transient frontier and predecessor storage

Safety BFS has independent opt-in frontier and predecessor budgets:

```sh
dive check docs/user/examples/Counter.tla --compat=tlc --seed 7 --fp 0 --workers 8 --frontier-memory-bytes 0 --output json
dive check docs/user/examples/Counter.tla --compat=tlc --seed 7 --fp 0 --workers 8 --predecessor-memory-bytes 0 --output json
```

Zero forces disk storage from the start. Omit the flags to keep the in-memory
reference path. `--frontier-directory DIR` and `--predecessor-directory DIR`
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

Checkpoints are a separate, bounded, safety-only feature. Archive source identity
includes the module name and exact bytes of adjacent local Java override classes.
The supported Hanoi `Bits.class` can pause and recover with unchanged class
bytes under the native worker/reference contract. Worker choices do not alter
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
- **Workers.** Worker width, chaos scheduling and `--sequential` may change on
  recovery without changing the ordered result. Seed and fingerprint defaults come
  from the archive. Runtime string constructors are supported; TLC mode uses an
  inline one-worker fallback for their encounter order.
- **Identity.** The executable, model, exact configuration bytes, profile, store
  and evaluation settings must match. Even a changed configuration comment needs
  a new snapshot.
- **Bounds.** The payload is limited to 64 MiB, with at most 65,536 discoveries or
  names, 1 MiB per state, key or name, 1,024 variables and depth 65,536. These are
  archive admission bounds, not a RAM budget. Transient frontier and predecessor
  budgets can be combined with checkpoints and may change on recovery.
- **Commits.** Archives are written to a private staging file, synced and
  atomically committed without replacing an existing file. Failures are reported;
  partial staging files are not checkpoints.
- **Format.** Archives use structural format 2 (`DIVECP02`). Older value-bearing
  archives refuse rather than guess; recreate them with the current executable.
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
memory budget or a checkpoint feature. Recovery, graphs and temporal combinations
refuse before work.
