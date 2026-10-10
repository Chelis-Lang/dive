# Native safety checkpoint and recovery

`dive check` can pause a supported safety search at a completed level and resume
its saved frontier. Pure, concrete INIT/NEXT models can pause and resume
through the deterministic worker pool, including supported pure VIEW and safety
symmetry. It refuses liveness, runtime
names/effects/checker reads and evaluator-driven deferred state values. These restrictions apply in both profiles and are reported explicitly.

```sh
dive check Counter.tla --config Counter.cfg --compat=tlc --seed 7 --fp 0 \
  --checkpoint counter.cp --checkpoint-after-levels 2 --output json
dive check Counter.tla --config Counter.cfg --compat=tlc \
  --recover counter.cp --output json
```

The first command pauses after two complete expansion levels. Zero pauses after
Init. A committed pause has `checkpoint.status: paused-incomplete`, an incomplete
resource-stop result, and no counterexample claim. It never reports proof of
safety. If search finishes before the requested boundary, its normal verdict is
returned with a note that no checkpoint was committed.

Recovery loads actual saved states, visited keys, predecessor IDs, ordinals and
frontier order. It does not rerun Init to regenerate the search queue or counts.
Recovery recomputes each saved visited key through the same configured path as
live discovery: symmetry canonicalization first, then VIEW, then the selected
store. Archives keep the original concrete states; they never replace a trace
state with its symmetry representative or its VIEW value. An imported key
mismatch or failed VIEW evaluation is an archive refusal, not a reachable model
error. Equal reduced keys cannot validate an illegal Init or Next witness.

Ordinary counterexample reconstruction can still replay saved initial/successor
ordinals to produce a concrete witness, followed by independent trace validation. An admitted archive has a `recovered`
receipt; the resumed search then produces its ordinary result. Requested worker
count and actual pool width remain in output. A pause immediately after Init
(`--checkpoint-after-levels 0`) reports one actual worker because the pool has
not started. Positive completed-level pauses and continued recovery use the
requested pool width; `--sequential` explicitly keeps inline execution. Native
graph recovery remains unsupported.

TLC resets generated and initial counters to recovered distinct states. Both
profiles use that policy by default. To retain original totals so they compare
with uninterrupted search, request `--checkpoint-counters retain`. This named
result-changing option is recorded in output, and TLC-profile runs become
`tlc+overrides`. The preserved counters do not change verdict, concrete trace,
queue order, distinct count or depth. Seed and fingerprint defaults on recovery
come from the archive and are labelled `checkpoint`.

The same executable, model, exact config source bytes, profile, store and semantic
evaluation settings must match. Worker width, chaos scheduling and explicit inline
execution are scheduling choices; they can change on recovery without changing
the ordered search result. Changing even a config comment requires a new
snapshot. Archive checksums are corruption checks, not signatures. Use only
trusted local archives: serialization does not independently prove reachability
or completeness. A resumed violation still needs independent trace validation.

The format bounds its payload to 64 MiB, discoveries/names to 65,536, each native
state/key/name to 1 MiB, variables to 1,024 and depth to 65,536. Concrete discovery
records, visited sets and archive encode/decode/reconstruction workspaces are
retained in memory. Transient frontier/predecessor budgets can separately move
the active queue and replay log to disk. Parallel candidates, resolved winner keys and worker scratch
are also outside the archive payload limit. These are archive admission bounds, not a total RAM budget.
Both transient budgets are opt-in and may change on recovery; directory settings
without their matching budgets refuse before file access. Native discovery
records and imported/serialized snapshots remain outside those budgets.

Writers use a private same-directory staging file, sync it, atomically commit a
complete immutable archive without replacing an existing file, remove staging,
and sync the directory. Partial staging files are not recoverable checkpoints.
An I/O or cleanup failure remains a failure; late cleanup/directory-sync failure
can leave a complete archive at the requested path and is reported explicitly.
No successful execution receipt is created for failed commits or invalid imports.

## VIEW and symmetry example

The checked-in [CpSymView module](../conformance/probes/native-checkpoint-keys/specs/CpSymView/CpSymView.tla)
and [configuration](../conformance/probes/native-checkpoint-keys/specs/CpSymView/CpSymView.cfg)
are a runnable example. Three interchangeable processes each increment a
counter. `SYMMETRY Perms` identifies process permutations, and `VIEW View`
ignores the bookkeeping variable `n`:

```tla
Perms == Permutations(Procs)
View == c
Inv == \A p \in Procs : c[p] < 2
```

From the repository root, use a fresh archive filename:

```sh
dive check conformance/probes/native-checkpoint-keys/specs/CpSymView/CpSymView.tla \
  --config conformance/probes/native-checkpoint-keys/specs/CpSymView/CpSymView.cfg \
  --compat=tlc --seed 7 --fp 0 --workers 8 \
  --checkpoint cp-sym-view.cp --checkpoint-after-levels 1 \
  --checkpoint-counters retain --output json
dive check conformance/probes/native-checkpoint-keys/specs/CpSymView/CpSymView.tla \
  --config conformance/probes/native-checkpoint-keys/specs/CpSymView/CpSymView.cfg \
  --compat=tlc --workers 8 --recover cp-sym-view.cp \
  --checkpoint-counters retain --output json
```

The pause is an intentional incomplete stop: `verdict` is `resource-limit`,
`checkpoint.status` is `paused-incomplete`, and `counterexample` is null. It has
4 generated states, 2 distinct states and one queued state. Requested workers
are 8; `options.workers.used` is 8 for this positive completed-level pause
and the resumed search. `options.checkpoint.parallel_recovery` is true.
The archive is committed despite this non-success verdict.

Recovery with `retain` reaches the invariant violation with 5 generated states,
3 distinct states and depth 3, matching the uninterrupted search and pinned TLC.
The returned witness contains original `c` and `n` values, rather than VIEW values
or symmetry representatives. Omitting `--checkpoint-counters retain` uses TLC's
reset policy instead: the same violation and concrete witness, 3 generated
states and 2 reported initial states. In both cases the receipt is `recovered`
and `options.checkpoint.independent_certificate` is false. Use `dive validate-trace`
with the returned trace data for the separate witness check; admission of the
archive alone is not that check.

A valid permutation declaration does not prove the specification is symmetric.
The author must ensure symmetry and VIEW are sound reductions for the properties
being checked. Symmetry with liveness and a general symmetry audit remain
unsupported. Key recomputation preserves these settings, including the selected
profile's value ordering, but cannot repair an invalid abstraction.

An effectful VIEW/SYMMETRY body, runtime string construction, checker reads or
unused effectful startup definition or configuration alias to an effectful body
returns an explicit `unsupported` result
before startup output or archive creation. Pure native `Permutations` is supported;
a local operator with the same name receives no special authority. Altered
archives, mismatching keys and failing imported VIEW evaluations yield explicit
archive/resource failures with no recovered receipt. See the
[source and complete raw recovery controls](../conformance/probes/native-checkpoint-keys/README.md)
for both stores/profiles, boundary-zero and later pauses, width/chaos requests,
and equivalent-state witnesses deliberately rejected by independent validation.

The Rust entry points are `dive_check::checkpoint::check` and
`dive_api::check_file_checkpoint`. Ordinary checking APIs remain unchanged.
See [ADR 0033](decisions/0033-native-safety-checkpoints.md),
[ADR 0038](decisions/0038-checkpoint-transformed-safety-keys.md), the
[pinned raw probes](../conformance/probes/native-checkpoint/README.md) and
[the TLC differences log](tlc-differences.md#d-007-exact-state-storage).

Native generated/initial counters are bounded by the largest positive TLC `long`
(9,223,372,036,854,775,807). Imported larger values refuse before binding;
continuation uses checked increments and stops explicitly before exceeding that
schema bound. Wrapped counts can never become a completed-search report.


Checkpoint admission also checks configured operator aliases before binding. For
example, `CONSTANT Gate <- Alias` may resolve through a Rust module context to a
local operator that calls `PrintT`. That model refuses before printing anything
or creating the checkpoint. A pure operator selected through the same alias is
supported. [Issue #164](https://github.com/Chelis-Lang/dive/issues/164) fixed an
admission escape in dive; the [preserved before-and-after controls](../conformance/probes/native-checkpoint-keys/README.md)
include the original effect and incomplete archive. Structurally valid public IR
and an archive checksum do not authorize effects or independently prove safety.

The configured-key path has a separate
[earlier ordered-execution integration receipt](../conformance/probes/native-parity-integration/current-controls/checkpoint-keys-ordered-integration/verification.json):
847 Rust tests, 144 recovery groups and 1,008 fresh CLI/witness runs, plus
CM1/ValidationTestSuite at 1/4/8 requested workers. That frozen producer executes
recovery inline; current parallel verification is recorded below. The earlier 831-test and original 738-test studies retain their
original source and observations; none are retagged as current execution.

## Deterministic parallel recovery

The same runnable CpSymView commands above use eight workers on both sides of
the archive. You may instead pause at four workers and recover at eight: omit
`--sequential` and set `--workers` independently on each command. The recovered
level comes from the archive; the engine never restarts its depth at one or
regenerates Init. Every completed level records new concrete discoveries,
predecessor ordinals and their already-resolved configured keys in reference
order before publishing the next frontier. A snapshot is written only at that
completed barrier, after the whole deciding prefix is committed.

With `retain`, verdict, generated/distinct/initial counts, queue, depth and
concrete counterexample match uninterrupted inline execution. With the default
TLC counter policy, generated and initial counts reset to recovered distinct
states exactly as before; worker scaling does not change that choice. Native
MAX_COUNTER stops are resolved in logical successor order, not whichever
worker reaches a local limit first. Any retained trace at a resource stop is
partial path evidence, not an invariant violation or a completed-search proof.

This supports pure finite safety only. Temporal graphs, runtime effects or name
construction, checker reads and evaluator-driven deferred values remain refused
before effects in both profiles. The
worker field reports pool width, not speedup or simultaneous useful work.
The [parallel recovery study](../conformance/probes/native-checkpoint-parallel/README.md)
keeps the previous inline-only captures separate from new width, cross-width,
counter-prefix and independent witness controls. The trusted-local archive
contract and all existing byte/state/depth/identity bounds remain unchanged.


## Earlier parallel integrated verification

The preserved source-bound [integration receipt](../conformance/probes/native-parity-integration/current-controls/checkpoint-parallel-final-integration/verification.json)
records 873 passing workspace tests, 144 recovery groups, 1,008 CLI runs,
176 counter-limit controls and 36 record-codec controls. CM1's 21 models and
ValidationTestSuite's 16 smoke models fully agree at 1/4/8 workers. The
configured-key and parallel campaigns preserve concrete independently checked
witnesses, stopping prefixes and original inline comparison executables.

This receipt identifies its frozen runtime source and binary. Earlier inline
receipts remain historical; their smaller source scope is not relabelled as
parallel evidence. These controls do not certify temporal checkpointing, spill
composition or the complete recovery feature family.
## Recovery with transient disk storage

The existing budgets work with pause and recovery, independently or together:

```sh
dive check conformance/probes/native-checkpoint-keys/specs/CpSymView/CpSymView.tla \
  --config conformance/probes/native-checkpoint-keys/specs/CpSymView/CpSymView.cfg \
  --compat=tlc --seed 7 --fp 0 --workers 4 \
  --frontier-memory-bytes 0 --predecessor-memory-bytes 0 \
  --checkpoint cp-spill.cp --checkpoint-after-levels 1 --output json
dive check conformance/probes/native-checkpoint-keys/specs/CpSymView/CpSymView.tla \
  --config conformance/probes/native-checkpoint-keys/specs/CpSymView/CpSymView.cfg \
  --compat=tlc --workers 8 --frontier-memory-bytes 0 --predecessor-memory-bytes 0 \
  --recover cp-spill.cp --checkpoint-counters retain --output json
```

These commands retain the same incomplete pause and concrete invariant violation
counts described above: final Retain is 5 generated/3 distinct/depth 3; omitting
Retain gives the default TLC reset, 3 generated and 2 initial states. VIEW and
symmetry affect stored keys, not returned concrete witness values. A pause's
transient files are cleaned before return; recovery reconstructs fresh queues/logs
from the native archive. It does not require or reopen the previous transient
files. You can recover with different budgets, omit them, or select inline mode.

Zero forces the selected transient family to disk immediately. The frontier
budget covers encoded queue data/index capacities separately per current/next
queue; the predecessor budget covers replay parent/ordinal record capacity.
Neither covers native discovery records (which also contain parent/ordinal
metadata), archive payload/JSON work, visited keys, decoded states, candidates,
worker scratch or reconstructed traces. Restoration feeds each decoded saved
frontier state directly into the configured queue; it does not retain a second
complete decoded frontier. The archive's separate 64 MiB/65,536 bounds remain.
No setting here is an aggregate process-memory limit.

Optional `--frontier-directory DIR` / `--predecessor-directory DIR` require their
respective budgets and an existing parent directory; omitted directories use
private auto-cleaned temporary storage. Read [frontier accounting](frontier-storage.md)
and [predecessor accounting](predecessor-storage.md). JSON `frontier_storage`
and `predecessor_storage` expose confirmed writes, retained capacity and failures;
`checkpoint.transient_storage_cleanup_completed` reports whether both families
closed without failure. A later close failure can leave an already committed
archive and its actual-byte receipt, but the check is a resource failure, with
storage/deciding-event evidence and no completed-search certificate. The archive
checksum remains a corruption check, not authentication or proof.

In Rust, set the same fields before calling the native entry point:

```rust,no_run
use dive_api::{CheckRequest, check_file_checkpoint, checkpoint};
let model = CheckRequest {
    file: "conformance/probes/native-checkpoint-keys/specs/CpSymView/CpSymView.tla".into(),
    config: Some("conformance/probes/native-checkpoint-keys/specs/CpSymView/CpSymView.cfg".into()),
    compat_tlc: true, seed: Some(7), fp: Some(0), workers: Some(4),
    frontier_memory_bytes: Some(0), predecessor_memory_bytes: Some(0),
    ..CheckRequest::default()
};
let native = checkpoint::Request {
    path: "cp-spill.cp".into(), action: checkpoint::Action::PauseAfterLevels(1),
    counters: checkpoint::Counters::Tlc, config_source_sha256: None,
};
let paused = check_file_checkpoint(&model, &native)?;
assert_eq!(paused.to_json()["checkpoint"]["status"], "paused-incomplete");
let resumed = check_file_checkpoint(
    &CheckRequest { workers: Some(8), ..model },
    &checkpoint::Request { action: checkpoint::Action::Recover, counters: checkpoint::Counters::Retain, ..native },
)?;
assert_eq!(resumed.to_json()["verdict"], "invariant-violation");
# Ok::<(), dive_api::ApiError>(())
```

[The spill recovery study](../conformance/probes/native-checkpoint-spill/README.md)
keeps original admission refusals/source bytes separate from actual recovered
searches, independent original/altered witnesses and failure controls. Effects,
runtime names/checker reads, evaluator-driven deferred values, temporal graphs
and Java checkpoint byte interoperability remain excluded in both profiles.

The transient-storage composition keeps that integrated worker/observer contract.
Its original pre-integration run is preserved with its own executable and source
map; a fresh combined-source recovery/failure/guide receipt is recorded separately
in the [spill study](../conformance/probes/native-checkpoint-spill/README.md).
The separate encoded-frontier and resident-predecessor budgets still exclude
native archive serialization, visited sets, candidate payloads and reconstruction
workspace. Neither archive checksums nor successful recovery authenticate or
independently certify imported snapshots.
