# Getting started

[Install](install.md) dive first. Then run the included model from the
repository root:

```sh
dive analyze docs/user/examples/Counter.tla --output json
dive check docs/user/examples/Counter.tla --compat=tlc --seed 7 --fp 0 --workers 4 --output json
```

`dive check` finds `Counter.cfg` beside the source. The run reports `success`,
five distinct states, six generated states and depth five: `Bound` holds on the
whole state space. The self-loop at four prevents a deadlock.

The first line of a human-readable report names the run settings: the
profile, store, seed, and the requested and used worker counts.

Now change only the configured invariant:

```sh
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterViolation.cfg --compat=tlc --seed 7 --fp 0 --output json
```

The result is an `invariant-violation` of `StrictBound` with a counterexample.
`dive check` uses TLC's process statuses, so this exits 12. A script that runs
under `set -e` must capture that expected nonzero status.

Before you treat a result as a certificate, read
[reports and errors](reports-and-errors.md) and
[trace validation](traces.md). For your own specifications, start with
[safety checking](checking.md).

Model checking needs no JVM. Only the developer conformance harness downloads
a pinned JRE and Java oracle, to compare dive with TLC.
