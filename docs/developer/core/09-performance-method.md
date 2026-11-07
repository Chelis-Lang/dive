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
- Launch it with `scripts/aws/bench-node.sh <sha> <dive-bench arguments>`
  (an SSO `AWS_PROFILE` for the dive account; nodes are tagged
  `project=dive`). It builds the revision, sets SMT and turbo off at 2.4 GHz,
  pins CPUs 0–15 and leaves `runs.jsonl` and `summary.json` under
  `/home/bench/out`. Fetch them with `scripts/aws/ssm.sh` in gzip+base64
  chunks (SSM truncates output at about 24 KB), keep only a compact JSON
  summary, and terminate the node.

**Linux gate node.** `scripts/aws/gate-node.sh` starts a c7i.16xlarge that
runs `cargo xtask gate --fast|--full` for pushed revisions, one at a time in
fresh worktrees; `scripts/aws/remote-gate.sh` drives it and prints the gate
report. Use it when local gates are slow. It is not a benchmark host.

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
   On macOS, also record the peak memory footprint (`/usr/bin/time -l`,
   "peak memory footprint"), and compare memory by footprint. On a loaded
   host, maximum RSS varies by up to 20 MB between runs of the same binary,
   because pages are reclaimed and compressed under pressure, while the
   footprint varies by under 0.5 MB. One run's RSS is not evidence of a
   regression (see `03-value-model.md` §12, the MCLamportMutex bisection).
4. For the new core, also run each ablation switch at 1 worker on the spike
   models.
5. Record the result as compact JSON (section 6). Put a summary table in the
   PR or in the milestone note in `10-plan.md`.

## 5. Ablation switches

Set `DIVE_CORE_OPT=-name` for one disabled strategy, or a comma-separated
list. The current names come from `dive_core::opts::SWITCHES`. Every switch
must leave all observables unchanged (`08-correctness.md` §6).

| Switch | Disables |
|---|---|
| `-delta-hash` | S1, S2: the full state hash is recomputed for each successor |
| `-early-probe` | S3: every successor is materialized before the probe |
| `-hashcons` | S4, S5: values are not deduplicated, and equality is structural |
| `-symbolic-member` | S5: membership enumerates symbolic sets |
| `-member-index` | S5: repeated normalized membership uses the original search; TLC comparisons always use that search |
| `-memo` | S6: no pure-result memoization |
| `-step-memo` | S6: no step-predicate body memoization |
| `-primed-memo` | S6: no memoization of definitions an action reads primed |
| `-parent-skip` | S6: no parent-delta skip |
| `-arena` | S7: transients are heap-allocated |
| `-eager` | S8: every lazy argument is a thunk |
| `-fold` | S8: no constant folding |

`inline-subst` is not an available switch: the implementation always binds
INSTANCE substitutions dynamically. The parser explicitly rejects it.

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
| M2 | Default: ≥2× TLC geometric mean at 1 and 8 workers and ≥1× on every benchmark. TLC compatibility: runtime/TLC ≤2 on every benchmark at both worker counts; investigate >1.5. Scaling remains ≥3.5× on the eligible population (ADRs 0006, 0119). |

## 8. Next step

On Day 0, rebaseline the current dive and TLC with this procedure: x86 node,
all 10 models, 1 and 8 workers, both profiles. The comparison base is
recorded in `10-plan.md`.
