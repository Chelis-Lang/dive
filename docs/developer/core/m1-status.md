# Core status

Direction: [ADR 0153](../../decisions/0153-core-is-the-product.md). Core is the
product engine. The release gates and their order are in
[RELEASE.md](../../RELEASE.md), and the current numbers are in the
[scoreboard](../../scoreboard.md). This page keeps the record of what Core
contains and its performance measurements. The phase-by-phase qualification
narratives up to `19cbefe1` are in git history. Their checkpoint records are
under `conformance/reports/core/`.

## Current state (2026-11-05)

- Core runs 246 of 312 small-tier shadow rows and 57 of 63 CM1 rows natively,
  with no disagreement against the Reference engine. The rest are named
  refusals.
- Under `--engine auto`, 122 small-tier models (TLC profile) and 118 + 4
  expected divergences (default profile) run on Core. The rest fall back to
  Reference.
- Core alone has never been run on the full Examples tier. That measurement is
  the first item on the critical path.
- MCQuicksort is refused (#781). The cause is the normalization-history
  machinery that ADR 0153 §2 replaces with canonical sets.

## Next bounded actions

Follow the [critical path](../../RELEASE.md#critical-path). In short:
1. Measure Core alone on the full tier, and publish the refusal-reason
   histogram.
2. Canonical sets in the default profile, deleting the history, receiver and
   authority paths.
3. Burn down refusals by reason bucket.
4. Core liveness at corpus scale.
5. Bundled libraries.
6. Nominal compat.
7. Remove the fallback.
8. Freeze the surface and ship.

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
- Pure materialized sequence replacement with one scalar class across all
  arguments, with a bounded partition-definition oracle and independent
  trace validation (`03-value-model.md` §12). All overlaps, mixed, nested, lazy and effectful
  sequence forms remain refused; this is no new census or milestone credit.

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

## 3. Performance measurements

### Completed checked-membership campaign (PRE-C2)

The frozen source is `75f0ac4f769323de7be53a439ff5445ef4975b68`.
All 300 planned cells are correct: ten explicitly selected models × six
TLC/default-core/TLC-profile-core arms at 1/8 workers × five repetitions.
Every requested worker count was used. The compact record is
`conformance/reports/perf/2026-10-28-core-checked-membership-x86.json`
(SHA-256 `9c69758013bc10915764cb23370cc0a285f76f5d410547ecc81eea51273373f1`).
Original runner `quiet_window_certified=false` remains unchanged; separately
observed preflight load, fixed-frequency, SMT/turbo and CPU/NUMA controls do
not replace that missing certificate. Data validation passed; qualification
remains incomplete and M1/M2 acceptance is not assessed.

| Profile | Paired TLC/core geomean 1w / 8w | Models faster than TLC 1w / 8w |
|---|---:|---:|
| default | 2.620916× / 2.678114× | 10/10 / 10/10 |
| TLC | 2.209552× / 2.358382× | 8/10 / 8/10 |

Selected median wall seconds and median paired scaling:

| Model | Profile | Core 1w / 8w | TLC 1w / 8w | Core scaling |
|---|---|---:|---:|---:|
| GameOfLife | default | 8.2430 / 1.5965 | 12.1021 / 2.5992 | 5.1478× |
| GameOfLife | TLC | 20.9598 / 3.2763 | 12.1021 / 2.5992 | 6.3970× |
| TLCSailfish1 | default | 43.8845 / 7.6144 | 51.3167 / 8.3403 | 5.7498× |
| TLCSailfish1 | TLC | 61.9935 / 9.9963 | 51.3167 / 8.3403 | 6.2063× |

GameOfLife and Sailfish remain slower than TLC in compatibility mode (#597).
For the large-state population, default/TLC scaling is only 1.307× / 1.334×
Bakery, 1.562× / 1.563× CoffeeCan and 4.061× / 3.865× Mutex. The all-model
scaling geomeans are not acceptance for that population. Memory and broader
capacity targets remain open. These timings precede #621 and do not validate
its C2 repair or a later executable.

Integration `e0cce6f534870898d261dddb7cc789a894429065` composes the five
performance commits onto `2efcb3bf`, preserving #621's C2 boundary and all
12 consumed cases. Its own complete fast gate passes 2,198 tests (34 declared
profile skips), smoke passes eight models at 1/4 workers, and strict shadow
passes 52 rows. Fresh review passes 891,600 differential comparison/cache
cases, 409 native runs and 22 validated counterexample words. Separately,
both-profile Sailfish 10k prefixes matched. Those are integration correctness
controls, not a new full gate or five-repeat performance population.

### Separate post-C2 Sailfish diagnostic

A separate source-`e0cce6f5` diagnostic uses the release executable with
SHA-256 `e0f365155354ffebe2d50d01374e390c9e71bded4c57e9c85148e7564e50182f`.
All three one-worker runs succeed with actual workers used 1, 109,604 distinct /
314,144 generated / depth 16. Wall times are 52.693730415 s pinned TLC,
45.938790702 s default core and 61.311964963 s TLC-profile core. Raw evidence
was archived and download hashes verified. The compact record is
`conformance/reports/perf/2026-10-28-core-post-c2-sailfish-x86.json`.
This is one repetition of three arms, with original runner quiet=false;
it is separate from the PRE-C2 300-row campaign and supplies no eight-worker,
whole-population, five-repeat or M1/M2 acceptance claim.

### Historical checkpoint (x86, c7i.metal-24xl)

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
  the M3 (308 → 185 MB) but had not been checked on x86 at that checkpoint. The completed measurement below records the later cap and its remaining tradeoffs.

### Historical matched default campaign

At exact source `2d55317a`, the dedicated c7i.metal-24xl run completes all
300 planned rows: ten models × three arms (core/reference/TLC) × 1/8 workers ×
five repetitions. Every row is correct; every requested worker count was used.
SMT and turbo were off, frequency fixed at 2.4 GHz, CPUs 0–15 and NUMA node 0.
The original runner's `quiet_window_certified=false` is retained; machine setup
is not substituted for a missing certificate. Later main changes never relabel
the measured executable. CoffeeCan uses the exact pinned safety benchmark
configuration, not its original liveness property set.

Paired per-repetition ratios give geometric means of **2.649× / 2.645× TLC**
and **7.295× / 6.747× the reference** at 1 / 8 workers. All ten models beat TLC
at both worker counts, exceeding M1's default-profile throughput threshold for
this measured source. At that source the five-repeat TLC-profile campaign had not yet run;
one both-profile repetition exposed the Sailfish compat regression (#597).
The completed PRE-C2 source-`75f0ac4f` population above supersedes that
measurement status, without changing this historical source qualification. This is not full semantic M1 acceptance or M2 performance acceptance.

| Model | Core seconds 1w / 8w | ×TLC 1w / 8w | Core RSS MiB 1w / 8w | Reference RSS MiB 1w / 8w | Core scaling 1→8 |
|---|---:|---:|---:|---:|---:|
| GameOfLife | 8.117 / 1.622 | 1.49 / 1.64 | 51.6 / 141.6 | 83.9 / 90.1 | 5.01× |
| TLCSailfish1 | 43.380 / 7.890 | 1.19 / 1.05 | 74.3 / 184.8 | 627.3 / 643.2 | 5.50× |
| EWD998PCal | 11.152 / 2.349 | 3.76 / 3.29 | 39.2 / 172.1 | 142.8 / 162.1 | 4.75× |
| MCBakery | 6.010 / 4.606 | 1.84 / 1.70 | 123.3 / 216.2 | 231.5 / 241.8 | 1.30× |
| MCLamportMutex | 10.021 / 2.474 | 1.70 / 1.78 | 108.8 / 243.7 | 298.8 / 332.6 | 4.05× |
| SimpleRegular | 2.750 / 0.719 | 1.98 / 3.23 | 53.3 / 192.1 | 187.3 / 228.8 | 3.83× |
| MCbtree | 4.279 / 1.120 | 9.58 / 7.02 | 56.6 / 175.9 | 142.4 / 163.3 | 3.82× |
| CoffeeCan1000BeansSafety | 1.020 / 0.669 | 2.23 / 2.50 | 107.9 / 206.1 | 147.8 / 155.4 | 1.53× |
| MCDisruptor_MPMC | 0.894 / 0.343 | 4.97 / 4.66 | 35.8 / 88.1 | 53.1 / 65.3 | 2.61× |
| BufferedRandomAccessFile | 13.180 / 2.173 | 3.86 / 3.69 | 39.5 / 146.0 | 31.5 / 41.6 | 6.07× |

All core RSS peaks are below TLC's, but memory is not uniformly below the
reference engine: at eight workers, BRAF is 3.51×, GameOfLife 1.57× and CoffeeCan
1.33× reference RSS. For the ≥0.4-million-state population, measured core
scaling is 1.30× MCBakery, 4.05× MCLamportMutex and 1.53× CoffeeCan; the broader
ADR 0006 scaling target is unproven. These are remaining engineering targets,
not reasons to change observable results. The complete source/binary/pin,
per-repetition, CPU and RSS qualification is
`conformance/reports/perf/2026-10-28-core-m1-default-x86.json`.

Startup/fold observations at the same binary retain every sample: 30 fresh
processes per engine/profile, core medians 3.144 ms default / 3.194 ms TLC,
all 120 process times below 3.972 ms. This addresses the trivial-model 10-ms
startup target (R8) on this host, not cold-cache or arbitrary-spec startup.
CoffeeCan's one-state stopped prefixes report compile 1.132 / 1.066 ms and
fold 1.069 / 1.014 ms, with whole-command 5.805 / 5.578 ms. Those prefixes
establish no verdict. The bounded measurements address the specific R9 probe;
R10 memory tradeoffs above remain. Record:
`conformance/reports/perf/2026-10-28-core-startup-fold-x86.json`.

### Historical single matched compat-guard diagnostic

The unchanged prototype #598 and its exact parent (`1374df4e` / `e2183389`)
complete the original TLC-profile Sailfish model in 300.021 / 836.998 seconds,
with identical success, 109,604 distinct / 314,144 generated / depth 16. This
is a 2.790× improvement in **one matched pair**, with peak RSS
77,602,816 / 78,966,784 bytes. It confirms direction; it is not five-repeat
acceptance and is still much slower than the earlier approximately 52-second
TLC observation. The prototype stays draft, no further optimization was folded
into this tranche, and no new all-model compat speedup is claimed. Record:
`conformance/reports/perf/2026-10-28-core-tlc-guard-quiet-pair.json`.

## 4. Decisions taken in M1, and why

- **Memo on by default (#575).** I first held this PR. The M3 showed fewer
  instructions but equal cycles, and M0's four-model ablation had found no
  gain. The quiet x86 A/B then measured 1.16× at 1 worker and 1.23× at 8
  over 9 models, so it went in. Perf PRs are now judged by cycles or x86
  wall time, not by instruction counts alone.
- **Liveness pulled from M3 into M1, producer only (`11-liveness.md`).** Two
  bench models needed it. The core builds the graph, and the existing
  analysis and validator check it, so their results are proven ones. Phase
  2 now has native persistent rows/CSR and graph-neutral reports. Full byte
  accounting and TLC-relative capacity qualification remain open (§6).
- **`Print` and effects.** An effect bit keeps printing and register
  expressions out of folding, speculation and memoization. A spec that
  prints runs on one worker, so output comes in TLC's order.
- **`RandomElement`.** The draws come from a stream that the API supplies to
  the core, which needs no new crate dependency.
- **`ENABLED` inside an INSTANCE** supports constant/operator parameters
  and proved identity mappings of root variables. Hidden/nonidentity variable
  substitutions, initial queries, state-dependent configured operators and
  query effects/recursive/nested calls remain refused. Open question 8 (the B3 cutoff) keeps its default: TLC's cutoff in both profiles.
- **Persistent function trees deferred (#532).** The measurement was
  negative.
- **Where TLC is wrong (stale lazy reuse, B2 and B4),** the approved contract
  requires sound evaluation by default and TLC reuse under `--compat=tlc`
  (`02-semantic-contract.md` §3a). Hidden-query cache support remains pending;
  D-027 is registered when that code lands.

## 7. Working notes for the next machine

- **Workflow.** Work on a branch and PR in a separate worktree, with its own
  `CARGO_TARGET_DIR` per worktree.
  - Wrap heavy runs in `scripts/gate-lock.sh`. It is first-in first-out,
    with a separate lane for `gate --fast`.
  - Use focused checks with feedback in seconds to a few minutes, then required
    CI for incremental PRs. Keep a draft while required checks/review are pending.
  - Run the full gate on the integrated milestone candidate and before release
    qualification, recording its exact source. Do not repeat it for every edit
    or split implementation into extra unconsumed prerequisites (AGENTS.md).
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
