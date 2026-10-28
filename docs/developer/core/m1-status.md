# M1 status at handover (2026-10-28)

This summarizes the core reboot from the end of M0 (#505, 2026-10-26) to the
handover on 2026-10-28. It is meant to be read first. The decision log and
the per-stream next steps are in `10-plan.md` (§5 and the 2026-10-28 handoff
entry). Each area's design doc has its own "Next step".

## 1. Where it stands

- `--engine=core` runs all 10 bench models. On every model, the counts,
  depth and verdict equal the TLC goldens.
- On quiet x86, the 9 models measured with memo on are faster than TLC at
  both 1 and 8 workers. The ratios range from 1.06× (TLCSailfish1, 8
  workers) to 9.4× (MCbtree, 1 worker); see §3.
- The M1 gate is ≥1× TLC on 8 of 10 bench models and the small tier at
  baseline. It is **expected to pass but not yet decided**: it needs one
  quiet x86 run of core, reference and TLC together (§6, step 1).
- The default engine is still `reference`.
  - `--engine=auto` runs the core and falls back to the reference engine
    before the search on any refusal the core names.
  - CI shadows the core against the reference engine on the smoke models.
- No known wrong result on the core. Every refusal names the feature.

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

## 3. Measured results (x86, c7i.metal-24xl, the measurement of record)

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

## 5. Open at handover

**PR #580**, the fix for #557: the reference engine wrongly applies a
constant-body `LAMBDA` passed to the folds, where TLC raises 2266 or 2154.
It is rebased onto `main` and touches `dive-eval`, so it needs a fresh full
gate before it merges.

**Known gaps.**
- `ENABLED` inside an INSTANCE with substitutions.
- SYMMETRY with liveness (`P4SymLive`).
- Liveness phase 2 (L5).
- `kSubset`; the sequence forms of the replace operators; `MapThenFoldSet`
  and the `Folds` module.
- `AllSubSeqs` membership under `--compat=tlc`.
- Two `Print` refusals: a run that prints and then ends in an error, and
  printing a lazy set.

**To triage.** The fixture shadow census has 10 differences:
- 6 are attributed to time-limit stops. They need naming and confirming.
- 2 are unseeded `RandomElement` runs in the census script, so the script
  needs `--seed`.
- `DfidInitialOnly` and `SimExcludedInitial` need a real triage.

**Untested path.** No fixture reaches the `Compiler::step` clear inside
`ENABLED`; disabling it leaves every test green.

**Unexplained regression.** About 11 G of MCLamportMutex's instruction
increase since M0 is spread over #510, #535–#545, #509 and smaller changes.
It is not yet measured in cycles.

**Open issues from this work.**

| Issue | What |
|---|---|
| #557 | The reference engine applies a constant-body operator that TLC rejects (fix in #580) |
| #527 | The core's `Len` of a function over `1..0` |
| #515 | The reference engine's 2280 on an instantiated Naturals operator |
| #504 | The JSON report omits a failed `ASSUME`'s name |
| #498 | Umbrella issue |

#550 (SimpleRegular scaling), #551 (MCBakery RSS) and #555 (MCDisruptor
speed) are fixed and closed.

**Watched risks.** Startup time (R9) and RSS (R10) have not been re-measured
on x86 since memo went back on.

## 6. Next steps, in order

1. **The M1 gate run.** Launch the bench node with
   `scripts/aws/bench-node.sh <main sha> --dive-bin ~/bins/dive-core --dive-revision <sha>+core --baseline-bin ~/bins/dive --baseline-revision <sha>+reference --profile default --workers 1,8 --reps 5 --tlc --timeout 900 --xmx 3g`.
   It covers all 10 models, core against reference and TLC. Record a compact
   JSON and a §5c in `10-plan.md`, including RSS with the memo cap. Then
   terminate the node.
2. **Merge #580** after a full gate, local or through
   `scripts/aws/gate-node.sh` and `remote-gate.sh`.
3. **Small-tier census under `--engine=auto` in both profiles.** If the
   evidence holds, propose `auto` as the default engine in
   `docs/open-questions.md`. That is M4 in the plan, but it may be ready
   earlier.
4. **Restart the three streams** from the handoff entry in `10-plan.md`:
   - stream 1: the stdlib gaps;
   - stream 2: `ENABLED` under substitutions plus D-027, the census fixes
     and the `Compiler::step` test;
   - stream 3: the two triages, L5 and `P4SymLive`.

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
