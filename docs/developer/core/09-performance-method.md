# Performance method

Status: design. It applies to all work on the new core. It replaces, for this
work, ADR 0104's 5% single-thread instruction-count bar (ADR 0117).

## 1. What counts

The gate is **wall-clock time on a quiet host**, measured at 1 and 8 workers
against pinned TLC on the same host. Every run checks counts against the
goldens; `dive-bench` already does this.

Instruction counts, cache simulation and profiles are diagnostics. They explain
results but never accept or reject a change. A single-thread instruction
counter cannot see lock contention, atomics or cache misses: #495, the largest
8-worker cost, is invisible to it.

## 2. Hosts

**x86 reference node.**
- An AWS bare-metal instance of the type used by the 2026-10-21 campaign
  (`docs/developer/performance.md`).
- It is launched for a campaign and terminated afterwards.
- Every milestone gate is decided on this host.

**Local Apple Silicon.** Used for development measurements, labelled as
indicative. Before each run, record `uptime` and the top CPU consumers. Discard
runs started under load average 2 or higher.

## 3. Benchmark set

The set is `crates/dive-bench/benchmarks.toml`:

- GameOfLife
- TLCSailfish1
- EWD998PCal
- MCBakery
- MCLamportMutex
- SimpleRegular
- MCbtree
- CoffeeCan1000BeansSafety
- MCDisruptor_MPMC
- BufferedRandomAccessFile

The M0 spike uses only these four:

- **CoffeeCan1000BeansSafety:** engine, store and hash costs.
- **MCLamportMutex:** sequences, records, nested `EXCEPT`; the headline gap.
- **SimpleRegular:** `EXCEPT` and `[D -> SUBSET S]` membership.
- **GameOfLife (stretch):** laziness, `CHOOSE` and recursion.

## 4. Procedure

1. Build the release profile from a clean worktree with its own target
   directory. Record the commit, `rustc` version and binary SHA-256.
2. Run `dive-bench` with `--tlc`, with at least 5 repetitions. Use the median.
   The schedule is fixed by `dive-bench`: model order fixed, worker and arm
   blocks reversed on odd repetitions.
3. Run at 1 and 8 workers in both profiles. Record wall time, CPU time, peak RSS
   and states per CPU second.
4. For the new core, also run each ablation switch at 1 worker on the spike
   models.
5. Record the result as compact JSON (section 6). Put a summary table in the
   PR or in the milestone note in `10-plan.md`.

## 5. Ablation switches

Each switch disables one strategy. Every switch must leave all observables
unchanged (`08-correctness.md` §6).

| Switch | Disables |
|---|---|
| `-delta-hash` | S1, S2: the full state hash is recomputed for each successor |
| `-early-probe` | S3: every successor is materialized before the probe |
| `-hashcons` | S4, S5: values are not deduplicated, and equality is structural |
| `-symbolic-member` | S5: membership enumerates symbolic sets |
| `-memo` | S6: no memoization |
| `-parent-skip` | S6: no parent-delta skip |
| `-arena` | S7: transients are heap-allocated |
| `-inline-subst` | S8: INSTANCE substitutions stay dynamic |
| `-eager` | S8: every lazy argument is a thunk |

The share of a strategy is the slowdown when it alone is disabled. Shares
interact, so the report gives each one's slowdown and the total.

## 6. Recording format

There is one JSON file per campaign under `conformance/reports/perf/`, with a
cap of 100 KB, holding:

- `host`: CPU, cores, OS, load at start.
- `binary`: commit, SHA-256, rustc version.
- `tlc`: oracle id and Java version.
- `runs`: one entry per (model, engine, profile, workers, switch), with the
  median and minimum wall time, CPU time and RSS, and `counts_ok`.
- `notes`.

Raw `dive-bench` output stays outside the repository (`AGENTS.md`, evidence
policy).

## 7. Targets

| Milestone | Target, on x86 |
|---|---|
| M0 (spike models, 1 worker, default profile) | ≥1.5× TLC on MCLamportMutex and SimpleRegular; ≥2.5× TLC on CoffeeCan1000BeansSafety; RSS no higher than the current dive; the interning store scales ≥6× at 8 threads |
| M1 | ≥1× TLC on 8 of 10 benchmarks at 1 worker |
| M2 | ≥2× TLC geometric mean at 1 and 8 workers; ≥1× on every benchmark; ≥3.5× scaling (ADR 0006) |

## 8. Next step

On Day 0, rebaseline the current dive and TLC with this procedure: x86 node,
all 10 models, 1 and 8 workers, both profiles. The comparison base is
recorded in `10-plan.md`.
