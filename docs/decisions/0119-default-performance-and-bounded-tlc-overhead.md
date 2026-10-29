# 0119: Default performance and bounded TLC compatibility overhead

- **Status:** accepted
- **Date:** 2026-10-29

## Context

ADR 0006 applies the same throughput targets to both semantic profiles. The
execution-core plan inherited its requirement to beat TLC on every benchmark.
The owner clarified that a faster default profile is valuable even when exact
TLC compatibility is slower, provided compatibility is not pathologically slow.
The owner selected a hard runtime ceiling of twice TLC, with investigation above
one and a half times TLC, and prioritized trusted core capability and temporal
capacity over engine cutover work.

The completed five-repeat campaign at `75f0ac4f` has 300 correct observations:
default beats TLC on all ten models at 1 and 8 workers, while compatibility
beats it on eight. That source predates the C2 normalization repair. The later
corrected-runtime Sailfish diagnostic has one repetition, not population
acceptance. Their source qualifications and original quiet-window fields remain
unchanged in `docs/developer/core/m1-status.md`.

## Decision

1. Preserve the default-profile interpreted-throughput targets: at least twice
   TLC's geometric-mean throughput at 1 and 8 workers, and no slower model in
   the established benchmark panel at either worker count.
2. Replace compatibility's requirement to beat TLC everywhere, and its
   twice-TLC throughput target, with a per-model runtime ceiling:
   `core_runtime / TLC_runtime <= 2` at both worker counts. Investigate a
   repeatable ratio above 1.5; a ratio above 2 blocks performance acceptance.
   A geometric mean cannot hide a model over the ceiling.
3. Calculate per-model factors as medians of within-repetition ratios, with
   at least five matched repetitions, actual worker evidence and correctness
   checked on every row. Default aggregate throughput gives each declared
   model equal weight. Missing, stopped, refused or wrong rows do not pass.
4. Keep all observable-parity, independent-word, soundness, memory, capacity,
   startup and scaling obligations. A performance allowance permits no
   semantic approximation, weakened validator or undisclosed fallback.
5. Prioritize a trusted opt-in core and bounded native temporal capture.
   Engine-default cutover, release ceremonies and consumer migration are not
   separate deliverables of this phase. Preserve useful integration tests;
   change engine selection only when capability and performance justify it.

This supersedes only the compatibility-throughput requirement in ADR 0006
and the corresponding active core milestone/performance gates. Other targets
remain goals with their existing scope. Historical results are not regraded or
relabeled as measurements of a later executable.

## Consequences

Compatibility regressions within the bound can coexist with an accepted
default-performance result. Correctness, admitted breadth and temporal
capacity take priority over turning an acceptable compatibility deficit into
a speed win. The general S5 membership index, finer memo keys and scaling
improvements remain measured opportunities, not prerequisites for every
correctness or capacity change.

The next phase is recorded in `docs/developer/core/10-plan.md`. It retains
explicit refusals for unimplemented semantics and does not declare M1/M2,
full TLC replacement or a default-engine switch complete.
