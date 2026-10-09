# Temporal INSTANCE substitution probes

Raw results come from the pinned TLC bundle in a clean run (`-lncheck final`,
`-workers 1`, seed and fingerprint polynomial 0). Trace capture is a separate
cross-checked pass. Regenerate from the repository root:

```sh
cargo run --release -p dive-conformance -- dev capture --repo-root . \
  --oracle tlc-2026.10.06.014338 \
  --specs crates/dive-api/tests/fixtures/temporal-substitutions/specs \
  --out crates/dive-api/tests/fixtures/temporal-substitutions \
  --timeout 30 IsReach IsStable IsAction IsQuant IsNested IsComplement IsStateTarget
```

Six supported models all generate 3 states, retain 2 distinct states and reach
depth 2. IsStable violates P (exit 13); the other five succeed. IsStateTarget
is a negative control: replacing a constant declaration with a state expression
is rejected by SANY (exit 4), and must not be accepted as expanded coverage.

API controls exercise both value profiles and both stores. Every returned
periodic claim is independently validated. Exhaustive Boolean words of lengths
1–4 check every closing edge against direct cycle algebra, including nested
substitution scopes, complements and priming. Used effects/checker reads,
higher-order substituted operators, exhausted planning budgets and unproven
substituted disabledness remain refusals. No exact TLC temporal-selection,
phase-L stopping or parallel capture claim is made.
