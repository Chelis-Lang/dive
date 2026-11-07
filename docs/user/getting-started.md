# Getting started

With Rust 1.99 and a C compiler installed, install directly from Git:

```sh
cargo install --git https://github.com/Chelis-Lang/dive --locked dive-cli
```

This installs the `dive` and supported-subset `tlc` binaries. Linux release
archives and checksum verification are described in the [README](../../README.md);
the first release has not been published yet.

From a source checkout, build the CLI with the pinned stable Rust toolchain:

```sh
cargo build --release -p dive-cli
```

The build needs a C compiler (`cc`), because dive links the mimalloc allocator.
Release builds use whole-program link-time optimisation, so they take longer
than debug builds (ADR 0102).

Use `target/release/dive` or put that binary on your PATH. Model checking itself
needs no JVM. TLAPS and the CommunityModules TLA+ sources are built in, so
ordinary specifications need no library-path flags or separate library download.
Local modules and explicit search paths take precedence over these bundled
libraries. Operator support is described in [modules](modules.md).

The developer conformance harness downloads a pinned JRE and Java
oracle separately; it is not the checker runtime.

Use `dive --help` to list available commands and `dive check --help` (or another
command's `--help`) for its options. Help needs no input files. Unimplemented
commands are omitted from the list and still give a named refusal if invoked.
`dive --version` includes the build's `git describe --always --tags --long` revision;
source archives built without Git report `unknown` instead. Human checking
reports name the requested engine (Core by default) and requested/used worker
counts.

Run the included model from the repository root:

```sh
dive analyze docs/user/examples/Counter.tla --output json
dive check docs/user/examples/Counter.tla --compat=tlc --seed 7 --fp 0 --workers 4 --output json
```

`Counter.cfg` is discovered beside the source. The safety run reports `success`,
five distinct states, six generated states and depth five. `Bound` holds on the
complete supported search. The self-loop at four prevents a deadlock.

Change only the configured invariant:

```sh
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterViolation.cfg --compat=tlc --seed 7 --fp 0 --output json
```

The result is an `invariant-violation` of `StrictBound` with a counterexample.
Its TLC-profile exit is 12, so an automation that uses `set -e` must capture
that expected nonzero status. See [reports](reports-and-errors.md) and
[trace validation](traces.md) before consuming the result as a certificate.

The examples are deliberately small. Accepting this module or finishing this
search does not establish support for every TLA+ construct or checking mode.
