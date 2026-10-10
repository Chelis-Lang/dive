# 0040: Pinned final SCC order and independently validated witness selection

- **Status:** accepted
- **Date:** 2026-10-10

## Context

Acceptance existence does not reproduce TLC's selected lasso. `LiveWorker`
visits pushed successors in reverse order, checks a component immediately,
retains its anchor/pop order, greedily discharges state/action/promise obligations,
and closes the postfix by fingerprint buckets containing ordered tableau nodes.
Canonical component sorting or a shortest-lasso selector loses that contract.

## Decision

Implement bounded `tlc_scc` visitation and `tlc_witness` selection on validated
supplied graph/label dimensions. These functions give graph evidence only.
Retain root FIFO, EA-false-root admission, first-visit links, deciding component
pop order and original product identities. Prefix traversal uses product pairs;
postfix closure uses fingerprint buckets in component insertion order. Preserve
TLC's greedy first gain, last eligible/unvisited choice, seen resets and
backtracking order, including its absence of an EA test in that backtracking scan.
Every resulting model candidate must be checked independently.

`tlc_final::check` constructs its own ordered execution graph from a bound model;
it never accepts caller-edited completed graph evidence as authority. It checks
OOSs and PEMs in encounter order, stopping at the first accepting component.
Original configured claims are attributed directly on the recovered periodic
word, separately from derived temporal-role labels and component membership.

The separate `dive_api::tlc_temporal::check_final` facade binds independently for
validation, checks the original word and its serialized JSON, and constructs a
private validated violation only when both actual reports are `Valid`.
Unsupported/invalid validation cannot become a violation. Resolved scheduling
is explicitly `final-only`, selection `tlc`, profile TLC and store fingerprint.
A holds result retains the store's collision qualifications.

Exact-store temporal proof is refused before execution: fingerprints must not
silently merge different concrete states while claiming exact identity. Symmetry,
VIEW, ALIAS/recovery hooks, accepting-sink shortcuts, periodic/RL/postcondition/
possible hooks, impure/introspective/runtime-name/opaque captures and unsupported
planning shapes retain named refusals. No ordinary strict CLI behavior changes.
Fixed existing limits remain; named witness-position bounds use the existing
vertex ceiling. Entry/cell/inspection bounds are per stage, not aggregate byte
or metadata accounting. Missing failure/work/worker evidence stays absent.

## Consequences

All 30 reference controls and 120 native outcomes are retained: 19 completed
field projections and 11 reference failures with no error-class agreement credit.
The pin's original `finalCheck` runs separately from observing private-method
projections. The observing graph copy matches final adjacency/roots, not disk
file offsets/write history. Synthetic Floyd acceptance and path/label checking
cover 4,096 small graphs without the production stack protocol. Fresh validation
rejects changed Init, Next/closing edges, loop targets and incomplete claims.

F-019 through F-022 remain partial/audit-required, F-023 remains missing and #140
remains open. Initial replay/error-class coverage, accepting-sink witnesses,
periodic scheduling, full corpus and ordinary strict integration still gate parity.

## Callback-free facade admission

Inspect structurally admitted sources before creating RunHost/binding: every
nonnative user body, every selected role/config replacement target, and standard
module nonnative top-level zero-arity bodies that startup can evaluate. Genuine
Def.std is native authority; module/name labels are not. Do not independently root
unused standard LET locals whose captured operator parameters do not execute at
startup. Refuse impure/live/introspective/runtime-name/opaque operator paths before
predicate evaluation. Legacy endpoint arena/ownership controls in this source
remain staged; shipping integration must coalesce the shared fallible
Model::validate_context authority before final Session planning. Explicit inline
sequential requests and model fallback remain refused until callbacks are certified.

[The user guide](../tlc-final-api.md) describes the public contract.
