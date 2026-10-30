# 0124: Budgeted capacity for ordinary native temporal files

- **Status:** accepted
- **Date:** 2026-10-30

## Context

The native CSR producer is implemented (ADR 0122), but inherited library count
ceilings still prevent original capacity targets from completing: CoffeeCan3000Beans
has 4,504,500 states, EWD998Chan has 1,524,022, and aba_asyn_byz generates
85,612,896 states. Removing only the million-state limit leaves edge, transition,
acceptance-materialization and attribution/word ceilings in later owners.
A budget stop after capture also formerly became a generic core refusal.

## Decision

Ordinary file requests actually running on the core use the existing whole-run
clock and sampled RSS/swap budget across capture, analysis and independent replay.
Capture entry counts, product vertices/edges/transition inspections, acceptance
buffer bytes/materialization work and attribution/word cells use checked machine
capacity. Native vertex and retained-row ids still require checked 32-bit conversion;
ordinals and all layouts retain checked arithmetic. Parallel capture keeps the
eight-source wave schedule, checks its actual budget for each emission, and uses
fallible provisional payload/frontier growth. Large product/search/word buffers
use fallible allocation and cooperative polls. No partial search proves holding.

Explicit Rust caller `Limits` and acceptance policies, reference-library routes,
explicit final/short selection and formula/tableau/property planning limits retain
their existing values and enforcement. This is no new temporal-language support.
Genuine resource stops preserve their original typed cause, stage, producer
statistics and full/prefix truth. A retained level-check error is transported before
an expired capture-factory poll can replace its actual temporal phase.

## Consequences

There is no model-specific admission rule. Reported numeric bounds describe the
actual selected owner policy. Machine capacity is finite, not a memory allowance.
RSS sampling is cooperative and cannot prevent every transient allocation peak;
standard-map and allocator internals remain opaque. This changes no hard allocator
limit and claims no measured capacity or performance improvement. Original target
completion, observable agreement and integrated qualification remain required.
