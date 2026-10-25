# 0114: Proven runtime-name-free concatenation

- **Status:** accepted
- **Date:** 2026-10-25

## Context

TLC value/exploration order makes runtime string encounter order observable.
The native concatenation operator can create strings, so the model's source
inventory conservatively requires sequential checking. A successful call with
a tuple operand cannot create a string; it may still fail with the ordinary
operand error. A type invariant alone proves neither the actual operand kind
nor the absence of another reachable name creator.

Replay and checkpoint admission also consume the conservative source inventory.
Removing that inventory flag when choosing workers would change their existing
contract. Worker admission therefore needs a separate, narrower proof.

## Decision

Build a constructor-grounded result analysis and an independent own-call walk
at model binding. Literal tuples and unchanged native Append seed successful
tuple results. Transparent definitions, branches and genuinely entered lexical
LET bindings propagate facts. A recursive cycle alone proves nothing. Every
argument and reachable child remains subject to its own call inspection.
No type invariant, particular model name or EXCEPT/update shape grants a fact.

The complete checking roots include startup assumptions, roles, actions, claims,
alias/postconditions and effective configuration bindings. Captured frames,
substitutions, original thunks, retained cache values and stored values are
inspected without forcing them or invoking a provider. Opaque origins decline.
The opaque result belongs to the actual evaluator Core, not merely shared IR;
a fact about supplied roots is not model-wide authority for omitted roots.

A closed operator-argument source path can forward only authenticated unchanged
binary scalar comparisons: equality, membership, subset and integer ordering.
Unknown callbacks, configured operator origins and opaque captured operators
remain conservative. Their argument expressions are still inspected, and a
comparison callback grants no tuple-result fact.

A nonnative zero-arity definition use may be a call-free leaf when normal
zero-arity evaluation returns its already-inspected configured value. This
requires an actual stored value and no redirect. It applies neither to an
operator application nor to the replaced body's result-kind equations.

After proving every real checking root, remove only concatenation's worker
restriction. Keep the original runtime-name and checkpoint feature inventory.
Other sequential causes and unknown source paths remain unchanged. This is a
result-preserving scheduling improvement in both profiles, with no new option
or divergence; evaluation, errors, counts at one worker and limits retain their
existing contracts.

All new walks use checked allocation and cooperative controls, with no count
or nesting ceiling. Controlled binding retains intrinsic resource/internal
errors, opaque interruption identity and startup output; legacy binding keeps
its existing error projection. The proof does not execute startup output twice
or move it away before a possible failure.

## Consequences

The binder performs additional analysis once, without per-state proof work.
Unknown cases may still serialize even when a stronger proof could admit them.
Replay and checkpoint support do not broaden merely because workers are admitted.

The complete gate and unchanged trace controls qualify the source change.
The [worker admission record](../survey/data/proven-tuple-worker-admission-2026-10-25.json)
separately records original workload observations. Selecting eight workers is
not evidence of utilization or throughput; those require a measured campaign.
