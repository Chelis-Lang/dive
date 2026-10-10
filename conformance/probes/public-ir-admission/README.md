# Public IR endpoint admission controls

Issue #157 preserves the original malformed-arena source, linked source/binary
hashes and raw panic logs in `../native-checkpoint/malformed-arena-review/`.
Changing only serialized `vars` to `[4294967295]` made both ordinary checking
and native checkpoint refusal escape a panic through error-report metadata.
These are native Rust API controls, not oracle semantic parity certification.

The repair uses the existing evaluator context admission authority before model
binding or checkpoint request/archive handling. Checked no-work metadata also
protects early option and panic-handler reports. Bound graph capture validates
arena/name ownership and predicate/action contexts before host/ASSUME effects.
Admission failures carry their existing stable DIVE-EVAL-C001..C007 diagnostics;
malformed or foreign inputs cannot produce a completed search or witness.

`crates/dive-check/tests/public_ir.rs` covers seven malformed arena mutations,
ordinary/native/binding paths, requested widths 0/1/4/65, six foreign or malformed
capture contexts, effectful ASSUME controls and valid metadata/exploration.
The private `metadata_controls` test calls the early-refusal and panic-report
constructor directly with invalid variable/order IDs.

The first focused run failed in three new fixture assumptions, preserved in
`before-fixture-repair.log` and the exact test source: the serialized order key
is `tlc_vars`, a constant TRUE invariant is omitted by model binding, and TLC
Print outputs both operands. Only those fixture/schema/expected-output details
were corrected. Refusal, zero-effect and unchanged valid-result assertions were
not weakened; the fixed focused controls passed.

Bare numeric ID and semantic-annotation provenance remain unproved. This repair
does not certify arbitrary caller-provided action projections or IR semantics;
F-035 stays partial and audit-required.

The first full gate passed 730 of 731 controls. The sole failure was the
previous context-admission source matrix pinning the intentionally changed
evaluator source. Its record remains unchanged, and all twelve old source
files are preserved under `../tlc-planning/native-source-context-admission/`.
A separate `public-ir-admission-source-matrix.json` verifies current source;
raw oracle bytes and every strict hash assertion remain checked. The initial
full-gate failure is retained as `before-source-matrix-repair.log`.

The frozen repair passed all 731 workspace tests, 20 Python controls,
documentation, dependency/layering and standalone trace checks. The fresh
release binary fully agreed on CM1 (21) and validation smoke (16), each at
1/4/8 workers. `fixed-reproducer.rs` is the original malformed-arena source;
its linked fixed libraries return both escaped-panic flags FALSE.
`delivery.json` records exact source, release/reproducer binary and raw-log
hashes. Historical checkpoint and oracle evidence is unchanged.
