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
