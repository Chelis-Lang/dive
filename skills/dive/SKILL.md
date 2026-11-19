---
name: dive
description: Check TLA+ specifications with Dive, a TLA+ toolchain in one Rust binary that gives TLC's results without a JVM. Use when model checking a .tla/.cfg pair, replacing TLC or tla2tools.jar in a script or CI job, simulating a spec, translating PlusCal, or validating a counterexample or implementation trace.
license: MIT
metadata:
  version: "1.0.0"
---

# Dive

Dive checks TLA+ specifications natively. It reads existing `.tla` and `.cfg`
files and needs no Java. One `dive` binary covers parsing, PlusCal translation,
model checking, simulation and trace validation, and a `tlc` command accepts
TLC's arguments.

Install with `cargo install dive-cli --locked` (Rust 1.99 or later and a C
compiler), or from the Linux archives on the GitHub releases page.

## Check a model

```sh
dive check Spec.tla --output json
```

Start with no other flags. Dive reads `Spec.cfg` beside the module; pass
`--config FILE.cfg` for another one. Read `verdict` and `exit_code` in the
JSON, and the counterexample when there is one.

Add `--compat=tlc` when results must match TLC's behaviour (32-bit integers,
TLC's value order and fingerprints). Without it, dive uses standard TLA+
semantics where TLC's behaviour is an implementation detail. The differences are
listed at https://jeffreykent.dev/docs/dive/tlc-differences.html.

To keep an existing TLC command line, call the adapter with the same arguments:

```sh
tlc -workers 4 -config Spec.cfg Spec.tla
```

## Exit codes

`dive check` uses TLC's process statuses by default:

| Exit | Meaning |
| ---: | --- |
| 0 | No error found on the complete state space |
| 10 | Assumption failed |
| 11 | Deadlock |
| 12 | Invariant violated |
| 13 | Action or temporal (liveness) property violated |
| 75 / 76 | Evaluation error (76 while evaluating an invariant) |
| 150 / 151 | Specification / configuration rejected |
| 2 | Usage error |
| 3 | Unsupported feature or resource limit |
| 4 | Internal error |
| 5 | Stopped by a time, memory, state or depth budget; no verdict |

`--exit-codes native` selects 0 success, 1 rejected input, 10 property
violation, 11 deadlock, 12 assumption failure and 13 evaluation error; 2, 3, 4
and 5 are the same in both conventions. A script under `set -e` must expect the
violation statuses.

## Refusals and stops

When dive meets something it does not implement, it does not guess. The
result is `unsupported`, exit 3, and the message names the feature. Report
that to the user as "not checked", never as a pass. Options dive does not
implement are left out of `--help` but still parse and refuse by name.

`stopped` (exit 5) means a budget ran out before the search finished. Nothing
was proved; raise the budget (`--time-limit`, `--max-memory`) or shrink the
model. There is no default time limit, and the default memory limit is half of
physical memory.

`dive explain CODE` explains a `DIVE-...` diagnostic code and suggests a next
step.

## Temporal properties

Check specifications with temporal properties (liveness, fairness) at one
worker:

```sh
dive check Spec.tla --workers 1 --output json
```

Safety checking scales with workers, but most temporal models run no faster at
eight workers than at one, and some refuse more than one worker. A liveness
violation reports a lasso counterexample and the complete list of properties
it falsifies in `properties`.

## Validate traces

Every counterexample `dive check` reports has already been replayed by an
independent validator. To validate a saved trace again, or a trace recorded
from an implementation, against the same specification:

```sh
dive validate-trace Spec.tla --trace trace.json --output json
```

The trace is a `dive-trace` JSON document or TLC's dumpTrace JSON, and every
state must assign every variable. The result is `valid`, `invalid` or
`unsupported`. Pass the same `--config`, `--compat`, `--seed` and `--fp` as the
check that produced the trace.

## Simulation and other commands

`dive simulate Spec.tla --traces N --depth D --seed S` samples behaviours. A
sample is never a proof, even when it finds no error. `dive translate` turns
PlusCal into TLA+ (`--write` updates the file), `dive parse` and
`dive analyze` check syntax and names without checking a model.

## Documentation

The Dive book is at https://jeffreykent.dev/docs/dive/. Start with
https://jeffreykent.dev/docs/dive/getting-started.html, and use
https://jeffreykent.dev/docs/dive/reports-and-errors.html for the report fields.
