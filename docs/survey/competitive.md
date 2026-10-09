# Competitive analysis: other TLA+ implementations

Gathered 2026-10-08. This is a preliminary pass. The `dive-conformance` harness will redo the
conformance numbers rigorously against TLC goldens; until then, treat the counts below as a
well-instrumented first look, not as the record.

Every statement is labelled:
- **measured**: we ran it, and the raw data is under `docs/survey/data/`;
- **observed**: we read it in the source, docs or history;
- **claimed**: the project says so and we have not verified it.

## Executive summary

dive is built regardless. The question here is what each competitor has, and where dive beats it.

1. **Nobody combines soundness, speed and an embeddable API.**
   - The closest to TLC-exact is **tla-mc**. On the small Examples models it never produced a wrong verdict or a wrong count, and it refuses what it does not support. It is bin-only in practice, though, has no structured output, diverges from TLC on 32-bit overflow without saying so, and raised a false liveness alarm under SYMMETRY (probe P4).
   - The most faithful port is **tlago**. It matched TLC wherever it finished, including the overflow error and CHOOSE order. But it is a Go port that brings the JVM's global-state design with it, and it is about 17–20× slower than TLC at 8 workers (measured).
   - The three others report wrong answers as success on the corpus or on our probes.
2. **Silent wrong answers are common.** On 140 small exhaustive models (measured):
   - tlaplusplus claimed success with a wrong state count on 12 models. 6 of them under-explored, by up to 99%. It missed one real safety violation outright, and reported evaluation errors as invariant violations on 16 models.
   - tla-rs reported wrong counts on 2 models. On a probe it silently ignored `ACTION_CONSTRAINT` (warning only, then "No errors found").
   - tla2 reported one false invariant violation (`MCbtree`) and one false liveness violation (`SingleLaneBridge`). It also labels refusals as "liveness failure" in its human output.
3. **Integer overflow is the most common silent divergence.**
   - TLC rejects `2147483647 + 1` with "TLC can't handle a number this big" (exit 255).
   - tla-mc, tla-rs, tlaplusplus and tla2 all report success. tlago reproduces TLC's error.
4. **The headline claims do not survive measurement as stated.**
   - tla2's "160/160 TLC parity" counts its own status mismatches as matches. Measured: 108/140, and the CLI does not build from the published repo without patching out 404 dependencies.
   - tlaplusplus's "TLC feature parity" (claimed) sits beside the silent under-exploration above (measured).
   - tla-mc's "same state counts" holds on what it accepts (measured: 99/99), but it accepts 99 of 140.
   - tla-mc's "~4× faster than TLC" (claimed) **reproduces** for its compiled mode: measured ≈3.9× geometric mean at 8 workers on 4 shared models, and ≈1.9× for the interpreter. Its interpreter is 3–4× slower than TLC on `GameOfLife`.
5. **A surprise in TLC itself.** `CHOOSE` over a set of strings picks the string that was *interned first* in the module text, not the lexicographically smallest. Adding an unused definition `Unused == "alpha"` above `Init` changes which element `CHOOSE x \in {"zeta","alpha","mid"} : TRUE` returns, from `"zeta"` to `"alpha"` (measured; probes P2 and P7). This breaks the charter's metamorphic rule that reordering definitions must not change results. dive must reproduce it under `--compat=tlc`. Whether a canonical order becomes the default is open question 8 (ADR 0010).
6. **Where dive wins:**
   - verdict honesty: refusal as a first-class result, in both the exit code and JSON;
   - two honest profiles (ADR 0010). `--compat=tlc` gives TLC-identical results, including the 32-bit overflow error, interning-order CHOOSE and floor division. The default profile is strictly better (big integers, refusing symmetry with liveness, deterministic parallel workers), and the output reports when a result would differ under `--compat=tlc`. Competitors that improve on TLC do it silently and offer no TLC mode;
   - deterministic parallel search that still gives shortest counterexamples;
   - an embeddable library with counterexamples as data;
   - startup and footprint: measured startup is tla-mc 2 ms, tla-rs 4 ms, tla2 17 ms, tlago 83 ms, TLC 335 ms and tlaplusplus 1,016 ms. dive's target is ≤ 10 ms.

   Performance is the most contested axis. tla-mc already shows that compiled specs beat TLC substantially; see the performance tables.

## Subjects and provenance

| Tool | Repo @ commit (date) | Licence | Lang | Own code (lines) | Status |
|---|---|---|---|---|---|
| tla2 | `joweeba/tla2` @ `5e828d3` (2026-04-29) | Apache-2.0, © Dropbox | Rust | ~585k on the check path; 372 crates under `crates/`, ~345 of them vendored crates.io forks | Squashed publish mirror (5 commits). Upstream `andrewdyates/tla2` and its git deps `andrewdyates/{z4,tMIR,LLVM2}` all return 404 |
| tla-rs (`tla-checker`) | `fabracht/tla-rs` @ `627fa61` (2026-10-07) | MIT OR Apache-2.0 per `Cargo.toml`; GitHub reports none | Rust | 36.9k `src/` + 7.1k tests | Active, 188 commits since 2026-01 |
| tla-mc | `ddalton/tla-mc` @ `09e1cc5` (2026-10-07) | MIT | Rust | 10.9k | 27 commits (2026-09-26..10-07), repo created 2026-10-07 from flint `formal/tlc-rs` |
| tlago | `glycerine/tlago` @ `905e94f` (2026-10-08) | MIT | Go | ~164k non-test (root 63k + `tlc/` 101k) | 1,723 commits in 11 days, WIP |
| tlaplusplus | `zoratu/tlaplusplus` @ `2884162` (2026-08-30) | **GPL-3.0** | Rust | ~110k `src/` + 15k tests | 983 commits (2026-02..08). Reviewed for ideas only; no code or text copied |

## Method (reproducible)

Everything ran inside Docker on an Apple M3 host (8 cores, 24 GB). The Docker Desktop VM is linux/aarch64 with 8 CPUs and 7.7 GB. Third-party code was never built or run on the host.

- **Images.**
  - `rust:1.99-bookworm` with `time` and `python3` added (`divecomp-run:1`) for building the Rust tools and running everything.
  - `golang:1.26-bookworm` for building tlago.
- **Oracle.**
  - TLC `tla2tools.jar`, sha256 `7beec0f0…ea78d` (the `tlc-2026.10.06.014338` pin).
  - Temurin JRE 17.0.20.1+1 aarch64, sha256 `b8efcd5a…7d77`.
  - CommunityModules 202610040242 jars, sha256 `b7fcd727…15ad` and `dddb19c3…7bfb7d`.
  - tlapm library at `d8b9747` on the classpath, as Examples CI does.
  - All hashes were verified against `conformance/oracles.toml`.
  - The jar sat in a directory next to the same CommunityModules jars that were also passed on `-cp`. TLC's manifest `Class-Path` pulls jars from its own directory, so the dive harness should keep the jar isolated and pass `-cp` explicitly.
- **Corpus.** tlaplus/Examples at `6a7b26a`.
- **Selection.** We used Examples' own `check_small_models.py` rule: every model with `mode == "exhaustive search"` and manifest `runtime <= 00:00:30`, which gives 140 models. The manifest's `workers` field is honoured (`MCDieHardest` needs 1).
- **Runner.** The runner is a throwaway Python script and is not committed. Each model runs once per tool, in a fresh container with `--network none`, the corpus and binaries mounted read-only, cwd set to the spec directory, and a module library directory holding the CommunityModules `.tla` files plus the TLAPS library. Exact per-tool invocations follow. They are written so they can be turned into `dive-conformance` external adapters (`adapter:<file.toml>`) directly: the command template, the library-path mechanism, and how to derive a verdict and distinct-state count from the output.

| Tool | Invocation (cwd = spec dir) | Library path | Verdict source |
|---|---|---|---|
| TLC | `java -XX:+UseParallelGC -Dtlc2.TLC.ide=Github [-Dtlc2.tool.impl.Tool.cdot=true if feature "action composition"] -cp tla2tools.jar:CM.jar:CM-deps.jar:tlapm/library tlc2.TLC M.tla -config C.cfg -workers W -lncheck final -cleanup -metadir /tmp/m` | classpath | exit code 0/10/11/12/13, `N states generated, M distinct states found` |
| tla-mc | `tla-mc -workers W -lib LIB -metadir /tmp/m -config C.cfg M.tla` | `-lib` or `TLAMC_LIB` | exit 0, 10, 12 (all failure kinds), 2 (refusal or error). Disambiguate on `^Error: Invariant`, `^Error: Deadlock reached`, `^Error: Temporal propert` |
| tla-rs | `tla M.tla --config C.cfg --max-states 100000000 --max-depth 100000 --check-liveness` | none; copy modules beside the spec | exit 0/1 only. Parse `No errors found`, `Reachable states: N`, `Invariant … violated!`, `Deadlock detected!` |
| tlaplusplus | `tlaplusplus run-tla --module M.tla --config C.cfg --workers W --bloom-auto-switch false --minimize-trace false --auto-tune false --numa-pinning false --skip-system-checks --trace-parents --work-dir /tmp/w --clean-work-dir true` | `TLA_LIBRARY_PATH` | exit 0, or 1 for both violation and error. Parse `^violation=(true\|false)` and `message: failed evaluating` (eval error shown as a violation) |
| tlago | `tlago -workers W -config C.cfg -lncheck final -cleanup -metadir /tmp/m -I LIB M.tla` | `-I` | TLC exit codes and TLC output lines |
| tla2 | `tla2 check M.tla --config C.cfg --workers W --output json --force --bfs-only` | `TLA_PATH` | JSON `result.status`, `result.error_type`, `statistics.states_distinct`. `error_type` `runtime_error` or `SYS_*` codes are refusals or errors, not verdicts |

- **Categories.**
  - **agree**: same verdict as the manifest and, for successful models with a recorded count, the same distinct-state count.
  - **wrong-count-claimed-success**: success with a different distinct count. A soundness problem when the count is lower (under-exploration). A conformance problem when higher.
  - **wrong-verdict-claimed-success**: success where TLC finds a violation (a missed bug).
  - **disagree**: a different failure verdict, or a violation where TLC finds none (a false alarm).
  - **refused / error**: no verdict, with a message (unsupported feature, parse error, unknown operator).
  - **crash**: panic or OOM kill.
  - **timeout**: over the per-model limit (TLC 300 s, tlago 300 s, tla-mc 300 s, tla2/tla-rs/tlaplusplus 180 s).
- **Caveat.** These runs shared the machine (up to 3 tools ran concurrently at 1–4 CPUs each), so their wall times are not performance data. Only the separate benchmark session below is.

## Conformance on 140 small exhaustive Examples models (measured)

| Tool | agree (count-exact / failure verdict / other) | wrong-count, claimed success | wrong verdict, claimed success | disagree (false alarm, other) | refused or error | crash | timeout |
|---|---|---|---|---|---|---|---|
| TLC (pinned) | 139 (121 / 16 / 2) | 0 | 0 | 1 (harness) | 0 | 0 | 0 |
| tla-mc | 99 (87 / 12 / 0) | 0 | 0 | 0 | 41 | 0 | 0 |
| tla2 (patched build) | 108 (93 / 14 / 1) | 0 | 0 | 2 false alarms + 2 cdot guards | 25 | 0 | 3 |
| tlago | 110 (94 / 15 / 1) | 0 | 0 | 1 harness + 2 cdot guards | 2 | 15 | 10 |
| tlaplusplus | 96 (83 / 12 / 1) | **12** (6 under-explored) | **1** | 1 false alarm + **16 eval errors reported as violations** | 5 | 4 | 5 |
| tla-rs | 44 (34 / 10 / 0) | **2** | 0 | 0 | 90 | 1 | 3 |

Notes on the table:
- **TLC's one disagreement** is `ewd998/EWD998ChanTrace`, which reads a trace file through a path relative to the Examples root. The runner uses the spec directory as cwd, so every tool fails it. It is a harness artefact; the dive harness must run with cwd at the corpus root.
- **"cdot guards".** `MCCRDT` and `MCReplicatedLog` `ASSUME TLCGet("-Dtlc2.tool.impl.Tool.cdot") = "true"`. Tools without the experimental `\cdot` flag fail the guard as designed. This counts as an honest refusal.
- **Under-exploration by tlaplusplus** (claimed success; TLC count in parentheses):

  | Model | tlaplusplus | TLC |
  |---|---|---|
  | `TLCSailfish1` | 976 | 109,604 |
  | `SDP_Attack_New_Solution_Spec` | 145 | 2,881 |
  | `MCNanoSmall` | 1,563 | 3,003 |
  | `Simple` | 362 | 723 |
  | `SimpleRegular` | 169,166 | 277,726 |
  | `LockHS` | 1 | 28 |

  It also missed the safety violation in `SDP_Attack_Spec/MC.cfg`, reporting success after 279 states.
- **Over-exploration by tlaplusplus:** `MCVoting` 599 vs 77, `MCParReach` 7,655 vs 393, and 4 more.
- **tla-rs wrong counts** both come from SYMMETRY:
  - `MCFlashWithMutex`: 8,096 vs 2,024. It printed a warning that the symmetry set did not match a constant, then ran unreduced.
  - `MCGermanCMPWithMutex`: 1,539 vs 1,107. Its canonicalisation is not a canonical form.

  The verdicts are still sound for safety, but the counts are not TLC's.
- **tla2's false alarms:**
  - `btree/MCbtree`: "Invariant KeyOrderPreserved is violated" after 85,940 states. TLC: success, 374,727 states.
  - `SingleLaneBridge/MC`: "Temporal properties were violated". TLC: success.
- **tla2's refusals are mislabelled.** Its human output says "Error: Liveness failure: … unsupported form: <>~A5" (exit 1). JSON correctly says `error_type: runtime_error`, `SYS_LIVENESS_RUNTIME_FAILURE`. Anyone parsing the text or the exit code sees a liveness violation.
- **tla-mc's exit codes conflate failure kinds.** It exits 12 for deadlock, liveness and evaluation errors as well as invariant violations. On `YoYoAllGraphs` an evaluation error ("cannot enumerate the infinite set Seq({1})") exits 12, the same code as a violation.
- **tlago crashes** are all one bug: `interface conversion: interface is nil, not tlago.sanyCanonicalLevelNode` on every Paxos-family and proof-heavy spec. TLC-style exit 255.
- **tla-rs refusals** are mostly front-end gaps: parse errors on proof syntax in `FiniteSetTheorems` and `NaturalsInduction`, cfg substitutions of parameterised operators, `INSTANCE` substitution of modules, and `LOCAL`. On `CoffeeCan1000Beans` it fails "ASSUME constraint evaluated to FALSE". This is its default `Nat = 0..100` truncation (observed in `src/stdlib.rs`) surfacing as an error rather than a wrong answer.

Raw per-model data: `docs/survey/data/conformance-small-models.csv`.

## Honesty probes (measured)

Each probe is a tiny spec run on every tool and on TLC. The specs are reproduced in the appendix. The expected value is TLC's result.

| Probe | TLC (oracle) | tla-mc | tla-rs | tlaplusplus | tlago | tla2 |
|---|---|---|---|---|---|---|
| P1 `x' = x + 1` past 2^31−1 | error: "can't handle a number this big" (exit 255) | **success, 3 states** | **success, 3 states** | **success, 3 states** | error (matches) | **success, 3 states** |
| P2 `CHOOSE` over `{"zeta","alpha","mid"}` | `"zeta"` (interning order) | `"zeta"` | `"alpha"` (lexicographic) | `"alpha"` | `"zeta"` | `"zeta"` |
| P2 `CHOOSE` over sets `{{3},{1,2},{}}`, non-empty | `{3}` (size first) | `{3}` | `{1, 2}` | `{3}` | `{3}` | `{3}` |
| P2 `CHOOSE` over `{<<2,1>>, <<1,2,3>>, <<1,3>>}` | `<<1,3>>` (length first) | `<<1,3>>` | `<<1,2,3>>` | `<<2,1>>` | `<<1,3>>` | `<<1,3>>` |
| P7 as P2 but with `Unused == "alpha"` above `Init` | `"alpha"` | `"alpha"` | `"alpha"` | `"alpha"` | `"alpha"` | `"alpha"` |
| P3 `Json!ToJson(<<1,2>>)` (Java override only) | `"[1,2]"` | refused: unknown operator `ToString` | error: type mismatch (misleading) | `"[1,2]"` (native) | `"[1,2]"` (native) | **false violation**: produced `"[1, 2]"` |
| P3 `ACTION_CONSTRAINT` pruning 4→5 | success, 5 states | refused: "ACTION_CONSTRAINT is not supported" | **success, 11 states** (warning only) | success, 5 | success, 5 | success, 5 |
| P4 SYMMETRY + liveness (`<>(x=b)` holds) | success, 1 state, with TLC's warning | **false liveness violation** | success, 2 states (symmetry silently not applied, warning) | success, 2 states | success, 1 | success, 1 |
| P5 `TLCSet` in ASSUME, `TLCGet(1)`, `TLCGet("level")` | violation at level 4 | refused: unknown operator `TLCGet` | violation at 4 (matches) | error: "no valid initial states found" (misleading) | matches | matches |
| P6 `(-7) \div 2 = -4 /\ (-7) % 2 = 1` | success | success | success | **false violation** (computes -3, -1: truncating division) | success | success |
| P8 parallel determinism, workers 1 vs 8 (3 runs each) | w=1: 63,251 states at violation, 46-state trace. w=8: 64,416 / 67,497 / 65,409 states, trace 46 | w=1 identical to TLC. w=8: 63,399 / 63,364 / 63,379, trace 46 | single-threaded: always 63,251 / 46 | w=1: 64,252 (≠ TLC). **w=8 trace 47, 48, 47 (not shortest)** | w=1 identical. w=8 varies, trace 46 | w=1 identical. **w=8: no trace at all** ("trace unavailable: --no-trace mode") |
| P9 `RandomElement(1..1000000)`, 3 runs | unseeded: 599378, 344064, 43074. `-seed 7`: 730700 ×3 | refused: unknown operator | 602628 ×3 (fixed seed, not TLC's sequence) | **1 ×3** (always the minimum; not random) | random per run, like TLC | 223055 ×3 (derived from fingerprint and seed) |
| M1 syntax error (missing `)`) | SANY parse error, line 6 col 1, exit 150 | `line 6 col 1: expected )` (no file name in the location) | only a *warning* "failed to parse operator 'Next'", exit 1 | **hangs at 1 state forever** (2 workers); with 1 worker, panic then "all 1 worker threads panicked" | SANY-identical text | ariadne-rendered span, `TLA_PARSE_ERROR` in JSON (no location in JSON) |
| M2 undefined identifier `step` | "Unknown operator: `step'", line 5 col 18–21 | "unknown identifier step" (no location) | runtime "undefined variable `step`" while evaluating Next | **hangs**; treats `step` as a model value, then panics "expected Int, got ModelValue" | SANY-identical | "semantic error: undefined identifier `step`" (no line or column) |

Raw data: `docs/survey/data/probes.csv` and `docs/survey/data/determinism-random.csv`.

**Reading the probes for honesty:**
- **Refuses rather than approximates:** tla-mc (P3 both, P5, P9). Its two exceptions are the silent integer widening (P1) and the false liveness alarm under SYMMETRY (P4).
- **Silent wrong answers:**
  - tla-rs: P1, P3 action constraint, P2 order.
  - tlaplusplus: P1, P6, P9, and the malformed-spec hangs.
  - tla2: P1, the P3 JSON formatting, and the dropped parallel trace.
- **Faithful by construction:** tlago, where it runs.

## Better than TLC without silent changes (ADR 0010 framing)

dive has two profiles. Under `--compat=tlc`, results are identical to TLC, proven over the full corpus. The default profile is strictly better where the owner has approved it, it reports every active option, and it flags results that would differ under `--compat=tlc`. The table records two things for each competitor, in each area where TLC has a known limitation:
- whether it offers better-than-TLC behaviour;
- whether it does so *without silently changing results*, which means it offers a TLC-identical mode and says when it deviates.

| TLC limitation | tla-mc | tla-rs | tlaplusplus | tlago | tla2 |
|---|---|---|---|---|---|
| **Parallel search is only approximately BFS**: counts at a violation and, per tlaplus#883, trace length vary with workers | Level-synchronous BFS. Measured: trace stays shortest at 8 workers; count at violation varies (like TLC). No option for deterministic counts | No parallelism | **Defaults are worse**: non-shortest traces at 8 workers, and its 1-worker count differs from TLC | Same as TLC (port) | Parallel mode **drops the counterexample** (measured). Work-stealing, not level-synchronous (observed) |
| **Unseeded randomness** (`RandomElement`, simulation) | Not implemented (refused) | Seeded and deterministic, but the **default silently differs from TLC** (fixed seed) | **Silently not random** (returns the minimum) | TLC semantics, `-seed` (observed) | Deterministic, fingerprint-derived: **silent default change** |
| **32-bit integers**: overflow is an error | **Silently** i64; wraps past 2^63 in release (observed) | **Silently** i64, checked at 2^63 | **Silently** i64, checked at 2^63 | TLC semantics (int32 plus overflow error) | **Silently** BigInt |
| **Human-only output** | None. Text only | `--json` (hand-built, no schema version, lossy value encoding); MCP server | None | `-tool` message framing, `-dumpTrace json` | `--output json` v1.3 with `soundness` and `completeness` blocks. But the JSON counterexample was empty on a violation (measured) |
| **Fingerprint-only exhaustiveness** (probabilistic) | No exact mode, no collision bound | Exact (full states), always | Exact fingerprint set by default, but **Bloom auto-switch on by default** (observed) | FP64 plus TLC's bound | `--store-states` exact option; bound in human output only |

**Conclusion for dive:** no competitor offers both a proven TLC-identical mode and a better mode, with the difference reported. Each one that improves on TLC changes the answer silently and offers no way back to TLC's behaviour. tlago is TLC-identical but offers nothing better. That combination is dive's positioning; see ADR 0006.

## Performance (measured)

**Setup.**
- **Machine.** Apple M3 (8 cores, 24 GB), Docker Desktop VM linux/aarch64.
- **Container.** A single container, `--cpus 8 --memory 7g --memory-swap 7g --network none`, image `divecomp-run:1` (`rust:1.99-bookworm` plus GNU `time`).
- **Timing and memory.** Each run is wrapped in `/usr/bin/time -v`. Wall time comes from its "Elapsed" line, or from a Python `perf_counter` for the startup row, since GNU time rounds to 10 ms. Peak RSS is GNU time's maximum resident set.
- **TLC flags.** TLC ran with Examples CI's flags (`-XX:+UseParallelGC`) and its default heap (a quarter of container memory).
- **tla-mc compiled.** The compiled rows (`-compile`) used a cache warmed beforehand, so build time is excluded. The cold build-plus-run times are listed separately.

**Plan.**
- **Specs.** 8 specs:
  - a trivial 1-state spec for startup;
  - `CoffeeCan1000Beans`, a state-space hog with simple arithmetic actions;
  - `MCBakery`, function-heavy;
  - `MCbtree`, set- and function-heavy;
  - `MCLamportMutex`, sequence-heavy: message channels as sequences, 724k states;
  - `ElevatorLivenessMedium`, liveness;
  - `GameOfLife`, with heavy per-state evaluation;
  - `SimpleRegular`, so that single-threaded tla-rs has a data point.
- **Workers.** 1, 2, 4 and 8 for every tool that handled the spec correctly in the conformance run. tlago ran at 8 workers only, because it is roughly 10–20× slower. tla2 and tlaplusplus ran at 8 workers only on specs where they are very slow.
- **Repetitions.** 3 reps, **interleaved**: rep → spec → tool → workers. Every run's verdict and distinct-state count were checked; every timed run below was correct.
- **Commands.** The driver (a throwaway Python script, not committed) uses exactly the per-tool invocations from the conformance table, with the worker count substituted.

**Timing and caveats.**
- **When.** Session 2026-10-08, approximately 14:22–16:48 UTC. Rep 1 ran ≈14:22–15:17, rep 2 ≈15:17–16:12 and rep 3 ≈16:12–16:48.
- **Contention.** The machine was quiet at the start. Other agents' work resumed at about **16:35 UTC**, so the tail of rep 3 (`ElevatorLivenessMedium`, `GameOfLife`, `SimpleRegular`) may be contended.
- **Noise.** Rep 2 also shows a slowdown on `MCbtree`, `MCLamportMutex` and `ElevatorLivenessMedium` for *all* tools; TLC `MCbtree` w=1 took 28.7 s, 65.1 s and 46.6 s across the three reps. That pattern points to host noise, not to any one tool.
- **How to read the tables.**
  - Absolute numbers are medians, with min–max in parentheses.
  - Comparisons use **ratios within the same rep**. The median of the three within-rep ratios is in the last table, and it is far more stable than the absolute times.
  - ⚠ marks points where either side's spread exceeds 1.3×.
  - Rep 1 is the quietest and agrees with the within-rep ratios.
- **Re-measure on a quiet machine:**
  - `MCbtree`, all tools;
  - `MCLamportMutex` TLC at w=4 and w=8, where TLC got slower with more workers (likely GC under the default 1.75 GB heap as well as noise);
  - `ElevatorLivenessMedium` TLC at w=1;
  - tla2 and tlaplusplus at w=8, which have very wide spreads.

To reproduce, build the binaries as in "Method", copy them into one directory, then run `dbench.sh bench-warm.json bench-warm.jsonl 1` followed by `dbench.sh bench-plan.json bench.jsonl 3`. `dbench.sh` is a `docker run` with the limits above that mounts the corpus, the probes, the oracle and the binaries read-only. The plan files list spec, model, tools and worker counts exactly as above.

### Startup (trivial 1-state spec, 1 worker; median of 3, min–max)

| Tool | wall s | peak RSS MB |
|---|---|---|
| tlc | 0.335 (0.277–0.344) | 102 |
| tla-mc | 0.002 (0.002–0.003) | 19 |
| tla2 | 0.017 (0.015–0.024) | 28 |
| tlaplusplus | 1.016 (1.016–1.024) | 62 |
| tlago | 0.083 (0.078–0.092) | 89 |
| tla-rs | 0.004 (0.002–0.006) | 3 |

### CoffeeCan1000Beans (501,500 distinct states)

| Tool | w=1 | w=2 | w=4 | w=8 | states/s at best | peak RSS MB (w=8 or max w) | speed-up 1→8 |
|---|---|---|---|---|---|---|---|
| tlc | 22.18 (20.51–23.01) | 14.02 (13.08–14.95) | 9.34 (9.22–11.37) | 9.27 (9.02–10.50) | 54,099 | 1287 | 2.39× |
| tla-mc | 4.26 (4.25–4.50) | 3.23 (3.23–3.42) | 2.69 (2.67–2.88) | 2.71 (2.55–2.80) | 186,431 | 277 | 1.57× |
| tla-mc-compiled | 3.70 (3.47–4.19) | 2.93 (2.76–3.88) | 2.56 (2.37–3.12) | 2.74 (2.28–2.87) | 195,898 | 267 | 1.35× |
| tla2 | — | — | — | 75.54 (38.49–81.70) | 6,639 | 4112 |  |
| tlaplusplus | 57.19 (56.29–64.57) | 24.79 (20.35–30.46) | 16.25 (14.18–16.45) | 173.81 (99.81–269.44) | 30,862 | 4396 | 0.33× |

### MCBakery (655,200 distinct states)

| Tool | w=1 | w=2 | w=4 | w=8 | states/s at best | peak RSS MB (w=8 or max w) | speed-up 1→8 |
|---|---|---|---|---|---|---|---|
| tlc | 11.68 (7.59–12.45) | 9.27 (6.26–10.50) | 7.86 (5.81–10.29) | 8.13 (5.88–11.38) | 83,359 | 839 | 1.44× |
| tla-mc | 10.04 (8.09–10.80) | 10.09 (7.47–10.38) | 8.62 (8.20–10.11) | 8.44 (8.35–9.91) | 77,630 | 232 | 1.19× |
| tla-mc-compiled | 4.58 (4.43–4.71) | 3.76 (3.16–4.02) | 3.78 (2.79–3.88) | 3.69 (2.54–4.27) | 177,561 | 232 | 1.24× |
| tla2 | 32.38 (31.34–72.51) | 16.44 (16.22–44.61) | 15.23 (15.15–40.40) | 15.50 (15.35–37.71) | 43,020 | 541 | 2.09× |
| tlaplusplus | 215.73 (163.16–286.39) | 202.59 (94.73–248.16) | 124.72 (114.44–197.02) | 199.37 (116.04–265.04) | 5,253 | 4622 | 1.08× |
| tlago | — | — | — | 119.48 (95.70–248.44) | 5,484 | 918 |  |

### MCbtree (374,727 distinct states)

| Tool | w=1 | w=2 | w=4 | w=8 | states/s at best | peak RSS MB (w=8 or max w) | speed-up 1→8 |
|---|---|---|---|---|---|---|---|
| tlc | 46.58 (28.69–65.05) | 26.36 (15.16–42.55) | 22.73 (8.99–35.41) | 23.37 (8.90–32.63) | 16,486 | 859 | 1.99× |
| tla-mc | 27.17 (17.44–41.15) | 15.62 (9.29–24.17) | 13.62 (5.64–17.54) | 12.19 (5.61–12.97) | 30,741 | 173 | 2.23× |
| tla-mc-compiled | 23.13 (11.28–27.08) | 14.00 (6.07–16.18) | 7.10 (3.64–10.91) | 4.41 (2.93–6.76) | 84,972 | 179 | 5.24× |
| tlaplusplus | — | — | — | 167.77 (131.93–273.26) | 2,234 | 2232 |  |

### MCLamportMutex (724,274 distinct states)

| Tool | w=1 | w=2 | w=4 | w=8 | states/s at best | peak RSS MB (w=8 or max w) | speed-up 1→8 |
|---|---|---|---|---|---|---|---|
| tlc | 12.88 (12.66–28.21) | 9.41 (6.62–20.78) | 8.44 (8.00–19.53) | 14.50 (9.13–20.66) | 85,814 | 883 | 0.89× |
| tla-mc | 16.96 (13.36–26.78) | 9.07 (8.07–16.51) | 5.81 (5.78–12.55) | 4.93 (4.72–8.39) | 146,912 | 85 | 3.44× |
| tla-mc-compiled | 7.60 (6.67–12.36) | 3.94 (3.73–7.11) | 2.60 (2.54–4.65) | 2.09 (2.01–3.83) | 346,543 | 87 | 3.64× |
| tla2 | 82.12 (76.96–111.74) | 36.26 (29.44–46.60) | 33.12 (26.06–46.81) | 30.29 (22.14–42.95) | 23,911 | 2724 | 2.71× |

### ElevatorLivenessMedium (4,122 distinct states)

| Tool | w=1 | w=2 | w=4 | w=8 | states/s at best | peak RSS MB (w=8 or max w) | speed-up 1→8 |
|---|---|---|---|---|---|---|---|
| tlc | 16.74 (9.86–19.20) | 7.03 (4.98–11.42) | 6.65 (5.62–10.22) | 5.85 (5.71–10.90) | 705 | 738 | 2.86× |
| tla-mc | 0.24 (0.19–0.24) | 0.15 (0.11–0.16) | 0.11 (0.07–0.13) | 0.14 (0.07–0.16) | 37,473 | 45 | 1.71× |
| tla2 | 6.17 (5.24–8.35) | 19.67 (18.76–22.89) | 19.93 (16.10–23.38) | 19.19 (16.34–24.12) | 668 | 398 | 0.32× |
| tlaplusplus | 3.06 (3.05–4.06) | 3.06 (3.06–3.07) | 2.50 (2.29–3.07) | 3.07 (3.04–5.03) | 1,649 | 395 | 1.00× |
| tlago | — | — | — | 166.17 (116.98–178.15) | 25 | 281 |  |

### GameOfLife (65,536 distinct states)

| Tool | w=1 | w=2 | w=4 | w=8 | states/s at best | peak RSS MB (w=8 or max w) | speed-up 1→8 |
|---|---|---|---|---|---|---|---|
| tlc | 9.03 (7.23–9.83) | 5.15 (4.18–8.36) | 3.02 (2.75–5.85) | 2.61 (2.26–3.44) | 25,110 | 737 | 3.46× |
| tla-mc | 30.02 (29.39–38.11) | 17.04 (15.27–21.58) | 11.43 (9.06–13.85) | 9.95 (7.90–11.08) | 6,587 | 102 | 3.02× |
| tla2 | — | — | — | 27.25 (21.22–28.69) | 2,405 | 840 |  |
| tlaplusplus | 69.51 (58.48–71.62) | 47.34 (46.54–60.48) | 31.48 (28.49–49.63) | 50.05 (46.33–51.78) | 2,082 | 1978 | 1.39× |

### SimpleRegular (277,726 distinct states)

| Tool | w=1 | w=2 | w=4 | w=8 | states/s at best | peak RSS MB (w=8 or max w) | speed-up 1→8 |
|---|---|---|---|---|---|---|---|
| tlc | 3.52 (3.40–6.74) | — | — | 3.98 (2.29–4.05) | 78,899 | 752 | 0.88× |
| tla-mc | 7.57 (4.93–7.76) | — | — | 1.68 (1.04–2.05) | 165,313 | 75 | 4.51× |
| tla2 | 10.29 (9.89–12.06) | — | — | 4.19 (4.14–4.59) | 66,283 | 429 | 2.46× |
| tla-rs | 23.43 (18.39–28.57) | — | — | — | 11,853 | 420 |  |

### Speed relative to TLC (median over reps of TLC wall / tool wall in the same rep; >1 means faster than TLC; ⚠ = either side has max/min spread > 1.3)

| Spec | Tool | w=1 | w=8 |
|---|---|---|---|
| CoffeeCan1000Beans | tla-mc | 4.93× | 3.64× |
| CoffeeCan1000Beans | tla-mc-compiled | 5.99× | 3.83× |
| CoffeeCan1000Beans | tla2 | — | 0.14× ⚠ |
| CoffeeCan1000Beans | tlaplusplus | 0.36× | 0.06× ⚠ |
| MCBakery | tla-mc | 1.15× ⚠ | 0.82× ⚠ |
| MCBakery | tla-mc-compiled | 2.55× ⚠ | 2.31× ⚠ |
| MCBakery | tla2 | 0.23× ⚠ | 0.38× ⚠ |
| MCBakery | tlaplusplus | 0.05× ⚠ | 0.04× ⚠ |
| MCBakery | tlago | — | 0.06× ⚠ |
| MCbtree | tla-mc | 1.65× ⚠ | 1.92× ⚠ |
| MCbtree | tla-mc-compiled | 2.40× ⚠ | 4.83× ⚠ |
| MCbtree | tlaplusplus | — | 0.12× ⚠ |
| MCLamportMutex | tla-mc | 0.96× ⚠ | 2.46× ⚠ |
| MCLamportMutex | tla-mc-compiled | 1.93× ⚠ | 5.39× ⚠ |
| MCLamportMutex | tla2 | 0.17× ⚠ | 0.48× ⚠ |
| ElevatorLivenessMedium | tla-mc | 80.00× ⚠ | 77.86× ⚠ |
| ElevatorLivenessMedium | tla2 | 2.30× ⚠ | 0.36× ⚠ |
| ElevatorLivenessMedium | tlaplusplus | 4.73× ⚠ | 1.92× ⚠ |
| ElevatorLivenessMedium | tlago | — | 0.05× ⚠ |
| GameOfLife | tla-mc | 0.24× ⚠ | 0.29× ⚠ |
| GameOfLife | tla2 | — | 0.12× ⚠ |
| GameOfLife | tlaplusplus | 0.13× ⚠ | 0.06× ⚠ |
| SimpleRegular | tla-mc | 0.69× ⚠ | 2.20× ⚠ |
| SimpleRegular | tla2 | 0.34× ⚠ | 0.87× ⚠ |
| SimpleRegular | tla-rs | 0.18× ⚠ | — |

### tla-mc compile (cold build + run, 8 workers, single run)

- CoffeeCan1000Beans: 17.6 s total, verdict success, 501500 distinct
- MCBakery: 10.1 s total, verdict success, 655200 distinct
- MCbtree: 9.9 s total, verdict success, 374727 distinct
- MCLamportMutex: 7.3 s total, verdict success, 724274 distinct

**Measured against claimed:**
- **tla-mc: claimed ~4.0× TLC compiled and ~1.9× interpreted at 8 workers** (geometric mean, Apple M1, 7 models). On the 4 models we share with its table, the within-rep medians at 8 workers are:

  | Model | compiled | interpreted |
  |---|---|---|
  | CoffeeCan | 3.8× | 3.6× |
  | Bakery | 2.3× | 0.8× |
  | btree | 4.8× | 1.9× |
  | LamportMutex | 5.4× | 2.5× |
  | **geometric mean** | **≈3.9×** | **≈1.9×** |

  **The claim reproduces.** But it is not uniform:
  - tla-mc's interpreter is **3–4× slower than TLC on `GameOfLife`** (measured), which is not in its table;
  - on liveness (`ElevatorLivenessMedium`) it is ~78× faster, because TLC's liveness checking dominates there;
  - compiling costs 7–18 s per spec cold (measured), so it pays off only for runs longer than that.
- **tla2: its `docs/next-steps.md` admits a "2-8x sequential performance gap vs TLC".** Measured within-rep: 0.17–0.34× TLC at 1 worker on Bakery, LamportMutex and SimpleRegular, and 0.12–0.87× at 8 workers. That is consistent with the admission. Its single-worker liveness run (Elevator) is 2.3× faster than TLC, but its parallel liveness is 3× *slower* than its own single-worker run. Peak RSS reaches 4.1 GB on `CoffeeCan1000Beans`.
- **tlaplusplus: claims "10.7× faster than TLC on many-core systems".** Measured on 8 cores: 0.04–0.36× TLC on every safety spec, 1.9–4.7× on the liveness spec. Its 8-worker runs are often *slower* than its 4-worker runs, it uses up to 4.6 GB, and it has about 1 s of fixed startup. The many-core claim cannot be tested on this machine. It is not supported at 8 cores.
- **tlago** (no performance claim): about 17–20× slower than TLC at 8 workers, with 25–166 s runs where TLC needs 5–8 s.
- **tla-rs** (no performance claim): single-threaded, about 5× slower than single-worker TLC on `SimpleRegular`. It has the smallest startup footprint (3 MB RSS).
- **Startup:**
  - tla-mc 2 ms and tla-rs 4 ms;
  - tla2 17 ms;
  - tlago 83 ms;
  - TLC 335 ms (JVM plus SANY, 102 MB RSS);
  - tlaplusplus 1,016 ms (fixed initialisation).
- **Memory.** On the largest safety spec, `CoffeeCan1000Beans` at 8 workers, peak RSS was:

  | Tool | Peak RSS |
  |---|---|
  | tla-mc | 277 MB |
  | TLC | 1.3 GB |
  | tla2 | 4.1 GB |
  | tlaplusplus | 4.4 GB |

  tla-mc keeps an 80–280 MB footprint throughout, 3–10× less than TLC.
- **Scaling 1→8 workers** (median walls):

  | Tool | Scaling |
  |---|---|
  | TLC | 0.9–3.5× |
  | tla-mc interpreted | 1.2–4.5× |
  | tla-mc compiled | 1.2–5.2× |
  | tla2 | 0.3–2.7× |
  | tlaplusplus | 0.3–1.4× |

  None scales near-linearly on these spec sizes (0.4–0.7 M states), so the 8-core numbers are dominated by fixed costs and contention.

## Rust ergonomics and API (observed, with measured samples)

| | tla-mc | tla-rs | tlaplusplus | tlago | tla2 |
|---|---|---|---|---|---|
| **Library API** | `lib.rs` exposes every module as `pub`, and the lib sets mimalloc as the global allocator (it leaks into embedders). `check::Checker::run() -> Outcome` exists but is not designed for embedding. Usable from `cargo test` only by shelling out, as its own tests do | `tla_checker::checker::check(&Spec, &Env, &CheckerConfig) -> CheckResult`, after `parser::parse`, `modules::merge_extended_declarations` and `config::apply_config`. Usable from `cargo test` (its `tests/oracle.rs` does). Behaviour depends on thread-local and global switches (`set_symbolic_integers`, enum caps, RNG) | `lib.rs` is re-exports, shaped for the CLI. No stable API | Go. `tlago.ModelCheck` is a toy bounded checker; the real checker sits behind process-global `tlc.Globals`, so it is not re-entrant | `tla_check::check_module(&Module, &Config) -> CheckResult` plus parallel, adaptive and portfolio variants and a trace-validation API. Usable from `cargo test`. Uses `panic = "unwind"` and `catch_unwind` in production |
| **Structured output** | None | `--json`: `status`, `stats{states_explored, transitions, max_depth, …}`, 22 status strings, no version | None (Rust `Debug` dumps in traces) | `-tool`, `-dumpTrace json\|tla\|dot` | JSON v1.3: `input`, `specification`, `soundness{mode, deviations}`, `completeness`, `result{status, error_type, error_code, suggestion}`, `counterexample{states[{variables, diff_from_previous, action}]}`, `statistics` |
| **Counterexamples as data** | No; printed (`Failure::Liveness` carries a `String`) | Yes: `Counterexample{trace: Vec<State>, actions}` | Internally only | JSON trace dump | Yes: `Trace{states}`. But empty in the JSON for the DieHard violation at 2 workers, and absent at 8 workers (measured) |
| **Diagnostics** (samples above, M1 and M2) | `line L col C: msg`, no file, no code | rustc-style with spans and notes, no stable codes; a parse failure in an operator is only a warning | anyhow strings; malformed specs hang | SANY-identical text and codes | ariadne spans (human); JSON loses the location |
| **MSRV / edition** | 1.88 / 2024 | none / 2024 | none / 2024 | Go 1.26 | 1.75 claimed (unverified; built with 1.93) / 2021 |
| **Dependencies** (measured, `cargo tree` unique) | 3 normal (mimalloc) | 137 normal, 181 all; no `Cargo.lock` committed | 326 normal, 363 all (AWS S3 SDK, tokio, sled in the default build) | 6 Go modules | 257 normal for `tla-cli`, 1,188 in the workspace lock, all vendored as `tla-*` forks |
| **Release build** (measured, Docker, 8 CPUs unless noted) | 12 s, 1.7 MB (`target-cpu=native`, fat LTO) | 54 s, 2.2 MB (`tla`) + 3.1 MB (`tla-mcp`) | 271 s, 33 MB | 32 s, 27.6 MB | Fails as published (404 git deps). After our patch (dropping 7 members), with the no-LTO `release-canary` profile at 4 CPUs: ~415 s total; the first attempt was OOM-killed at link under 3.2 GB. 27.6 MB |
| **`unsafe`** (observed) | 2 (fd limit) | 1 | — | n/a | ~950 across own crates (JIT and LLVM2 most) |

## Architecture and code quality (observed)

### tla2
- **Parser and IR.** A logos lexer and hand-written recursive descent build a rowan lossless syntax tree. That lowers to an AST, then to a typed IR (`tla-tir`), and on to a bytecode VM ("default ON with interpreter fallback") and a Cranelift JIT. Its own `docs/next-steps.md` says the JIT is a "net negative" with a "0% dispatch hit rate".
- **Values.** An enum with `SmallInt(i64)` that overflows into `BigInt`, `Arc` sorted sets, and about 15 lazy set kinds.
- **Fingerprints.** TLC's FP64 table exists but is used only for the `TLCFP` operator. State deduplication uses salted XOR, FNV and xxh3, so it is not TLC-compatible.
- **Store and parallelism.** A CAS-sharded fingerprint set, mmap and disk tiers, and a Bloom pre-check with an exact fallback. Parallel work-stealing with first-violation-stops.
- **Liveness.** A tableau plus Tarjan, with an on-the-fly mode.
- **Error handling.** Guard evaluation errors are suppressed and counted (`statistics.suppressed_guard_errors`), with a stderr warning that the "state count may be incorrect". The run is not refused.
- **Tests.** About 8,200 `#[test]` items, a differential TLC comparison script, fuzz targets and Kani harnesses.
- **The "160/160" claim** (`docs/next-steps.md`, 2026-04-02) rests on `tests/tlc_comparison/spec_baseline.json`. There, `verified_match: true` is set on 223 of 224 entries, including 25 whose statuses differ. Strict agreement in that file is 194/205.
- **Admissions in `docs/next-steps.md`:**
  - "Close the 2-8x sequential performance gap vs TLC";
  - "MCNanoMedium … 10x slower than TLC";
  - blocked "#3893: EXISTS ctx.clone() — mark/restore unsound with LET".
- **AI generation and maintainability.** Strong signals: about 3,000 "Part of #NNNN" comments, 112 "Wave N" markers, an `agent` build profile, 130 CLI subcommands, scope reaching hardware model checking and Petri nets, and a squashed history. The upstream is gone, so maintenance risk is very high.
- **Licence.** Apache-2.0, so ideas can be borrowed with attribution.

### tla-rs
- **Design.** A hand-written parser whose AST is evaluated directly; there is no IR and no SANY-equivalent analysis. `Value` is an enum with `Int(i64)` (checked) and `BTreeSet`/`BTreeMap` collections.
- **Store.** Exact: full states in an `IndexSet`. Single-threaded.
- **Liveness.** A tableau plus SCC.
- **Approximations by design** (observed, all documented somewhere but none refused):
  - `Nat` is `0..100` and `Int` is `-100..100` unless `--symbolic-integers` is given.
  - `ACTION_CONSTRAINT`, `ALIAS` and `POSTCONDITION` are ignored with a warning.
  - Definitions named `Inv*`/`TypeOK*` are auto-checked when the cfg has no `INVARIANT`.
  - A definition that fails to parse becomes `Expr::Unparsed` and errors only if evaluated.
  - An unresolvable `EXTENDS` is silently skipped.
- **Tests.** A liveness-only differential oracle against TLC (236 cases); no safety or state-count differential.
- **Positioning.** Honest that it is "a lightweight alternative … for specs that fit its supported subset". It does not claim parity.
- **Licence.** MIT or Apache-2.0 (per `Cargo.toml`).

### tla-mc
- **Design.** A hand-written lexer and parser with SANY-like junction layout. Proofs are skipped textually, and there is no level checking. Modules are flattened; `INSTANCE` is expanded by renaming. The AST is resolved to an `Expr` IR.
- **Engines.** Three over one BFS: interpreter, closures, and generated Rust (`-compile` writes a crate, builds it with `cargo build --release` and `target-cpu=native`, caches it, then `exec`s it).
- **Values.** Interned strings and model values in **TLC's UniqueString token order**, sorted `Arc<[Value]>` sets, and lazy sets.
- **Fingerprints.** A wyhash-style mix plus fmix64, not FP64. No exact mode and no collision bound.
- **Store.** A sharded open-addressing fingerprint set that spills to sorted files with Bloom filters and a sparse index. The level queue also spills. Checkpoint and resume refuse a changed spec.
- **Parallelism.** Level-synchronous BFS with scoped threads.
- **Symmetry.** Simulates TLC's `Hashtable` variable order to choose the same representatives as TLC.
- **Liveness.** No tableau. Fair-SCC search (Emerson-Lei for SF) over a disk-backed graph, with fixed accepted shapes and refusal of everything else.
- **Integers.** Plain `i64` (`+ - *`), with no 32-bit overflow error.
- **Tests.** Python sweeps against Examples manifests and flint's TLC gate, plus cross-engine agreement. Only 7 `#[test]`.
- **AI generation and maintainability.** 26 of 27 commit messages carry `Co-Authored-By: Claude Opus 5.5`. One author.
- **Licence.** MIT.

### tlago
- **Approach.** A mechanical, file-by-file Go port of SANY and TLC pinned to a tlaplus commit: a JavaCC grammar port, SANY's Generator and level checking, and TLC's `Value` hierarchy.
- **Values and fingerprints.** `IntValue{int32}` with TLC's overflow error. FP64 with the same polynomial table, random index by default.
- **Store, parallelism and liveness.** Ports of the FPSet family, disk queue, `Worker`, the liveness tableau and the Java override modules (Json, Graphs, CSV, IOUtils, Bitwise, …). Also distributed TLC over TCP.
- **Tests.** Ports of TLC and SANY JUnit suites, plus SANY-XML and Apalache-IR goldens. No state-count sweep against Examples (observed).
- **AI generation and maintainability.** 1,723 commits from one author in 11 days, with an `AGENTS.md` of directives to an agent. Heavily agent-driven.
- **Embeddability.** The port inherits the JVM's global mutable state, so it is not embeddable.
- **Licence.** MIT, ported from MIT code.

### tlaplusplus (ideas only, GPL-3.0)
- **Front end.** Module structure is recovered with line-oriented heuristics; there is a separate expression parser and a text "splitter" for action disjuncts. Lambdas store source text. There is no lossless syntax tree and no semantic analysis. Several of its own recorded soundness fixes trace back to this design.
- **Evaluators.** Two evaluators (interpreted and compiled closures) diverged; they added a shadow-compare mode and later unified them.
- **Fingerprints.** 64-bit ahash with fixed seeds.
- **Store.** Lock-free sharded and NUMA-aware, with a Bloom auto-switch on by default and zstd-compressed queue spill.
- **Operations.** S3 checkpointing and distributed exploration with Bloom-based dedup.
- **Error handling.** Swallowed evaluation errors are counted, but the run still reports success.
- **Tests.** Chaos failpoints with swarm mode, cargo-mutants audits, 8 fuzz targets, and state-graph digest snapshots.
- **Claims.** "182/182 analysis, 174/182 full model checking" against Examples. Its TLC differential gate covers 13 specs.
- **AI generation and maintainability.** A 543-line `CLAUDE.md`, 14 `RELEASE_*` files, and task-ID commits. Breadth has outrun the semantic foundations.

## Where dive can win

Prioritised within each axis.

**Soundness and conformance**
1. **A refusal verdict is first-class.** It gets a distinct exit code, a JSON `verdict: "unsupported"` naming the feature, and never shares an exit code with a violation. Every competitor except tlago mixes refusals, evaluation errors and violations in some channel (tla-mc exit 12, tla2 text, tlaplusplus exit 1, tla-rs exit 1).
2. **Integers: better by default, TLC on request, never silent.** The approved default is big integers, and `--int=tlc` (part of `--compat=tlc`) reproduces TLC's overflow error, so P1 matches there. When a default-profile result would differ under `--compat=tlc`, as an overflow past 2^31 would, the output says so. Four of five competitors widen integers silently, with no TLC mode and no flag in the output.
3. **TLC CHOOSE order under `--compat=tlc`, including string interning order** (P2 and P7). `--choose-order=canonical` gives a text-independent order and restores metamorphic invariance; whether it becomes the default is open question 8. Only tla-mc, tlago and tla2 match TLC here; tla-rs and tlaplusplus diverge silently.
4. **Never swallow evaluation errors.** tlaplusplus and tla2 count and continue; dive must stop with an error verdict, as TLC does.
5. **A cfg directive dive does not implement is a refusal, never a warning.** tla-rs ignores `ACTION_CONSTRAINT`, `ALIAS` and `POSTCONDITION`; tla-mc silently drops `ALIAS` and `POSTCONDITION`.
6. **SYMMETRY with liveness:** refuse by default with a diagnostic (owner-approved), and `--symmetry-liveness=tlc` warns and proceeds exactly as TLC does. tla-mc's false alarm on P4 and tla-rs's silently dropped symmetry show the cost of getting this wrong.
7. **Run the full corpus.** The bar to clear is tla-mc's 99/140 with zero wrong and tlago's 110/140 with zero wrong; both are measured here.

**Determinism**
1. **Deterministic parallel BFS** (level-synchronous with a deterministic merge), so counts at a violation and the chosen counterexample are identical at 1 and N workers. TLC itself varies (P8: 64,416 to 67,497 states at 8 workers). No competitor offers this. tla-mc is the closest: its traces stay shortest, but its counts vary.
2. **Seeded and replayable randomness.** Default: TLC's semantics and `-seed`. Option: a dive seed that is recorded in the output and replayable. Competitors either refuse or silently fix the value.

**Performance**
1. **Startup.** Match tla-mc and tla-rs (2–4 ms measured) rather than TLC (335 ms) or tlaplusplus (1 s).
2. **Throughput.** Interpreted dive should beat TLC on *every* spec, which tla-mc's interpreter does not (`GameOfLife` 0.24–0.29×). A compiled tier (`dive-compile`) is how to beat TLC by the margins tla-mc shows (≈3.9×). See the targets in ADR 0006.
3. **Memory.** Compact fingerprint sets and spill. tla-mc's 80–280 MB against TLC's 0.7–1.3 GB shows the headroom; tla2 and tlaplusplus reach 4+ GB.
4. **Parallel scaling.** Better than TLC's measured 0.9–3.5× from 1 to 8 workers, with results identical at every worker count. No competitor offers that combination.

**Ergonomics**
1. A small, curated `dive-api` with no global state, re-entrant, and no global allocator imposed on embedders. Counterexamples as typed data.
2. A versioned JSON schema that always carries the counterexample, the mode (exact or fingerprint) and the collision bound. tla2's schema is the best model to beat, but it drops traces in parallel mode.
3. Diagnostics with file, line, column, a stable code and a fix hint in *both* human and JSON output. tla2 loses locations in JSON; tla-mc omits file names.
4. A light dependency footprint: tla-mc proves a useful checker can have 3 dependencies, while tla2 needs about 1,200 vendored crates.

**Architecture**
1. Keep the soundness-critical core small and auditable. The competitors with the most code (tla2 at about 585k lines, tlaplusplus at about 110k) have the most silent failures.
2. Use one evaluator semantics with differential checks between tiers from day one (interpreter against `dive-compile`).

## Ideas worth borrowing

| Idea | Seen in | Licence | Status for dive |
|---|---|---|---|
| Compile the spec to Rust, build with cargo, cache by content hash, `exec` the result; keep the interpreter for short runs | tla-mc (`src/codegen.rs`) | MIT | Adopt the idea for `dive-compile`; independent implementation; credit in `docs/provenance.md` if code is consulted |
| Intern strings and model values in TLC's UniqueString order so CHOOSE and set order match TLC | tla-mc (`value.rs`), tlago (port) | MIT | Adopt; derives from TLC's `UniqueString` (MIT). Record TLC as the provenance |
| Simulate TLC's `Hashtable` variable order to pick TLC's symmetry representatives | tla-mc (`varorder.rs`) | MIT | Consider for SYMMETRY count conformance; provenance is TLC behaviour |
| Level-synchronous parallel BFS, workers decoding successors privately | tla-mc (`check.rs`) | MIT | Adopt as the basis for deterministic parallel BFS |
| Fingerprint set that spills to sorted runs with Bloom and sparse index; checkpoint at level boundaries; refuse resume on a changed spec | tla-mc (`store.rs`) | MIT | Adopt the design for `dive-store` |
| Liveness as fair-SCC search (Emerson-Lei for SF) on a disk-backed graph, refusing unsupported shapes | tla-mc (`liveness.rs`) | MIT | Consider as a fast path beside a full tableau; refusal behaviour adopted |
| JSON output with explicit `soundness` (mode, deviations) and `completeness` blocks | tla2 (`json_output/types.rs`) | Apache-2.0 | Adopt the concept in dive's schema, plus collision bound and refusal verdict |
| `--require-exhaustive`, `--allow-incomplete`, `--store-states` exact mode flags | tla2 | Apache-2.0 | Adopt the concept (charter's exact mode) |
| Counting suppressed errors in statistics | tla2, tlaplusplus | Apache-2.0 / GPL (idea only) | **Reject** the behaviour; dive refuses instead |
| Port TLC's `-tool` message framing and `-dumpTrace json` | tlago | MIT | Consider `--output tlc-tool` for drop-in tooling |
| SANY-identical diagnostics text as a compatibility mode | tlago | MIT | Consider as an output mode; dive's own diagnostics stay richer |
| Shadow-compare two evaluators and log divergences | tlaplusplus | GPL-3.0, idea only | Adopt the technique (independent; published practice) |
| Swarm chaos testing of the store and spill layer: random failpoints; final counts must equal a control run | tlaplusplus | GPL-3.0, idea only | Adopt the technique (Groce and Regehr swarm testing; independent implementation) |
| State-graph digest snapshot tests | tlaplusplus | GPL-3.0, idea only | Adopt the technique (independent) |
| cargo-mutants audit of evaluator files | tlaplusplus | GPL-3.0, idea only | Adopt (tooling, not code) |
| Compressed in-memory spill tier before disk | tlaplusplus | GPL-3.0, idea only | Consider (generic technique) |
| A degraded store mode (Bloom), only if the verdict says so | tlaplusplus (does it silently) | GPL-3.0, idea only | Adopt only as an explicit, reported mode |
| Annotating "noise variables" not read by the violated invariant in a counterexample | tlaplusplus | GPL-3.0, idea only | Consider as a diagnostic feature (independent) |
| MCP server for agent clients | tla-rs | MIT/Apache-2.0 | Consider later on top of `dive-api` |
| Liveness differential corpus against TLC (236 cases) | tla-rs (`tests/liveness_corpus`) | MIT/Apache-2.0 | Consider importing cases as corpus input (with attribution) after licence check |

No code or text from tlaplusplus was copied. Ideas taken from it are recorded as independent implementations of published techniques.

## Appendix: probe specs

The probe specs are reproduced below. They are imported verbatim as the harness corpus `probes` (`conformance/corpus/probes/`, ADR 0005 amendment 1), with TLC goldens from the gating oracle, per the charter rule on surprising TLC behaviour; `Trivial`, a benchmark, is not. The per-tool invocations above are the harness adapters in `conformance/adapters/`.

### P1Overflow

```tla
---- MODULE P1Overflow ----
\* Probe: integer arithmetic beyond 2^31. TLC's integers are 32-bit and
\* overflow is an evaluation error. A tool with wider integers that reports
\* "no error" here has silently diverged from TLC.
EXTENDS Integers
VARIABLE x
Init == x = 2147483646
Next == x < 2147483648 /\ x' = x + 1
Inv == x > 0
====
```

`P1Overflow.cfg`:

```
INIT Init
NEXT Next
INVARIANT Inv
CHECK_DEADLOCK FALSE
```

### P2Choose

```tla
---- MODULE P2Choose ----
\* Probe: which element does CHOOSE pick? The language only promises that
\* CHOOSE is deterministic; TLC picks the first satisfying element of the set
\* in its internal normalized order. For strings, TLC orders by interning
\* order of the string token, not lexicographically. Each invariant pins one
\* plausible answer; the cfg checks the "lexicographic" answers, so a
\* violation reveals which order the tool uses.
EXTENDS Integers
VARIABLES s, t, u
Init == /\ s = CHOOSE e \in {"zeta", "alpha", "mid"} : TRUE
        /\ t = CHOOSE e \in {{3}, {1, 2}, {}} : e # {}
        /\ u = CHOOSE e \in {<<2, 1>>, <<1, 2, 3>>, <<1, 3>>} : TRUE
Next == UNCHANGED <<s, t, u>>
SLex == s = "alpha"
TSetOrd == t = {3}
USeqOrd == u = <<1, 3>>
====
```

`P2Choose_seq.cfg`:

```
INIT Init
NEXT Next
INVARIANT USeqOrd
```

`P2Choose_set.cfg`:

```
INIT Init
NEXT Next
INVARIANT TSetOrd
```

`P2Choose_str.cfg`:

```
INIT Init
NEXT Next
INVARIANT SLex
```

### P7ChooseIntern

```tla
---- MODULE P7ChooseIntern ----
\* Probe: identical to P2Choose's string case except that an unused
\* definition mentioning "alpha" appears earlier in the module. If CHOOSE
\* follows string interning order, the chosen element changes even though
\* the semantics of Init did not.
EXTENDS Integers
Unused == "alpha"
VARIABLES s
Init == s = CHOOSE e \in {"zeta", "alpha", "mid"} : TRUE
Next == UNCHANGED s
SLex == s = "alpha"
====
```

`P7ChooseIntern.cfg`:

```
INIT Init
NEXT Next
INVARIANT SLex
```

### P3JavaOverride

```tla
---- MODULE P3JavaOverride ----
\* Probe: an operator whose TLA+ definition is a placeholder and which only
\* works through TLC's Java override (CommunityModules Json!ToJson is defined
\* as CHOOSE over STRING). A tool without the override must refuse; evaluating
\* the TLA+ body literally cannot terminate or yields an arbitrary string.
EXTENDS Integers, Sequences, Json
VARIABLE x
Init == x = ToJson(<<1, 2>>)
Next == UNCHANGED x
Inv == x = "[1,2]"
====
```

`P3JavaOverride.cfg`:

```
INIT Init
NEXT Next
INVARIANT Inv
```

### P3ActionConstraint

```tla
---- MODULE P3ActionConstraint ----
\* Probe: a cfg directive some tools do not implement. ACTION_CONSTRAINT prunes
\* the step 4 -> 5, so TLC finds exactly 5 distinct states (0..4). A tool that
\* ignores the directive explores 0..10 (11 states) and still says "no error".
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x < 10 /\ x' = x + 1
Inv == x <= 10
NotFive == x' # 5
====
```

`P3ActionConstraint.cfg`:

```
INIT Init
NEXT Next
INVARIANT Inv
ACTION_CONSTRAINT NotFive
CHECK_DEADLOCK FALSE
```

### P4SymLive

```tla
---- MODULE P4SymLive ----
\* Probe: symmetry reduction combined with liveness. x alternates between the
\* two model values forever under weak fairness, so <>(x = b) holds. Under
\* SYMMETRY the two states collapse into one class, and a checker that checks
\* liveness on the quotient graph can report a spurious violation (TLC warns
\* that this combination is unsound). An honest tool refuses or warns loudly.
EXTENDS TLC
CONSTANTS a, b
VARIABLE x
S == {a, b}
Init == x = a
Next == x' = IF x = a THEN b ELSE a
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Sym == Permutations(S)
EventuallyB == <>(x = b)
====
```

`P4SymLive.cfg`:

```
CONSTANTS a = a
          b = b
SPECIFICATION Spec
SYMMETRY Sym
PROPERTY EventuallyB
```

### P5TLCGetSet

```tla
---- MODULE P5TLCGetSet ----
\* Probe: TLC-module state. TLCGet("level") is the length of the behaviour
\* prefix (1 for initial states), so Inv fails at x = 3. TLCSet/TLCGet(1) keep
\* a per-worker register: ASSUME sets it, and Init reads it back.
EXTENDS Naturals, TLC
ASSUME TLCSet(1, 42)
VARIABLE x
Init == x = 0 /\ TLCGet(1) = 42
Next == x' = (x + 1) % 10
Inv == TLCGet("level") < 4
====
```

`P5TLCGetSet.cfg`:

```
INIT Init
NEXT Next
INVARIANT Inv
```

### P6IntDiv

```tla
---- MODULE P6IntDiv ----
\* Probe: \div and % with negative operands. In TLA+ (Integers), a \div b is
\* floor division and a % b lies in 0..b-1 for b > 0. So -7 \div 2 = -4 and
\* -7 % 2 = 1. Truncating division (Rust's / and %) gives -3 and -1.
EXTENDS Integers
VARIABLE q, r
Init == q = (-7) \div 2 /\ r = (-7) % 2
Next == UNCHANGED <<q, r>>
Inv == q = -4 /\ r = 1
====
```

`P6IntDiv.cfg`:

```
INIT Init
NEXT Next
INVARIANT Inv
```

### P8ParallelDet

```tla
---- MODULE P8ParallelDet ----
\* Probe: determinism across worker counts. Three counters advance by 1 or 2.
\* The bad state a = b = c = 30 is reachable by paths of 45 to 90 steps, so a
\* breadth-first search must report a 46-state trace (45 steps). A search that
\* is only approximately breadth-first can report a longer trace, and the
\* number of states explored before the violation can vary run to run.
EXTENDS Naturals
VARIABLES a, b, c
Init == a = 0 /\ b = 0 /\ c = 0
Next == \/ \E d \in {1, 2} : a + d <= 40 /\ a' = a + d /\ UNCHANGED <<b, c>>
        \/ \E d \in {1, 2} : b + d <= 40 /\ b' = b + d /\ UNCHANGED <<a, c>>
        \/ \E d \in {1, 2} : c + d <= 40 /\ c' = c + d /\ UNCHANGED <<a, b>>
Inv == ~(a = 30 /\ b = 30 /\ c = 30)
====
```

`P8ParallelDet.cfg`:

```
INIT Init
NEXT Next
INVARIANT Inv
CHECK_DEADLOCK FALSE
```

### P9Random

```tla
---- MODULE P9Random ----
\* Probe: RandomElement. TLC picks a pseudo-random element; without -seed the
\* choice differs between runs. The invariant fails in the initial state so
\* every tool prints the value it chose; comparing runs shows whether the
\* choice is seeded, replayable, or silently fixed.
EXTENDS Naturals, TLC
VARIABLE x
Init == x = RandomElement(1..1000000)
Next == UNCHANGED x
Inv == x < 0
====
```

`P9Random.cfg`:

```
INIT Init
NEXT Next
INVARIANT Inv
```

### M1Syntax

```tla
---- MODULE M1Syntax ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1
Inv == x < 5
====
```

`M1Syntax.cfg`:

```
INIT Init
NEXT Next
INVARIANT Inv
```

### M2Undefined

```tla
---- MODULE M2Undefined ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = x + step
Inv == x < 5
====
```

`M2Undefined.cfg`:

```
INIT Init
NEXT Next
INVARIANT Inv
```

### Trivial

```tla
---- MODULE Trivial ----
\* Startup-time benchmark: one state, one invariant.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Inv == x = 0
====
```

`Trivial.cfg`:

```
INIT Init
NEXT Next
INVARIANT Inv
```
