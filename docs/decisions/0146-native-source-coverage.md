# 0146: Native producer source-work observation

- **Status:** accepted
- **Date:** 2026-11-03

## Context

The IR already retains exact original UTF-8 ranges, JavaCC positions and
instantiation contexts. The graph observer measures states/edges, not expression
coverage. Pinned `ACoverageTest` demonstrates that TLC's coverage is an
action-context logical cost tree, including lazy quantifier and CHOOSE demand.
A flat evaluator counter cannot claim those costs. Compiled Core expressions
also lack the source map needed to expand their work into native source rows.

## Decision

Add explicit `--source-coverage` and a default-false public checking request flag.
The TLC-equivalent setting is off. In either semantic profile this is labelled
`native-source-work`, never TLC cost parity. Ordinary file checking statically
selects one Reference worker; Auto reports that selection before work and
explicit Core refuses. Genuine temporal obligations and explicit temporal,
checkpoint/recovery and core-sample combinations remain unsupported in this
first route. Models containing ENABLED refuse this observation option until
compiled query source maps are implemented; ordinary checking is unchanged.

An optional collector is bound to the exact immutable IR and attached before
producer startup. It counts actual Reference expression entries, Init/Next walker
visits and generator emissions, including failing demands. Counter observation
never evaluates, normalizes or prints a value. The file entry stops collection
when the checker returns, before fresh certification/replay or presentation;
diagnostic evaluator/generator paths do not contribute. Existing binding recipes
remain unobserved by default. Foreign collector/IR association refuses before
execution. Counters saturate explicitly rather than wrapping.

Reports aggregate by exact static IR source/instance-definition location, include
zero rows, distinguish synthetic originless work and omit embedded standard-module
rows. Emissions are native generator callbacks, not distinct states or committed discoveries. Returned
metrics cover the actual producer prefix even for an error or budget stop, and
never certify model success. Serialized reports remain schema 3 with additive
optional fields; trace formats do not change.

IR origins do not encode a dynamic caller tree. A nested static `I!Double`
definition reached through two outer instances therefore aggregates their work
in its one source row, labelled `context: static-ir-origin`. The source counters
do not invent outer names or claim logical action-context separation.

The disabled evaluator has no counter allocation, callback or atomic operation.
One optional branch sits inside the existing non-inlined evaluator/generator
dispatch to avoid duplicating observation code throughout hot wrappers. Focused
overhead diagnostics accompany the implementation; full performance acceptance
remains an integrated milestone/release task.

## Consequences

Users can inspect real source work without enabling Java or treating node IDs as
locations. Native counters intentionally change when evaluation/cache strategies
change. This does not implement TLC `-coverage`, compiled source coverage,
distributed metrics or debugging. The logical action-context cost tree, Core
source maps and DAP remain mandatory follow-up work; the coverage/debugger family
is not complete.

## Follow-up

ADR 0150 adds a separate final TLC logical-cost slice. The native option and
its source-work contract above remain unchanged; compiled maps and DAP remain
unfinished.
