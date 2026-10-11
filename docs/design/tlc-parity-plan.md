# TLC parity delivery plan

Approved direction, 2026-10-10. This plan extends M1/M5/M6 through the complete
pinned TLC feature set. It is a delivery contract, not a completion report.
The [feature ledger](../tlc-parity.md) records current implementation and evidence.

## Definition of done

The target is the pinned TLC/SANY/PlusCal bundle in `conformance/oracles.toml`,
with approved better defaults and a tested `--compat=tlc` counterpart. Full
Rust-native distribution, checkpoint/recovery and PlusCal are included. Java
RMI worker interoperability, Java checkpoint import and arbitrary JVM plugin
loading are excluded; custom operators require a native override implementation.
LSP, native compilation and Verus remain after parity.

The strict baseline is 83 agreements and 71 explicit refusals across 154 entries
in the committed standard report. The separate 53-model final/short study has
26 truth/count matches, 22 refusals and five timeouts. Neither a mixed-profile
temporal match nor a feature refusal increases strict agreement totals.

Parity requires every ledger family to be certified, every in-scope source
surface mapped to acceptance cases, no feature refusals in the release corpus,
and no unexplained disagreement, pending validation or timeout. Resource limits
must remain explicit failures. A limit is not a way to excuse a supported TLC
feature indefinitely. All selected rows, including failures, remain in reports.

Counterexamples must be independently checked. Under TLC's known unsound
symmetry/liveness mode, reproducing an invalid quotient witness must never
be reported as independently validated. Retain its validation failure and
document the upstream limitation explicitly.

## Nine delivery phases

| Phase and packages | Implementation | Acceptance |
|---|---|---|
| **1 — Denominator and evidence** (`P1-ledger`, `P1-corpus`, `P1-regressions`) | Maintain the semantic ledger and pinned source inventory; import TLC model/JUnit inputs, expand ValidationTestSuite and upstream wrong-answer regressions. Map every source surface to positive, negative and interaction controls. | Every selected corpus case has one disposition. Every feature has a stable ID, owner package, evidence and exit criteria. Publish model outcomes separately from feature certification. |
| **2 — Symmetry** (`P2-bind`, `P2-keys`, `P2-trace`, `P2-audit`) | Finish name lookup, source-aware warning images, disable heuristics, model-value validation and closure attachment. Permute all value classes recursively; reproduce TLC representatives and symmetry-before-VIEW. Preserve concrete predecessor paths. Build optional audit and both symmetry/liveness policies. | The 11 selected symmetry models and probes match at 1/4/8 workers in both stores and under chaos. Default rejects malformed declarations; compat reproduces TLC. Unreduced default liveness disables reduction for the whole run. |
| **3 — Temporal language** (`P3-domains`, `P3-scopes`, `P3-plan`) | Fix bounded finite lazy domains first (#107). Finish recursive/higher-order/temporal argument forwarding, substitution and INSTANCE scope. Extend independent enabledness covers alongside evaluator support. Port TLC simplification, DNF, EA/AE grouping and tableau numbering. | All 53 temporal study models reach supported language planning. Java probes establish branch/error/order equivalence, and independent word/mutant controls verify the wider language. Resource bounds remain unchanged. |
| **4 — Strict integrated liveness** (`P4-capture`, `P4-exact`, `P4-schedule`, `P4-view`, `P4-validator`) | Retain graphs through deterministic parallel expansion; preserve duplicate/stutter edges and phase-L atom evaluation. Finish error/stopping order and counts, SCC/hash/tableau-dependent TLC witness selection and property attribution. Implement final/default/off/sequential schedules, effects and sound VIEW closing evidence. | Ordinary `dive check --compat=tlc` checks temporal models without final/short overrides. Strict probes and selected models match the applicable exact fields. Worker widths 1/4/8 use actual parallel capture where semantics allow, with independently validated concrete traces. |
| **5 — Modes** (`P5-sim`, `P5-generate`, `P5-dfid`) | Implement simulation/generation, initial/action selection, depth/count limits, random streams/arils, statistics and all pinned schedulers, including RL variants. Add DFID visitation/iteration semantics. | Mode-specific oracle controls cover constraints, invariants, action/temporal properties, termination, seeds and continuation. Default simulation replay is stable across workers. Never describe simulation as exhaustive checking. |
| **6 — Remaining operators/services** (`P6-stdlib`, `P6-errors`, `P6-runtime`, `P6-output`, `P6-host`, `P6-debug`) | Audit every standard/CommunityModules binding and TLA+ fallback. Preserve construction provenance and lazy order for #61. Finish ALIAS, POSTCONDITION, continuation, periodic/POSSIBLE/RL hooks, trace loading/exploration and output controls. Add coverage, native debugger and custom override registration. Implement granted environment/files before granted process execution. | All inventoried surfaces have tested mappings, including aliases/nested arguments and failure precedence. No unsupported executable operator in the acceptance corpus. Host access stays denied without explicit category grants even under compat; granted runs record external input evidence. |
| **7 — Memory, recovery and distribution** (`P7-spill`, `P7-checkpoint`, `P7-distributed`) | Bound frontiers/candidates and spill values, stores, graphs and SCC arrays. Version native checkpoints with model/profile identity, stream and graph state. Use ordered native work units with committed effects and retry-safe resolution. | Tiny-budget spill equals memory runs; interrupted/resumed runs equal uninterrupted runs. Multiple processes match local results across worker loss, retry and reassignment without duplicate counts/effects. Corrupt/incompatible checkpoints reject clearly. |
| **8 — PlusCal** (`P8-parse`, `P8-translate`, `P8-maps`) | Implement both syntaxes, processes/procedures, labels, atomic regions, nondeterminism and fairness. Cover all pinned translator options, generated sections, version checks, checksum/update behavior and source mappings. | Upstream positive/negative translation cases pass; translations are compared against the pin and both generated models check equivalently, with validated traces. |
| **9 — Certification/release** (`P9-api`, `P9-matrix`, `P9-release`) | Finish public APIs, complete the interaction/corpus matrix, run deep/seeded/metamorphic/fuzz campaigns, perform separate checker/validator/harness reviews and publish reproducible reports. | All families certified; no feature refusals, unexplained disagreements, unresolved timeouts or pending validations. Required CI green and independent review findings resolved before release. |

## Execute in parallel without blocking the checker

Three implementation tracks operate beside the lead's integration/review work:

- **Temporal:** phase 3, phase 4, then remaining temporal operator/error cases.
- **Symmetry and storage:** phase 2, then phase 7. Declaration/keys can proceed
  while temporal scope support is implemented.
- **Coverage and modes:** phase 1 corpus work and phase 6 operator controls,
  then simulation/DFID and PlusCal. These must not wait for exact lasso selection.

Independent validation follows each new temporal/symmetry capability in the
same increment. It never obtains successors through the checker enumerator.
Define common report and graph contracts before shared edits; retain graph
state/edge order, duplicate edges, original property identities and concrete
closing evidence across those contracts.

The first merge cycle is #109 truthful execution-stage metadata, #107 bounded
lazy domains, #104 complete symmetry binding/first canonicalized safety models,
and #108 isolated profiles of the five temporal timeouts. Large scope does not
justify large unreviewable PRs: ship these as independently verified increments.

Existing issue mapping:

| Existing issue | Delivery package |
|---|---|
| #2 — model corpora/ValidationTestSuite | `P1-corpus`, `P9-matrix` |
| #16 — upstream semantic regressions | `P1-regressions` |
| #28 — value hot paths | `P6-stdlib`, performance work accompanying `P7-spill` |
| #53 — slow safety models | safety performance and `P7-spill` |
| #61 — construction-dependent errors | `P6-errors` |
| #63 — scoped temporal/fairness coverage | `P3-scopes`, `P4-validator` |
| #66 — exact temporal integration | `P3-plan`, phase 4 |
| #104 — full symmetry | phase 2, `P4-view` |
| #107 — finite lazy domains | `P3-domains` |
| #108 — five temporal timeouts | `P4-capture`, `P7-spill` |
| #109 — failure-stage/work metadata | first merge cycle, then all report producers |

These package names are work identifiers, not newly created GitHub issues.
Create one tracking issue per concrete deliverable before its implementation;
never claim an issue exists from this document alone. Close existing issues
manually only after the agreed evidence is committed and reviewed.

## Public interfaces

CLI and library backends share resolved options: profile, mode, workers,
liveness schedule, witness policy, symmetry, storage/resource limits and host
grants. Add library entry points for simulation, translation, checkpoint/resume
and distribution. Reports distinguish requested/actual workers, execution stage,
available partial versus complete statistics, active overrides and independent
validation. Change report schemas intentionally and version migrations; preserve
existing harness consumers while adding new fields.

Never trust a backend's `validated` claim. The harness invokes the standalone
validator and preserves invalid/undecidable outcomes. Compatibility reports
must distinguish a reproduced TLC limitation from a sound validated result.

## Verification, performance and release hygiene

For fixed-seed single-worker runs compare verdict, counts, depth, stopping
position, error class/code/phase and counterexample choice where specified.
Test wall-clock/multi-worker observations separately with explicit field
contracts; do not excuse arbitrary differences with a blanket exemption.

Each new feature gets positive, rejection and interaction probes, both profiles
and stores, 1/4/8 workers, scheduling chaos, forced spill and recovery where
applicable. Retain every source selection and raw report. Goldens remain
generated only, with pinned provenance and an overwrite reason. Add graduated
features to the PR gate without removing existing cases.

Keep the existing performance targets: startup at most 10 ms, at least 2× TLC
geometric-mean throughput, at least 3.5× scaling from one to eight workers, and
the memory/liveness overhead budgets in the M5/M6 plans. Use isolated,
interleaved runs on the same machine and specifications. Optimization must keep
comparison fields identical; correctness evidence is not a speed measurement.
Do not increase bounds or remove checks to make a timeout disappear.

Every PR passes the full local gate, required GitHub checks and review before
squash merge. Fix failing main first. Separate worktrees/caches and bounded
compile concurrency preserve isolation; benchmarks do not block independent
implementation. The lead owns GitHub coordination and merges, without polling
loops. Update the ledger, changelog, provenance, divergences and maintained
TLC-differences log alongside the code that changes their claims.

The oracle bundle remains pinned. Refresh requires a separately reviewed
decision and fully explained old/new differences, never an automatic cadence.

### Final-only corpus census and next supported boundaries

The first frozen `dive-final-tlc` full small selection retains all 140 explicit
refusals and 138 raw subprocess observations in
`conformance/studies/tlc-final-cli-first/README.md`. It provides no general
corpus or feature-family certification. The immediate follow-ups are real
ordinary safety execution for empty temporal obligations (with original
counters, bounds, worker and witness evidence), minimal probes for opaque
operator/scope admission, conditional planning and concrete witness recovery.
Do not raise resource defaults, weaken callback-free admission or invent
product evidence to promote a refusal. Periodic schedule and strict integrated
error/stopping coverage remain separate obligations.

The #189 [source-qualified backend](../periodic-backend.md) supplies actual node-pointer bucket/snapshot and joined source-wave facts for explicit partial requests. The next ordinary integration must compose the separate callback controller and elapsed-ratio kernel with real triggering/queue-finish/suspension/PERIODIC/checkpoint/stop authority, preserving each original OOS/PEM update/error boundary. Never use identities, file bytes, partial success or a caller boolean as a replacement for those contracts; the ordinary CLI remains refused.


P4/#189 lifecycle increment connects actual dispatch-mutex authority and internally sampled controller/ratio polling to source-owned graph growth, with mandatory final SCCs at actual EOF. Commit-boundary timing is explicitly narrower than ordinary TLC scheduling; no Holds/ordinary verdict or family credit. PERIODIC/checkpoint and exact stop/clock/replay contracts remain pending.

The ADR0054 supplied pure PERIODIC seam follows ADR0052 real native lifecycle. It preserves configured binding refusal and original bounds. Next ordinary schedule work must establish actual snapshot/name lookup, queue progress/stop timing, checkpoint effects and fresh independent deciding witness replay; do not promote supplied-predicate or callback controls to strict verdicts.
