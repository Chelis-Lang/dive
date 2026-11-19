# Getting started

[Install](install.md) dive first. Then, in an empty directory, write a small
counter specification and its configuration:

```sh
cat > Counter.tla <<'TLA'
---- MODULE Counter ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x < 4 THEN x + 1 ELSE x
Bound == x <= 4
StrictBound == x < 4
EventuallyFour == <>(x = 4)
====
TLA
cat > Counter.cfg <<'CFG'
INIT Init
NEXT Next
INVARIANT Bound
CFG
```

Analyze it, then check it:

```sh
dive analyze Counter.tla --output json
dive check Counter.tla --compat=tlc --seed 7 --fp 0 --workers 4 --output json
```

`dive check` finds `Counter.cfg` beside the source. The run reports `success`,
five distinct states, six generated states and depth five: `Bound` holds on the
whole state space. The self-loop at four prevents a deadlock.

The first line of a human-readable report names the run settings: the
profile, store, seed, and the requested and used worker counts.

Now change only the configured invariant:

```sh
cat > CounterViolation.cfg <<'CFG'
INIT Init
NEXT Next
INVARIANT StrictBound
CFG
dive check Counter.tla --config CounterViolation.cfg --compat=tlc --seed 7 --fp 0 --output json
```

The result is an `invariant-violation` of `StrictBound` with a counterexample.
`dive check` uses TLC's process statuses, so this exits 12. A script that runs
under `set -e` must capture that expected nonzero status.

Before you treat a result as a certificate, read
[reports and errors](reports-and-errors.md) and
[trace validation](traces.md). For your own specifications, start with
[safety checking](checking.md).

These files are also in `docs/user/examples/` of the source repository. Most
other guides use that copy, so run their commands from the root of a checkout.
