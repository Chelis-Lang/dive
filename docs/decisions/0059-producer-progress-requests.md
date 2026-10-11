# 0059: Producer-owned expansion requests and quiescence

- **Status:** accepted
- **Date:** 2026-10-11

## Context

Issue #201 advances F-023 beyond commit-only polling. Pinned
`AbstractChecker.runTLC` calls periodic work before checking completion and joins
workers only on its normal path. `ModelChecker.doPeriodicWork` suspends the real
queue, checks liveness before the configured hook and resumes only the matching
success path. Local `tlc2.tool.Worker` can finish Next before its temporal graph
write. The distributed `TLCWorker` is a different backend.

The complete 33 controlled/natural reference rows retain actual local workers,
real queue calls and delegated unchanged FastTool evaluation. An explicitly
instrumented native `Progress.Gate` blocks the first actual Next predicate;
controlled force injection is labelled separately from natural runs. A forced
candidate returns 2116 with workers alive; the probe's later finish/join is
cleanup, never credited to `runTLC`. Worker division-by-zero retains 2179 and
`keepCallStack=true` even though `runTLC` returns zero: outer error replay is a
separate obligation. Original failed driver epochs remain available.

## Decision

Add an internally owned timer thread and a lock shared with actual expansion
claim admission. Publishing a pending request blocks new claims; admitted work
returns and the real pool joins. Service happens under its existing borrowed,
non-cloneable mutex pause. Only afterward are remaining source claims resumed.
Retained outputs are not regenerated or dropped. Default observer behavior and
ordinary safety checking do not opt in.

Private receipts retain the original submitted identities, claimed IDs,
completed expansion outputs, error-bearing outputs, speculative generation
count and the prior logical counters. These are separate facts: completed
expansion does not establish invariant success, a Phase-L write or source commit.
Claims may complete out of order. Pending timer requests coalesce; the next
interval begins when service consumes a request. Initial request force is
separate from the existing controller's liveness-force flag.

`PeriodicSnapshot::execute_requests` composes only with the actual privately
retained configuration selection. It preserves source/predicate admission and
the ordinary refusal marker. This first slice services requests diagnostically
before current-wave publication; the existing ordered commit path alone invokes
the controller/SCC/configured hook. No mid-wave SCC is run on unwritten output.

Keep fixed eight-source scratch waves and all existing limits. Retained pause
metadata is charged cumulatively against the same `graph.steps` quota before
retention. Inspection limits are not heap/byte or complete evaluator bounds.
Timer intervals outside 1..=2,147,483,647 ms refuse before timer/thread setup in
the snapshot path. Failed Init cannot create a successful quiescence receipt.

## Consequences

This is operational native claim/pause evidence, not Java queue suspension,
ordinary schedule/count parity, a clock certificate or a model verdict. A partial
success never reports Holds. Actual configured-hook false/error and candidate
precedence remain distinct; fresh memory/JSON witness replay remains required.
Ordinary CLI checking and checkpoints remain named refusals.

The next production boundary must establish ordered safety/output publication
for a contiguous completed prefix under owned quiescence, or preserve a named
refusal when source/error order cannot be proved. Only a graph actually written
at that prefix can enter periodic SCC checking. Real queue stop/EOF/clock/error
and deciding-counter contracts must compose before ordinary verdict admission.

After an actual expansion cut, timer requests are cancelled separately from
claim admission. Earlier chaos-ordered unclaimed sources still drain through
the original ordered winner path; aborted metadata cannot replace that error.
The one pending aborted request is a bounded stopping observation, not serviced
hook/SCC evidence. Genuine safety EOF may precede a later temporal budget stop.
