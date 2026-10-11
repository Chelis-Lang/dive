# 0081: Source-owned finite tuple ranges for indexed stack replay

- **Status:** accepted
- **Date:** 2026-10-11

## Context

The receiving ADR 0071 source independently replays the original 32 sequential
procedure words. A fresh census preserves those admissions; eight additional
process-indexed stack words remain Unsupported in TLC order and Valid in canonical
order. Clean primary TLC at workers 1, seed 0 and fp 0 reports invariant violations
with the same generated/distinct/queue/depth counts as the native checker on all
eight process cases. Supplied stack values cannot establish namespace provenance.

## Decision

Extend the private whole-word logical proof with finite functions whose range is
independently tuple-proved. A necessary Init equality must establish the range;
every initial disjunct/conditional branch must establish it. Next must preserve it
on every branch, through equality, fixed UNCHANGED projections or single-index
EXCEPT updates whose every replacement is tuple-proved. Existential actions over
closed finite source domains may enter bounded analysis scopes, without evaluating
bindings. Universal actions are not a preservation rule: an empty domain can make
them true without preserving any variable.

Only indexed reads of the sealed range gain tuple shape. Definitions, actual lazy
parameters, LET and substitution scopes retain the configured bound meaning.
Current-state facts never justify primed self-reference. Mutually dependent tuple
and tuple-range facts are monotonically pruned until jointly inductive. Selectors
and tuple elements still pass the existing all-role runtime-name walk; actual Init,
Next and ending predicates still run independently on the concrete supplied word.

No successor generator, JSON-derived history, callback/provider forcing or caller
proof flag is added. Preserve the existing 256-variable, shared 32,768 logical-visit,
depth 64 and 256-visit operand policies, plus the separate 8,192 all-role visit limit.
The additional range rules consume the existing budget, rather than raising it.
These are inspection policies, not exact CPU or aggregate memory bounds. Exhaustion
and unsupported shapes retain the named runtime-order refusal. Public arbitrary-
state enabledness/lasso APIs retain empty history hypotheses.

## Consequences

The bounded independent admission may validate additional stored process frames.
Opaque maps, string/mixed replacements, missing assignments, nested updates,
primed self-reference, unproved domains, recursive contexts and runtime effects
remain refused. Function domain contents and namespace/record spellings are not
registered from the witness. Malformed values or foreign context/transport names
still require the existing structural authority; a valid tuple hypothesis is not
an execution certificate. No checker behavior, schema, default, quota or dependency
edge changes. Issue #124 and F-029 remain partial.

Qualification belongs to separately frozen source/runtime receipts. Preserve the
before census, all original failed setup attempts and the historical ADR 0071
producer separately; do not retag them as evidence for the additional range rules.

## Qualification addendum (2026-10-12)

The distinct frozen final source passed all eight local gate stages, all
69 literal CI Python invocations (64 distinct) and 1,698 tests with zero skipped.
A fresh executable repeated the original census and complete process matrix,
semantic frame mutations, clean primary pairs, actual guide and CM1/VTS widths.
The original before/first producers and interrupted preparation attempts remain
separate; current metadata does not relabel their execution. See the probe
README and runnable guide for exact selection and bounded evidence. The
checker fallback remains unchanged; a closed-operand-only shared authority
is a separate future increment (#230).
