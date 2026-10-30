# 0130: Close ordinary native action construction without widening queries

- **Status:** accepted
- **Date:** 2026-10-30

## Context

ADR 0129's bounded native atom path completes original ABA but cannot evaluate
EWD998Chan's constructed refinement projections. Two bounded local debug timing
diagnostics of unchanged original N=3 source/config identify the inner Next and
Nonstuttering action atoms as 86–89% of reference atom time. Descriptor counts do
not describe cost: Init and outer channel fairness together are a small part;
the hidden inner Enabled query is a separate semantic problem. These diagnostics
are source-selection evidence, not production throughput or capacity results.

Reference Nonstuttering evaluates its subscript in ordinary source and target
states, compares them, then evaluates the action. It does not create Enabled's
Context.branch. Applying the Enabled identity-substitution refusal to this
ordinary value operation is conservative, rather than required by B3.

## Decision

Extend only the optional ordinary Predicate/Nonstuttering lane using the existing
Role/Handoff compiler and isolated Machine. Keep shared label/error demand order,
original source/first physical target handles, eager admission, products/lassos,
reference attribution and independent fresh replay unchanged.

The callback-free guarded-action certificate admits a single scalar binder over
a proved interval for function construction and bounded CHOOSE; source-local
finite integer enums/difference supply closed domains. It inspects actual scopes,
unused/cached captures, names, contexts and constructor histories. The existing
canonical producer and closed-predicate certificates retain their old boundary.
Compiler acceptance alone never supplies this permission. Existing Red,
KEEP_LAZY-aware application/Beta and substitution/LET machinery retain when a
constructor is built or its body demanded. CHOOSE retains actual interval order,
first witness and error behavior; TypeOK is not a uniqueness proof.

Ordinary guarded updates contain one clause and one key. They evaluate a
certified closed base and actual scalar key, check membership before RHS demand,
and bind lexical @ to the old scalar leaf. The replacement must be scalar. A
missing/incompatible key declines before it can suppress reference warning2144;
optional errors discard scratch and run reference. Existing root/immutable-binder
updates additionally retain their exact actual-domain guard. Preserve function
keys, order and tuple/record/general flags; precharge traversal/copies before
allocation under the same4096 work quota and original owned controls.

Ordinary scoped Nonstuttering retains substitutions in before/after modes and
checks change before the action. True Enabled still uses its existing
identity/cutoff proof and narrower immutable-binder/no-@ update subset. No hidden
successor slots, B2/B4 cache approximation, FcnSet/RecordSet constructor permission,
multiple/nested update permission or general deferred provider is introduced.

No new backend, mode, cache, dependency edge or semantic divergence. Both profiles
use the same proven optional path, with reference fallback for unknowns.

## Consequences

The first target is the two expensive EWD ordinary action closures; expected
2/6 descriptor coverage must be measured on unchanged source and consumed calls.
Actual truth comparisons and focused adversarial tests precede a matched
full-dispatch timing screen. A weak measured result stops this
intervention before broader qualification; it does not authorize widening to
outer channel updates or hidden queries. Independent review and integrated
milestone qualification retain their existing release role.
