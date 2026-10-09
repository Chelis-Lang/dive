# Finite CHOOSE inside temporal predicates

Raw clean TLC runs use the pinned bundle, one worker, seed/fingerprint 0 and
`-lncheck final`; trace capture is a separate cross-checked run. Reproduce:

```sh
cargo run --release -p dive-conformance -- dev capture --repo-root . \
  --oracle tlc-2026.10.06.014338 \
  --specs crates/dive-api/tests/fixtures/temporal-choose/specs \
  --out crates/dive-api/tests/fixtures/temporal-choose \
  --timeout 30 CfMin CfMax CfTuple CfNested CfLazy
```

CfNested succeeds; the other four violate P (exit 13), including stuttering
behaviors. Every model generates 3 states, retains 2 and reaches depth 2.
Both value profiles/stores match final truth/counts; every returned periodic
claim validates independently. These choices have unique qualifying values,
so canonical versus TLC ordering does not change their results. Ambiguous
choice still uses the existing profile's proven value order (D-006).

Static analysis binds actual constant domains or pure unknowns for state domains.
The domain's level contributes even when the predicate body ignores its binder;
for example `H(S) == []<>(\E b \in S : TRUE)` cannot be folded to a constant
when S is a state-dependent argument. Concrete entry guards evaluate each
actual domain and retain the existing CHOOSE evaluator. Missing qualifying
values and empty domains raise errors; no sample or invented value is supplied.
Effects, checker reads, infinite and oversized domains remain explicit refusals.

Controls compare planner truth and independent word evaluation with direct
cycle algebra for every Boolean word of lengths 1–4 and every closing edge.
They also check captured domains whose quantifier/constructor bodies are
constant, valid versus unsatisfied choices, and no effectful preflight output.
Strict TLC temporal selection, phase-L timing and parallel graph capture are
still pending.
