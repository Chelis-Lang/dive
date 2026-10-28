# M1 acceptance status (2026-10-28)

M1 is implemented across the main workstreams, but **acceptance remains open**.
The default engine remains `reference`. Use `10-plan.md` for the milestone
contract and this page for current evidence and the next bounded steps.

## 1. Current qualification

- All ten benchmark models run on the core. The prior memo A/B establishes
  useful default-profile improvements, but its cross-campaign TLC ratios below
  are historical indications, not the quiet matched M1 decision.
- The repeatable, seeded fixture census is now implemented (#595). Its first
  complete run selects 1,361 of 1,474 configurations: 99 parser exclusions and
  14 ambiguous module selections are explicitly accounted for. It runs both
  profiles at one worker with a 20-second native limit and 3-GiB memory limit.
- That run is **not clean shadow acceptance**. Default: 983 agree, 242 refused,
  three successor findings, six budget stops, 100 trace-refuted and 27
  trace-unconfirmed. TLC: 959 agree, 258 refused, three successor findings,
  four stops, 104 trace-refuted and 33 trace-unconfirmed. These are core/reference
  shadow outcomes, never TLC parity counts. Exact source and qualification:
  `conformance/reports/core/2026-10-28-fixture-census-v1.json`.
- The stronger harness (#594) rejects absent, independently unconfirmed or
  refuted required words and matching unclassified outcomes. Earlier census
  totals cannot be promoted through that change retrospectively.
- The takeover found and fixed an `ENABLED` step-cache leak (#593) capable of
  both false and missed violations; the old assertion that no wrong result
  was known is withdrawn. Mutation controls exercise the actual boundary.
- Excluded Init depth and omitted Next endings now match pinned TLC (#589).
  The reference literal-bodied operator-argument bug is fixed (#580), with
  a passing full gate; its checked user example was corrected in #599.
- Static fallback, named refusals and independent validation remain required.
  No default-engine change is proposed while the acceptance findings remain.

### Fresh small-tier control

Exact source `0f0c6ad1`, one worker, all four small-tier corpora, seed/fp 0,
TLC-derived native budgets capped at 900 seconds and 8 GiB per run. Both
profiles complete with raw harness exit 0 and zero baseline changes. The
qualification is `conformance/reports/core/2026-10-28-small-auto-source0f.json`.

| Profile | Strict agree | Expected divergence | Refused | Strict agreements actually on core | Static / dynamic fallback | Early agreements |
|---|---:|---:|---:|---:|---:|---:|
| default | 144/156 | 7 | 5 | 121 | 23 / 1 | 2 |
| TLC | 150/156 | 0 | 6 | 125 | 22 / 1 | 2 |

Default's 125 core executions include four expected divergences; its 23 static
fallbacks include three. Thus 125 core executions is not 125 strict default
agreements. TLC also has 458/458 SANY agreements. There are no disagreements,
budget stops or pending required-word validations in these controls. This
establishes `auto` baseline preservation, not universal core parity. The fallback
rate still exceeds M2's 5% target.

Five refusals are common: MCDistributedReplicatedLog and EWD998ChanID need VIEW
quotient/closing-state evidence; SpanTreeRandom needs effects/dynamic-binding
support in temporal planning; TLC/TestGraphs needs effects in finite-binding
preflight; EWD998ChanTrace needs IOEnv. TLC also refuses P4SymLive's SYMMETRY
quotient liveness. Packed L5 storage alone cannot implement these semantics.

### Trace candidate qualification

The qualification is `conformance/reports/core/2026-10-28-invariant-error-candidate.json`,
with an explicit per-ID comparison beside it. The complete census at candidate
`c831d615` has the same 1,361 fixture IDs and
identical fixture bytes as the starting run. All 37 default / 38 TLC-profile
invariant-error findings change to shadow agreement; every untargeted fixture
status remains unchanged. Default is now 1,020 agree, 242 refused, six stops,
63 trace-refuted, 27 unconfirmed and three sampler findings; TLC is
997 / 258 / 4 / 66 / 33 / 3. This candidate predates the sampler fix and is
**not a final-main run**. It adds no pinned-TLC agreement credit.

PR #604 passed its full gate (37m41s, 2,688 whole-suite Rust tests, both small
tier baselines and the required shadow selections) and a fresh independent
adversarial review. It uses trace version 2 only for named invariant errors,
with existing endings still writing version 1 and outer reports still schema 3.
It remains unmerged until the required known-consumer advance notice is posted.
The 50 residual ending-metadata findings per profile compare a reported
`TLC_NESTED_EXPRESSION` wrapper with an inner cause; they need separate typed
context and independent replay, not a blanket wrapper exemption.

## 2. What was built (about 70 PRs since #505)

**Values and store (stream 1).**
- Inline size-classed node columns (#510).
- Lazy function, set-predicate and union values (#517), and TLC's lazy
  values (#520).
- Integers of any size (#519, D-004) and model values (#533).
- `isNorm` and literal view bits (#535), and TLC intern-token string order
  (#526).
- Lock-free intern hits (#567).
- Library operators, through a standard-library hook (#554, #553 and #564):
  - Bags (#525, #554);
  - the TLC module (#556), with `Print`, `PrintT` and `ToString` (#568),
    `RandomElement` (#571) and `TLCSet`/`TLCGet` (#583);
  - CommunityModules `SequencesExt`, `Functions`, `Bitwise`, the folds,
    `FoldSet` and `FoldFunctionOnSet` (#561, #564, #569).

**Compiler and laziness (stream 2).**
- TLC comparison errors (#509) and error classes (#531).
- The constant-fold bound, R9 (#512), and arena rollback, R10 (#514).
- INSTANCE substitutions (#521).
- Operator arguments (#539), lazy `LET` in split actions (#541) and
  conjoined initial predicates (#537).
- Lazy infinite unions (#544) and `UNION {S}` compiled as `S` (#559).
- A failing `ASSUME` is reported (#547).
- Faster rectangle-domain functions and small set differences (#578).
- `ENABLED` (#581).
- Memo tables: back on by default (#575, after #530 had turned them off) and
  bounded by the memory budget (#584).

**Generation and engine (stream 3).**
- The complete `getNextStates` rules (#508), traces (#511), error endings
  (#516), budgets (#518) and the parallel engine (#523).
- `--engine=auto` with fallback (#524, #549).
- `--compat=tlc` on the core at any worker count (#545, #548).
- Binding failures (#546), fingerprint errors (#536) and D-010 notes (#543).
- Implied properties (#560), SYMMETRY in both profiles (#563, #574), ALIAS
  (#565), VIEW (#570) and VIEW with SYMMETRY (#576).
- The string and transient memory fixes (#562, #566).
- Faster step predicates (#579).
- Liveness on the core (#572 design, #573, #577, #582). The core builds the
  behaviour graph, and the reference engine's `dive-liveness` and
  `dive-trace` check it, so verdicts and counterexamples are the reference
  engine's.

**Harness, correctness and bench (lead).**
- The harness runs on a chosen engine (#529).
- Shadow comparison of the core with the reference engine, in CI and the
  full gate (#534, #538), with successor sampling (#542).
- The x86 checkpoint and memo A/B (#552, #585).
- The AWS bench and gate scripts (`scripts/aws/`) and the shared gate lock
  (`scripts/gate-lock.sh`).

## 3. Historical checkpoint (x86, c7i.metal-24xl)

| Model | ×TLC, 1 worker | ×TLC, 8 workers |
|---|---|---|
| MCbtree | 9.4 | 7.4 |
| MCDisruptor_MPMC | 5.1 | 4.7 |
| CoffeeCan1000BeansSafety | 2.3 | 2.5 |
| SimpleRegular | 2.0 | 3.2 |
| MCBakery | 1.8 | 1.7 |
| MCLamportMutex | 1.8 | 1.7 |
| EWD998PCal | 1.5 | 1.6 |
| GameOfLife | 1.5 | 1.6 |
| TLCSailfish1 | 1.2 | 1.06 |
| BufferedRandomAccessFile | not yet measured on x86 | — |

- **Sources.** Core times are from the memo A/B at `fdaf57a2`
  (`conformance/reports/perf/2026-10-28-core-memo-ab-x86.json`). TLC times
  are from the checkpoint at `f1ed932a`
  (`conformance/reports/perf/2026-10-27-core-m1-x86.json`), measured on a
  separate node of the same type, so the ratios are indicative.
- **Changes since those runs.**
  - #579 later cut EWD998PCal's refinement check from 14 s to 5.5 s.
  - #578 cut GameOfLife's cycles by 27%.
  - BufferedRandomAccessFile runs on the core only since #581: 10.5 s
    against 79 s for the reference engine on the M3, indicative.
- **Memory.** The core's peak RSS is far below TLC's: tens to a few hundred
  MB, against 0.8–1.5 GB. With memo on, 8-worker RSS roughly doubled on some
  models (MCLamportMutex 184 → 419 MB). The cap in #584 brought that down on
  the M3 (308 → 185 MB) but has **not been checked on x86**.

## 4. Decisions taken in M1, and why

- **Memo on by default (#575).** I first held this PR. The M3 showed fewer
  instructions but equal cycles, and M0's four-model ablation had found no
  gain. The quiet x86 A/B then measured 1.16× at 1 worker and 1.23× at 8
  over 9 models, so it went in. Perf PRs are now judged by cycles or x86
  wall time, not by instruction counts alone.
- **Liveness pulled from M3 into M1, producer only (`11-liveness.md`).** Two
  bench models needed it. The core builds the graph, and the existing
  analysis and validator check it, so their results are proven ones. Phase
  2 (L5, a compact graph sized to TLC's capacity) is still to do.
- **`Print` and effects.** An effect bit keeps printing and register
  expressions out of folding, speculation and memoization. A spec that
  prints runs on one worker, so output comes in TLC's order.
- **`RandomElement`.** The draws come from a stream that the API supplies to
  the core, which needs no new crate dependency.
- **`ENABLED` inside an INSTANCE with substitutions** is refused for now.
  Open question 8 (the B3 cutoff) keeps its default: TLC's cutoff in both
  profiles.
- **Persistent function trees deferred (#532).** The measurement was
  negative.
- **Where TLC is wrong (stale lazy reuse, B2 and B4),** the default profile
  is sound and `--compat=tlc` reproduces TLC (`02-semantic-contract.md`
  §3a). D-027 is registered when that code lands.

## 5. Remaining acceptance and implementation

The census identifies distinct obligations; one fix cannot waive another family:

| Obligation | Evidence and next action |
|---|---|
| Sampled level context | Three findings per profile lose `TLCGet("level")` context. #600/#601 preserve the actual sampled expansion level and independently check expected successors. |
| Invariant evaluation-error words | 37 default / 38 TLC findings need a named invariant and exact underlying cause, with independent prefix and failure-order validation. The scoped trace-v2 candidate addresses this part of #490 only. |
| Other terminal error words | 50 findings per profile still compare a 2103 wrapper with an underlying ending cause; retain #490 and classify each evaluation site before changing representation. |
| Constraint/action error words | 13 default / 16 TLC refuted words remain in the #499 families. Preserve actual terminal evaluation ordering; do not exempt the last state broadly. |
| Errors while expanding the final state | 17 default / 23 TLC words are independently unconfirmed; #434 needs an independent successor-generator strategy. |
| Missing words | Ten required counterexample documents per profile are absent. Matching reports supply no substitute. |
| Resource stops | Six default / four TLC runs stop with native exit 5. They remain incomplete. `ErDeepRecursion` separately returns a classified resource-limit refusal in both profiles. |
| M1 performance decision | Complete the matched five-repetition default campaign, all ten models, 1/8 workers; record medians, correctness and memo-capped RSS. |
| Compat performance | #597: one completed quiet Sailfish repetition takes core 841.01 s versus TLC 52.02 s at one worker, despite correct counts. The comparison-guard prototype #598 is diagnostic until matched quiet measurements and gates pass. |
| Correctness coverage | Fresh small-tier `auto` comparison in both profiles, with actual engine/fallback accounting, plus the M1 ablation matrix. Do not substitute reference-only gate results for core coverage. |
| Startup and memory | Re-measure startup and retain memo-cap RSS evidence; identify remaining regressions before claiming the risks closed. |

Known implementation gaps remain: `ENABLED` inside INSTANCE substitutions,
SYMMETRY with liveness (`P4SymLive`), L5 graph capacity, `kSubset`, sequence
replace forms, `MapThenFoldSet`/`Folds`, TLC-profile `AllSubSeqs` membership and
the two printing refusals. L5 requires state storage as well as packed topology
(`11-liveness.md`); `kSubset` needs a symbolic value and startup/validator support
(`03-value-model.md`). Neither is a one-function completion.

M2 still requires full-tier core coverage, at most 5% fallback, a 2× TLC
geometric mean at both worker counts and no slower benchmark. M3 retains the
remaining execution modes, checkpoints and storage work. M4 default replacement
requires its own decision. Historical 182/217 full-tier agreements predate the
core reboot and do not establish current core breadth.

## 6. Execution order

1. Finish the small, source-bound correctness fixes and their required gates;
   retain failing baseline census evidence and rerun the same selection.
2. Complete and preserve the quiet matched default campaign; evaluate its gate
   independently of the TLC-compatible slowdown. Test the single guard prototype
   against its original algorithm before any performance merge.
3. Run `auto` against both small-tier baselines with full accounting. Keep the
   reference default while required words or supported paths remain unproven.
4. Address remaining error-word families by evaluation site, with smallest
   pinned reproducers and independent validation controls. Then resume the
   documented stdlib/compiler/L5 streams, each as a single-scope PR.

The corrective discipline is concrete: no invented agreement credit, no weakened
validator, no stale build identity, no source sharing between heavy jobs, no
performance inference from instruction counts or mismatched campaigns, and no
large graph rewrite before the existing state-storage boundary is explicit.

## 7. Working notes for the next machine

- **Workflow.** Work on a branch and PR in a separate worktree, with its own
  `CARGO_TARGET_DIR` per worktree.
  - Wrap heavy runs in `scripts/gate-lock.sh`. It is first-in first-out,
    with a separate lane for `gate --fast`.
  - Open a PR as a draft until its local gate passes.
  - Run the full gate for any PR that touches `dive-eval`, `dive-check`,
    `dive-value`, `dive-store`, `dive-liveness`, `dive-model` or
    `dive-trace`.
- **AWS.** Nodes run on credits, are tagged `project=dive`, and are
  terminated after use, at most two at a time.
  - `scripts/aws/` needs an SSO `AWS_PROFILE` for the dive account.
  - Only benchmarks and gates run there, never agents.
- **macOS.** On the old Mac, macOS scanned every newly linked test binary,
  which turned a 20-minute fast gate into hours. Add the terminal under
  System Settings → Privacy & Security → Developer Tools.
- **Security.** An earlier bench bootstrap printed the GitHub token stored in
  the SSM parameter `/dive/bench/gh-token` into node logs and a local
  transcript. Rotate that token if it has not been rotated already. The
  scripts in `scripts/aws/` never print it.
