# Source-owned periodic backend checkpoints

`dive_liveness::periodic_backend::execute` runs real model initialization and
source commits with explicit partial liveness checkpoints. It is a Rust backend
diagnostic. It does not enable ordinary periodic temporal checking in
`dive check --compat=tlc`, and none of its outcomes means that liveness holds.

Run the Cargo-owned example:

```sh
cargo run -p dive-api --example periodic_backend --locked
```

It binds a model whose state remains `x=0` while the property requires recurring
`x=1`. The forced request checks once after complete initialization, then after
the first source commit. The actual capture stops at a candidate and prints:

```text
sources=1 pointer_size=1 last_check_size=1 actual_pool=4 candidate_requires_replay=true model_proof=false
```

The complete Rust example is [compiled by Cargo](../crates/dive-api/examples/periodic_backend.rs).
For an already bound `model`, the central call is:

```rust
use dive_check::CheckOptions;
use dive_liveness::{periodic_backend::{self, Policy}, phase_graph::Limits};

let options = CheckOptions { workers: 4, ..CheckOptions::tlc(0, 0) };
let run = periodic_backend::execute(
    &model,
    &options,
    Limits::default(),
    Policy { force: true, ..Policy::default() },
);
assert!(!run.model_proof());
```

Binding is a separate caller step. This endpoint cannot undo effects that a caller
already allowed during binding. Its source/scoped admission refuses unsupported
effects before backend evaluation; use the Cargo example’s pure model and do not
interpret this as a sandbox for arbitrary prior binding callbacks.

`Policy::default()` requests unforced checking after successful complete Init
and each whole source commit, with growth threshold 0.1. It does not select a
wall-clock interval. `Schedule::Final` suppresses unforced partial checks; a
forced request still runs them. The force field applies to every explicit
request, unlike the outer periodic controller's force flag which is cleared
after a successful check. The threshold uses TLC's strict floating-point
comparison, including NaN/infinity and zero-size snapshots.

`run.checkpoints()` exposes readonly, producer-derived receipts:

| Field | Meaning |
| --- | --- |
| `sources()` | Number of whole source callbacks already committed; zero is completed Init |
| `stats()` | Actual logical source-prefix counters; not speculative pool counts |
| `before()` / `after()` | OOS-ordered pointer-table sizes and last-check snapshots |
| `growth_due()` | Actual `doLiveCheck` growth result, separate from final/force admission |
| `barrier()` | Submitted generation/check jobs have joined for this fixed native source wave; absent during initialization |
| `decision()` | Skipped, NoCandidate, Candidate, or Failed; never Holds |
| `candidate_check()` | Index of the first deciding selection, requiring independent replay |

For null tableaux, pointer size counts actual written fingerprint keys; initial
roots and unfinished successor placeholders do not count. For non-null tableaux,
it counts distinct fingerprint buckets, including registered/undone and done-only
buckets. Multiple particles with the same fingerprint count once. File bytes,
written-record totals and product-node counts are different quantities.

Last-check snapshots start at one. A processed OOS records its current size after
its PEM loop, including when that loop finds a candidate. A failed/thrown check
and later unprocessed OOSs retain their old snapshots. Successful repeated checks
use fresh SCC traversal state; a candidate stops the actual capture before later
source commits. `run.stop()` retains the typed deciding stop and real worker/count
facts. `DIVE-L0054` means an unvalidated diagnostic candidate stopped the run,
not that a model-checking violation was certified. Other input, scoped planning,
capture and resource failures keep their original typed stage.

`run.complete()` is present only after complete safety exploration and graph
construction. It still does not prove liveness. All selections require fresh
independent memory/JSON/periodic-word replay before any model-checking verdict.
The private receipt constructors exclude caller-supplied graphs, sizes, write
masks and claimed validation flags. `options()`, `policy()` and `limits()` retain
the actual caller inputs; requested width is separate from the actual pool width.

The joined-wave barrier states only that all submitted jobs returned. Later
sources in that at-most-eight-source wave may have been evaluated speculatively;
their work does not enter logical stopping counters. Initialization has no pool. A stopped inline Init or initialization-completion
consumer reports one actual inline worker and the requested width; an admission
refusal before evaluation reports zero.
This is not Java's queue-wide suspension or finish flag, and it does not establish
ordinary clock or checkpoint timing.

Existing capture, graph, scoped purity/material and Phase-L limits remain in
force. Pointer projection, snapshot/receipt retention, graph construction, SCC
and witness work share the original graph-step quota. Java's signed-int pointer
count is additionally enforced. Phase-L predicate/value limits remain separate.
A failed reservation can appear in attempted accounting while the rejected
operation never executes. No entry/inspection quota is an aggregate memory or
file-byte bound, and no incomplete/limited run reports success.

The same named refusals as source-owned graph execution remain: unsupported
scopes/operators, live checker-state access, effects/host callbacks, foreign
IR/name/context ownership, unavailable execution modes and fixed structural
bounds. Ordinary periodic clock/ratio/queue/quiescence races, PERIODIC hooks,
checkpoint transactions and mode-specific full witnesses remain pending. The
four controlled Java IO failures are retained reference failures, not native
error-class matches. Default and TLC profiles acquire no new CLI schedule from
this API.

See the [reference controls](../conformance/probes/periodic-backend/README.md),
[ADR 0051](decisions/0051-source-qualified-periodic-backend.md),
[partial SCC guide](partial-temporal-checks.md) and
[remaining parity work](design/tlc-parity-plan.md).
