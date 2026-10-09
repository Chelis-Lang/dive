# State-dependent finite temporal predicate domains

Eight raw probes use the pinned TLC bundle, one worker, seed/fingerprint 0 and
`-lncheck final`. The deciding run is clean; trace capture is separate and
cross-checked. Reproduce from the repository root:

```sh
cargo run --release -p dive-conformance -- dev capture --repo-root . \
  --oracle tlc-2026.10.06.014338 \
  --specs crates/dive-api/tests/fixtures/temporal-state-domains/specs \
  --out crates/dive-api/tests/fixtures/temporal-state-domains \
  --timeout 30 DfMap DfFilter DfFunction DfNested DfLazy DfPrime DfBad DfEnabled
```

DfNested succeeds; the other seven violate P (exit 13). Each generates 3 states,
retains 2 distinct states and reaches depth 2. Infinite stuttering is part of
the behavior: a toggling Next relation alone does not imply fairness.
Both value profiles and both stores match truth/counts; returned periodic
claims pass a fresh independent validator, including DfEnabled's finite cover.

Domains are evaluated in each actual state or edge, not during planning.
Static analysis uses pure unknown bindings that the interpreter cannot force.
Concrete preflight bounds domain entries and Cartesian bindings (4,096 by
default), traversal depth (256) and visits (100,000 maximum). These are entry
bounds, not a byte-memory or general runtime guarantee. Effects, checker reads,
opaque bindings, infinite/oversized concrete domains and state-dependent
**temporal** quantification remain named refusals. A state/action predicate
quantifier is distinct from expansion of a quantifier over a temporal formula.

Exhaustive Boolean words of lengths 1–4 compare planner atoms and independent
word truth with cycle algebra at every back edge. Controls also cover missing
successor domains, pure-unknown forcing, empty current versus oversized future
domains, and effectful preflight. No exact TLC temporal ordering, lasso selection,
phase-L stopping, or parallel graph capture is claimed.
