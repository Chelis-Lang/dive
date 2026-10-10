# Native safety checkpoint and recovery

`dive check` can pause a supported safety search at a completed level and resume
its saved frontier. This first increment runs with one actual worker and accepts
pure, concrete INIT/NEXT models, including supported pure VIEW and safety
symmetry. It refuses liveness, runtime
names/effects/checker reads, evaluator-driven deferred state values and transient storage
budgets. These restrictions apply in both profiles and are reported explicitly.

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
count remains in output; actual width is one. Parallel/native graph recovery is
not implemented by this slice.

TLC resets generated and initial counters to recovered distinct states. Both
profiles use that policy by default. To retain original totals so they compare
with uninterrupted search, request `--checkpoint-counters retain`. This named
result-changing option is recorded in output, and TLC-profile runs become
`tlc+overrides`. The preserved counters do not change verdict, concrete trace,
queue order, distinct count or depth. Seed and fingerprint defaults on recovery
come from the archive and are labelled `checkpoint`.

The same executable, model, exact config source bytes, profile, store and semantic
evaluation settings must match. Changing even a config comment requires a new
snapshot. Archive checksums are corruption checks, not signatures. Use only
trusted local archives: serialization does not independently prove reachability
or completeness. A resumed violation still needs independent trace validation.

The format bounds its payload to 64 MiB, discoveries/names to 65,536, each native
state/key/name to 1 MiB, variables to 1,024 and depth to 65,536. Concrete discovery
records, frontiers, visited sets and encode/decode/reconstruction workspaces are
retained in memory. These are archive admission bounds, not a total RAM budget.
The first slice refuses both frontier and predecessor transient storage settings,
including directory settings without budgets.

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
are 8; `options.workers.used` is 1 and its reason explains the inline restriction.
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
