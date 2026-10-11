# Concrete normalization before VIEW

This repair keeps the pinned `TLCStateMut.fingerPrint` order: normalize every
concrete variable, then evaluate VIEW, even without symmetry. A projected key
does not replace the concrete predecessor or certify a trace.

The minimal `before/reverse-mixed/Compound.tla` stores
`{<<1>>, [a |-> 1,b |-> 1]}` in an ignored variable. With seed7/fp0/one worker,
the clean pinned jar throws EC1000 and exits255 during initial fingerprinting.
Frozen native source `2c2ebda3940109fae34a033749b63a8560f8834c` instead reports
an invariant violation12 with four states and unprintable/null values. Those
states have no serializable independent trace and are not agreement evidence.
`before/main-equivalent-550d959/` separately preserves the lead's current-main
reproduction at1/4/8 and its own exact producer/source/binary readback.

The bounded repaired path explicitly refuses normalization errors where the
exact TLC EC/message counterpart remains unproved. It never replaces such an
error with success or an invariant verdict. Ordinary numeric/set/tuple/record
controls preserve counts and concrete states. Rust API controls check full,
completed-level pause, retain/reset recovery, original concrete witness and
changed Init/Next/final values at1/4/8, both profiles/stores and chaos scheduling.
Randomness under VIEW remains refused in this isolated repair; the separate
transformed-stream study is not part of its supported scope.

Raw paths may contain capture-machine directories because output is preserved
verbatim. Documentation and metadata use repository-relative paths.

## Runnable CLI controls

From the repository root, the finite set control finds four states and an
invariant violation; the reverse mixed set refuses without a counterexample:

```sh
dive check conformance/probes/view-concrete-normalization/before/set/Compound.tla --compat=tlc --seed 7 --fp 0 --workers 8 --store exact --output json
dive check conformance/probes/view-concrete-normalization/before/reverse-mixed/Compound.tla --compat=tlc --seed 7 --fp 0 --workers 8 --store exact --output json
```

These are expected exit12 and exit3 controls, respectively. Inspect the
resolved worker width and incomplete statistics rather than treating an
`unsupported` response as a model verdict. The `view_normalization` Rust API
tests use `CheckRequest`, `check_file_checkpoint` and `SpecModel::validate`;
they compare every report field except exact documented checkpoint/worker/
profile/elapsed metadata, assert retain profile labels separately, and retain
all concrete witness and changed-state assertions. Reset counts stay distinct
from retain counts. Archives remain trusted local continuation data, not
independent reachability certificates.

## Incomplete-state diagnostic boundary

Both `incomplete-wellformed-primary` cases leave `y` unassigned under VIEW.
Pinned TLC refuses Init with EC2102/exit255 and Next with EC2109/exit75.
The native assignment test requires that refusal before key normalization at
both profiles, stores, widths and scheduling controls.

The separate `incomplete-primary` experiment uses the reverse mixed set while
leaving `y` unassigned. Init printing instead throws EC1000/exit255; Next emits
EC2109/exit75 and its mixed value can also fail trace formatting. These raw
streams remain unproved diagnostic interactions. Assignment admission is not
a claim of exact mixed-state printing or error-class conformance.

## Lazy normalization and preserved failed prerequisite

`before/rejected-lazy-gate/` retains the complete `15fda58d` source-qualified
full gate: 1102/1103 tests passed, zero skipped, and all seven other stages
passed. The unchanged `empty_frontier_and_deferred_view_admission` test found
that unconditional normalization must preserve memory VIEW over lazy filters.
`before/15fda-current-authority/` retains its source/matrix bytes separately.

`lazy-primary/` contains six clean primary controls and the exact pinned
`SetPredValue`, `SetCupValue`, `SetCapValue`, `SetDiffValue` and `UnionValue`
source bytes. All six initial controls finish with 2 generated / 1 distinct / depth 1 and no
predicate Print output. The implemented filter path follows `SetPredValue`
by visiting only its stored bound set through a direct `Any` downcast to the
evaluator-owned representation. It does not call provider `kind`, `key`,
`as_any`, `elements` or `member`; the unknown-provider negative asserts zero
calls. Nested tuple filters share the existing depth/work budget.

Lazy set operations also have a materialized-cache normalization contract;
that broader representation remains explicitly refused here. Callback-bearing
filters remain refused by spill and checkpoint admission. The supported memory
filter fixture and truly invalid heterogeneous concrete set are different
boundaries. Neither refusal may be interpreted as a completed model verdict.

The additional `nested-filter` and `filter-function-range` primaries also
complete with 2 / 1 / 1 and no Print output. `filter-invalid-range` preserves
the pinned EC1000/exit255 when the filter’s stored function-set range contains
the reverse heterogeneous set. The direct operand control asserts the valid
stored range is normalized, the invalid range refuses, and neither path runs
the predicate; accepting a filter cannot hide an invalid bound value.

## Current source-qualified delivery

[current/receipt.json](current/receipt.json) binds the fresh release campaign
to `060489e7845b38b894d67b58c9a8c3a9fa18094c`. All 420 compiled inputs
are archived and hashed, independently of later documentation-only delivery
commits. The full local gate passes all eight stages and 1106 tests with zero
skips; the unchanged lazy VIEW test that failed the earlier prerequisite now
passes. Two test threads took 809.528 seconds for nextest execution; no
180-second per-test bound or 15-minute CI job bound changed.

The fresh campaign preserves all 144 checker selections, 132 original
candidates and 396 altered candidates across both profiles/stores, widths
1/4/8 and chaos scheduling. It records 120 checker violations and 24 named
refusals, including the reversed mixed set in both profiles. The approved
D-009 comparison option does not yet cover this normalization boundary.

Independent validation gives 96 valid originals and 288 semantic mutant
rejections. The remaining 36 originals and 108 mutants are `invalid` at
decoder shape admission; they receive no semantic replay, rejection or
certificate credit. Twenty-four originals are checker-emitted forward-mixed
words whose canonical JSON set order the decoder rejects in both profiles.
The other twelve are separately supplied default-reverse candidates with the
same mathematical values, not checker witnesses. No checker report in these
cases contains a `Valid` claim. [Follow-up #190](https://github.com/Chelis-Lang/dive/issues/190)
tracks this remaining default-normalization and structured-decoding limitation.

The separate homogeneous API campaign retains all 504 ordinary/pause/recovery
executions, 216 independently valid original/recovered witnesses and 216
semantically rejected altered witnesses. Both profiles/stores, widths1/4/8,
chaos schedules and reset/retain counters remain covered. All 21 CM1 and 16
ValidationTestSuite smoke models fully agree at widths1/4/8. The release
lazy-value campaign retains 54 cases, including named lazy-operation cache
refusals. Six runnable CLI guide commands and the compiled Rust recovery
guide pass; the Rust guide independently validates the concrete resumed
VIEW/symmetry witness with both transient storage budgets zero. Archives
remain trusted-local continuation data, not authenticated reachability proofs.

`current/runner-history/` preserves unsuccessful launcher expectations and
the discovered decoder boundary. The original default-reverse expectation
was not met; the whole selection was retained with actual classifications.
The conformance wrapper initially parsed stdout while model rows are written
to stderr; the completed21-model agree report is preserved, and a corrected
launcher recaptured both required selections. A nested filter containing a
lazy union is explicitly refused rather than treated as a filter-only
positive. The Rust consumer also retains its initial typed-path compiler
error and lock correction. No implementation, original test assertion,
oracle result or timeout was changed to obtain these passes.
