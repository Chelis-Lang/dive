# Closed committed-prefix diagnostic

Final runtime producer: `c3198757d86dc84b2e710adc15d88f3367638f64`.
The full workspace gate belongs to immutable producer
`ed31d97378d43744c89a2bac2a48307ca10251ac`: 43 stages and all 1,032 tests passed,
with zero skipped. Only `capture.rs` rustdoc captions changed afterward;
non-documentation lines are byte-identical. Forty final-source stages separately
check formatting, offline evidence, Clippy, the unchanged eight prefix controls,
doctests and rustdoc. The final release/runtime source and binaries remain unchanged
through their closed consumers.

The Cargo example, 29 native controls, isolated old-timer/new-request replay and
all 21 CM1 plus 16 validation-smoke models pass at 1, 4 and 8 workers. Each native
epoch retains 84 files: 76 request observations and eight independent EOF replays.
Each also retains eight valid bounded prefix replays and four explicit refusals
at the unchanged independent-validator work bound. Repeated epochs are not added
as distinct model coverage.

The 24 actual controlled Worker/queue reference observations remain separate from
native runtime. Selected transaction ranges, graph-through source callbacks and
successful physical frontier append are different facts. No ordinary schedule,
TLC stop-count agreement, model absence or feature-family certificate is claimed.

[User contract](../../../docs/committed-periodic-prefix.md),
[full gate](gate-current/after.json),
[final-source qualification](gate-current/final-documentation-closed.json),
[runtime](gate-current/runtime-closed.json),
[delivery index](delivery-index.json).
