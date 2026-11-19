# dive

**A faster TLA+ toolchain.** Same specs, same results as TLC, about 5x faster.

- **5x faster than TLC** on average across 169 models from tlaplus/Examples. 118 of them run at least 3x faster, and 31 at least 10x faster.
- **Minutes become seconds.** `MCInnerSerial` takes 8.9 minutes in TLC and 4.4 seconds in dive. `MCLeastCircularSubstringMedium` drops from 3.3 minutes to 2 seconds.
- **Instant on small specs.** `DieHard` checks in about 6 ms. TLC takes about half a second.
- **A fraction of the memory.** 37–110 MiB where TLC uses 0.8–1.4 GiB.
- **Drop-in.** Reads your existing `.tla` and `.cfg` files. Includes a `tlc` command that accepts TLC's arguments.
- **One binary** for model checking, simulation, parsing, PlusCal translation and trace validation.

```sh
cargo install dive-cli --locked
dive check MySpec.tla
```

## Install

From crates.io, with Rust 1.99 and a C compiler installed:

```sh
cargo install dive-cli --locked
```

This installs `dive` and `tlc`, a command adapter that accepts TLC's
command-line arguments. Linux x86_64 and aarch64 archives with both binaries
and SHA-256 checksums are on the Releases page.

To build from a checkout, run `cargo build --release -p dive-cli`.

## Quickstart

Write a small specification and its configuration, then check it:

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

dive finds five distinct states and reports no error. It picks up
`Counter.cfg` because it sits next to the module. Use `--output json` for
machine-readable results, and `dive check --help` for the other options.

## Compatibility with TLC

dive's results match TLC's. In a few places the default follows standard
TLA+ semantics where TLC's behavior is an implementation detail. `--compat=tlc`
switches to TLC's behavior, and
[where dive differs from TLC](docs/tlc-differences.md) lists each case.

## Performance

Each comparison above was measured at one worker on the same host, with
every run checked against TLC's result. TLC's times include JVM startup, and
its memory was measured at `-Xmx3g`. The 169 models are the tlaplus/Examples
models both checkers finish with the same result. The method and per-model
numbers are in
[release performance results](docs/developer/release-performance-results.md).

## Documentation

Start with [getting started](docs/user/getting-started.md) and the
[user manual](docs/user/README.md). The [changelog](CHANGELOG.md) lists
changes by release. Maintainer documentation starts at [AGENTS.md](AGENTS.md).

## Contributing

dive does not accept outside pull requests at this time. Issues are welcome.
See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. See [LICENSE](LICENSE).
