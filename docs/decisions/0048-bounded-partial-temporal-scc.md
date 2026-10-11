# ADR 0048: Bounded partial temporal SCC checks

- Status: Proposed implementation boundary
- Date: 2026-10-11
- Tracking: #185, #66, F-023

## Context

The pinned `LiveWorker.checkSccs` processes a partial graph during periodic
checks. Initial records and successor records without a disk write are skipped,
including their edges; they are not complete vertices with empty adjacency.
Completed records keep first-root, reverse successor stack and EA-false root
queue order. Each successful non-final `LiveCheck.check0` restores the graph
pointer table, and the next check observes newly committed records.

The existing native final SCC entry requires every record to be written. It
cannot be used unchanged for periodic checking. A successful partial check says
only that this check found no accepting component; unexplored states remain.

## Decision

Add a bounded partial SCC projection and actual ordered-source checkpoint
entry, using the same private committed Phase-L record authority that inserts
original concrete successors and done-tableau records. A partial check never
returns `Holds`, completed exploration, or a model certificate. Candidates keep
original product identities, component/anchor/pop order and pinned witness
selection, and need fresh independent word and model-trace replay.

Keep ordinary strict temporal mode refused in this increment. Explicit prefix
projections and forced reference checks do not establish the ordinary clock
schedule or exact suspend/quiescence/stop-count behavior. The next schedule
integration must match the actual `ModelChecker.doPeriodicWork` order:
threshold/runtime gate, quiescence, temporal check, periodic hook, checkpoint
or worker resume. Strict growth/runtime comparisons, NaN/infinity and forced
checks remain distinct. `off` skips SCC checking, not temporal insertion checks.

## Bounds and authority

Existing graph vertex/edge/acceptance-cell and inspection ceilings stay fixed.
Repeated checks charge the same original graph.steps quota as Observer insertion and recursive work; the separately reported partial subtotal is not a second quota. Session predicate/value limits remain distinct. These are entry
and work bounds, not aggregate retained-memory or byte accounting. Caller-edited
node fields or masks may create a supplied-graph diagnostic only; they cannot
create an actual execution certificate or a model verdict. Model/context
ownership and callback-free admission precede graph construction.

## Evidence obligations

Retain all selected reference rows: unfinished roots; finished root to
unfinished child; completed self-cycle; later cycle; EA-false root enqueue;
multiple OOS; dynamic tableau revisit; successful check followed by growth.
Compare actual traversal and witness fields, retain every refusal/failure, and
validate positive and mutated periodic words and concrete witnesses separately.
No current whole-corpus, ordinary-schedule or feature-family credit follows
from this bounded increment.
