# dive

**D.I.V.E. (Deterministic Invariant Verification Engine)** is a ground-up Rust implementation of the TLA+ toolchain: parsing and semantic analysis equivalent to SANY, explicit-state model checking equivalent to TLC, simulation, trace validation, PlusCal, a library API and a language server. It has no JVM dependency.

The standard is an **unimpeachably sound port**. Every result is verified continuously against the canonical Java tools. Where `dive` cannot match TLC exactly, it refuses with an explicit `unsupported` result instead of approximating.

> **Status: early development.** `dive parse`, `dive analyze`, configuration
> parsing, safety `dive check`, finite pure safety `dive simulate`, and
> `dive validate-trace` are implemented.
> The protected conformance gate compares safety results with pinned TLC
> goldens. Rust callers can use bounded final temporal checking with
> independently validated counterexamples. Pure safety symmetry now retains
> concrete validated traces. Eligible ordinary TLC-profile requests use bounded
> final temporal checking; other ordinary temporal routes and unsupported
> symmetry interactions retain named refusals. Configured native random replacements refuse
> before startup until their stream authority is proved. See `docs/scoreboard.md`
> for measured coverage and `docs/ROADMAP.md` for the remaining roadmap.

The checking-only `dive tlc` and installed `tlc` adapter accepts common TLC
checking flags and module/configuration basenames. `-lncheck final` selects the
existing bounded, independently validated short-lasso route. Tool messages and
other TLC mode protocols still refuse. See
[the checking guide](docs/user/checking.md#tlc-checking-entrypoints) for supported
flags, path resolution and statuses.


Native PlusCal translation supports bounded debug-header serialization and a separate
`--ast` in-memory original AST operation for measured procedure/macro and compound
ownership, plus measured termination, done-disjunct, wrapping,
sequential version and label-reporting options. File preferences return an
in-memory plan; broader AST shapes, backends and filesystem writes remain unfinished. See
[the translator guide](docs/pluscal-translation.md).

## Documentation

Start with the [user manual](docs/user/README.md) and its
[runnable examples](docs/user/examples/README.md). The
[developer documentation map](docs/developer/README.md) connects design, known
issues, provenance and conformance evidence.

- [`docs/CHARTER.md`](docs/CHARTER.md): mission, scope, soundness contract and architecture.
- [`AGENTS.md`](AGENTS.md): how to work in this repo, for humans and agents.
- [`docs/ROADMAP.md`](docs/ROADMAP.md): milestones.
- [`docs/scoreboard.md`](docs/scoreboard.md): corpus results and the roadmap feature checklist.
- [`docs/tlc-differences.md`](docs/tlc-differences.md): where dive differs from TLC and why, with `--compat=tlc` restoring TLC's exact behaviour.
- [`docs/temporal-api.md`](docs/temporal-api.md): final temporal checking for Rust callers.
- `docs/tuple-only-trace-replay.md`: strict independent replay of proved tuple sequence calls and remaining namespace limits.
- [`crates/dive-sim/README.md`](crates/dive-sim/README.md): seeded finite simulation and its current limits.
- [`docs/pluscal-rewriting.md`](docs/pluscal-translation.md): bounded in-memory block replacement and explicit translation-body checksums.
- [`docs/decisions/`](docs/decisions/): architecture decision records.
- [`docs/survey/`](docs/survey/): oracles, corpora and the competitive analysis.
- [`docs/conformance.md`](docs/conformance.md): latest conformance summary (generated).

## Building

```sh
cargo xtask install-hooks   # once per clone
cargo build --workspace
cargo nextest run --workspace
```

The toolchain is pinned in `rust-toolchain.toml`, and rustup installs it automatically.

For the supported bounded temporal subset, ordinary TLC-profile checking uses
final analysis with an independently validated short lasso:
`dive check Model.tla --compat=tlc --fp 0 --seed 7 --output json`.
The native profile also exposes explicit selection with
`dive check Model.tla --lncheck final --lasso short --output json`.
See [temporal checking](docs/temporal-api.md#command-line-access) for scope,
lossless traces and compatibility limits.

The pure safety DFID slice is available with
`dive dfid Model.tla --max-level 8 --seed 7 --output json` and
`dive_api::dfid_file`. Reaching the depth bound reports incomplete exploration;
only a separate completion certificate permits `model_proved: true`. See
[DFID controls and limits](crates/dive-check/tests/fixtures/dfid/README.md).
