# 0145: Owned simulation runtime and finite random source relations

- **Status:** accepted
- **Date:** 2026-11-03

## Context

ADR 0116 defines simulation by valid observable behavior and reproducible native
seeds, excluding TLC's internal draw history. The previous runtime allowed one
actual Init RandomElement call and could not continue a source binder's host.
Its independent proof only admitted a direct scalar initial equality.

A bounded search of source-defined choices can prove more words without copying
a producer stream. Expression-cache domains alone cannot isolate failed
attempts: shared values retain normalization history and lazy caches.

## Decision

Create a single-use simulation Runtime before binding and retain its host through
startup, assumptions, Init, Next and reporting. Existing low-level foreign-host
refusals and shared-evaluator ownership remain; ordinary file simulation installs
the owned host automatically. The actual configuration is supplied after binding.

Native finite words use a bounded source choice search. Each alternative starts
with a fresh source/config binding, interner, host, values and caches; only integer
choice indices survive. An attempt executes all predicates together, retaining
its real startup and lexical correlations. Init and supported Next assignments are independently constructed from
empty slots. Next uses freshly reconstructed source-bound actions, preserving
constant arguments frozen by getActions and uncached action LET scopes while
ordinary RHS expressions retain their actual caches. Unsupported action shapes
decline structurally before evaluation. Ordinary predicate fallback cannot grant
random Init/Next support.

Action reconstruction retains the shared pre-startup transformed-randomness
admission policy, including unused references. Source evaluation errors are
terminal, distinct from a structural decline or comparison with supplied witness
data. A pure wrong-kind witness keeps its Invalid classification through a fresh
predicate attempt; direct RandomElement domain errors retain their native codes.

Scalar witness values may prioritize actual source-domain members without
normalization, callbacks or compound-value inspection. This is search order,
never membership authority. Function/record/product random selection follows
native component demand, including empty function domains. The previous direct
scalar Init membership proof is retained.

Bounds are 65,536 attempts, 256 source selections/constructor depth and 65,536
materialized domain entries; i64 intervals use checked indices. Existing run
control applies to fresh binding and evaluation. Exceeded proof bounds yield no
success. Deferred providers, register-dependent random prefixes, randomness
inside ENABLED and unproved Init/Next assignment forms remain named refusals. A new defaulted
Host hook preserves those reached ENABLED restrictions without changing ordinary
checker hosts. No producer certificate, RNG position or new wire version exists.

## Consequences

Reference simulation gains repeated/Next and compound/captured finite choices,
with independently checked examples and negative controls. A seed still
reproduces the producer, while replay proves support rather than a chosen draw.
Core can continue refusing unavailable effects, and Auto reports Reference
selection before execution. Search can be expensive for complex words; refusal
or budget exhaustion remains distinct from an invalid word or proved violation.
General randomization operators, multi-worker simulation, broader effects and
full mode parity remain unfinished.
