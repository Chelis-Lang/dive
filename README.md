# dive

**D.I.V.E. (Deterministic Invariant Verification Engine)** is a native Rust
TLA+ toolchain. It parses and analyzes specifications, checks supported models,
translates supported PlusCal algorithms, and independently validates
counterexamples. Checking needs no JVM.

## dive and TLC

- **Same answers.** On the tested tlaplus/Examples corpus, dive gives TLC's
  result on every model it checks, apart from a short list of registered
  differences. At one worker, counting TLC's JVM start-up, it takes about a
  fifth of TLC's time on the geometric mean.
- **No JVM.** dive is a native binary.
- **Named refusals.** When a specification needs something dive does not
  implement, the run ends as `unsupported` and names the feature. dive never
  approximates a result.
- **One switch for TLC behaviour.** The default profile uses canonical TLA+
  semantics where TLC's behaviour is an implementation artifact. `--compat=tlc`
  selects TLC's behaviour instead.

[Where dive differs from TLC](docs/tlc-differences.md) lists every difference,
its TLC-compatible setting and the known gaps in `--compat=tlc`.

## Install

With Rust 1.99 and a C compiler installed, from crates.io:

```sh
cargo install dive-cli --locked
```

or from the repository:

```sh
cargo install --git https://github.com/Chelis-Lang/dive --locked dive-cli
```

Either installs `dive` and the `tlc` command adapter, which accepts TLC's
checking arguments.

Linux x86_64 and aarch64 archives with both binaries, their license notices
and SHA-256 checksum files are on the repository's Releases page. Verify an
archive with `sha256sum --check ARCHIVE.tar.gz.sha256`, extract it, and put the
binaries on your PATH. The archives are built on Ubuntu 24.04 and need
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
| TLC compatibility | `--compat=tlc` selects TLC's behaviour; [known gaps](docs/tlc-differences.md) are listed. |

A result is never reported as proved when its required checking or validation
is unsupported. Not every TLC mode is covered; what is missing refuses by name.

## Documentation

- [User manual](docs/user/README.md), [checking](docs/user/checking.md),
  [PlusCal](docs/user/pluscal.md) and [temporal checking](docs/user/temporal.md).
- [Rust API](docs/user/rust-api.md) and [trace validation](docs/user/traces.md).
- [Changelog](CHANGELOG.md).

## Building from source

The source checkout pins its toolchain in
[rust-toolchain.toml](rust-toolchain.toml). Build with
`cargo build --release -p dive-cli`. `cargo xtask gate --fast` runs the same
format, lint, test and documentation checks as CI.

## Contributing

dive does not accept outside pull requests at this time; issues are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

Maintainers: [AGENTS.md](AGENTS.md), the [charter](docs/CHARTER.md),
[provenance](docs/provenance.md), the [developer guide](docs/developer/README.md),
the [release gates](docs/RELEASE.md) and the [scoreboard](docs/scoreboard.md).
