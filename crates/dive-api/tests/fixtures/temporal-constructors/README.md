# Finite temporal data-constructor probes

Raw results use the pinned TLC bundle at one worker, seed/fingerprint 0 and
`-lncheck final`. The deciding run is clean; trace capture is a separate,
cross-checked pass. Regenerate from the repository root:

```sh
cargo run --release -p dive-conformance -- dev capture --repo-root . \
  --oracle tlc-2026.10.06.014338 \
  --specs crates/dive-api/tests/fixtures/temporal-constructors/specs \
  --out crates/dive-api/tests/fixtures/temporal-constructors \
  --timeout 30 BcMapGood BcMapBad BcFilterGood BcFunctionGood BcNestedGood BcAction BcEmpty BcStateDomain
```

Five supported cases succeed. BcMapBad and BcAction violate P (exit 13).
Each supported case generates 3 states, retains 2 distinct states and reaches
depth 2. Both profiles/stores match truth/counts; every returned periodic claim
passes fresh independent validation. Exhaustive Boolean words of lengths 1–4
check every closing edge against direct cycle algebra, including function
arguments that retain captured state lazily.

BcStateDomain succeeds in both TLC and final/short dive. Its state-dependent
domain is evaluated at each actual state, never frozen as constant data.
See `../temporal-state-domains/` for the expanded bounded controls.
Effects, checker reads and infinite domains refuse without executing an
effectful preflight. Pure constant-domain predicate constructors are inspected
without enumerating bindings; the two `constant-predicate-binders` corpus
controls cover holding and false 5,000-element constructors against the pinned
oracle, including fresh genuine and forged word validation.
Recursive constructors and opaque independent ENABLED
covers remain unsupported. This does not prove TLC-exact planning, temporal
stopping, lasso selection or parallel graph capture.
