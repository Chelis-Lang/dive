# Temporal LET scopes

Generated through the harness with the pinned JRE, oracle and module bundle:

```sh
DIVE_CACHE_DIR=<verified-cache> cargo run -p dive-conformance --release -- dev capture \
  --oracle tlc-2026.10.06.014338 \
  --specs crates/dive-api/tests/fixtures/temporal-let/specs \
  --out crates/dive-api/tests/fixtures/temporal-let \
  LtState LtNested LtGuard LtAction LtCaptured LtDisabled
```

`LtState`, `LtNested`, `LtAction` and `LtCaptured` succeed. `LtGuard` and
`LtDisabled` violate P. Every model generates 3 states, retains 2 distinct
states and has depth 2. Clean runs decide; each violation's JSON comes from
a separate cross-checked trace pass. API tests match truth/counts in both
profiles and stores and independently validate every periodic claim.

Zero-arity LET values remain lazy in their enclosing scope; parameterized
local operators retain parent frames. These probes cover captured state,
nested local calls, guards, priming and values captured inside quantification.
`LtDisabled` needs independent disabledness on a fair stuttering cycle; recurring
occurrence alone cannot certify it. Short-word controls cover every boolean word
of length 1–4 and every closing target. Used effects and checker reads refuse
before evaluation. Necessary-cover controls compare against exhaustive assignments
and verify the entire original relation for every witness; missing covers,
state-dependent action arguments and budgets remain named refusals.

No TLC-exact selection, phase-L timing or periodic schedule claim is made.
