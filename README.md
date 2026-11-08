# dive

**D.I.V.E. (Deterministic Invariant Verification Engine)** is a native Rust
TLA+ toolchain. It parses and analyzes specifications, checks supported models,
translates supported PlusCal algorithms, and independently validates
counterexamples. Checking needs no JVM.

**Status: pre-release.** Core is the only checking engine. On the pinned
tlaplus/Examples corpus it has no unregistered disagreement with TLC and is
faster than TLC on the geometric mean; the remaining release gates are listed
in the [release gates](docs/RELEASE.md), with current numbers in the
[scoreboard](docs/scoreboard.md).

## Install

With Rust 1.99 and a C compiler installed:

```sh
cargo install --git https://github.com/Chelis-Lang/dive --locked dive-cli
```

This installs `dive` and the supported-subset `tlc` command adapter. The source
checkout pins its toolchain in [rust-toolchain.toml](rust-toolchain.toml).

The release workflow prepares Linux x86_64 and aarch64 archives containing both
binaries and their license notices, alongside SHA-256 checksum files. No release is published yet.
When available, obtain matching archives from the repository's Releases page,
verify with `sha256sum --check ARCHIVE.tar.gz.sha256`, extract, and put the
binaries on your PATH. GNU/Linux archives are built on Ubuntu 24.04 and require
compatible system libraries.

## Three-command quickstart

Create a small specification and its configuration, then check it:

```sh
cat > Counter.tla <<'TLA'
---- MODULE Counter ----
EXTENDS Naturals
VARIABLE n
Init == n = 0
Next == n' = IF n < 4 THEN n + 1 ELSE n
Bound == n <= 4
====
TLA
cat > Counter.cfg <<'CFG'
INIT Init
NEXT Next
INVARIANT Bound
CFG
dive check Counter.tla
```

The invariant holds over five distinct states. The self-loop at four prevents
a deadlock. Existing specifications use the same `dive check Spec.tla` command;
the adjacent configuration is discovered automatically.

Use `dive analyze Spec.tla` for semantic analysis, `dive translate Spec.tla` for
PlusCal, and `dive check --help` for checking options. Use `--output json` for
automation. See [getting started](docs/user/getting-started.md) and
[reports and errors](docs/user/reports-and-errors.md) for exits and report fields.

## Supported and refused

| Area | Current boundary |
|---|---|
| TLA+ and PlusCal | Native parsing/analysis and supported translation; unsupported constructs are named explicitly. |
| Model checking | Explicit-state safety and supported liveness/fairness checks; resource limits and unimplemented combinations remain explicit. |
| Counterexamples | Independent validation against the specification; a stopped or unsupported run proves no completed result. |
| Core values | Finite sets use canonical mathematical identity; unsupported symbolic identities refuse. |
| TLC compatibility | `--compat=tlc` selects existing compatibility switches; [known gaps](docs/tlc-differences.md) remain documented. |

Core is the engine of `dive check`, `dive simulate`, `dive dfid` and
`dive tlc`; what it does not implement is a named refusal, never a fallback.
A result is never
reported as proved when its required checking or validation is unsupported.
The first release is governed by R1–R6, not complete coverage of every TLC mode.

## Documentation and development

- [User manual](docs/user/README.md), [checking](docs/user/checking.md),
  [PlusCal](docs/user/pluscal.md), and [temporal checking](docs/user/temporal.md).
- [Rust API](docs/user/rust-api.md) and [trace validation](docs/user/traces.md).
- [Roadmap](docs/ROADMAP.md), [release gates](docs/RELEASE.md), and
  [developer guide](docs/developer/README.md).
- [AGENTS.md](AGENTS.md), [charter](docs/CHARTER.md), and
  [provenance](docs/provenance.md).

For a source checkout, run `cargo xtask install-hooks` once and
`cargo build --release -p dive-cli`. Development uses focused checks and required
CI; full qualification is reserved for release or engine cutover.
