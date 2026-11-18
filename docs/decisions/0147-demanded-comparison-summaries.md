# 0147: Demanded comparison-summary candidate adoption

- **Status:** rejected (candidate adoption)
- **Date:** 2026-11-03

## Context

Sailfish's transient compound membership misses cannot use the persistent-value
witness cache. A bounded demand diagnostic found that about 71% revisited a
complete set-only shape already computed in the same Arena revision. That is
eligibility evidence, not a measured payoff.

Candidate `88a5845a05c676adcca2a1e5fc0274388ca3f906`, tree
`287d4d2e4ce4cd3743d28cf1aecef2c129395ec8`, encoded only a successful guard's
complete summary. Hits joined a fresh query shape and preserved original scalar,
model and search checks. Unknown shapes, errors and refusals took the original
path. Private bounded storage shared the existing membership allowance, with
exact handles, weak owners and full reset/rollback revisions.

Independent correctness review passed after fixing a prototype-only omission in
`Memo::clear()`. That review permitted a bounded performance screen; it did not
grant adoption.

## Decision

Do not adopt or merge this candidate. Against baseline
`a86a15311545e16a2e81d74e21efb76c6b9dc85d`, the first screen completed all
30 Sailfish rows correctly across both profiles and actual widths one/eight.
One-worker rows retained 109,604 distinct states, 314,144 generated and
depth 16.

TLC/W1 candidate/base runtime ratios were 0.9809497073, 0.9819680741 and
0.9818455696: median **0.9818455696**, a 1.82% reduction against the required
10%. The default/W8 guard also failed: median **1.0233723248**, with two of
three pairs above 1.02. No repeatable RSS failure occurred in the observed cells.
The controller stopped with physical exit 10. The remaining **186 intended rows
were unrun**, including all ablation and other control models.

Independent review verifies actual rows, worker widths, pinned counts, source
trees, executables and every archived member. Full-window observations support
housekeeping-only sampling and stable controls. Aggregate SSM last-CPU metadata,
sampling uncertainty and idle frequency ratios retain explicit limitations:
there is no blanket quiet certificate or causal attribution. See the
[performance record](../../conformance/reports/perf/2026-11-03-phase7-demanded-summaries.json).

## Consequences

No cache code, option or default change ships from this experiment. No rescue
variant follows. This rejects one candidate's adoption, not the broader S5/S6
expensive-body, exact-read or immutable-scope strategies. Eligibility alone must
not substitute for measured benefit.

The prototype's ablation was not executed. These observations establish neither
the mechanism's isolated cost nor current-main performance, and satisfy no
ADR 0119 release floor, ceiling, memory, startup or scaling obligation. Feature
implementation and the integrated correctness milestone continue independently.
