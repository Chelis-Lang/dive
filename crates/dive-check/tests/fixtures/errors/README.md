# TLC error-site matrix

These 175 minimal models isolate errors in assumptions, initialization,
successor generation, invariants, state and action constraints, implied
initial and action properties, VIEW, fingerprinting, ALIAS and POSTCONDITION.
`ErAssumeEnabled` is a SANY level rejection; the other 174 are checker cases.

The transcripts in `tlc-2026.10.06.014338/` were captured from the pinned
`94d0c50` oracle with `dive-conformance dev capture`. Each `.command` records
the complete command, using normalized cache and temporary paths. Clean
`.stdout`, `.stderr` and `.exit` are authoritative; `.trace.*` files are the
separate, potentially perturbing trace-capture pass. Tests read the clean run.

Reproduce a cell after `oracle fetch` and `corpus fetch`:

```sh
cargo run -p dive-conformance --release -- dev capture \
  --oracle tlc-2026.10.06.014338 \
  --specs crates/dive-check/tests/fixtures/errors/specs \
  --out /tmp/dive-error-capture ErNextUnassigned
cargo test -p dive-check --test errors
```

The matrix compares the first error's code and complete nested message,
deciding error class and exit status, counts, and counterexample states at
1 and 4 workers with both fingerprint and exact stores. Default-profile runs
also compare except for the registered unbounded-integer divergence, D-004.

Ten value-method errors have unverified deciding classes: TLC's call-stack
replay wraps them differently depending on the value's source location,
which dive does not yet retain on values. Their messages, counts and states
are checked, but their exit status is explicitly outside the agreement claim.
The harness treats `tlc.verified = false` as unsupported, never agreement.
`ErFpLambda` is refused. ALIAS transforms the displayed values in ten cells;
dive currently reports raw states with an explicit note, so those display
values are excluded from this matrix's agreement claim. `tests/errors.rs`
lists every such exception by name and reason.

Important measured distinctions:

- An error while checking an initial state can print that state inside the
  error message without producing a counterexample. EvalException and
  RuntimeException paths differ; implied initial properties have no trace.
- An ordinary Java-override type error is wrapped once and replayed as
  `TLC_NESTED_EXPRESSION`; overflow, assertion and division-by-zero exceptions
  keep their own classes during successor replay.
- An invariant evaluation error on a newly inserted state may resume
  expansion during TLC's call-stack replay. This can alter counts or raise a
  different deciding error. The checker ports those replay rules.
- POSTCONDITION runs after the complete search. It sees final counters, and
  constant postconditions fail through TLC's outer runtime-exception path.

The ported paths and pinned Java methods are recorded in `docs/provenance.md`.
