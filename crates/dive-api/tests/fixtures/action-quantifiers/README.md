# Finite quantified predicates and actions

Generated with the verified pinned JRE/oracle/module bundle. These are clean
TLC final runs; each violation trace uses a separate cross-checked pass.

```sh
DIVE_CACHE_DIR=<verified-cache> cargo run -p dive-conformance --release -- dev capture \
  --oracle tlc-2026.10.06.014338 \
  --specs crates/dive-api/tests/fixtures/action-quantifiers/specs \
  --out crates/dive-api/tests/fixtures/action-quantifiers \
  QfReach QfDisabled QfStrong QfAllState QfTuple QfNested
```

`QfReach`, `QfTuple` and `QfNested` succeed. `QfDisabled`, `QfStrong` and
`QfAllState` violate P. The first three action probes generate 4 states,
retain 3 distinct states, and have depth 3; the boolean predicates generate
3 states, retain 2, and have depth 2. Final API tests match truth and counts
in both profiles and stores; every returned periodic claim validates independently.

Finite constant domains bind each body context before purity/level checks.
Nested quantifiers retain outer values, tuple patterns destructure actual data,
and empty forall/exists retain their true/false identities. Independent ENABLED
covers allow finite domains read from the current state, then verify the entire
original quantified relation for each complete successor assignment. State-dependent
quantifier domains in the temporal planner/word preflight remain named refusals;
primed/infinite domains, missing covers, effects and budgets never mean disabledness.

No phase-L stopping, periodic schedule, parallel graph capture or TLC-exact
lasso selection claim is made.
