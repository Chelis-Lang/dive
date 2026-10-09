# Pure helpers called with action-level arguments

Raw clean TLC runs use the pinned bundle, one worker, seed/fingerprint 0 and
`-lncheck final`; trace capture is a separate cross-checked run. Reproduce:

```sh
cargo run --release -p dive-conformance -- dev capture --repo-root . \
  --oracle tlc-2026.10.06.014338 \
  --specs crates/dive-api/tests/fixtures/temporal-action-args/specs \
  --out crates/dive-api/tests/fixtures/temporal-action-args \
  --timeout 30 AaMath AaNested AaCaptured AaSequence
```

All four violate P (exit 13), generating 3 states, retaining 2 and reaching
depth 2. Stuttering violates their recurring non-stuttering step requirement.
Both value profiles/stores match final truth/counts; periodic claims validate
independently. Cases cover arithmetic helpers, nested forwarding, captured LET
parameters and sequence-prefix helpers such as `Prefix(log, log')`.

Pure value/action helpers retain action-level arguments lazily in their caller
scope and evaluate them on the actual edge. Constants remain value-bound.
Temporal helpers still accept only constant/state-level arguments; temporal
arguments, higher-order calls, effects and opaque contexts remain refusals.
The domain entry, visit and depth bounds are unchanged.

Independent cycle algebra checks every Boolean-coded integer word of lengths
1–4 at every closing edge. Negative controls refuse Print, checker reads and
unsupported temporal forwarding without evaluating effects. Exact TLC phase-L
behavior, lasso choice and parallel graph capture remain pending.
