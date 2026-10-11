# Structural tuple-function storage (#195)

This is a representation defect repair, not a mathematical key change. The
minimal `Apply/TupleStorage` model constructs an integer-domain function and
applies it to `"a"`. The clean pinned TLC stream carries EC1000 then 2103 and
reports `Attempted to apply function with integer domain to the non-integer
argument "a"`. Before the repair, memory and predecessor-only execution already
matched this text, while frontier spill and recovery gave a tuple-index error.
All had one generated/distinct state and depth one, so matching exit 75 alone
was not sufficient evidence.

`before/manifest.json` keeps the entire fresh72-row old selection, clean TLC raw
stream and12 actual version-1 archives, source/executable identities and hashes.
It is separate from the first d749 nondiscriminating control and discriminating
apply observations retained under
`../trace-representation-v2/codec-tuple-loss/`. None is retagged as new execution.

Format2 introduces structural tag0A for nonliteral tuple functions and retains
literal tag08. Canonical encoding and FP64 intentionally still identify
mathematically equal functions. CP02/FQ02/FI02 reject older value-bearing
containers; the predecessor ID/ordinal-only format is unchanged. Direct/nested
literal, tuple-function and contiguous-domain ordinary functions receive
structural/key/fingerprint controls. Old actual header/schema bytes receive named
refusal checks, not a magic-string inspection alone.

Reproduce local API and storage controls:

```sh
cargo test -p dive-value --test codec structural_tuple_function
cargo test -p dive-store --lib old_value_bearing_frontier
cargo test -p dive-check --lib structurally_ambiguous_version_one
cargo test -p dive-api --test storage_tuple_function -- --test-threads 2
```

The API cross-product retains both profiles/stores,1/4/8 workers, original and
chaos scheduling, memory/frontier0/predecessor0/both0 and completed-level0
pause/recovery. `Witness` emits an opt-in version2 concrete word whose original
is independently validated and whose Init/middle/final mutants reject. These
are witness proofs, not native archive authenticity/reachability certification.
Native application error metadata remains `verified:false`. Queue/log budgets
remain independent scoped limits, not total RAM or reconstruction limits.
`current/receipt.json` closes the corrected producer
`ead7260e48d69fc111bd8a94a60b526c6f68b1a2`:445 frozen Git/source input pairs,
all eight gate stages and1167/1167 tests with zero skipped. Fresh release controls
retain all192 check/pause/recovery triples,192 independently Valid original words,
576 Invalid mutants and12 actual old CP01 archive refusals. Clean primary streams
for both models are paired separately. Error-site metadata remains `verified:false`.
The exact Cargo guide and CM1’s21/ValidationTestSuite’s16 models pass at1/4/8.

Run `python3 -I conformance/probes/storage-tuple-function/verify.py` to read back
this bounded evidence. This is source/evidence integrity, not a certificate for
arbitrary archives or the complete storage family. Legacy v1 JSON still has its
separate representation limitations; only the structural native codec/container
formats change here.

The frozen producer archive remains unchanged when this slice is composed with
other work. `current-source.json` separately checks the same 445 source paths in
the receiving checkout and states whether they still equal that producer. A
false equality field is an explicit integration boundary: the old runtime and
gate remain evidence about `ead7260`, while the receiving source needs its own
fresh gate and runtime controls. Updating current hashes does not retag the old
executables or certify the receiving behavior.

The first dc0f full gate is retained under
`before-current-format-fixtures-gate/`:1160/1167 tests passed, seven existing
current-payload rewriters failed at the old CP01 header; all other stages passed.
The corrected epoch migrates their headers without changing the existing
assertions/case selection and strengthens the generic corruption check.
`current-header-mutation/` retains the deliberate CP01 mutation failure and
restored CP02 pass, proving that generic refusal reaches payload admission.
Explicit actual old-format migration refusals remain independent controls.

The current local two-thread gate took1312.43 seconds; nextest took1072.973
seconds on the shared8-core M3. New wrapper durations were0.218s(old archive),
24.481s/26.407s(application, default/TLC),28.721s/40.736s(witness, default/TLC).
The CI per-test180-second bounds and all selections are unchanged. This local
whole-stage duration is not evidence that a Linux15-minute job completes; the
receiving eight-partition CI must verify its own run. Complete per-test timings
are retained in `current/current-gate-timing.json`. Resident seen-state budgets
remain pending under#205; this repair does not add a total-memory limit.
