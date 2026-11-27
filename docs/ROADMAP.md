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

v0.1.0 has shipped. It met its release gates, including
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

dive 0.2.0, released with Sail 0.4.0, is in qualification. It ships the
default-path Core speed and memory work, strong fairness on Core, the
reworked simulator and the incremental recheck APIs. Its gates and path are in
[RELEASE.md](RELEASE.md), tracked in #1418.

## Next: symbolic proofs (v0.3)

The symbolic strategies are in the source tree but are not part of the 0.2.0
installed product. v0.3 ships them working on every install route: the Linux
and macOS archives, `cargo install dive-cli` and Sail, with no extra install.
The work is a Rust port of the worker on a chosen BDD back end, the existing
worker tests as the port's regression oracle, versioned certificate and
receipt formats, and macOS arm64 archives. Sail's matching release runs its
symbolic checks on the installed dive.
[Epic #1417](https://github.com/Chelis-Lang/dive/issues/1417) · milestone
`v0.3: symbolic proofs`

## Competitive milestones

Judge each milestone on the question a practitioner actually asks. Exhausting a finite graph, finding a path at a bound, proving one named invariant, checking quantified fairness and discharging an unbounded theorem have different success criteria. Record the original source, CFG, property, edits, resource limits, result and replay evidence per run. Compare cold end-to-end time, peak memory and repeat-edit time on matching tasks. Retain stopped runs as evidence about limits. Reuse the recorded long explicit runs when establishing a new method's baseline. [Pinned peers and measurement procedure](internal/survey/verification-workloads.md) · [Performance data index](../conformance/reports/perf/README.md)

| Priority and workload | What governs scale | Next acceptance result |
|---|---|---|
| 1. Exhaustive safety on original source. Compare compiled tla-mc and tlx on KVS, Flint and an aggregate holdout. | A dense, rankable state can use direct exact membership. Coupled writes and sparse reachability change the crossover with decision diagrams. Exchangeable processes permit population vectors only when every action and property respects renaming. Deep interpreted guard trees dominate some compiled workloads. | Extend the installed automatic selector beyond its measured KVS crossover. Preserve exact counts, depth and validated violations. Beat the fastest comparable peer on a second source family and publish total cold time as well as the cost of edits. [Compiled peer panel](internal/survey/verification-workloads.md#compiling-the-finite-relation) · [Shared native operators and step properties on Flint](https://github.com/Chelis-Lang/dive/issues/1395) |
| 1. Fair temporal properties on Elevator, the token ring and Kafka. Compare Manyan. | The number of physical transitions, fairness predicates and reachable pairs of states and tableau nodes govern the cost. A passing check must exhaust the graph. A violation can finish on a closed prefix. | On four, five and six floors Core found independently validated counterexamples faster than Manyan. The seven-floor default completed in 226.29 s while the peer reached its 480 s cutoff. Beat the remaining three-floor complete proof and the published token ring, then move cold search beyond seven floors. Supplied-word replay validates the 241- and 251-floor Elevator words. At 501 floors, quantified dispatch fairness completes and the existential response reaches its work limit. Cold discovery and supplied-word replay have separate size records. Keep all original properties and replay each reported word on its actual configuration. [Seven-floor trials and input pins](../conformance/reports/perf/2026-11-22-elevator-seven.json) · [Passing-model target](https://github.com/Chelis-Lang/dive/issues/1311) · [501- and 1,000-floor replay](https://github.com/Chelis-Lang/dive/issues/1388) |
| 2. Maintaining a changing spec and proving a named invariant. Compare cold Dive, Sail-assisted rechecks and Recomp-Verify. | Reuse depends on which grounded actions changed, which property reads an edited value, and the cost of reconstructing a predicate. Sets of messages and finite tuple relations occur throughout the published protocols. | In the original 30-second panel Recomp-Verify alone finished six configurations. Recheck the matched panel after closing the remaining gaps, including quorum-valued state, and preserve the shared successes. Repeat a property edit, an action restriction, an expansion and an undo in separate processes. Publish first-proof cost, steady-state cost and break-even, including neutral cases. Validate changed transitions and independently check every reported bug. [Edit histories and trust boundary](https://github.com/Chelis-Lang/sail/issues/793) · [Original Recomp-Verify results](../conformance/reports/perf/2026-11-22-recomp-originals.json) · [Quorum-valued state](https://github.com/Chelis-Lang/dive/issues/1306) |
| 2. Bounded bug finding and implementation histories. Compare Manyan and Apalache on the authors' 46 configurations. | Paths can revisit states. A partial log can admit several internal steps between observations. Both questions need a path witness. | Reproduce bounded safety, violations and too-short executions. Keep a resource cutoff inconclusive. Use the 512-event etcd log, bad vote and repaired witness as regression controls. Check an additional released protocol history with missing implementation events. [Bounded-path implementation](https://github.com/Chelis-Lang/dive/issues/1299) · [etcd controls](../conformance/reports/perf/2026-11-22-etcd-event-histories.json) |
| 3. Invariant discovery and authored proofs. Compare Scimitar and TLAPS. | A finite counterexample to induction can rule out a candidate lemma cheaply. A proof for arbitrary process sets needs quantified obligations with the correct assumptions. Replay, simplification and solver results each have a distinct trust boundary. | The original AsyncRaft log-safety proof checks all source actions with 1,000 messages per channel and supplied lemmas. Derive a complete election strengthening with the Scimitar graph withheld, transfer channel induction to ZeusReliableCommit, and prove the four-writer SlateDB recovery path. Preserve a failing obligation after deleting a necessary premise. Independently discharge the full authored theorem and measure total proof time before proposing native replacement of an external backend. [Lemma selection and external controls](internal/survey/verification-workloads.md#selecting-a-supporting-lemma-on-a-published-proof) · [Cold Raft lemma discovery](https://github.com/Chelis-Lang/dive/issues/1390) · [Zeus channel induction](https://github.com/Chelis-Lang/dive/issues/1392) · [SlateDB replay-cursor proof](https://github.com/Chelis-Lang/dive/issues/1305) · [Native proof acceptance slice](https://github.com/Chelis-Lang/dive/issues/1307) · [Proof-system scope](open-questions.md#deductive-proof-system-scope) |

The next parallel workloads include two with published operational targets. On the original WiredTiger storage model, regenerate its 87,143 executable tests and run them against storage. On BlockingQueuePoisonApple, resolve all three original fair termination properties on the configuration that stopped at 1.07 billion distinct states and 224 GiB. The pickup queue also specifies separate work on exact temporal symmetry, source-level path bounds, protocol routing and proof reuse across Sail processes. [WiredTiger acceptance](https://github.com/Chelis-Lang/dive/issues/1383) · [Queue acceptance](https://github.com/Chelis-Lang/dive/issues/1386) · [Ranked pickup queue](https://github.com/Chelis-Lang/dive/issues/799#issuecomment-7087728826)

Two results set the size targets. The installed symbolic checker exhausted an enlarged EmergencyCall configuration with **1,400,862,255,552 distinct states** in 1,262.70 seconds and 2.42 GiB. On the released SlateDB configuration exact Core counted **4,910,385,790 symmetry representatives** covering **58,777,049,012 labeled states**. Index published measurements by count definition, property and source availability. Raise the next parameter on a released protocol with a reproduced baseline or an independently proved bound. Prefer a short refuting witness or a complete proof when that answers the user's question. [Published scale survey, original sources and growth cases](developer/large-models.md)

Product coverage runs alongside these milestones. Of 38 feature families, seven are implemented and 31 are partial. Those counts measure feature contracts, not 31 distinct bugs. Rechecking nine previously refusing configurations found eight still refusing and one independently validated animation result. Group work by the observed blocker and user workflow, starting with VIEW plus liveness and explicit host grants. Track PlusCal translation, replay and refinement alongside checker throughput. [Coverage scoreboard](scoreboard.md) · [Feature-family audit](https://github.com/Chelis-Lang/dive/issues/1280)

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
DFID, storage, debugger and editor support, host I/O and Verus export can
follow a concrete user need.

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
