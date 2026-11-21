# Roadmap

Dive is a deterministic TLA+ checker for maintaining existing specifications.
The next work targets three jobs: reaching a verified counterexample quickly,
finishing demanding safety and fairness checks on practical hardware, and using
the same model in implementation CI or a proof workflow.

Core is the product engine. The default follows canonical TLA+ semantics where
TLC's behavior is an implementation artifact, and TLC compatibility remains
nominal. Every new checking path must preserve the documented observables or
register a precise divergence, independently replay counterexamples, and
give named refusals for unsupported inputs. [ADR 0153](decisions/0153-core-is-the-product.md)

## Starting point

v0.1.0 has shipped. It met the [first-release gates](RELEASE.md), including
169 covered Examples configurations with zero disagreements, bundled
libraries, and stable CLI and JSON reports. The 169 agreeing checks ran
5.1× faster than pinned TLC by geometric mean at one worker including
startup. The larger 48-model subset ran 3.2× faster. The
[scoreboard](scoreboard.md) holds the measured coverage and refusals.
[Release results](developer/release-performance-results.md)

Performance depends on the workload. Core beat TLC on EmergencyCall f3
at one worker and lost at eight. Core completed PaxosLease Retry at 2.18
billion distinct states on 32 physical cores, about 9% faster than pinned
TLC with higher peak RSS. The opt-in finite symbolic strategy completed KVS
Large within a budget where both explicit checkers stopped. Its completion is
a finite reachability result. Current main also has scoped temporal workers,
compact edges and evidence reuse for eligible property edits.
[Large-model results](developer/large-models.md) ·
[Symbolic strategy](user/symbolic.md)

## Delivery order

1. **Establish a shared comparison panel.** Use the
   [pinned workload selection and measurement procedure](internal/survey/verification-workloads.md)
   to compare v0.1.0, current Core, pinned and released TLC, and applicable native
   peers. Publish per-model completion, named refusals, startup, exploration,
   scaling and memory together. Record comparable whole-run results and use
   bounded progress for stopped cases. Start with exact input controls before
   running the large configurations. Extend the current performance evidence
   and group open work by the measured bottlenecks.
   [Performance backlog](https://github.com/Chelis-Lang/dive/issues/799)
2. **Improve single-host scaling.** Rebaseline EmergencyCall f3 and the temporal
   panel. Profile successor evaluation, allocation and sharing, duplicate
   detection, ordered commit, graph capture and temporal analysis. Use
   BPConProof and TLCSailfish2 to distinguish wide frontiers from retained
   aggregate state and graph costs. The target is to match or beat same-host
   TLC at eight workers on f3 and to complete one external fairness case that
   currently stops in the fixed envelope. If current main already completes
   that case, show a reproducible speedup instead. Check determinism and
   replay witnesses at every tested width.
   [Temporal scaling](https://github.com/Chelis-Lang/dive/issues/1028) ·
   [Large holdouts](https://github.com/Chelis-Lang/dive/issues/1034)
3. **Qualify a compiled finite safety tier.** Reuse the bound finite relation
   to generate complete actions over packed states. KVS and transaction
   commit give starting controls for an order-of-magnitude gain on eligible
   models. Use aggregate consensus as the wider-layout holdout. Measure
   source admission, fallback work, cold compile-and-check, warm search,
   and exact membership capacity at one and eight workers. Preserve
   generated multiplicity, diagnostics, resource stops and independently
   replayable shortest witnesses. Keep work on Core's evaluator and
   temporal checking for cases outside the finite fragment. Expand spill or
   distributed execution when a pinned practical workload still exceeds the
   tuned single-host envelope.
   [Compiled safety measurements](internal/survey/verification-workloads.md#compiling-the-finite-relation)
4. **Complete practitioner workflows.** Rank named blockers by workflow impact.
   Start with the aggregate message bags in etcd Raft, the Kafka KIP-996
   fairness planner and the VIEW/liveness refusal. Qualify PlusCal, fairness
   and refinement using original external sources and the Foundation's
   differential hardening cases where they expose an applicable gap.
   Extend trace replay through the CLI or Rust API on one upstream CI fixture,
   with both an accepted trace and a corrupted one. Give test generators the
   minimum labelled graph export they can consume, with action parameters,
   reductions and completeness. Keep instrumentation, test generation and
   alternate event-order search with their callers until a concrete contract
   shows what dive needs to support.
   [Remaining Core coverage](https://github.com/Chelis-Lang/dive/issues/910) ·
   [VIEW and liveness](https://github.com/Chelis-Lang/dive/issues/428) ·
   [PlusCal scope](https://github.com/Chelis-Lang/dive/issues/798) ·
   [Trace validator work](https://github.com/Chelis-Lang/dive/issues/490)
5. **Qualify the finite symbolic strategy on varied models.** Use KVS Small
   and Medium for count controls, Large for completion, and transaction commit
   or another differently structured model to test generality. Test a changed
   transition relation as well as property edits. Keep bounded search, exact
   finite closure and finite induction distinct in reports. Cross-check a
   constraint-heavy model with Apalache where the question and semantics match.
   Keep exact population counts on anonymous finite state tables. Require
   source-derived permutation invariance, concrete labeled counts and witness
   multiplicity, exact BFS depth, and independent replay of a concrete trace.
   Reject an index-sensitive mutation before applying the reduction.
   [Population checker and qualification data](developer/large-models.md#checking-anonymous-processes-by-population)
   Use production workloads to decide which symbolic refusals to address.
   [Current symbolic contract](user/symbolic.md)

The proof handoff can start alongside the comparison panel. Use proof-bearing
source as-is, screen candidate invariants in batches where the finite search
supports it, and show a pinned external TLAPS proof on the same source.
Start with TCommit and then the Raft induction artifact. Use that workflow
to evaluate upstream improvements, a companion, and a full proof-system
reimplementation. The full option has a separate scope and trust decision.
[TLAPS options and acceptance tests](internal/survey/verification-workloads.md#tlaps-options-and-evidence) ·
[Open decision](open-questions.md#deductive-proof-system-scope)

## Adoption evidence

Publish a rerunnable peer comparison, an external CI adoption that the model
maintainer can reproduce, and a finite-check-to-proof walkthrough. Demonstrate
agent use through Sail or another external harness with a safety failure, a
fairness failure and a transition edit that invalidates reuse. Hold the task,
obligations and check budget constant across checkers, and verify the result
in a clean environment. Report time to a replayed witness, total session time
including setup, and whether the task succeeded. Invite upstream maintainers
to review the inputs and share small cross-tool regression cases with the
community. [Workloads and sources](internal/survey/verification-workloads.md)

The post-v0.1 milestone tracks this work. Other existing work on simulation,
DFID, storage, debugger and editor support, host I/O, macOS packaging and
Verus export can follow a concrete user need. The
[v0.1 gates](RELEASE.md) record what shipped.

## Completed foundation milestones

| Milestone | Delivered |
|---|---|
| M0 Foundation | Workspace, lints, layering check, lean CI, commit hooks, charter and AGENTS.md |
| M1 Oracles and harness | Pinned TLC/SANY snapshots and JRE; pinned Examples, CommunityModules and TLC test corpora; `dive-conformance` with goldens, reports and worker invariance; the survey in `docs/internal/survey/` |
| M2 Competitive analysis | `docs/internal/survey/competitive.md` and the positioning ADR |
| M3 Syntax and values | Lossless parser with spans and recovery; `dive-value` with canonical order and FP64 fingerprints |
| M4 Semantics and configuration | SANY-equivalent `dive-sema`; the TLC `.cfg` format |
| M5 First verdicts | IR, evaluator, standard library, deterministic parallel safety checking, the independent `dive-trace` validator |
| M6 Liveness and symmetry | Final/short liveness with fairness, symmetry reduction, VIEW, the TLC profile |

Earlier phase narratives (Phases 4–8) are in git history. Their checkpoint
records remain under `conformance/reports/core/`.
