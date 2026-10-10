# Pure conditional predicates in final TLC mode

The explicit final mode accepts pure state conditionals through the same bounded
planner, scoped predicate evaluator and independent replay as other supported
predicates. This repairs the earlier blanket conditional refusal; it does not
change the ordinary periodic schedule or certify the whole liveness family.
Tracking: [#168](https://github.com/Chelis-Lang/dive/issues/168).

```tla
---- MODULE IfStateViolation ----
EXTENDS Integers
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Live == <>[](IF x = 1 THEN x = 2 ELSE x = 0)
====
```

With `INIT Init`, `NEXT Next`, and `PROPERTY Live` in the configuration:

```sh
cargo run -p dive-cli -- check \
  conformance/probes/tlc-state-if/models/IfStateViolation/IfStateViolation.tla \
  --config conformance/probes/tlc-state-if/models/IfStateViolation/IfStateViolation.cfg \
  --compat tlc --lncheck final --lasso tlc --seed 0 --fp 0 \
  --workers 4 --output json
```

The violation has the original configured property identity, actual worker and
exploration counters, a schema-2 final-only receipt, and an independently replayed
counterexample. Both in-memory and lossless JSON replay must validate before a
violation receives credit. Adding an invalid initial state, closing edge or
property claim cannot turn a receipt into a valid witness.

Rust callers use the same explicit facade:

```rust,no_run
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let path = std::path::Path::new(
    "conformance/probes/tlc-state-if/models/IfStateViolation/IfStateViolation.tla",
);
let analysis = dive_sema::analyze_file(path, &dive_sema::Options::default())?;
let ir = std::sync::Arc::new(dive_ir::lower(&analysis).map_err(|error| format!("{error:?}"))?);
let config_text = std::fs::read_to_string(path.with_extension("cfg"))?;
let cfg = dive_config::parse(&config_text).into_config().ok_or("configuration parsing failed")?;
let options = dive_check::CheckOptions {
    workers: 4,
    ..dive_check::CheckOptions::tlc(0, 0)
};
let report = dive_api::tlc_temporal::check_final(
    &ir, &cfg, &options, dive_liveness::phase_graph::Limits::default(),
)?;
match report.outcome() {
    dive_api::tlc_temporal::Outcome::Violation(witness) => {
        assert_eq!(witness.validation().validation, dive_trace::Validation::Valid);
        println!("{:?}", witness.document().properties);
    }
    dive_api::tlc_temporal::Outcome::Holds(_) => {}
    dive_api::tlc_temporal::Outcome::Safety(_) => {}
}
# Ok(())
# }
```

Obtain `ir` and `cfg` with the existing front-end API; the complete executable
controls are in `crates/dive-api/tests/tlc_state_if.rs`. The
[final API guide](tlc-final-api.md) documents loading, actual entry/work ceilings,
profile admission, safety routing, receipts and error qualifications.

TLC's conversion rules still apply. It converts the guard and both branches of
an unresolved state conditional before falling back to the whole state atom.
For example, `[]<>(IF x=1 THEN 1 \div 0=0 ELSE x=2)` raises code 2179 during
planning, before workers or fingerprints. A nonrecursive helper call can instead
retain a whole call after a conversion exception, deferring that error to a
predicate evaluation. Pure conditionals do not imply that every branch error is
supported or that all runtime error codes are certified.

A behavior may stutter forever even if `Next` normally cycles. The retained
`IfStateHold` development control therefore legitimately violates its property;
`IfHoldTrue` is the separate tautological hold control. No validator was relaxed
to make the first expression hold.

Pure local arguments, LET scopes, named INSTANCE substitutions, multiple
properties, bracketed action conditionals and tableau entry predicates are
covered by the selected controls. Effectful branches, live checker getters,
opaque captured operators and unproved bindings remain named refusals before
callbacks. Operator arguments (`Arg::Op`) and external/opaque override captures
remain subject to the unchanged shared scoped-planner and model-admission
refusals; the new structural scan grants them no permission. Its whole-arena
entry/work admission ceiling is distinct from the existing formula construction
limit, even though both use the fixed formula-node setting. VIEW, symmetry,
exact temporal identity, unsupported disk recovery,
periodic scheduling and other final-mode restrictions remain unchanged. Resource
stops receive no complete graph or witness credit. The retained `states` limit
bounds captured representatives, not aggregate memory or the distinct counter
including an already-inserted stopping state.

Evidence is in `conformance/probes/tlc-state-if/`: clean pinned actual
`processLiveness`/`LiveCheck` projections, raw original final checks, complete
pre-admission native observations and separate historical captures. The complete selected current CLI census retains all 60 rows at 1, 3, 4 and 8
workers: 36 independently validated violations, four holds and 20 named refusals.
The refusals include all four deciding reference-error cases; they receive no
matching-error credit. The 12 planning/identity refusals report zero workers;
the eight runtime-error refusals retain the actual execution width. The controls
retain failures alongside supported projections; none is a full-corpus or family
certification. The original 140-row small census remains source-qualified and
unchanged. See [ADR 0036](decisions/0036-conditional-planning-error-order.md)
and [provenance](provenance.md) for the shared conversion authority.

The temporary temporal-IF identity restriction scans the **whole admitted IR**,
including unused/imported definitions and call arguments. A temporal IF such as
`[](IF x=0 THEN x=0 ELSE x=1 => <>(x=2))` remains explicitly refused: the pinned
initial particle is 3 while the preserved native observation selected 4.
Matching words or counts does not excuse that identity disagreement. The scan
uses the existing 4,096 formula-entry/work ceiling, structural child/definition
edges and user-call argument checks in addition to metadata; changing a node's
level/effects cannot authorize this case. A large arena or structural walk can
therefore be refused even when the configured property does not use the IF.
This conservative staged restriction remains until the separate numbering fix
tracked in [#170](https://github.com/Chelis-Lang/dive/issues/170).
