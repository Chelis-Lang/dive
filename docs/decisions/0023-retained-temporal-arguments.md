# 0023: Retain temporal expression argument contexts

- **Status:** accepted
- **Date:** 2026-10-10

## Context

A formal parameter's static SANY level does not establish the level of the
expression actually passed to it. `Identity(p) == p` can receive `[]<>x`.
Forcing that argument under an empty state, or treating the callee's formal as
an ordinary value atom, loses its temporal semantics and caller scope.

The pinned `Liveness.astToLiveAppl` binds actual expression arguments with
`Tool.getVal(..., false)` before translating the body. Clean paired controls
in `conformance/probes/temporal-forwarding/` establish direct/local calls,
forwarded nested INSTANCE bodies and forwarded ENABLED formulas. They also
retain two SANY-valid exported INSTANCE calls that TLC refuses with code 2213
and exit 255. This slice does not accept those calls as a new default.

## Decision

Retain pure temporal expression arguments with `Evaluator::scope_apply`; never
force them as constants. Before admission, walk their actual lexical contexts
under the existing expansion/depth bounds. Follow lazy locals, substitutions
and user definitions; reject effects, live checker reads and opaque operators.
The planner subsequently translates the actual temporal expression at each
use. Unused arguments are not evaluated to guess their truth.

The independent periodic interpreter keeps its own context admission walk and
classifies expressions using their retained scoped level. It does not depend
on the liveness planner or successor enumerator. Independent ENABLED cover
search remains bounded and keeps its existing refusal rules; this increment
forwards supported ENABLED expressions rather than weakening those rules.

Temporal expression arguments inside a retained INSTANCE substitution context
remain explicitly unsupported. The conservative restriction covers the pinned
exported-formal refusal until a more precise context/branch/error proof exists.
Both profiles share this support and these restrictions. No settings or
resource defaults change.

## Consequences

Direct/local temporal expression forwarding and supported nested substituted
bodies gain final/short truth evidence. Higher-order arguments, recursive
functions/operators, opaque captured operators and broader INSTANCE formal
forwarding remain staged gaps. Recursive local function probes are retained as
paired refusals: proving their self-binding and branch-sensitive finite guard
is separate work, not a reason to trust a raw level annotation.

This decision does not certify exact TLC simplification/tableaux, phase-L
scheduling, lasso selection or parallel graph capture. Feature families
F-006/F-007/F-019/F-021 remain partial and require audit.


Unchecked Rust expression references and retained lazy expression IDs now
produce typed refusals at the planner/periodic endpoints, including missing
scoped definitions. Missing references have no invented source span. The raw
native `native-unchecked-context-before.*` repro in the probe directory records
the prior panics; it is not an oracle result. Constructing an evaluator with an
invalid redirect target can still panic before these endpoints run (#129).
General caller-created IR/context validation remains a separate partial API
obligation; this increment does not redesign the evaluator constructor.
