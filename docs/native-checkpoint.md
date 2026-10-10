# Native safety checkpoint and recovery

`dive check` can pause a supported safety search at a completed level and resume
its saved frontier. This first increment runs with one actual worker and accepts
pure, concrete INIT/NEXT models. It refuses VIEW, SYMMETRY, liveness, runtime
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

The Rust entry points are `dive_check::checkpoint::check` and
`dive_api::check_file_checkpoint`. Ordinary checking APIs remain unchanged.
See [ADR 0033](decisions/0033-native-safety-checkpoints.md), the
[pinned raw probes](../conformance/probes/native-checkpoint/README.md) and
[the TLC differences log](tlc-differences.md#d-007-exact-state-storage).

Native generated/initial counters are bounded by the largest positive TLC `long`
(9,223,372,036,854,775,807). Imported larger values refuse before binding;
continuation uses checked increments and stops explicitly before exceeding that
schema bound. Wrapped counts can never become a completed-search report.
