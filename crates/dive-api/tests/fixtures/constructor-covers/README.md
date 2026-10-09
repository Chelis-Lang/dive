# Independent finite-constructor covers

Regenerate raw results with the pinned clean oracle (trace capture is a separate
cross-checked pass):

```sh
cargo run --release -p dive-conformance -- dev capture --repo-root . \
  --oracle tlc-2026.10.06.014338 \
  --specs crates/dive-api/tests/fixtures/constructor-covers/specs \
  --out crates/dive-api/tests/fixtures/constructor-covers --timeout 30 \
  CcMapDisabled CcFilterDisabled CcFunctionDisabled CcFunctionReach EnabledConstructorCover
```

Three `Cc*Disabled` cases violate P (exit 13); `CcFunctionReach` succeeds.
Each generates 4 states with 2 distinct states and depth 2. Both profiles and
stores reproduce these outcomes/counts; all periodic claims validate independently.
The fair stuttering endings require proving that Go is disabled, not observing
that it recurs.

`EnabledConstructorCover` succeeds with 16 generated states, 8 distinct states
and depth 1. Its invariant pins six ENABLED identities over every Boolean/set
state. The independent validator checks all 48 corresponding decisions and
re-evaluates each witness in the original action. Exhaustive assignment controls
also check current-state domains, empty bindings and nested constructor capture.
Primed state domains, uncovered successor reads, effects and work limits refuse;
a missing proof never certifies disabledness. No successor enumerator is used.
