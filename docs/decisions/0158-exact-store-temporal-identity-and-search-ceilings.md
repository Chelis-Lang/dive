# 0158: Exact-store temporal identity and search ceilings

- **Status:** accepted
- **Date:** 2026-11-18
- **Amends:** ADR 0082 §Store and identity

## Context

ADR 0082 refused temporal checking over the exact store "until the exact store
can provide a temporal proof". Core's behavior graph (`NativeSink`) named each
vertex by the 64-bit state key, so two distinct states with equal keys would be
one vertex. That is sound only under the fingerprint assumption the report
already qualifies. Core also refused `--max-states` and `--max-depth` with
temporal properties.

Exact storage plus state/depth ceilings is the policy of embedders such as
Sail. They need an unconditional exhaustiveness claim, and a structured stop
instead of a runaway run.

The human report also printed "Exact full-state storage." on every Core
temporal run without collision figures. Core never computes them, so a
fingerprint run made an exactness claim it had not earned.

## Decision

**Identity.** The in-memory exact seen set stores each retained identity with
its discovery id. When an already-seen state is reached, the explorer reports
that id in `graph::Step::seen`. In exact mode the native graph resolves seen
targets only through these ids and never through keys, so equal keys of
distinct states stay distinct vertices. Fingerprint mode is unchanged: it
resolves by key, and `seen` is always `None`.

Mixing the modes is an internal error. So is a seen step without an id in
exact mode, or an id that names no discovered vertex. A new exact insert
predicts its id from the predecessor log, and the log push must confirm that
id. Otherwise the run fails with an internal error rather than producing a
wrong graph.

**Storage.** `dive check --store exact` runs temporal properties on Core. Spilled
or reclaimed storage is still refused by name with temporal properties: frontier,
candidate or predecessor spill, disk seen sets, and an explicit Store
reclamation trigger. Each counterexample is still validated independently by
`dive-trace`. The lasso is built from graph rows and the safety prefix from the
predecessor log.

**Reports.** The human temporal report states the store: "Exact full-state
storage." for exact runs, and "Fingerprint storage (64-bit); collision
probability not computed." for fingerprint runs, because Core does not compute
the probability. The JSON report keeps schema 3. It carries the store in
`options.store` and the retained identities in `core.metrics.exact_states`.

**Search ceilings.** `--max-states` and `--max-depth` apply to temporal checks
on Core. A ceiling reached during graph capture is a resource stop: verdict
`stopped`, exit 5, phase `graph-capture`, `capture_completed: false`, no
counterexample, never `Holds`. A violation already proven at a completed level
before the ceiling is still reported (D-014). A ceiling equal to the model's
size gives the full verdict, because a ceiling refuses only a state beyond it.
Under `--compat=tlc` these ceilings are dive options with no TLC equivalent
(D-026). Leaving them unset is the TLC-equivalent setting.

## Consequences

Exact and fingerprint temporal runs agree on every verdict, count, property and
lasso of the small tier (the store is result-preserving, D-007). Exact runs
carry the stronger claim. The exact store costs memory per retained identity;
measuring it is part of the release performance plan.

Spilled temporal storage remains future work under ADR 0024 and ADR 0031.
