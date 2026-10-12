# Roadmap

Milestones match the GitHub milestones on `Chelis-Lang/dive`. Each milestone ends with:
- a review pass (a red-team agent or a human);
- a conformance report delta;
- ADRs for the decisions it made.

The charter's soundness contract applies throughout.

See [the current sprint status](sprint-status.md) for shipped work, the latest
conformance evidence and the remaining M1, M5 and M6 packages.
The [TLC parity delivery plan](design/tlc-parity-plan.md) is the approved path
through the complete pinned feature set. The [parity ledger](tlc-parity.md)
separates semantic certification from literal source surfaces and model counts.

## M0: Foundation

Delivers:
- the workspace with one stub crate per charter component;
- pinned toolchain, workspace lints, `cargo deny`, and crate layering enforcement;
- lean CI (`ci.yml`): AI authorship guard plus workspace gate;
- the commit-msg hook, `AGENTS.md`, and the charter with its corrections appendix;
- GitHub tracking.

**Exit:** CI green on `main`, and an AI-trailer commit rejected by both the hook and CI.

## M1: Oracles and conformance harness

Delivers:
- pinned oracles: a snapshot of TLC 1.8.0 re-hosted in this repo's releases, plus v1.7.4; both use a Temurin JDK fetched by SHA-256;
- pinned corpora: tlaplus/Examples, CommunityModules, ValidationTestSuite, and the tlaplus repo's own tests;
- the `dive-conformance` harness:
  - oracle and corpus fetch;
  - TLC `-tool` output parsing;
  - golden generation with provenance;
  - per-model reports in JSON and `docs/conformance.md`;
  - worker-count invariance runs;
  - a cross-check against the Examples manifests;
- the survey of oracles and adjacent work in `docs/survey/`.

**Exit:** the first oracle-only report (TLC against TLC goldens and the Examples manifests) is committed, and every model's outcome is accounted for.

## M2: Competitive analysis

Measures tla2, tla-rs, tla-mc and tlago through harness adapters, and reads tlaplusplus's source (it is GPL, so reference only). Each is assessed on:
- conformance, including whether it honestly refuses what it doesn't support;
- performance: startup, states per second, memory, and scaling from 1 to 8 workers;
- Rust ergonomics;
- architecture.

Outputs:
- `docs/survey/competitive.md`, covering where `dive` can beat each tool;
- an ADR on positioning and which designs to borrow;
- baseline numbers in `dive-bench`.

This is not a build-or-buy gate: `dive` is built regardless.

## M3: Syntax and values

- **`dive-syntax`:**
  - a lossless lexer and CST with spans;
  - error recovery and stable error codes;
  - round-trip, property and fuzz tests;
  - parse acceptance matching SANY on every corpus module.
- **`dive parse`** with `--output json`.
- **`dive-value`**, in parallel:
  - canonical ordering, hashing and FP64-compatible fingerprints;
  - property tests.

## M4: Semantics and configuration

- **`dive-sema`:** SANY-equivalent acceptance and rejection, covering the module system, levels and the proof-language syntax, measured against the Examples and SANY test corpora.
- **`dive-config`:** the TLC `.cfg` format.

## M5: First verdicts

- **IR and evaluation:** `dive-ir`, `dive-eval` and the `dive-stdlib` core.
- **Safety checking:** `dive-check` breadth-first safety checking, in both exact and fingerprint modes. It is designed from the start for *deterministic* parallel search (ADR 0010): the first release may run one worker, but its architecture must already be level-synchronous with deterministic ordering, so that `--workers N` gives results identical to TLC's single-worker run.
- **Trace validation:** the `dive-trace` validator replays every counterexample.
- **Conformance gate:** the standard conformance tier becomes a PR gate on a small subset.

The design is in ADRs 0013–0016. The sequenced plan, with work packages, conformance milestones, the gate subset and benchmark targets, is `docs/design/m5-plan.md`. The safety checker and parallel engine are implemented; storage, errors, performance and corpus follow-ups remain open.

## M6: Liveness, fairness and symmetry

- **Liveness:** `dive-liveness` checks temporal properties with weak and strong fairness, through TLC's tableau and possible-error models, on a behaviour graph built by the deterministic parallel search. Counterexamples are lassos or stuttering endings, deterministic at any worker count, and validated by `dive-trace` (fairness and property included).
- **Symmetry:** pure safety reduction now uses the bounded group and canonical keys in both stores, retaining concrete traces. `--compat=tlc` uses TLC's group, warning rules and representative order. Deferred/lazy permutations, unverified startup contracts and randomness remain partial. The intended default for symmetry with liveness is unreduced checking (D-005); TLC's behavior stays under `--symmetry-liveness=tlc`. Liveness symmetry modes and the symmetry audit remain planned and refuse explicitly.

The separate [symmetry safety study](../conformance/reports/studies/symmetry-safety.md)
records all eleven intended manifest targets plus the extra Voting selection
agreeing at four workers, with the original one-worker timeout preserved.
It does not increase the historical standard 83/71 report totals or certify
complete symmetry parity. The required CM1 gate now exercises symmetry-bearing
Paxos Voting at 1/4/8 workers without removing an existing case.

The design is in ADRs 0017–0019 (proposed). The plan, with work packages, conformance milestones, gate additions and benchmark targets, is `docs/design/m6-plan.md`.

## Through full TLC parity

The approved nine-phase delivery plan includes complete symmetry, temporal
language, strict integrated liveness, simulation/generation and DFID, remaining
operators/configuration/output/debugger services, bounded-memory spill, native
checkpoint/recovery, native distributed checking, PlusCal and release
certification. Independent tracks can implement modes and corpus coverage while
exact liveness selection is completed. Each phase has explicit acceptance
criteria in [the plan](design/tlc-parity-plan.md); current capabilities and
remaining interactions are in [the ledger](tlc-parity.md).

Native protocols/checkpoints satisfy the approved scope; interoperability with
Java workers/checkpoints and arbitrary JVM plugin loading are excluded. Host
access still requires explicit category grants even under `--compat=tlc`.

## After parity

- **Integration:** LSP on the certified front end/checker.
- **Compilation:** native specification compilation and the Verus export path.

Public checking, simulation, translation, checkpoint and distributed library
APIs with `cargo test` integration are part of parity, not postponed here.

M6 finite-predicate increment: final/short checking and independent validation
support constant-domain forall/exists inside state/action predicates. Six pinned
models match final truth/counts in both profiles/stores; a separate 36-state probe
checks 108 quantified ENABLED decisions. Complete L1/L4/L5 coverage remains pending.

The next ordinary-liveness boundary (#185/#66) is bounded partial SCC checking
on actual committed Phase-L records. Explicit prefix diagnostics and repeated
forced reference checks do not establish the wall-clock threshold/quiescence/
error/stop/checkpoint schedule; that execution contract must be ported next
without routing ordinary strict checks through final-only or short witnesses.

Periodic backend increment #189 now derives actual pointer-bucket growth/check-size snapshots and stops the native source capture on its first explicit partial candidate. The [user guide](periodic-backend.md) qualifies that diagnostic contract. Real ordinary clock, queue suspension, ratio/PERIODIC/checkpoint transactions and complete stopping witnesses still gate ordinary temporal admission.


Issue #189 now has a bounded native clock/dispatch diagnostic seam: internally sampled source-commit polling plus actual EOF final SCCs. Ordinary periodic CLI support still requires queue/timer stop-prefix equivalence, hook/checkpoint transactions and independent deciding witness certification.

F-023 next-stage evidence now includes a bounded pure supplied-expression hook at actual native pauses. Configured PERIODIC snapshot binding, host/checkpoint effects, asynchronous queue timing, exact stop counts and ordinary deciding-witness replay remain open; diagnostic hook results earn no family/model credit.

Issue #197 adds a bounded pure configuration-owned snapshot diagnostic. Startup effects and unproved constructor/alias work refuse before binding; configured and selected identities remain separate. Ordinary scheduler admission still requires real queue/timer stop-prefix, checkpoint, exact count and independently certified deciding-witness contracts. See [the configured snapshot guide](configured-periodic-snapshot.md).

F-023/#201: producer-owned timer requests now have a real claim barrier/joined mutex pause diagnostic. Next: publish the maximal contiguous completed prefix through the same safety/output/count/graph authority before controller checks, retaining out-of-order scratch. Ordinary CLI scheduling and checkpoints stay refused until the complete queue/clock/stop/witness contracts compose.

F-023/#208: request-owned diagnostic service composes the real appended prefix and configuration-owned hook, including empty requests. Ordinary CLI/schedule/error/count/checkpoint certification remains open; no family-completion credit.

F-023/#213 now has privately reconstructed producer-owned candidate admission with fresh independent memory/JSON replay. No caller graph/trace/Valid flags certify a word. Next: prove actual ordinary timer/queue/stop/count/end/checkpoint composition before enabling ordinary verdicts; partial/no-candidate outcomes never imply Holds.

F-023/#220 retains genuinely absent configured hooks through the shared binder,
owned request service and independent candidate admission. Existing mandatory
APIs remain required; no supplied TRUE replaces absence. This removes a source
injection prerequisite without proving ordinary clock/queue/stop/count/checkpoint
composition or Holds. See the [current executable contract](optional-periodic-hook.md).


### Exhausted-queue terminal service (#222)

Bounded additive [Rust terminal diagnostics](terminal-periodic-service.md) preserve
pinned `AbstractChecker.runTLC`, `ModelChecker.doPeriodicWork/modelCheck`,
`Worker.run` and `StateQueue.finishAll/suspendAll` ordering. A genuinely finished
queue refuses suspension and skips the hook before final selection. The complete
60 corrected full CLI and 60 controlled actual outer/queue observations remain
separate from native authority. Original misleading safe labels and observer
compile failures are retained; independent replay confirms their legal stutter.
No new TLC bug/default divergence is inferred. Existing ordinary CLI/clock/queue,
checkpoint/error/count equivalence remains unproved; F-023 stays missing and
audit-required. Controller Complete and independently Valid concrete words are
not temporal Holds or model completion. See [ADR0076](decisions/0076-exhausted-queue-terminal-service.md).
