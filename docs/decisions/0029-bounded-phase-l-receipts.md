# 0029: Bounded concrete phase-L receipts

- **Status:** accepted
- **Date:** 2026-10-10

## Context

An edge's stored destination or generator order cannot reproduce TLC's liveness
predicate checks. Worker collects concrete incoming successors in `SetOfStates`,
including already seen states, appends stutter and checks bins in slot order.
Graph/tableau insertion can evaluate additional predicates between OOS groups.

## Decision

Add `dive_liveness::phase_l::Session` as a separate inline diagnostic stage.
Fresh bound planning privately retains pure scoped predicates and original
configured identities, separately from derived roles. The caller supplies
accepted concrete successors with increasing generator ordinals; the stage
never reconstructs Next or substitutes stored representatives. Admission,
constraint and safety-stop provenance is not certified by this interface.

Derive fingerprints internally in TLC variable order, with the requested pinned
polynomial. Require TLC value/comparison/exploration mode; refuse VIEW/symmetry
and lazy/infinite/opaque state values before invoking their callbacks. Enumerated
material sets/functions are preflighted recursively. Follow low-32-bit masked
buckets, linear probes, incoming-to-existing equality, first representatives,
grow-before-duplicate and old-slot reinsertion. Continue probing incomparable
material states, without swallowing other Rust errors. Forced keys exist only
in a separately named scalar structural diagnostic; they cannot enter a model
receipt or earn fingerprint evidence.

Maintain one logical multiplier history in source commit order: capacity starts
at `16 * multiplier` and the multiplier increments once after a successful
source if capacity grew. Failed growth retains the prior concrete prefix while
recording actual speculative work. Completed source counts do not advance on
failure. This first stage executes inline and reports actual workers one;
wider diagnostic requests are named refusals with zero executed workers.

For null-tableau OOS groups, evaluate all action bins in slot/bin order, then
source-state bins. `LNAction` bracket checks evaluate subscript values before
body; equal subscripts short-circuit nonstuttering/stuttering actions as TLC
does. The ordinary evaluator's `StepAngle` body-first contract is not changed.
Tableau initial particle consistency is a separate ordered diagnostic. Source
receipts containing a tableau retain collection then refuse bin execution:
dynamic consistency/revisitation is not yet implemented and can decide an error
before another OOS. Every receipt declares `graph_insertion=false`.

Bounds cover admission count, slot capacity, material visits, retained result
cells, collection/predicate-expression work, and recursion depth 256. Formula,
graph and successor defaults are not raised. These are entry/work bounds, not
aggregate byte accounting. Failure carries original errors, actual workers and
prefix counters plus the unfinished collection/bin receipt when available.

State-kind IF syntax reachable from a retained predicate is conservatively
refused. Pinned `astToLive` can evaluate constant branches while native planning
still retains the whole IF atom (#147). Preserve that prior disagreement; the
refusal gains no planning credit. Shared body-first `PredicateAtom` and independent
periodic semantics have separate correction work (#149).

## Consequences

The full selections retain 55 forced-key structural controls, 35 actual synthetic
`LiveCheck` callback controls, 51 bound-model rows and 20 clean TLC runs. Compiler
failures, initial adapter mistakes, invalid/corrected action/import controls and
before-fix native observations remain byte-exact historical evidence. Native
model fields are counted independently of refusals and graph-pending rows.

Independent trace validation rejects forged Next/closing edges and property
claims even when diagnostic receipts can be constructed. Neither receipt nor
successful bin projection is a model proof. Strict integrated liveness remains
refused; F-020/F-021/F-022 stay partial/audit-required and F-023 missing. #140
remains open. The next increment must attach concrete ordered receipts to graph/
product insertion and actual pool commits before SCC/lasso/scheduling work.

## Concrete graph increment

`phase_graph` connects a completed pure safety capture to actual persisted native
nodes for one null-tableau OOS. Capture now has a separate actual-pool entry with
bounded original concrete rows; the legacy inline capture is preserved. Source
commits retain one logical SetOfStates capacity history independent of requested
pool width. The graph projection retains original configured properties and
separate derived role names; it does not substitute one field for another.

Capture completes before temporal predicates run. Its statistics represent
observed capture work and are not an integrated TLC temporal stopping prefix.
Graph, bin and PEM-input bounds are checked before allocation/predicate execution;
per-level speculative per-source quotas are named conservative structural bounds.
Entry/work limits do not claim aggregate byte accounting. A graph result always
has `model_proof=false`; receipt and commit insertion flags remain distinct.

The complete fifteen-model pinned selection, synthetic collision control and
fifteen clean TLC outcomes are retained in `conformance/probes/tlc-phase-l-graph/`.
Multiple-OOS/tableau revisit ordering and strict SCC/lasso/schedule integration
must follow this increment. #140 remains open and F-020/F-021/F-022 partial,
F-023 missing. No default or integrated-liveness refusal is changed.
