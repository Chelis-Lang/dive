# Ordered temporal execution evidence

This bounded Rust library increment connects actual safety initialization/source
callbacks to real temporal graph insertion. It is intermediate evidence under
#140, not a strict model-checking verdict. Ordinary strict checking stays refused.

The complete `selection.json` contains 26 cases. Actual pinned `LiveCheck` runs
produce 15 completed graph projections, three zero-OOS role/binding cases and
eight failures. Every case, compiler stream, model and raw failure is retained.
The Rust census compares all completed final roots, state/action labels,
transitions, product registrations and **actual** per-fingerprint tableau record
encounter order at 1/3/4/8 workers. A source count is not graph agreement.

`ExecMultiple` has two distinct actual OOSs. `ExecRevisit` discovers new product
nodes after the source fingerprint is done, exercising immediate recursive
re-enumeration. Null OOSs share TLC's null grouping key; synthetic duplicate
null groups are not claimed as multiple-OOS coverage. Source receipts retain
`graph_insertion=false`; the separate persisted commits prove actual insertion.

The graph callback executes per group: action bins over successor slots, state
bins, tableau-node/slot consistency, done marking, product insertion and immediate
recursive visits before the next group. Recursive visits check state bins,
stutter/action bins and particle consistency, then materialize each complete
action `StateVec` before constraint/edge predicate checks. Original concrete
values survive duplicate/seen-state admissions; stored target representatives
never replace them. Recursive emissions and retention are reported separately
from main safety counts. Entry, slot, cell, inspection and recursion bounds are
fixed; these are not aggregate byte budgets.

The accepting-sink/empty-PEM early witness path remains a named refusal before
later work. Incomplete and nonboolean recursive-constraint classification,
state-kind IF planning, VIEW/symmetry, impure/introspective/runtime-name effects
and opaque providers remain explicit boundaries. A complete captured graph is
not a proof: no exact TLC SCC/lasso choice or periodic schedule is certified.
Standalone independent word validation remains separate from graph construction.

## Stopping and initialization controls

`clean-tlc-reference/` retains eight actual `tlc2.TLC` executions at the pin,
workers one, seed zero, polynomial zero and final liveness checking, without
`-dumpTrace`. The source callbacks reproduce temporal division error 2179 before
later-source safety and enumeration failures. Their logical stopping prefix is
three generated, two distinct, one queued, depth one. Conversely an earlier
same-source invariant failure prevents the temporal source callback.

Initial product consistency runs after enqueueing a new root, before invariant
checking. Callback-prefix counts are explicitly separate from TLC's `doInit`
error replay: replay can enumerate more roots or print a later invariant error.
`TemporalTableauInitBeforeInvariant` records both the first division error and
secondary invariant failure. `TemporalTableauInitBeforeLaterRoots` retains replay
counts two/two/two rather than presenting the first callback's one/one/one as TLC
final counters. The first four initial controls also preserve constant-invariant
startup and PROPERTY role-splitting observations; they earn no tableau coverage.

A stopped execution carries the typed deciding error, actually persisted graph
prefix, actual pool width and last source callback's logical counters alongside
its batch's actual speculative emissions. Initialization uses one execution
thread before requested pool setup. Preflight failures use zero; lost execution
evidence is absent, never invented. User-provided observer callbacks alone earn
no temporal correctness credit.

## Independent checks and history

Actual product words are replayed through standalone `dive-trace`, including
complete configured claims. Mutants alter Init, Next, closing targets or omit a
false property. Captured `PrintT` and foreign IR/interner cases are refused before
callbacks/pool evaluation. Existing scoped-word, tableau, null-graph and random
reachability controls are rerun; original studies receive no new corpus credit.

`historical-first-compiler/` retains the first compiler failure and all 26 selected
inputs (zero executed rows). `historical-before-record-order/` retains the whole
first successful capture: its `records` display sorted particle indices, while
the native record encounter order was preserved. The failed comparison is kept
as an observation; the subsequent probe adds a separate real `record_order`
field. Nothing is sorted away from the production graph. Earlier source matrices
and their exact producer files live under each family's
`historical-before-execution/`; the execution matrix identifies the combined current source separately from
those preserved producer files. Bounded graph/source evidence is not a verdict.

Reproduce the actual graph capture with:

```sh
python3 conformance/probes/tlc-phase-execution/capture.py \
  --java "$JAVA17" --javac "$JAVAC17" --jar "$PINNED_TLC_JAR" \
  --reason "Record the complete ordered execution graph selection"
python3 -I scripts/check-phase-execution.py
cargo test -p dive-check --test observed -p dive-liveness --test phase_execution
```

F-020/F-021/F-022 stay partial/audit-required, F-023 missing and #140 open. The
next implementation joins these actual products to pinned SCC/lasso/witness and
stopping behavior, including the accepting-sink path. No named refusal is lifted
because a diagnostic or product projection completed.

Combined integration retains fixed eight-source expansion waves, checkpoint
frontiers and callback-free bound-role validation. A three-wave regression
checks source-prefix totals at 1/3/4/8 workers in both stores under shuffled
scheduling. Explicit inline observation is refused before callbacks until its
ordering is independently verified. Original separately tested execution
sources remain under `native-parity-integration/history/execution`.
