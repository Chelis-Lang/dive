# Native checkpoint source contract

The grid fixture and raw pinned TLC transcripts precede the native implementation.
They use TLC `2026.10.06.014338` (`94d0c50`), one worker, seed 7 and fp 0, without
`-dumpTrace`. `tlc-2026.10.06.014338/provenance.json` pins the jar, raw bytes and
source files reviewed. No oracle golden has been edited.

The uninterrupted grid (`N=1600`) finishes with 5,123,201 generated states,
2,563,201 distinct states, zero queued states, depth 3,201 and exit 0. The developer
property `-Dtlc2.TLC.stopAfter=1`, with `-checkpoint 1`, stops this captured run at
2,843,484 generated, 1,423,343 distinct, 1,511 queued and depth 1,692. TLC nevertheless
prints “Model checking completed” and exits 0. This incomplete prefix is not a
proof that every reachable state was checked.

Recovering the timestamp directory created below `-metadir` restores 1,423,343
seen states and 1,511 queued states. It finishes at 3,703,060 generated states,
2,563,201 distinct states, zero queued and depth3,201. The generated counter is
reset to the recovered distinct count, as is `numberOfInitialStates`.
The exact relation is `5,123,201 - 2,843,484 + 1,423,343 = 3,703,060`.
The first attempted recovery used the parent metadata directory and failed 255
because `vars.chkpt` was absent; this raw failed run is preserved separately.

`ModelChecker.checkpoint` suspends workers, stages queue, trace, FPSet and intern
names (and liveness when enabled), resumes exploration, then commits each file.
`recover` restores these structures and skips initial-state generation for safety.
`Worker` generated counters are not persisted. `TLCTrace`, `DiskStateQueue` and
`MemFPSet` stage `.tmp` files and rename them to `.chkpt`. `InternTable` restores
names early, before parsing/evaluating the recovered model.

The first native format retains all counters, but its default recovery reports
the TLC reset. Explicit `--checkpoint-counters retain` restores original totals;
this is a result-changing option, not an approved new default. Native pauses are
incomplete results, never successful exhaustive verdicts. Native bytes have no
Java checkpoint interoperability claim. See [ADR 0033](../../../docs/decisions/0033-native-safety-checkpoints.md) and D-007.

Reproduction (fresh output directories, pinned cache tools supplied explicitly):

```sh
python3 -I conformance/probes/native-checkpoint/capture_tlc.py JAVA JAR OUT
python3 -I conformance/probes/native-checkpoint/capture_controls.py DIVE OUT
```

The TLC script preserves timed prefixes rather than expecting wall-clock stop
counts to repeat. The native script retains 144 full/pause/recover control groups
(576 raw runs), including six pinned safety deciding-event fixtures, both
profiles/stores, requested widths 1/4/8 and chaos scheduling. Native receipts never
claim independent archive certification. Independent positive/broken-witness
controls additionally live in `crates/dive-api/tests/native_checkpoint.rs`.

The captured native study in `native/index.json` passed all 576 raw runs.
The fixed source passed 727 workspace tests, 20 Python controls, documentation,
layering, standalone trace and dependency checks. CM1 (21 models) and
ValidationTestSuite smoke (16 models) fully agreed at 1, 4 and 8 workers.
`delivery.json` records source/binary hashes and the exact gate logs.
The separate malformed public-IR arena panic in `malformed-arena-review/`
is tracked as issue #157; it is not checkpoint-file admission evidence.
