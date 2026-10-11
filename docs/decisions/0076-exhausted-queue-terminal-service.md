# 0076: Preserve exhausted queue authority before terminal service and joins

- **Status:** accepted; bounded diagnostic implementation
- **Date:** 2026-10-11

## Context

Issue #222 follows the source-owned optional-hook service. Pinned
`AbstractChecker.runTLC` calls `doPeriodicWork` before consulting `done` and
joining workers. `ModelChecker.modelCheck` reaches `finalCheck` only after
successful `runTLC` and no deciding error. A dispatch pause alone does not
establish the real state queue's finished flag.

The complete [reference selection](../../conformance/probes/terminal-periodic-service/README.md)
retains 60 original full TLC observations, a corrected complete 60-row selection
and 60 separately instrumented actual `modelCheck` runs. The corrected safe
property is `<>(x=0)`. Original `<>(x=1)` labels were misleading: legal initial
stuttering violates that property. Fresh independent memory and JSON replay
validate the original reported one-state stutter and reject all four mutants.
This was a fixture-selection error, not a TLC soundness bug.

## Decision

An additive private borrowed token comes only from successful real `run_levels`
frontier exhaustion, after successful ordered source publication and physical
append. The request lane atomically stops publishing/claiming and retains any
actual unserviced request. A joined dispatch mutex holds the callback before
scoped timer and pool joins. Failed Init, source, safety, cut, quota or publication
error does not create terminal authority. Caller booleans, zero counters and
copied request receipts cannot replace it.

The terminal adapter models the *finished* queue's failed suspension separately
from request service's actual joined pause. Controller short-circuit/growth/ratio
order stays pinned; after failed suspension no partial check, configured hook,
checkpoint or resume runs. Existing postjoin final selection can follow only a
successful matching terminal receipt. It confirms actual timer/pool closure,
then uses unchanged original concrete provenance and fresh independent replay.

## Consequences

Mandatory and optional existing APIs retain their paths. A distinct optional
terminal diagnostic opts into this transaction. All source/profile/startup,
graph/material/partial/witness limits remain unchanged and cumulatively metered.
No aggregate memory or evaluator instruction bound is claimed. Checkpoint,
ordinary asynchronous timer/queue/clock/error/count equivalence and temporal
absence remain unproved; ordinary checker/CLI refusals remain. A valid word
certifies that concrete violation, not an ordinary checking schedule or model
completion. See the [executable guide](../terminal-periodic-service.md).

The delegation adapter installs reflection observers around actual queue/live
implementations without replacing their results; instrumentation changes timing.
Natural full CLI controls remain separate. Native teardown joins threads after
failure, whereas Java can return a nonzero periodic result before joining; native
cleanup is not credited as Java stop lifecycle equivalence.


### Distinct partial-stop and full-CLI observations (#222)

The separately controlled 12-row queue study holds a genuinely dequeued state:
physical queue size 0 is not finished while a Worker owns outstanding work.
Actual suspension succeeds, partial checking precedes the hook, and FALSE or
division stops without final selection; a deciding temporal2116 precedes FALSE.
These bounded scheduler observations are separate from the natural full CLI.
The original observer discards the integer returned by `AbstractChecker.modelCheck`;
its earlier description as a void method was incorrect and is preserved as history.
The separate corrected twelve-row observer records actual returns at widths 1/4/8:
TRUE 0, FALSE 2104, division 1000, and temporal violation 2116. Reflected
`errorCode` 0 and observer process exit 0 are separate fields. The subprocess
exits zero because its adapter does not forward the method result to `System.exit`;
none of these early-stop observations establishes temporal success.

The separate unmodified full CLI FALSE-hook counter exits 10, with 3,913,590
states generated/distinct and one queued state after 4.2456s; it prints no success
and no 2104 message. The declared 512MiB heap and 30s observer bound remain recorded.
This is an intentional-boundary observation, not a newly declared TLC bug. A
native early hook/service stop remains incomplete or refused and cannot be Holds.
All original selections, cleanup events and raw command/timing facts are retained
in [the evidence](../../conformance/probes/terminal-periodic-service/README.md).
