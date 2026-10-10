# 0041: Native bounded sequential procedure calls and returns

- **Status:** accepted (bounded implementation, full family pending)
- **Date:** 2026-10-10

## Context

The pinned `PcalTranslate.ExplodeCall` saves the continuation, procedure name,
locals and parameters on `stack`; sets actual parameters simultaneously; then
initializes locals in declaration order. `ExplodeReturn` restores the saved
continuation, locals and parameters, then takes `Tail(stack)`. A procedure with
no final return falls through to the literal `Error` program-counter value;
it does not automatically return. Primary translator success can still produce
an invalid enclosing TLA module.

The source parser already owns declarations and call/return statements. The
first native path must preserve their source associations without inventing
scoped renaming, procedure/process storage ownership or recursive call semantics.

## Decision

Normalize one explicitly labelled sequential procedure with disjoint parameter,
local, global, label and generated names. Existing owned branch/loop/assignment
normalization is shared. Calls and returns become typed continuation nodes with
bounded procedure indices, not copied procedure bodies per invocation. Generation
matches stack records, simultaneous actual assignment, sequential local
initialization, return restore order, procedure wrappers and `Next` action order.
A call immediately followed by `goto` uses the measured folded continuation.

Keep default and TLC profiles identical on admitted behavior. Nested/recursive
calls, multiple procedures, procedure/process ownership, automatic procedure
labels, macro/procedure interactions, fairness modifiers and unproved scoped
name disambiguation remain named refusals. Source grammar rejection and complete
artifact admission are separate stages. All translations report nonproof output.

Action and normalized-node budgets apply across the main body and procedure;
large procedure bodies are not cloned at every call. File rewrite/checksum,
precise mapping-format and full historical option parity remain pending.

## Consequences

The bounded sequential call/return path can be checked with generated finite
models and independently replayed witnesses. Every primary invalid output and
native refusal remains in the complete source selection. F-034/F-035 stay partial
and audit-required; this slice cannot certify the procedure family as a whole.
The dependency graph is unchanged.
