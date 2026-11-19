# dive

dive is a TLA+ model checker. It reads specifications and configurations
written for TLC and gets the same results, faster. It also parses and
analyzes specifications, translates PlusCal, and validates counterexample
traces.

## Performance

We timed dive and TLC on the 169 tlaplus/Examples models where both finish
with the same result. At one worker, dive took about a fifth of TLC's wall
time (geometric mean 0.195x, median 0.148x), and no model took more than
twice TLC's time. dive is a native binary, so there is no JVM to start: a
full check of a small model takes about 5 ms. It also uses far less memory.
On the benchmark models dive peaked at 37 to 110 MiB at one worker, where TLC
used 795 to 1,449 MiB.

The measurements and how they were taken are in
[release performance results](docs/developer/release-performance-results.md).

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

## Documentation

Start with [getting started](docs/user/getting-started.md) and the
[user manual](docs/user/README.md). The [changelog](CHANGELOG.md) lists
changes by release. Maintainer documentation starts at [AGENTS.md](AGENTS.md).

## Contributing

dive does not accept outside pull requests at this time. Issues are welcome.
See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. See [LICENSE](LICENSE).
