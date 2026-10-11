# 0064: Independent tuple-only sequence admission for whole-word replay

- **Status:** accepted
- **Date:** 2026-10-11

## Context

The issue #124 repair prevents a supplied word from installing runtime string
order that makes the word validate itself. The conservative bound-model marker
also includes tuple-only `Tail`, `Concat` and `SubSeq` calls. Existing independent
action and temporal predicate APIs already prove these operand shapes without
evaluation; whole-word replay still refused them wholesale.

Six fresh pinned primary violations and separate original/corrected native
characterizations retain the distinction. The original concat selection
mislabelled `x=1` as actual although its Next produces `x=2`; that epoch is kept
uncredited and a corrected epoch labels the inputs properly. No old raw output
or oracle golden is edited.

## Decision

Keep the conservative checker marker and sequential policy. Discharge only the
independent validator's guard through a bounded scoped audit of all checking
roles and assumptions. Reuse the existing tuple-only native proof within
`dive-trace`, preserving its standalone build without `enumerate`. There is no
new dependency edge or execution-profile option: admitted calls evaluate with
the existing TLC counterpart.

The audit resolves actual redirects and argument scopes and inspects nested
operands too. It evaluates no domain/predicate, interns no name and normalizes
no value. Stored values are inspected without provider calls. Opaque bindings,
operator actuals, recursion, unknown shapes or budget exhaustion retain the
existing explicit refusal. The inspection budget is 8,192 steps/depth 64, with
the existing 256-step tuple-shape proof per native call.

Use a closed nonconstructing-native list; neither a pure-operator annotation nor
the existing runtime-name marker is an exhaustive no-name-construction proof.
In particular JSON conversion still refuses in a model requiring this admission.
Unknown natives, host and introspection calls receive no guessed purity credit.

Apply admission before native validation and before lossy TLC interpretation.
Retain original namespace-forgery negatives, actual/altered words and scope/
alias/assumption controls in both relevant transport paths. User functionality
and remaining bounds are documented in `docs/tuple-only-trace-replay.md`.

## Consequences

Strict replay can admit a proved tuple operation without guessing missing
runtime-name history. This does not admit state-variable procedure stacks;
those need a separately proved inductive shape contract. It does not broaden
simulation, recovery, ordinary liveness scheduling, provider execution or
family certification. Original unpaired development logs stay unpaired;
publication requires newly frozen source and fresh receiving validation.
