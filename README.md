# dive

**D.I.V.E. (Deterministic Invariant Verification Engine)** is a ground-up Rust implementation of the TLA+ toolchain: parsing and semantic analysis equivalent to SANY, explicit-state model checking equivalent to TLC, simulation, trace validation, PlusCal, a library API and a language server. It has no JVM dependency.

The standard is an **unimpeachably sound port**. Every result is verified continuously against the canonical Java tools. Where `dive` cannot match TLC exactly, it refuses with an explicit `unsupported` result instead of approximating.

> **Status: early development.** The workspace, CI and conformance infrastructure are being built. No `dive` command produces verdicts yet. See [`docs/ROADMAP.md`](docs/ROADMAP.md).

## Documentation

- [`docs/CHARTER.md`](docs/CHARTER.md): mission, scope, soundness contract and architecture.
- [`AGENTS.md`](AGENTS.md): how to work in this repo, for humans and agents.
- [`docs/ROADMAP.md`](docs/ROADMAP.md): milestones.
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
