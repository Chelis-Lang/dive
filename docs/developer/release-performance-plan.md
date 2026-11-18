# Release performance qualification

This is the runbook for the performance measurements that qualify dive 0.1.0
and the first Sail release that runs native temporal jobs on it (Sail 0.3.0).
It is written for an engineer or agent who has not seen the work that led to
it. Tracking issue: #1011. The Sail side is tracked in
[Chelis-Lang/sail#764](https://github.com/Chelis-Lang/sail/issues/764).

## Purpose and status rule

The measurements record how the release performs, so that users get numbers
we have measured and regressions have an issue before the tag.

- **R2 is the only hard gate** ([RELEASE.md](../RELEASE.md)): Core is faster
  than TLC on the geometric mean over the R1 agreeing models, and no model is
  more than 2x slower than TLC. D1 below measures it.
- Every other measurement is directional. A ×TLC multiple or a Sail overhead
  is an aim, not a gate. Correctness is never directional: a wrong verdict, a
  count that differs from the golden or an unvalidated counterexample is a bug
  that blocks, whatever the timing.
- Every regression gets a GitHub issue with the `perf` label, in dive or in
  Sail, naming the model, the measurement, the previous number and the source.
  A regression is a model above 1x TLC (R2), or a figure worse than the last
  committed record for the same measurement on the same host type.
- Every run carries an explicit time and memory budget (`--time-limit`,
  `--max-memory`, a harness `--timeout` or a Sail `wall_time_ms`). A run that
  stops at its budget is reported as stopped, never dropped and never retried
  until it fits.

## When

1. On the release-candidate source: `main` after the history rewrite (the
   rewrite changes every commit id, so measurements taken before it name
   commits that no longer exist).
2. The dive measurements (D1 to D7) run before the `v0.1.0` tag. D1 must pass
   before tagging.
3. The Sail measurements (S1 to S6) run after dive 0.1.0 is on crates.io and
   Sail PR [#763](https://github.com/Chelis-Lang/sail/pull/763) builds against
   it, before the Sail 0.3.0 release.

Prerequisites on the dive side:

- #1010 (Core liveness with the exact store and state/depth ceilings) is
  merged. D3's liveness half, D6's temporal ceilings and every Sail
  liveness job depend on it. Until it lands, Core refuses `--store exact` and
  `--max-states`/`--max-depth` with temporal properties, and Sail returns
  `Unsupported` for those jobs.
- #983 (the harness enforcing the R2 procedure) is open at the time of
  writing. While it is open, D1's repetitions, canaries, clean environment
  and record are carried out by a driver script, and the record says so in
  `deviations`, as the 2026-11-10 record did. Check its state before starting.

## Where

All timed runs happen on AWS nodes in the dive account, driven from a local
machine. Agents never run on the nodes. Terminate every node when its
measurement is fetched, and confirm with `aws ec2 describe-instances`.

### dive nodes

[`scripts/aws/bench-node.sh`](../../scripts/aws/bench-node.sh) launches a
c7i.metal-24xl, sets SMT off, turbo off, the performance governor and
2.4 GHz minimum and maximum frequency, stops snapd and the apt timers, builds
the given revision with `cargo build --release --locked -p dive-cli -p
dive-bench`, fetches the oracle and corpora, and runs `dive-bench` as user
`bench`. [`scripts/aws/ssm.sh`](../../scripts/aws/ssm.sh) runs commands on a
node and fetches files. Both need:

| Variable | Meaning |
|---|---|
| `AWS_PROFILE` | an SSO profile for the dive account |
| `AWS_REGION` | optional, default `us-east-1` |
| `DIVE_AWS_AMI` | an Ubuntu 24.04 x86_64 AMI id |
| `DIVE_AWS_INSTANCE_PROFILE` | an IAM instance profile that can read the token parameter and accept SSM commands |
| `DIVE_AWS_GH_TOKEN_PARAM` | the SSM SecureString parameter holding a GitHub token (the repository is private until release); the token is never printed |

`DRY_RUN=1` prints the generated user data without launching.

Two properties of `bench-node.sh` do not meet the R2 procedure, so D1 uses
the node differently from the other measurements:

- It runs `dive-bench` under `taskset -c 0-15 numactl --membind=0`. The R2
  procedure binds the harness with `numactl --cpunodebind=0 --membind=0` and
  calls a narrower CPU list triage. For D1, launch the node with a trivial
  benchmark argument (`--list`) so it bootstraps and finishes, then run the
  harness yourself through `ssm.sh` with the procedure's binding.
- Its root volume is a default gp3 (3000 IOPS, 125 MiB/s). The procedure
  needs 16000 IOPS and 1000 MiB/s for TLC's disk-backed queue. Raise it on the
  running node with `aws ec2 modify-volume --volume-id <id> --iops 16000
  --throughput 1000` and wait until `describe-volumes-modifications` reports
  `optimizing` or `completed` before timing.

It also writes wrappers `~/bins/dive-core`, `core-memo` and `core-nomemo`
that set `DIVE_ENGINE`. Core is the only engine now; use `~/bins/dive`
directly. `dive-bench` removes an inherited `DIVE_ENGINE` anyway.

Record on every node: `lscpu`, `numactl --hardware`, the governor, minimum and
maximum frequency, SMT and turbo state, THP mode
(`/sys/kernel/mm/transparent_hugepage/enabled`), kernel, AMI id, volume IOPS
and throughput, and `sha256sum ~/bins/dive ~/bins/dive-bench`.

A node may be shared only between measurements run one after another. Never
run a build, a gate or a second measurement while one is timing.

### Sail hosts

Sail's own scripts launch its hosts, in Sail's checkout:

- `scripts/perf-host.sh up LANE c7i.8xlarge`, then `sync`, `run`, `down`
  (Linux x86-64; the release matrix uses c7i.8xlarge). It reads `AWS_REGION`,
  `SAIL_PERF_TTL_MIN` (the self-termination watchdog, default 480 minutes) and
  `SAIL_PERF_STATE_DIR` (default `~/.sail-perf`), and needs AWS credentials
  for the account that holds Sail's hosts.
- `scripts/perf-host-mac.sh up LANE [AZ]` (mac-m4.metal; `SAIL_MAC_AMI` and
  `SAIL_MAC_INSTANCE_TYPE` override the defaults). A Mac dedicated host is
  billed for at least a day and AWS releases it on a schedule set at `up`.
  Run D7 on the same Mac host as S6 to share that allocation.

## Outputs

- dive records: one compact JSON file per measurement under
  `conformance/reports/perf/`, named
  `<date>-release-<measurement>-<source8>.json` (for example
  `2026-12-01-release-d2-memory-1a2b3c4d.json`), except D1, which keeps the
  R2 name `<date>-r2-procedure-<source8>.json`. Each record is at most 100 KB.
- A summary in [scoreboard.md](../scoreboard.md): the `## Speed` section gets
  the new R2 numbers (replacing the old ones), and a short "Release
  qualification" list links each record with its headline figure.
- Sail records live in Sail's repository under its evidence rules (a
  `docs/validation/` page and compact JSON), with the dive release, worker
  SHA-256 and Sail source they measured.
- No logs, stdout or stderr dumps, raw JSONL, `.gz` files or tarballs in
  either repository. Keep raw output outside the repositories (an S3 prefix in
  the dive account is fine) and cite it by SHA-256 in the record.
  `scripts/check-repo-budget.py` runs in the pre-commit hook.
- One PR per repository for the records, after all measurements are in. Then
  comment on #1011 with the PR and the headline numbers, and open the `perf`
  issues.

Every dive record carries these common fields:

```json
{
  "measurement": "D2",
  "plan": "docs/developer/release-performance-plan.md#d2-memory",
  "source": "<40-hex commit>",
  "dive_binary_sha256": "<hex>",
  "oracle": "tlc-2026.10.06.014338",
  "corpus": "examples@6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f",
  "host": {"instance": "c7i.metal-24xl", "cpu": "...", "smt": "off", "turbo": "off",
           "frequency_khz": 2400000, "governor": "performance", "binding": "...",
           "storage": "...", "thp": "...", "kernel": "...", "ami": "..."},
  "command": "<exact command line, with paths relative to the corpus root>",
  "raw": {"location": "<s3 prefix>", "sha256": "<hex>"},
  "deviations": [],
  "rows": {}
}
```

`rows` holds the per-model results described under each measurement.

## Fixtures

All model paths are relative to the Examples corpus root, pinned in
[`conformance/corpora.toml`](../../conformance/corpora.toml) (`examples`,
commit `6a7b26a3`), as fetched by `dive-conformance corpus fetch` into
`$DIVE_CACHE_DIR`. The R1 agreeing set is the `agreeing` category of the
newest R2 record in `conformance/reports/perf/` (169 models in
`2026-11-10-r2-procedure-e3835968.json`); recompute it from the D1 run before
using any list below, and replace a model that has left it with the next one
from the same size class, recording the swap.

| Module | Config | Golden result | Distinct states | Depth |
|---|---|---|---:|---:|
| `specifications/lamport_mutex/MCLamportMutex.tla` | `MCLamportMutex.cfg` | holds | 724,274 | 61 |
| `specifications/SlushProtocol/Slush.tla` | `SlushSmall.cfg` | holds | 274,678 | 43 |
| `specifications/SlushProtocol/Slush.tla` | `SlushLarge.cfg` | holds | 244,335,240 | 59 |
| `specifications/Disruptor/MCDisruptor_MPMC.tla` | `MCDisruptor_MPMC.cfg` | holds | 112,929 | 81 |
| `specifications/Disruptor/MCDisruptor_MPMC.tla` | `MCDisruptor_MPMC_liveliness.cfg` | holds, liveness | 14,365 | 61 |
| `specifications/bosco/bosco.tla` | `bosco.cfg` | holds | 1,072,452 | 10 |
| `specifications/MultiCarElevator/Elevator.tla` | `ElevatorSafetyLarge.cfg` | holds | 59,007,145 | 44 |
| `specifications/MultiCarElevator/Elevator.tla` | `ElevatorLivenessMedium.cfg` | holds, liveness | 4,122 | 36 |
| `specifications/MultiCarElevator/Elevator.tla` | `ElevatorLivenessLarge.cfg` | holds, liveness | 50,653 | 39 |
| `specifications/CoffeeCan/CoffeeCan.tla` | `CoffeeCan1000Beans.cfg` | holds, liveness | 501,500 | 1 |
| `specifications/CoffeeCan/CoffeeCan.tla` | `CoffeeCan3000Beans.cfg` | holds, liveness | 4,504,500 | 1 |
| `specifications/SpecifyingSystems/Liveness/MCLiveInternalMemory.tla` | `MCLiveInternalMemory.cfg` | holds, liveness | 4,408 | 10 |
| `specifications/SpecifyingSystems/RealTime/MCRealTimeHourClock.tla` | `MCRealTimeHourClock.cfg` | liveness violation `ErrorTemporal` | 216 | 2 |
| `specifications/SpecifyingSystems/AdvancedExamples/MCInnerSerial.tla` | `MCInnerSerial.cfg` | holds, liveness | 195 | 5 |
| `specifications/aba-asyn-byz/aba_asyn_byz.tla` | `aba_asyn_byz.cfg` | holds, liveness | 5,843,977 | 45 |
| `specifications/FlashProtocol/MCFlashWithMutexLive.tla` | `MCFlashWithMutexLive.cfg` | holds, liveness | 8,096 | 21 |
| `specifications/ewd840/EWD840.tla` | `EWD840.cfg` | holds, liveness | 302 | 9 |
| `specifications/DieHard/DieHard.tla` | `DieHard.cfg` | invariant violation `NotSolved` | 14 | 7 |
| `specifications/SlidingPuzzles/SlidingPuzzles.tla` | `SlidingPuzzles.cfg` | invariant violation `KlotskiGoal` | 24,005 | 117 |

Configs sit beside their modules. Counts are the pinned TLC golden's
(`conformance/golden/tlc-2026.10.06.014338/examples/`); TLC reports depth 1
for the CoffeeCan liveness runs.

## dive measurements

Unless a step says otherwise: one worker, `--seed 0 --fp 0`, the default
profile, the release build of `dive` from the release-candidate source
(`cargo build --release --locked -p dive-cli --bins`, no `RUSTFLAGS`), one
measured process at a time, and every result checked against its golden
(verdict, distinct and generated counts at one worker, depth, violated
property, exit code). A run that disagrees with its golden invalidates that
row; open a correctness issue, not a perf issue.

### D1: fresh R2

The gate run. It follows the
[R2 measurement procedure](performance.md#r2-measurement-procedure) exactly;
where this section and the procedure differ, the procedure wins.

- **Host.** c7i.metal-24xl nodes bootstrapped by `bench-node.sh <sha> --list`,
  volume raised to 16000 IOPS and 1000 MiB/s, nothing else running. The
  2026-11-10 run used five nodes: one for the small class and four large
  shards.
- **Binding.** `numactl --cpunodebind=0 --membind=0` around the harness, for
  both TLC and Core. Not `taskset -c 0-15`.
- **Environment.** `env -i PATH=... HOME=... DIVE_CACHE_DIR=...`, plus
  `GH_TOKEN` only for `oracle fetch` and `corpus fetch`.
- **Page cache.** Before timing, read the corpus checkout, module libraries,
  JRE tree, `tla2tools.jar` and the dive binary (for example `find <dir>
  -type f -exec cat {} + >/dev/null`).
- **Classification.** One `--jobs 1` TLC pass over the selection on the
  reference host type, recorded before any Core run: small is TLC under
  10 s, large is 10 s or more.
- **Commands** (per node, on its shard):

  ```sh
  numactl --cpunodebind=0 --membind=0 env -i PATH="$PATH" HOME="$HOME" DIVE_CACHE_DIR="$DIVE_CACHE_DIR" \
    target/release/dive-conformance run --impl tlc --corpus examples --tier all --workers 1 \
    --jobs 1 --xmx 24g --no-sany --report-dir "$TLC_OUT" --no-summary [--filter <model>]...
  numactl --cpunodebind=0 --membind=0 env -i PATH="$PATH" HOME="$HOME" DIVE_CACHE_DIR="$DIVE_CACHE_DIR" \
    target/release/dive-conformance run --impl dive:core --impl-profile dive --corpus examples \
    --tier all --workers 1 --jobs 1 --xmx 24g --dive-bin ~/bins/dive \
    --tlc-timings "$TLC_OUT"/<run-id>.json --budget-cap 30000 \
    --report-dir "$OUT" --no-summary [--filter <model>]...
  ```

  `dive-conformance` is built on the node from the same source
  (`cargo build --release --locked -p dive-conformance`).
- **Repetitions.** Small class: six repetitions in one session, each timing
  TLC then Core over the class, order reversed in alternate repetitions;
  the model's ratio is the median of six. Large class: one pass; a ratio in
  [0.9, 1.1] or [1.8, 2.0], or a Core budget stop, gets three fresh
  back-to-back pairs (order alternating) with Core's budget at 2.5x that
  pair's TLC time, and the median of three is the ratio. The driver script
  does this until #983 lands.
- **Canaries.** Only when reusing cached TLC times: re-time the three shortest
  large models three times each before any Core run; a median off by more
  than 5 %, or a mean signed difference above 2 %, means re-taking the cache.
  A fresh release run takes fresh TLC times, so no cache is reused.
- **Sharding.** Fixed before the run: large models by longest processing time
  on the classification pass's TLC times, round-robin otherwise. Each node
  times TLC and Core on its own shard.
- **Attempts.** The gate number is the first complete conforming attempt.
  List every attempt; discard one only for a recorded host fault.
- **Record.** `conformance/reports/perf/<date>-r2-procedure-<source8>.json`,
  shaped like
  [`2026-11-10-r2-procedure-e3835968.json`](../../conformance/reports/perf/2026-11-10-r2-procedure-e3835968.json):
  `gate`, `procedure`, `source`, `dive_binary_sha256`, `oracle`, `corpus`,
  `host` (instance and node count, CPU, SMT, turbo, frequency, governor,
  binding, storage, THP, jobs, xmx, budget cap), `classification`,
  `sharding`, `summary` (`agreeing`, `geomean`, `fixed_set_geomean`,
  `above_1x`, `above_2x`, `stopped`), `not_timed` with reasons, `deviations`
  (name #983 while it is open), and `models` with `cls`, `shard`, `cat`,
  `ratio`, and `reps` (small) or `first_pass` and `pairs` (large banded).
- **Follow-up.** Post the summary on the pinned v0.1 status issue, update the
  `## Speed` section of the scoreboard, and open or update a `perf` issue for
  every model above 1x. Any model above 2x blocks the tag.

### D2: memory

Two parts, on one quiet bench node.

**Benchmark set peak RSS.** The ten models of
[`crates/dive-bench/benchmarks.toml`](../../crates/dive-bench/benchmarks.toml),
at 1 and 8 workers, both profiles, with TLC for comparison:

```sh
scripts/aws/bench-node.sh <sha> --dive-bin ~/bins/dive --dive-revision <sha> \
  --profile both --workers 1,8 --reps 5 --tlc --timeout 900 --xmx 3g
```

`dive-bench` reads peak RSS from GNU `/usr/bin/time -v` on each run. Record
per model, tool, profile and worker count: the median and range of
`max_rss_bytes` over the five repetitions, the median wall time, and TLC's
peak RSS beside dive's. TLC's figure depends on `-Xmx3g`; say so in the
record.

**Liveness capture RSS.** Every R1 agreeing model whose report has
`check_kind: "temporal"`. Select them by running each R1 agreeing model whose
config has a `PROPERTY` or `PROPERTIES` entry once and keeping those whose
JSON report says `temporal`; refinement-only properties report `safety` and
drop out. The large ones expected to stay in are `ElevatorLivenessLarge`,
`ElevatorLivenessMedium`, `CoffeeCan3000Beans`, `CoffeeCan1000Beans`,
`aba_asyn_byz`, `cf1s_folklore`, `bcastByz`, `MCCRDT`, `MCInnerSerial`,
`MCFlashWithMutexLive`, `YoYoAllGraphs`, `nbacg_guer01`,
`MCDisruptor_MPMC_liveliness`, `MCLiveInternalMemory`,
`MCLiveWriteThroughCache` and the DijkstraMutex `LSpec-model`. For each:

```sh
/usr/bin/time -v ~/bins/dive check <module.tla> --config <model.cfg> --workers 1 --seed 0 --fp 0 \
  --time-limit <2.5 x the D1 TLC time, at least 60s> --max-memory 64g --output json > report.json
```

three repetitions. Record per model: peak RSS from `time -v` (median and
range), `resources.memory.observed_max_bytes`, distinct states, the
`core.metrics.store.memory.known_retained_bytes_lower_bound`, wall time, and
bytes per distinct state. A stop at the memory or time budget is a row with
`stopped` and its phase, and gets a `perf` issue.

Row fields: `model`, `workers`, `store`, `reps`, `peak_rss_bytes` (median,
min, max), `observed_max_bytes`, `distinct`, `wall_ms`, `bytes_per_state`.

### D3: exact versus fingerprint store

The time and RSS cost of `--store exact` against the default fingerprint
store. The safety half can run now; the liveness half needs #1010.

- **Safety:** the ten `benchmarks.toml` models.
- **Liveness:** `CoffeeCan1000Beans`, `ElevatorLivenessMedium`,
  `MCLiveInternalMemory`, `MCFlashWithMutexLive` and `aba_asyn_byz`.

`dive-bench` has no store option, so drive `dive check` directly:

```sh
for rep in 1 2 3; do
  for store in fp exact; do   # reverse the order on even repetitions
    /usr/bin/time -v ~/bins/dive check <module.tla> --config <model.cfg> --workers 1 --seed 0 --fp 0 \
      --store "$store" --time-limit <budget> --max-memory 64g --output json > "$store-$rep.json"
  done
done
```

Both stores must give the same verdict, counts and depth as the golden. Record
per model and store: median wall time and peak RSS; the exact/fp ratio of
each; `options.store` from the report (to confirm the store actually used).
Exact is expected to cost memory; a ratio worse than the last record, or a
liveness model that agrees under fp and not under exact, gets an issue.

### D4: worker scaling

1, 4 and 8 workers on the large R1 models: the `large` class of the D1 record
(TLC 10 s or more; 48 models in the 2026-11-10 record). Write a benchmark set
file outside the repository with one `[[bench]]` entry per model (the format
of `benchmarks.toml`: `name`, `model`, `why`) and run:

```sh
~/bins/dive-bench --set <large-set.toml> --dive-bin ~/bins/dive --dive-revision <sha> \
  --profile default --workers 1,4,8 --reps 3 --timeout 3600 --xmx 24g \
  --runs ~/out/runs.jsonl --summary ~/out/summary.json
```

on a bench node bound to one NUMA node. Record the binding: with `taskset -c
0-15` the 8-worker runs have 16 CPUs, which is enough; say which binding was
used. `dive-bench` pairs each N-worker run with the same repetition's
1-worker run and reports the median speed-up, and records requested and used
workers. Record per model: wall time at 1, 4 and 8 (median), the 4- and
8-worker speed-ups, used workers and any sequential reason (temporal models
may run sequentially; that is a row, not an error). Generated counts at more
than one worker are diagnostics only (ADR 0086). Put the SHA-256 of the set
file in the record.

### D5: startup, simulation and trace validation throughput

**Startup.** `docs/user/examples/Counter.tla` with `Counter.cfg` (a few
states). Fifty fresh runs each of `dive --version`, `dive parse
docs/user/examples/Counter.tla` and

```sh
~/bins/dive check docs/user/examples/Counter.tla --config docs/user/examples/Counter.cfg --workers 1 --seed 0 --fp 0 --output json
```

timed by a driver that records each process's wall time (`perf_counter`
around `subprocess.run`). Record the median, p90 and max of each, and peak
RSS of the `check` runs. The R2 small class already compares end-to-end time
with TLC's JVM start; D5 isolates dive's own floor.

**Simulation.** `MCLamportMutex.cfg` and `bosco.cfg`, safety-only models:

```sh
~/bins/dive simulate <module.tla> --config <model.cfg> --traces 10000 --depth 100 --seed 0 \
  --stats basic --time-limit 600s --max-memory 16g --output json
```

three repetitions. Record traces per second and states per second (from the
`basic` statistics and the wall time), and peak RSS.

**Trace validation.** Produce each counterexample once, then time its
validation:

```sh
~/bins/dive check <module.tla> --config <model.cfg> --workers 1 --seed 0 --fp 0 --output json > run.json
python3 -c 'import json; r=json.load(open("run.json")); json.dump(r["counterexample"]["trace"], open("trace.json", "w"))'
~/bins/dive validate-trace <module.tla> --config <model.cfg> --trace trace.json --seed 0 --fp 0 --output json
```

on `DieHard.cfg` (7 steps), `SlidingPuzzles.cfg` (117 steps) and
`MCRealTimeHourClock.cfg` (a lasso). Ten validations each; record the median
wall time, states per second, and that every result is `valid`. Also time
`--sample` validation of 100 traces saved from the `MCLamportMutex`
simulation (`--inputs` or `--trace` per [traces.md](../user/traces.md)).

### D6: stop latency

How promptly a run stops once a budget or ceiling is reached. Latency is the
process's wall time minus the time at which the stop was due.

- **Time budget.** `ElevatorSafetyLarge.cfg` and `SlushLarge.cfg` (safety),
  `ElevatorLivenessLarge.cfg` (temporal, so the stop can fall in capture or
  temporal analysis), at 1 and 8 workers, with `--time-limit` 5s, 30s and
  120s. Five repetitions each. The clock starts after the specification and
  configuration are loaded ([checking.md](../user/checking.md#time-and-memory-budgets)),
  so measure the load time separately (a run with `--max-states 1`) and
  subtract it. Record wall time, `stopped.elapsed`, `stopped.phase`, exit
  code (5) and the latency.
- **State and depth ceilings.** `SlushLarge.cfg` with `--max-states`
  1000000 and 10000000, and `--max-depth` 10 and 30, at 1 and 8 workers.
  Record wall time, the reported distinct states and depth against the
  ceiling (overshoot), exit 5, and the time from the last progress report to
  exit.
- **Temporal ceilings** (after #1010): `ElevatorLivenessLarge.cfg` with
  `--store exact --max-states 20000` and `--max-depth 20`. Expect exit 5 with
  no claim; any proven violation retained at a completed level is reported.

### D7: macOS arm64 spot check

Information only, never a gate (macOS is not a v0.1 gate). On the Sail Mac
host used for S6 (mac-m4.metal), build dive from the release-candidate
source and run D2's benchmark-set part with `--workers 1,8 --reps 3 --tlc`
and D5's startup part. `dive-bench` reads RSS from `/usr/bin/time -l` there.
Record the macOS version and chip, and mark the record `informational`.

## Sail measurements

Sail runs D.I.V.E. as a library inside its isolated `sail-runtime-worker`
process. A temporal job is a `sail_api::temporal::Job`: `obligation_id`,
sorted `requirement_ids`, a native `request` (a closed map of source files,
root module, config, and one operation: `check` with `max_states` and
`max_depth`, `simulate` with `traces` and `depth`, or `validate` with an ITF
trace), and `wall_time_ms` of at most five minutes. The parent verifies the
worker's SHA-256 against its installation manifest, runs a private copy,
writes one JSON request on stdin and reads one bounded response. Sail's
checking policy is fixed: exact store, seed 0, fingerprint 0, one worker,
explicit state and depth bounds. See Sail's `docs/temporal-jobs.md`,
`docs/spec/temporal-jobs.md` and the "Native temporal jobs" section of
`docs/runtime.md`.

Admission limits that shape the fixtures: imports only from the closure and
the embedded `Naturals`, `Integers`, `Sequences`, `FiniteSets` and `Bags`
(no `TLC`, no CommunityModules); at most 1,024 files and 8 MiB of source; ITF
transport refuses model values, so traces for `validate` must use integers,
strings, sets, sequences, records and functions only.

### Sail-side harness work (for the runner)

Sail has no temporal benchmark today. Before S1, add one in Sail (its own PR,
under Sail's rules), for example a `sail-bench temporal` subcommand or a
script beside `scripts/perf-real.sh`. It needs to:

1. Build a `Job` from a corpus model: read the module closure from the pinned
   Examples checkout, put the files in the request map, set `max_states` to
   twice the golden's distinct states (at least 1,000) and `max_depth` to
   100,000, and refuse the fixture if Sail's admission refuses it.
2. Time the phases of one embedded job (S1), from inside the parent:
   worker SHA-256 verification and private copy, process spawn until the
   child is running, request encoding and write, the worker's own D.I.V.E.
   call, response read and decoding, and Sail's independent replay or
   validation of returned traces. If the worker cannot report its D.I.V.E.
   call time, time the same `dive_api` call in-process from the harness (as
   `crates/sail-dive/tests/native.rs` does for parity) and use the
   difference.
3. Submit the same job three ways (S2): `sail runtime execute --file
   job.json`, the same with `--socket` against a running `sail serve`, and the
   MCP `runtime_execute` tool with and without a daemon already running.
4. Emit one compact JSON row per job with the fields listed below, and keep
   raw output outside the repository.

Every Sail row carries: Sail source, dive release (`backend_revision`),
worker SHA-256, host, the fixture (module, config, golden counts), the
operation and bounds, `wall_time_ms`, the outcome and completion kind, and
the timings.

### S1: per-job overhead against the dive CLI

For each S3 fixture: the embedded Sail job against `dive check` (or
`simulate`, `validate-trace`) on the same model with Sail's settings
(`--store exact --workers 1 --seed 0 --fp 0 --max-states <same>
--max-depth <same>`), on the same host. Five repetitions, alternating order.
Record total wall time for each, the difference, and the S1 phase breakdown
(SHA-256 verify and copy, spawn, encode, D.I.V.E. call, decode, independent
replay). Include `EWD840.cfg` (302 states) as the small model where overhead
dominates.

### S2: embedded, daemon socket and MCP autostart

`MCLiveInternalMemory.cfg` (check) and `DieHard.cfg` (validate), each through
the three paths, cold (no daemon, fresh cache directory) and warm (daemon up,
one job already run). Ten repetitions per cell. Record the median and p90
wall time per path and state, and the daemon start time on MCP autostart.

### S3: real-sized fixtures

At least two models per kind, all from the R1 agreeing set and inside Sail's
admitted module set (checked statically against the corpus; the harness
confirms by admission):

| Kind | Fixtures (config; module in the fixture table) |
|---|---|
| Safety check | `MCLamportMutex.cfg`, `SlushSmall.cfg`, `MCDisruptor_MPMC.cfg` |
| Liveness/fairness check (exact store and bounds; needs #1010) | `CoffeeCan1000Beans.cfg`, `ElevatorLivenessMedium.cfg`, `MCLiveInternalMemory.cfg`, `MCDisruptor_MPMC_liveliness.cfg`; plus `MCRealTimeHourClock.cfg` for a validated temporal counterexample |
| Simulation | `MCLamportMutex.cfg` and `bosco.cfg`, `traces` 1,000, `depth` 100 |
| Trace validation | ITF traces from Sail check jobs on `DieHard.cfg`, `SlidingPuzzles.cfg` and `MCRealTimeHourClock.cfg`, plus 100 traces returned by the `MCLamportMutex` simulation |

`acp/ACP_NB_WRONG_TLC.cfg` and `MissionariesAndCannibals.cfg` violate
properties too but use model values, which ITF refuses; they are not trace
fixtures. Every fixture must finish inside the five-minute `wall_time_ms`;
a `Stopped` or `TimedOut` outcome is a finding, not a timing. Each check must
give the golden's verdict and distinct states, and each liveness job must
report completion kind `temporal`. Three repetitions; record wall time and
whole-tree peak RSS (from `perf-mem.py`) per fixture.

### S4: timeout, cancellation and capacity

On `ElevatorSafetyLarge.cfg` (long enough to outlast every deadline here,
with `max_states` above its golden count):

- **Deadline.** `wall_time_ms` 1,000, 5,000 and 30,000. Record the time from
  the deadline to the `TimedOut` result.
- **Cancellation.** Ctrl-C on `sail runtime execute`, client disconnect on
  the socket path, and MCP cancellation, each at a random point after the
  worker started (seeded, and the seed recorded). Record the time to the
  `Cancelled` result.
- **Reaping.** After each, confirm no `sail-runtime-worker` or descendant of
  the job's process group survives (`ps -o pid,pgid,command`), and record the
  time until the group is empty.
- **Capacity.** With the daemon at its two active executions, cancel one and
  immediately submit another. Record the time until the new job is admitted
  and that it is admitted, not refused.

Ten repetitions per case.

### S5: concurrency at the admission limit

`sail serve` admits eight client connections and two active executions. Run
two `MCLamportMutex.cfg` checks at once, then a third submitted while
both run, then four, through the socket path, under `perf-mem.py`. After
#1010, repeat with `CoffeeCan1000Beans.cfg` for liveness capture memory:

```sh
python3 -I scripts/perf-mem.py --output run.memory.json --state daemons.json -- <client command>
python3 -I scripts/perf-mem.py --snapshot --output run.memory.json --state daemons.json
```

Record per run: `tree_peak_kb`, `daemon_peak_kb`, `largest_kb`, each job's
wall time and outcome, how the third and fourth jobs were handled (queued or
refused, and the time to admission), and the single-job time from S3 for
comparison.

### S6: Sail release matrix hosts

Run S1 to S5 on Linux c7i.8xlarge (`perf-host.sh`) and on mac-m4.metal
(`perf-host-mac.sh`), from the installed release-candidate archives (the
`sail` binary with its staged worker and manifest), as Sail's release matrix
does. Record each platform's installed `sail` and worker SHA-256 beside the
results.
