# 0097: CLI checker statuses independent of evaluator semantics

- **Status:** accepted
- **Date:** 2026-10-14

## Context

Physical process status is a parity observable (ADR 0086). The corrected
154-model measurements in ADR 0096 retain default 76 Agree /18
ExpectedDivergence /60 Unsupported. Fourteen differences are solely native
invariant 10 versus TLC 12, or frontend 1 versus TLC 150. Keeping all 18
while resolving all 60 refusals caps default agreement at136/154, below 95%.
ExpectedDivergence is never Agree; original reports and failed floors remain.

ADR 0096 deliberately preserved the existing native CLI/API contract while
repairing measurement. That historical decision did not approve this change.
On 2026-10-14 the lead, exercising the owner's delegated judgment toward
eventual parity, approved this concrete CLI default. The expert's earlier
roadmap alone is not a new-default authorization.

## Decision

Ordinary `dive check` uses TLC physical exit conventions in both semantic
profiles. `--exit-codes native|tlc` selects presentation independently of
integers, CHOOSE/exploration, random behavior, workers and other evaluator
settings. The default is `tlc`; its TLC-equivalent setting is `tlc`.
A native exit override with TLC semantics is labeled `tlc+overrides`.

The typed `ExitConvention` and additive `CheckReport::exit_code_for`,
`to_json_with_exit_convention` and `render_human_with_exit_convention` APIs
select that convention. Historical `CheckReport::exit_code`, `to_json`,
`render_human` and native `exit::*` constants remain unchanged for Rust callers.
CLI JSON records `exit_code`, `options.exit_codes` and `semantic_profile`.

Statuses follow the existing deciding class, never a reported wrapper or
corpus path. Invariants 2107/2110 use 12; initial/action properties 2108/2112
and supported cyclic liveness use 13; spec rejection uses150 and config
rejection 151. Success 0, usage 2, refusal/resource 3 and internal 4 retain their
contracts. A status does not certify an unverified class or missing witness.
The finite temporal 2116/exit 12 limitation is not implemented by this change.

## Consequences

CLI clients relying on 10/1 select `--exit-codes native`. Rust callers keep
historical behavior and can opt into the new typed method without changing
semantic profiles. Only checker exits change; translate, parse, analyze,
validation, simulation and DFID retain their respective contracts.

Direct copied-CLI measurements now show equal 12 statuses for both D-010
models, with both counterexamples independently validated. Missionaries retains
61 distinct /226 generated; ACP retains 12645 /64357 /970 queued. Their registry
rows remove only obsolete 12/10 pairs and retain the exact count exceptions.
No comparison, pin, golden, model ID or witness requirement is weakened. Historical D-023 evidence remains; matching runtime exits may earn
Agree only after every other obligation holds in new complete measurements.

The conditional default projection is 90 Agree /4 ExpectedDivergence /60
Unsupported; it is not a measured result or a replacement floor. At least 57
of those refusals must additionally agree for 147/154. Fresh independent review,
full gate, all four complete reports and CI remain required before publication.
This is not completion of the drop-in TLC CLI, full-tier 95% or the roadmap.
