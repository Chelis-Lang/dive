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
Historical/current source and runtime receipts will be recorded separately when
the corrected full gate and release consumers close.

The first dc0f full gate is retained under
`before-current-format-fixtures-gate/`:1160/1167 tests passed, seven existing
current-payload rewriters failed at the old CP01 header; all other stages passed.
The corrected epoch migrates their headers without changing the existing
assertions/case selection and strengthens the generic corruption check.
`current-header-mutation/` retains the deliberate CP01 mutation failure and
restored CP02 pass, proving that generic refusal reaches payload admission.
Explicit actual old-format migration refusals remain independent controls.
