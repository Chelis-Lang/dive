# dive

**A deterministic TLA+ checker with independently validated counterexamples.**

- **5.1× faster than pinned TLC** by geometric mean across 169 agreeing tlaplus/Examples checks at one worker, including startup.
- **Minutes become seconds.** `MCInnerSerial` takes 8.9 minutes in TLC and 4.4 seconds in dive. `MCLeastCircularSubstringMedium` drops from 3.3 minutes to 2 seconds.
- **Instant on small specs.** `DieHard` checks in about 6 ms. TLC takes about half a second.
- **Lower memory on the ten-model release panel.** At one worker, peak RSS was 37–110 MiB for dive and 0.8–1.4 GiB for TLC.
- **Existing TLA+ models.** Reads `.tla` and `.cfg` files and includes a `tlc` command for TLC-style invocations.
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

Dive compares verdicts and state counts with pinned TLC on its conformance
suite and independently replays counterexamples. Unsupported constructs
produce a named refusal. The default follows standard TLA+ semantics where
TLC's behavior is an implementation detail. `--compat=tlc` selects the available
compatibility options. See the [compatibility guide](docs/user/compatibility.md).

## Performance

The release timings compare one-worker runs on the same host and include
JVM startup. TLC's memory was measured at `-Xmx3g`. The geometric mean covers
the 169 tlaplus/Examples configurations with matching results or a documented
expected difference. The 48 larger configurations in that set ran 3.2×
faster by geometric mean. The methods and per-model timings are in the
[release performance results](docs/developer/release-performance-results.md).

The optional finite symbolic checker completed KeyValueStore Large with
594,616,815 distinct states. The large-model measurements include timings,
memory, independent checks and explicit scaling results.
[Symbolic checking](docs/user/symbolic.md) · [Large-model comparisons](docs/developer/large-models.md)

## Documentation

Start with [getting started](docs/user/getting-started.md) and the
[user manual](docs/user/README.md). The [changelog](CHANGELOG.md) lists
changes by release. Maintainer documentation starts at [AGENTS.md](AGENTS.md).

## Contributing

dive does not accept outside pull requests at this time. Issues are welcome.
See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. See [LICENSE](LICENSE).
