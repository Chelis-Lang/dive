# M0 results

Status: M0 spike on `core/m0-spike` (umbrella issue #498). The core runs all
four spike models under `dive check --engine=core`. Its verdicts, distinct and
generated counts and depths equal the pinned TLC goldens. Timings are from
one noisy Apple M3 and are **indicative only**. The gating measurements are
the x86 runs of `09-performance-method.md`, which the lead runs on quiet bench
nodes. The owner treats the ×TLC targets of `10-plan.md` §2 as directional
aims, not hard gates.

## 1. What was built

| Part | Where | Doc |
|---|---|---|
| Handles, strings, store, arena, hashing, order, builders, cursors | `crates/dive-core/src/{handle,strings,store,arena,node,heap}.rs` | `03-value-model.md` |
| Binder handoff, plan compiler, interpreter | `crates/dive-core/src/{compile,plan,eval,convert}.rs`, `crates/dive-api/src/core_engine.rs` | `04-plan-compiler.md` |
| TLC's `getNextStates` rules used by the spike models | `crates/dive-core/src/generate.rs` | `05-successor-generation.md` |
| Keyed memo tables, parent-delta skip | `crates/dive-core/src/memo.rs`, `crates/dive-explore` | `06-memoization.md` |
| Single-thread BFS with an early seen-set probe | `crates/dive-explore/src/lib.rs` | `07-engine.md` |
| Strategy switches (`DIVE_CORE_OPT`) | `crates/dive-core/src/opts.rs` | `09-performance-method.md` §5 |
| Store microbenchmark | `crates/dive-core/examples/store_scaling.rs` | this page, §5 |

The entry point is `dive check --engine core` (or `DIVE_ENGINE=core`). The
benchmark driver needs no flags with `DIVE_ENGINE=core`. The JSON report keeps
`verdict` and `statistics.{generated,distinct,depth}`, and adds
`"engine": "core"` and a `core` object: the disabled switches, plan statistics,
probe and discard counts, conjunct skips, store and arena statistics, thunk
counts and memo statistics per site. Anything the core does not implement is
an `unsupported` verdict that names it. There is no fallback.

**Deviations from the design**, each listed again in its document's "Next step"
section:

- The core reuses the reference binder's split actions, constants, overrides
  and startup values, instead of the native split of `04-plan-compiler.md` §2.
- Evaluation errors are reported as `unsupported`. Error classes (§5 of
  `04-plan-compiler.md`) are not built.
- There are no big integers (61-bit inline integers only, with overflow
  refused) and no lazy function values (function constructors are
  materialized). Tuple and record literal flags are not part of identity.
- A zero-arity top-level definition of state level is evaluated at most once
  per frame activation. TLC re-evaluates it with the same result.
- `x' \in` a union and `x' \subseteq S` are refused.
- There are no traces and no predecessor log. `--max-memory` is not enforced
  inside the core. `--time-limit` is.
- `-inline-subst` does not apply, because M0 refuses INSTANCE. M0 adds
  `-fold` (constant folding) to the switch list.

## 2. Counts against the goldens

Oracle `tlc-2026.10.06.014338`, one worker, default profile. The CoffeeCan run
uses `dive-bench`'s `CoffeeCan1000BeansSafety` configuration (the golden's
configuration without `PROPERTY`; liveness does not change TLC's counts).

| Model | Verdict | Generated | Distinct | Depth | Golden |
|---|---|---|---|---|---|
| CoffeeCan1000BeansSafety | success | 2,000,002 | 501,500 | 1 | equal |
| SimpleRegular (N = 8) | success | 1,454,776 | 277,726 | 25 | equal |
| MCLamportMutex | success | 2,729,079 | 724,274 | 61 | equal |
| GameOfLife (N = 4, stretch) | success | 131,072 | 65,536 | 1 | equal |

Each count also matched under every strategy switch (§4), except
MCLamportMutex under `-symbolic-member`. There, materializing the
redirected `Nat` is refused as unsupported, which is the sound outcome.
`crates/dive-api/tests/core_golden.rs` checks the four models against the
goldens. It is slow and corpus-bound, so it is excluded from CI's `ci`
profile, and the two largest models are `#[ignore]`d in debug builds. Run them
with:

```sh
cargo nextest run -p dive-api --test core_golden --release --run-ignored all
```

`crates/dive-api/tests/core_engine.rs` (in CI) compares the core with the
reference engine on fixtures that cover records, `EXCEPT` with `@`, sequences,
`SUBSET`, function sets, `\A` in an action, `CASE`, `LET`, recursion,
`CHOOSE`, a constraint, an invariant violation and a deadlock. It does this
under each switch and all switches together, and checks the refusals.

## 3. Timing

Method: release build with fat LTO, one worker, `--seed 0 --fp 0`, wall and
user time from `/usr/bin/time -l`. `uptime` was recorded before each run. TLC
is the pinned `tla2tools.jar` on the pinned JRE, with the harness's flags
(`-XX:+UseParallelGC -Xmx3g -Xss8m`, `-workers 1 -fp 0 -seed 0`). The
reference engine is measured at the same commit.

The host was an Apple M3 (4 performance and 4 efficiency cores, 24 GB). It was
shared with other builds, and the 1-minute load average ranged from 6 to more
than 190 during the campaign. Wall times under load are not comparable
between runs. The table below is a rerun at `11dd6573` (rebased on `main`, so
the reference engine includes the #497 host-lock fix). It has three
interleaved runs per row, at load averages from 5.7 to 16. Raw JSON lines are
kept outside the repository.

| Model | core wall (s) | reference wall (s) | TLC wall (s) | ×TLC | ×reference | RSS core / reference / TLC (MB) |
|---|---|---|---|---|---|---|
| CoffeeCan1000BeansSafety | 0.58 (0.52–1.18) | 1.55 (1.53–1.78) | 1.59 (1.48–1.75) | 2.7 | 2.7 | 219 / 134 / 567 |
| SimpleRegular | 1.54 (1.34–1.59) | 6.61 (5.56–11.11) | 3.54 (3.46–6.83) | 2.3 | 4.3 | 36 / 176 / 1012 |
| MCLamportMutex | 6.50 (6.40–18.80) | 19.59 (19.23–19.75) | 11.42 (11.22–14.01) | 1.8 | 3.0 | 73 / 280 / 1158 |
| GameOfLife | 5.45 (5.30–10.37) | 19.33 (15.42–22.95) | 10.97 (9.40–15.85) | 2.0 | 3.5 | 79 / 64 / 1073 |

Wall times are medians, with the range in parentheses. The ratios are ratios
of medians. The geometric mean is 3.3× the reference engine and 2.2× TLC.
Against the §2 criteria of `10-plan.md`, these are indicative on M3:

- **Kill** (under 2.5× the current dive): not triggered.
- **×TLC aims:**
  - MCLamportMutex 1.8× and SimpleRegular 2.3× (aim 1.5×);
  - CoffeeCan 2.7× (aim 2.5×);
  - GameOfLife 2.0×, so the middle band does not apply.
- **RSS no higher than the current dive:** missed on CoffeeCan and
  GameOfLife (§6).
- **Store ≥6× at 8 threads:** not reached on this host (§5).

### 3.1 Raw campaign

The first campaign ran five interleaved repetitions of core, reference and TLC
for each model. Repetitions 1 to 3 ran at load averages of 22 to 47, and
repetitions 4 and 5 at 6 to 21. Median wall time over all five, in seconds,
at `ad44f504`, before the fix of §6 and on the base before #497:

| Model | core | reference | TLC |
|---|---|---|---|
| CoffeeCan | 2.50 | 6.61 | 6.49 |
| SimpleRegular | 5.73 | 10.46 | 6.64 |
| MCLamportMutex | 25.30 | 56.83 | 14.65 |
| GameOfLife | 5.50 | 15.81 | 11.50 |

Under load, the core and the reference engine lose more wall time than TLC:
their user time stays near their quiet wall time, but their wall time
triples. This is why only the quiet runs are used above.

## 4. Strategy metrics and ablation

Metrics are from the default run (all strategies on).

| Metric | CoffeeCan | SimpleRegular | MCLamportMutex | GameOfLife |
|---|---|---|---|---|
| Successors discarded before any interning (S3) | 1,498,502 of 2,000,002 (74.9%) | 1,177,050 of 1,454,776 (80.9%) | 1,772,077 of 2,729,079 (64.9%) | 65,536 of 131,072 (50.0%) |
| Intern hit rate (store hits / interns) | ≈0% (501,504 inserts) | 86.0% (444,518 / 516,886) | 95.9% (4,211,206 / 4,393,259) | 0.05% |
| Memo lookups, hit rate | 36,864, 0% (all 9 sites off) | 102,409, 13% (25 of 34 sites off) | 27,386,096, 75.8% (65 sites on) | none |
| Conjuncts skipped by parent delta | 0 of 501,500 (initial states) | 632,748 of 2,499,534 (25.3%) | 5,220,581 of 9,428,093 (55.4%) | 0 of 65,536 (initial states) |
| Thunks forced / cached reads | 2,006,000 / 0 | 0 / 0 | 4,035,637 / 1,672,406 | 9,502,720 / 9,371,648 |
| Plan nodes, folded subexpressions | 152, 2 | 1,280, 92 | 3,275, 69 | 149, 23 |

The ablation slowdown is the median user time over three runs with one
switch off, divided by the median with all switches on. **Indicative only.**
The runs overlapped load averages from 6 to more than 190. Several ratios
below 1.0 are noise of that size, not speedups. Rerun on x86.

| Switch off | CoffeeCan | SimpleRegular | MCLamportMutex | GameOfLife |
|---|---|---|---|---|
| (all on: user s) | 0.81 | 1.40 | 9.62 | 7.46 |
| `-delta-hash` | 1.21× | 1.21× | 1.32× | 1.07× |
| `-early-probe` | 1.16× | 1.10× | 0.97× | 0.91× |
| `-hashcons` | 0.68× | 1.27× | 0.96× | 0.74× |
| `-symbolic-member` | 1.27× | 1.49× | unsupported | 0.76× |
| `-memo` | 0.79× | 1.16× | 0.84× | 0.77× |
| `-parent-skip` | 0.74× | 1.17× | 0.87× | 0.83× |
| `-arena` | 0.75× | 1.14× | 0.85× | 0.92× |
| `-eager` | 0.80× | 1.34× | 0.79× | 0.91× |
| `-fold` | 0.79× | 1.81× | 1.19× | 1.09× |

Memory effects are more robust than times. `-hashcons` raises peak RSS from
69 MB to 467 MB on MCLamportMutex and from 36 MB to 89 MB on SimpleRegular.
`-memo` lowers it from 69 MB to 49 MB on MCLamportMutex (the memo tables).
`-fold` raises CoffeeCan from 268 MB to 355 MB.

Observations to confirm on x86:
- Hash-consing (S4) is the largest memory win where states share structure.
- Delta hashing (S1, S2) and constant folding are consistent time wins.
- The memo tables did not pay for themselves at M0's table size. On
  MCLamportMutex, `Memo::get` is the top self-time entry, even with a 76% hit
  rate. Their size and keys are an M1 item (`06-memoization.md`).

## 5. Store scaling

`store_scaling` interns 1,000,000 values per thread into one shared store: a
4-element set inside a 3-tuple. Half of each thread's values are shared with
every other thread (hits), and half are private (inserts). Three runs,
LTO release, load average about 14:

| Threads | Speedup over 1 thread (three runs) |
|---|---|
| 2 | 1.99, 1.57, 1.76 |
| 4 | 3.57, 3.16, 3.17 |
| 8 | 4.83, 4.22, 3.23 |

The ≥6× at 8 threads target cannot be reached on this host, which has 4
performance and 4 efficiency cores and was under load. Rerun on the x86 bench
nodes.

## 6. Watched risks

These are recorded in `10-plan.md` §4 as R9 and R10.

**R9: CoffeeCan's plan compile time.** Compile time is 0.19–0.43 s of a
0.6 s run. The cause is constant folding of the initial predicate's set
`{c \in Can : c.black + c.white \in 1..MaxBeanCount}`. The compiler enumerates
the 1,002,001 candidate records and interns the 501,500 survivors and the
set. This is Init's own work moved to compile time, not overhead: with
`-fold`, the same work runs in Init instead, and the total time is about the
same. A plan-compile budget belongs to M1. One option is to fold only below a
size bound and stream larger filters, as the generator already does for
unfolded filters.

**R10: CoffeeCan's peak RSS.** The core used 268 MB against 126 MB for the
reference engine. There were two causes:
1. A materialization cache (fixed at `11dd6573`). Normalizing a persistent
   symbolic set interned its whole enumeration, so the 1,002,001 candidate
   records were interned instead of only the 501,500 survivors. After the
   fix the store holds 501,504 nodes and the run peaks at 220 MB.
2. Folding transients (open). While folding, the compile arena holds all
   1,002,001 candidate records, because a bump arena cannot free rejected
   ones. Interned records then pay a `Box` per node in the store. M1 options
   are to stream the fold into the store with an arena rollback for rejected
   candidates (with a rule that no slot written since the mark survives), and
   to store node data inline in the store's chunks.

GameOfLife also peaks higher on the core (79 MB against 64 MB). This is the
arena during the 65,536-state initial enumeration, which is not reset until
Init is complete.

## 7. Gaps and risks for M1

- Lazy function values: GameOfLife evaluates `sc` and `Sum` eagerly.
- Error classification, traces, the predecessor log, `--max-memory` inside the
  core and parallelism are not built.
- The memo table size, and keys finer than whole variables.
- `Heap::view` and `Store::node` pointer chasing is the top self-time on
  MCLamportMutex and GameOfLife. Store node data inline.
- Canonical enumeration order is used throughout. Counts do not depend on it at
  one worker. Traces, `CHOOSE` under `--compat=tlc` and runtime names will.
