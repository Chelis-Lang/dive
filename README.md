# dive

**D.I.V.E. (Deterministic Invariant Verification Engine)** is a ground-up Rust implementation of the TLA+ toolchain: parsing and semantic analysis equivalent to SANY, explicit-state model checking equivalent to TLC, simulation, trace validation, PlusCal, a library API and a language server. It has no JVM dependency.

The standard is an **unimpeachably sound port**. Every result is verified continuously against the canonical Java tools. Where `dive` cannot match TLC exactly, it refuses with an explicit `unsupported` result instead of approximating.

> **Status: early development.** `dive parse`, `dive analyze`, configuration
> parsing, safety `dive check` and `dive validate-trace` are implemented.
> The protected conformance gate compares safety results with pinned TLC
> goldens. Rust callers can use bounded final temporal checking with
> independently validated counterexamples; ordinary `dive check` still
> refuses temporal and symmetry models. See [`docs/sprint-status.md`](docs/sprint-status.md)
> for measured coverage and the remaining roadmap.


## Documentation

- [`docs/CHARTER.md`](docs/CHARTER.md): mission, scope, soundness contract and architecture.
- [`AGENTS.md`](AGENTS.md): how to work in this repo, for humans and agents.
- [`docs/ROADMAP.md`](docs/ROADMAP.md): milestones.
- [`docs/tlc-parity.md`](docs/tlc-parity.md): the pinned feature ledger and remaining path to full TLC parity.
- [`docs/tlc-differences.md`](docs/tlc-differences.md): where dive differs from TLC and why, with `--compat=tlc` restoring TLC's exact behaviour.
- [`docs/temporal-api.md`](docs/temporal-api.md): final temporal checking for Rust callers.
- [`docs/pluscal-rewriting.md`](docs/pluscal-rewriting.md): bounded in-memory block replacement and explicit translation-body checksums.
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

For the supported bounded temporal subset, opt into final checking and an
independently validated short lasso with
`dive check Model.tla --lncheck final --lasso short --output json`.
See [temporal checking](docs/temporal-api.md#command-line-access) for scope,
lossless traces and compatibility limits.
