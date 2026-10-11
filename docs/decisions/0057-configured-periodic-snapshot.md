# 0057: Configuration-owned periodic snapshot diagnostic

- **Status:** accepted
- **Date:** 2026-10-11

## Context

Issue #197 extends the supplied-expression diagnostic in ADR 0054. A supplied
`Pred` is not the configuration's hook. The pin snapshots `Defns` after
substitutions but before constant caching, then `SpecProcessor.processPeriodic`
looks up the configured spelling in that snapshot. It requires an `OpDefNode`
with no parameters and retains its original body. Constant-role entries can
throw `ClassCastException`; that exception has no TLC message code.

The complete 26-case primary and additive 12-case startup/precedence selection
are retained in `conformance/probes/periodic-snapshot`. Real unused startup
`PrintT` output precedes even missing-name and arity errors. Merely rejecting a
hook after binding therefore cannot ensure callback-free admission.

## Decision

Share the existing binder with an additive private diagnostic switch and a
privately constructed `PeriodicSnapshot`. Ordinary binding still refuses
`_PERIODIC`. Preserve configured spelling, selected definition identity and
original body separately. Exposed diagnostic models retain an exact ordinary
refusal marker. Private diagnostic execution revalidates its owned source,
retained configuration and predicate, then removes exactly that producer marker
from a private clone; unrelated unsupported entries remain.

`dive_liveness::periodic_snapshot::bind` runs structural input validation and
conservative source/startup admission before binding. It roots nonnative user
bodies, standard top-level zero-arity startup definitions, selected roles and
actual matching module-context alias targets. Effects, runtime names, opaque
operators, cycles and unproved constant startup constructors refuse. Structural
nonconstant witnesses are resolved against actual replacements rather than
trusting editable level annotations. The small startup grammar inspects lazy
LET bodies without forcing them. The selected hook then passes ADR 0054's pure
zero-state admission before clocks or workers.

Inspection remains 50,000 units/depth 128. Separate startup reservations and
shared hook-work reservations use the existing checked cubic policy weight.
This is conservative admission policy, not an independently proved evaluator
complexity, instruction, heap or byte bound. Graph/Session ceilings do not change.

## Consequences

Pure configured bodies can drive actual native diagnostic pauses with original
FALSE-equality/evaluation errors and configuration-owned provenance. They never
certify ordinary scheduling, temporal Holds or a violation. Actual candidates
require fresh independent word, memory and JSON replay; valid replay is still
candidate evidence, not complete schedule authority.

Startup/effect restrictions are honest refusals, not error-class agreement with
the pin. Configured constant-definition captures, INSTANCE substitutions,
state/action/temporal hooks and Java exception boundaries remain explicit.
Ordinary scheduling still requires queue/timer stop-prefix equality, checkpoint
transactions, exact counts and complete deciding-witness certification.
