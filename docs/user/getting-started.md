# Getting started

From a source checkout, build the CLI with the pinned stable Rust toolchain:

```sh
cargo build --release -p dive-cli
```

Use `target/release/dive` or put that binary on your PATH. Model checking itself
needs no JVM. The developer conformance harness downloads a pinned JRE and Java
oracle separately; it is not the checker runtime.

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
