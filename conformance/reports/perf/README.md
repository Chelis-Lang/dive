# Performance records

Compact JSON records of dive performance measurements. Each record identifies the experiment and its sources and hosts. Raw release runs (per-run JSON lines,
console output, reports) are not in the repository; they are tarballs in the
dive evidence archive bucket (us-east-1), and each release record cites its tarball by
name and SHA-256 so a later reader can fetch it and check that it is the
same file.

The 0.1.0 release records are listed first. They are the baseline for
comparing later releases. The human-readable results page is
[`docs/developer/release-performance-results.md`](../../../docs/developer/release-performance-results.md),
the method is
[`docs/developer/release-performance-plan.md`](../../../docs/developer/release-performance-plan.md),
and the R2 procedure is in
[`docs/developer/performance.md`](../../../docs/developer/performance.md#r2-measurement-procedure).
The scripts that produced the records are in
[`scripts/perf/release/`](../../../scripts/perf/release/README.md).

The [peer comparison panel and source pins](../../../docs/internal/survey/verification-workloads.md)
extend this evidence to native checkers, implementation traces and proofs.

The [temporal analysis phase measurements](2026-11-21-temporal-analysis-phases.json) separate predicate labeling from accepting-cycle search on seven corpus configurations. Elevator Large spent 399.92 of 404.35 seconds in the product phase at eight requested workers.

The [source-driven EmergencyCall measurements](2026-11-21-source-whole-values.json) contain the installed Linux worker identities, monolithic and 64-cell counts, wall time, peak RSS, independent exact comparisons and trace controls.

The [compiled finite safety comparison](2026-11-21-compiled-finite-safety.json) records the exact Core and compiled KVS Medium runs, binding and compilation cost, same-host tlx, TCommit, and the named SlateDB holdout refusal.

The [Linux allocator comparison](2026-11-21-linux-allocator-arenas.json) records the mimalloc 3.3.2 and 3.7.0 arena probes on Intel and Graviton, original Flint time to 233 million exact states, ordinary Sailfish, and paired KVS, SlateDB and EWD998 runs. The `fixed_release` object contains the 3.7.0 results.

The [KRaft pre-vote measurements](2026-11-21-kafka-liveness.json) contain the replayable witness for infinitely many reachable states in the released liveness CFG and the smaller configurations that both engines exhausted.

The [poison queue comparisons](2026-11-21-poison-queue.json) cover the original five-by-five deadlock witnesses, complete small fair graphs and a checked projection of ordinary message identities.

The [etcd election-term witnesses](2026-11-21-etcd-election-terms.json) reproduce the unbounded election counter in the released three-server configuration.

The [native peer panel](2026-11-20-native-peer-panel.json) records process
measurements against pinned Manyan, tlx and tla-mc, plus the same-binary
guard experiment. [Results and interpretation](../../../docs/internal/survey/verification-workloads.md#native-peer-measurements)

The [guarded message subset measurements](2026-11-21-guarded-message-subsets.json) compare native Core and TLC on the published delivery action, the full three-process no-fault model, and independently replayed counterexamples.

The [exact population searches](2026-11-21-source-occupancy.json) cover both labeled counts and BFS depth for the original TCommit at 3 through 8,192 managers, a separate cyclic source, and replayed/tampered counterexamples. The [ordinary BDD and induction record](2026-11-21-wide-source-induction.json) includes the matched 256-manager closed graph.

The [population edit-session measurements](2026-11-21-occupancy-edits.json) pair fresh labeled searches, portable proofs and native replay after property and transition edits at 8,192 managers.

## 0.1.0 release records

| File | Family | Question | Source | Host |
|---|---|---|---|---|
| `2026-11-18-r2-procedure-08a14fba.json` | D1, R2 gate | Is Core faster than TLC on the corpus (geomean below 1x, no model above 2x)? | `08a14fba` | 5 x c7i.metal-24xl |
| `2026-11-18-release-d2-memory-08a14fba.json` | D2 | Peak RSS of dive against TLC, and of liveness capture per state | `08a14fba` | c7i.metal-24xl (small node, one shard node) |
| `2026-11-18-release-d3-store-08a14fba.json` | D3 | What does `--store exact` cost against the fingerprint store? | `08a14fba` | c7i.metal-24xl (small node) |
| `2026-11-18-release-d4-scaling-08a14fba.json` | D4 | How does wall time scale at 1, 4 and 8 workers? | `08a14fba` | 9 x c7i.metal-24xl (4 on-demand, 5 spot) |
| `2026-11-18-release-d5-throughput-08a14fba.json` | D5 | Startup latency, simulation throughput, trace-validation throughput | `08a14fba` | c7i.metal-24xl (small node) |
| `2026-11-18-release-d6-stop-latency-08a14fba.json` | D6 | How soon after a time, state or depth limit does a run stop? | `08a14fba` | c7i.metal-24xl (small node) |
| `2026-11-18-release-d7-macos-08a14fba.json` | D7 (informational) | Does the macOS arm64 build behave like the x86 one? | `08a14fba` | MacBook Air M3, shared laptop |
| `2026-11-19-release-s7-sail-vs-tlc-ce94df74.json` | S7, release gate | Sail+dive against a fresh `java -jar tla2tools.jar` per check | dive `3d5923a1`, Sail `ce94df74` (and four other Sail builds) | c7i.8xlarge, fresh node |

All D records share the oracle `tlc-2026.10.06.014338` (pinned JRE), the
tlaplus/Examples corpus at `6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f`, and,
except D7, the dive binary with SHA-256
`97675f9d446dbd188684f9f519e1313a86ae7e7faf57a7964d42d1115498e014`. That
binary was built once (`cargo build --release --locked -p dive-cli --bins`, no
`RUSTFLAGS`) and copied to every node, because independent builds of the same
source on different nodes were not byte-identical.

### Host and method (D1 to D6)

c7i.metal-24xl in us-east-1: Intel Xeon Platinum 8488C, SMT off, turbo off,
2.4 GHz fixed, `performance` governor, one NUMA node with 48 CPUs, gp3 at
16000 IOPS and 1000 MiB/s, Ubuntu 24.04.5 LTS, kernel 7.0.0-1017-aws, THP
`madvise`. Every timed process ran under `numactl --cpunodebind=0
--membind=0` and `env -i` with only `PATH`, `HOME` and `DIVE_CACHE_DIR`, one
checker at a time per node, page cache warmed first. The `host` object of
each record repeats these facts.

Every run is checked against its TLC golden (verdict, distinct and generated
counts at one worker, depth, violated property, exit code). A run that hits
its time or memory budget is recorded as stopped. It is never dropped, and
never retried until it fits.

### Raw archives

Prefix `release-perf/2026-11-18/` holds the D records' tarballs:

| Tarball | Holds | Cited by |
|---|---|---|
| `small-node-raw.tar.gz` | D1 small class, D2 benchmark set and first liveness batches, D3, D5, D6 | D1 `raw.files`, D2 `raw.files`, D3/D5/D6 `raw.sha256` |
| `shard1-d1-raw.tar.gz` to `shard4-d1-raw.tar.gz` | D1 large shards | D1 `raw.files` |
| `shard1-d4-raw.tar.gz` to `shard4-d4-raw.tar.gz`, `spot1-d4-raw.tar.gz` to `spot5-d4-raw.tar.gz` | D4 repetitions (`shardN` is node `od-N`, `spotN` is `spot-N`) | D4 `raw.files` |
| `shard2-d4-d2-raw.tar.gz` | D4 on od-2 plus the D2 liveness runs moved there | D2 and D4 `raw.files` |
| `mac-local-raw.tar.gz` | D7 | D7 `raw.sha256` |

Prefix `release-perf/2026-11-19/s7/` holds the S7 tarball. The S7 record
cites the SHA-256 of its three harness files (`harness_sha256`) but not of the
tarball; the tarball `s7-raw.tar.gz` had SHA-256
`e0de442aa30e7205c4934b0d97829300c9e5560acd3fc4b898a211f2b62e3c90` when it was
packed (not rechecked against the bucket copy since).

To fetch one, use the AWS CLI with read access to the bucket, then check the
digest before reading anything:

```sh
aws s3 cp s3://<evidence-bucket>/release-perf/2026-11-18/small-node-raw.tar.gz .
shasum -a 256 small-node-raw.tar.gz   # must equal the record's value
```

## Later R2 records

| File | Question | Source | Host |
|---|---|---|---|
| `2026-11-20-r2-procedure-c14687e6.json` | Does the harness-driven R2 (`dive-conformance r2`, #983) reproduce the 08a14fba record? | `c14687e6` | 5 x c7i.metal-24xl |

The 2026-11-20 run is the first R2 that the harness carried out end to end,
with no driver script and no hand-recorded deviations. It used the same
175-model selection and found the same 127/48 class split. The geomean is
0.190x over 169 agreeing models, against 0.195x in the reference. No model is
above 2x. Only `MCKVsnap` is above 1x, at 1.11x. `AllocatorImplementation` is
now 0.96x. Its `versus_reference` field lists each model whose ratio moved
outside [0.9, 1.1] of the reference ratio. All of them moved because Core got
faster; the large-model TLC times stayed within 2.5 % of the reference. Raw
files are under the `release-perf/2026-11-20-r2-983/` prefix.

## Field dictionary

Units are in the field names: `_ms` milliseconds of wall time, `_bytes` bytes,
`_kb` kibibytes (GNU `time %M` and `/proc` `VmHWM`), `_s` seconds. Peak RSS
is the process's maximum resident set from `/usr/bin/time -v` (Linux) or
`-l` (macOS) unless a field says otherwise. "Median" is over the repetitions
the record names. Model keys are corpus config paths
(`specifications/<dir>/<name>.cfg`) unless noted.

### Common top-level fields (D1 to D7)

| Field | Meaning |
|---|---|
| `gate` / `measurement` | `R2` for D1; `D2` to `D7` otherwise |
| `plan`, `procedure` | anchor in the plan or procedure doc |
| `date`, `source` | measurement date and full dive commit |
| `dive_binary_sha256`, `harness_binary_sha256`, `dive_bench_sha256` | SHA-256 of the binaries that ran |
| `oracle`, `corpus` | pinned TLC oracle id and Examples commit (D1 adds `--tier all`) |
| `host` | machine facts; D7 adds `quiet: false` and `load_average_1m_range` |
| `command` | the exact command shape, a string or one string per part |
| `raw` | `location` (bucket and prefix) and `files` (tarball to SHA-256) or `sha256` |
| `deviations` | every way the run departed from the plan, in prose |
| `benchmark_set_sha256` | SHA-256 of the `dive-bench` set file (D4: one per set, keyed by node or chunk tag) |
| `summary` | headline numbers derived from `rows` or `models` |

### D1: `2026-11-18-r2-procedure-08a14fba.json`

- `classification`, `sharding`: how the 175 models were split into the small
  class (TLC under 10 s in a `--jobs 1 --timeout 15` pass) and four large
  shards (longest processing time on golden TLC times).
- `summary`: `agreeing`, `small`, `large` (model counts); `geomean` over
  agreeing models; `fixed_set_geomean` counting each excluded model as 2x;
  `median`, `min`, `max`; `above_1x` (model to ratio); `above_2x`, `stopped`
  (lists); `gate_result`.
- `models.<cfg>`: `cls` (`small` or `large`), `cat` (`agreeing` or
  `excluded:<outcome>`), `ratio` (Core wall time / TLC wall time, the harness's
  `elapsed_ms`). Small models add `reps`: the six per-repetition ratios
  (`ratio` is their median). Large models add `shard`, `tlc_ms`, `core_ms`
  (one pass). Large models in the 0.9x to 1.1x band add `first_pass`, `pairs`
  (three fresh pair ratios) and `pair_orders` (`tlc-core` or `core-tlc`);
  their `ratio` is the median of the pairs.
- `not_timed`, `attempts`: models outside the timing and why; earlier attempts.

### D2: `2026-11-18-release-d2-memory-08a14fba.json`

- `rows.benchmark_set[]` (the ten `crates/dive-bench/benchmarks.toml` models,
  5 repetitions): `model`, `profile` (`default` or `tlc`), `workers` (1 or 8),
  `dive_rss_median`/`_min`/`_max` (bytes), `dive_wall_ms_median`,
  `dive_wall_ms_range`, `tlc_rss_median`, `tlc_rss_range` (bytes, TLC at
  `-Xmx3g`), `tlc_wall_ms_median`, `all_correct`, `workers_used`.
- `rows.liveness.<cfg>` (64 temporal models, 1 worker, fingerprint store,
  3 repetitions): `check_kind` (`temporal`), `peak_rss_bytes`
  (`median`/`min`/`max`), `observed_max_bytes` (dive's own budget sampler),
  `known_retained_lb` (store lower bound from the report), `distinct`,
  `wall_ms` (median), `bytes_per_state` (peak RSS / distinct),
  `agrees_with_golden`, `registered_divergence` (when the counts match a
  registered entry instead), `stopped`.
- `liveness_selection`: how the temporal models were found (a probe run of
  every agreeing model with a `PROPERTY`, keeping reports that say
  `check_kind: temporal`). This list is also the way to tell temporal from
  safety models in D4.

### D3: `2026-11-18-release-d3-store-08a14fba.json`

- `rows.<model>.fp` and `.exact` (3 repetitions, store order alternating):
  `wall_ms` (median), `wall_range_ms`, `peak_rss_bytes` (median), `rss_range`,
  `options_store` (what the report says ran), `check_kind`, `verdict`,
  `distinct`, `depth`, `agrees_with_golden`.
- `rows.<model>.exact_over_fp`: `wall` and `rss` ratios of the medians.
- Model keys here drop `specifications/` and `.cfg`; the CoffeeCan safety-only
  config is spelled out in its key.

### D4: `2026-11-18-release-d4-scaling-08a14fba.json`

- `nodes`: what `od-1`..`od-4` and `spot-1`..`spot-5` were.
- `rows.<cfg>.w1|w4|w8`: `reps`, `wall_ms_median`, `wall_ms_range`,
  `peak_rss_median` (bytes), `stopped` (count of stopped repetitions),
  `all_correct`, `all_correct_or_registered`, `registered_divergence` (entry
  ids, when present), `workers_used` (from the report), `sequential_reason`
  (present only when a report gave one; none did), `verdicts`, `unsupported`
  (refusal text, when the cell refused), `runs_rep_node_wall_ms` (one
  `[repetition, node, wall_ms]` triple per run).
- `rows.<cfg>.speedup4`, `.speedup8`: median over repetitions of the
  1-worker wall time divided by the same repetition's N-worker wall time.
  `speedupN_is_lower_bound` marks SlushLarge, whose 1-worker runs stopped.
- `summary`: `speedup4_median`, `speedup8_median`, `stopped_cells`,
  `not_correct_cells`, `unsupported_cells`, `registered_divergence_cells`,
  `cells_missing_reps`, and `SlushLarge_with_D1_one_worker_time` (speed-ups
  against D1's completed 1-worker SlushLarge time).

### D5: `2026-11-18-release-d5-throughput-08a14fba.json`

- `rows.startup.version|parse|check`: `runs` (50), `median_ms`, `p90_ms`,
  `max_ms`, `rc` (exit codes seen); `check` adds `peak_rss_bytes`.
- `rows.simulation.<cfg>`: `reps[]` (`wall_ms`, `exit_code`, `peak_rss_bytes`,
  `generated`, `stopped`), `result`, `traces_started`, `generated`,
  `stopped_elapsed_ms`, `traces_per_second`, `states_per_second` (both from
  `stopped.elapsed_ms`, the budget clock), `note`.
- `rows.trace_validation.<kind>|<cfg>` (`validate` or `sample`): `runs`,
  `median_ms`, `max_ms`, `total_ms`, `results` (result to count), `rc`,
  `trace_states`, `states_per_second`.

### D6: `2026-11-18-release-d6-stop-latency-08a14fba.json`

- `rows.time_budget.<model>|workers=<n>|--time-limit <t>` (5 repetitions):
  `wall_ms_median`, `exit_codes`, `phases` (stop phase from the report),
  `latency_ms_median`/`_max` (wall minus load time minus the limit),
  `wall_minus_limit_ms_median`/`_max`, `stopped_elapsed_over_limit_ms_max`
  (the report's own stop clock minus the limit), `load_ms` (median wall of the
  `--max-states 1` runs at that worker count), `runs[]` with the same fields
  per run plus `resource`, `stopped_elapsed_ms` and `distinct`.
- `rows.state_depth_ceilings.<model>|workers=<n>|<flag> <value>` and
  `rows.temporal_ceilings...` (3 repetitions): `wall_ms_median`,
  `exit_codes`, `phases`, `overshoot` (distinct or depth beyond the ceiling),
  `distinct`, `depth`, `verdict`, `runs[]`.

### D7: `2026-11-18-release-d7-macos-08a14fba.json`

- `rows.benchmark_set.<bench-name>`: `model` (corpus config), and per arm
  `dive-default-w1`, `dive-default-w4`, `tlc-w1`, `tlc-w4`: `reps`,
  `wall_ms_median`, `wall_ms_range`, `peak_rss_median` (bytes),
  `all_correct`, `workers_used`; `dive_over_tlc_w1`, `dive_over_tlc_w4`
  (ratios of medians).
- `rows.startup.version|parse|check`: `n`, `median_ms`, `p90_ms`, `max_ms`.
- `summary`: geomean and max dive/TLC at 1 and 4 workers.

### S7: `2026-11-19-release-s7-sail-vs-tlc-ce94df74.json`

- `sources.<label>`: the Sail commit, a note and `worker_sha256` for each Sail
  build: `before` (Sail PR #763 head), `cache`, `mimalloc`, `final` (the
  shipped fix) and `lto` (an experiment). `dive_backend` and `dive_cli`: the
  dive revision Sail embeds and the CLI built from it.
- `tlc`: oracle, jar and JRE digests, and the TLC command.
- `reps` (5) and `order` (how the path order rotated).
- `latency.<label>.<fixture>.<path>` for `tlc`, `dive`, `embedded`,
  `execute`, `serve`, `mcp`: `wall_ms` (`median`/`min`/`max`/`n`), `agrees`,
  `peak_rss_kb` (largest process of the tree) or `daemon_hwm_kb` (warm
  daemon). Each fixture adds `outcome`, `completion`, `distinct`.
- `loop.<label>`: 100 checks over four small fixtures; per path `total_ms`
  (spread), `per_job_median_ms`, `per_job_p90_ms`, `disagreements`, and
  `mcp_init_ms`, `daemon_start_ms`, `daemon_hwm_kb` where they apply.
- `through.<label>`: `tlc-c2` and `serve-c2` (`jobs_per_s` spread,
  `total_ms`, `disagreements`); `serve-c8` records jobs refused with
  `SAIL6201 busy`, not a throughput figure.
- `first_job`: the first job after installing a worker and the one after it.
- `only`: Sail-only paths (a bounded exact-store check, three trace
  validations), timed, never turned into a multiple.
- `tlc_over_path_paired_median.<label>`: TLC time over path time within each
  repetition, then the median, per fixture and for the loop and throughput.
- `disagreements_total`: jobs in every path that disagreed with the golden
  (zero).

## Caveats for later readers

- **D4 repetitions ran on different nodes** of the same type, image and setup.
  `runs_rep_node_wall_ms` names the node of every run. Within one repetition
  the 1-, 4- and 8-worker runs of a model ran on the same node, so a
  speed-up compares like with like, with one exception: SlushLarge's third
  repetition ran each worker count on a different spot node.
- **D4 does not mark temporal models.** Use D2's `rows.liveness` keys (the
  reports said `check_kind: temporal`) to split D4 into temporal and safety
  models. The temporal ones mostly do not scale; see
  [Open finding: #1028](../../../docs/developer/release-performance-results.md#open-finding-1028).
- **Stopped runs are kept.** D4 SlushLarge at one worker stopped at the
  3,570 s limit in all three repetitions, so its speed-ups are lower bounds;
  D5 bosco simulation stopped at its 16 GiB budget. Neither was retried.
- **Registered divergences.** D2 `MCKVsnap` (D-005) and `SpanTreeRandom`
  (D-002), and D4 at one worker `MCCRDT`, `VoteProof`, `MCKVsnap` and
  `SpanTreeRandom`, differ from the golden counts exactly as registered in
  `conformance/expected-divergences.toml`; the records say so per cell.
  Generated counts at more than one worker are diagnostics only (ADR 0086).
- **D4 refusals.** `MCKVsnap` and `SpanTreeRandom` refuse temporal checking
  at 4 and 8 workers (#1020); their `w4`/`w8` cells hold the refusal, not a
  time.
- **D2 TLC memory** is the JVM's footprint at `-Xmx3g`, not TLC's minimum.
  D2's second liveness batch was split across the small node and one shard
  node; the raw files say which run ran where.
- **D6 negative latencies** for `ElevatorLivenessLarge`: its load run itself
  stops in graph capture, so subtracting it overstates load. Use
  `wall_minus_limit_ms` there.
- **D7 is informational.** It ran on a shared laptop (one-minute load 2.9 to
  12.6), with an arm64 binary of the same source (different SHA-256), a reduced
  selection, at 1 and 4 workers. Do not compare it with the bench nodes.
- **S7 is a different setup.** A virtualized c7i.8xlarge on a shared physical
  host, SMT on, frequency not controlled, no CPU binding, a different dive
  revision (`3d5923a1`) from D1 to D7, and Sail's fixed policy (exact store,
  one worker). Its multiples are within-repetition TLC/path ratios on small
  fixtures; its large-model statements are composed from D1, not measured.
- **D1 band pairs** use a Core budget of 2.5x TLC, made by scaling the TLC
  timings passed to the harness by 1.25; the record's `deviations` lists this
  and the other departures from the procedure (#983).

## Mining the records

The records are plain JSON; Python's standard library is enough. For example,
the D4 temporal/safety split:

```sh
python3 - <<'EOF'
import json
P = "conformance/reports/perf/"
d4 = json.load(open(P + "2026-11-18-release-d4-scaling-08a14fba.json"))["rows"]
live = json.load(open(P + "2026-11-18-release-d2-memory-08a14fba.json"))["rows"]["liveness"]
for cfg, row in sorted(d4.items(), key=lambda kv: kv[1]["speedup8"] or 0):
    kind = "temporal" if cfg in live else "safety"
    print(f'{kind:8} {row["speedup8"]} {cfg}')
EOF
```

To compare a later release, build its records with the same scripts and
compare per-model `ratio` (D1), `speedup8` (D4) or medians (D2, D3, D5, D6) by
model key; the keys are stable corpus config paths.

## Large-model structure and edit reuse

The original CCF comparison records matched exact checks at one and eight
workers, resource stops at the larger bound, and certified cursor lengths.
[Core frame narrowing on original CCF](2026-11-21-ccf-core-frame-narrowing.json)

The packed safety follow-up records cold and warm source-generated checks,
native peers, exact orbit ranks, packed aggregate actions, temporal product
rows and profile-guided compilation on two dedicated CPU hosts.
[Packed safety benchmark observations](2026-11-20-packed-core-panel.json) ·
[Methods and results](../../../docs/internal/survey/verification-workloads.md#packed-states-and-complete-actions)

[Next large-model experiments and their baselines](../../../docs/developer/large-models.md#next-experiments).

[Transaction-commit domain growth](2026-11-20-tcommit-domain-growth.json) records four complete searches, including 128 resource managers.

The [seven-key KVS fixed points](2026-11-20-seven-key-reachability.json) give arbitrary-precision counts for the same source with and without transaction symmetry.

The November 19 engineering runs separate the cost of storing more states, the cost of expanding more successor choices, and the cost of repeating a previous check. These records preserve the measurements behind [#799](https://github.com/Chelis-Lang/dive/issues/799), [#1028](https://github.com/Chelis-Lang/dive/issues/1028) and [#1034](https://github.com/Chelis-Lang/dive/issues/1034). They use development builds. Quote the model, checker, workload, units and host with any number.

| Question | Record | What to look up |
|---|---|---|
| Where did explicit Core run out of memory? | [Explicit storage and scheduling](2026-11-19-large-model-explicit.json) | `fingerprint_resize`, `predecessors` and `runs.local-original`. The resize probe inserted 402,653,185 keys. The original KVS Large run finished in 6,693.531 s with 594,616,815 distinct states and sampled 18.31 GiB RSS plus swap. |
| Can the independent validator replay large labeled counterexamples? | [KVS finite witness replay](2026-11-21-kvs-independent-replay.json) | Identical two-state words on the original 16- and 102-transaction sources, changed-state and prior-source rejections, heterogeneous configuration controls and seven paired ordinary checks. |
| What does the five-piece Car Talk holdout actually compute? | [Expression, closed-form bound and pinned reproducer](../../../docs/developer/large-models.md#car-talk-evaluates-a-constant-expression) | All 243 signed weights, the unique solution, smaller TLC controls and why this configuration has no reachable-state count. |
| How much does a finite function frame save in CCF? | [Two upstream edits, original and enlarged models](2026-11-21-ccf-function-frames.json) | Terminal Core and TLC comparisons, source digests, named refusals and the equivalence controls. |
| Does a large proof tree slow protocol iteration? | [LuPastry+ source size, analysis and executable controls](2026-11-21-pastry-source-scale.json) | 35,029 source lines, matched frontend acceptance and diagnostics, routing-digit definitions, eight-address message membership and split-action plans at 16 addresses, 32/64-address parallel invariant checks and the original 128-address action before and after the fold-budget fix. |
| Can the printed Pasture model be reproduced? | [Five Pasture configurations](2026-11-21-pasture-node.json) and [reconstruction instructions](../../../docs/developer/large-models.md#reproduce-the-pasture-node-model) | Exact Core and pinned TLC reproduce the published counts and depths on one CPU. |
| How does table layout change labeled reachability and induction? | [Shared transaction/key coordinates](2026-11-21-symbolic-table-orders.json) | Complete 3.55-trillion-state four-key search, 102-slot induction, identical-bit-order checks, failed orders, negative and stale predicate controls. |
| Does the symbolic checker count beyond 64 bits on a coupled transaction protocol? | [Six-key KVS with three transactions](2026-11-20-isolated-symbolic-growth.json) | Complete fixed point, full integer counts, transaction-history cross-check and the stopped five-key frontier control. |
| What happens when the EmergencyCall number sets grow? | [Four full safety checks and native controls](2026-11-21-emergencycall-growth.json) | Up to 570,694,082,176 distinct states, plus a validated seven-state F1 counterexample and a constrained configuration with an empty Init. |
| How expensive is independent replay with thousands of named processes? | [Transaction Commit concrete table and pair validation](2026-11-21-table-pair-validation.json) | Genuine traces, forged controls, ordinary sparse table lookup, exact quantifier behavior and retained memory. |
| Which obligations hold in the released Disk Synod model? | [Configured Isabelle proof and source controls](2026-11-21-disk-synod-invariants.json) | Five history invariants and deadlock freedom, four exact Core/TLC controls, and an independently replayed conflicting decision when quorums cease to intersect. |
| How many states does the full EmergencyCall initial predicate reach? | [Exact finite-value counts, 64 cells and native controls](2026-11-21-emergencycall-finite-values.json) | 49,911,672,768 reachable states at depth 44, four published controls, independently computed cell counts and terminal ordinary-store measurements. |
| Can the symbolic checker preserve record-valued PaxosLease symmetry? | [Closed reduced configurations, group and memory controls](2026-11-21-paxoslease-symmetry.json) | Matches ordinary exact Core and TLC on separate proposer and acceptor permutations. |
| Which published workloads were reproduced on the same host? | [Sixteen EmergencyCall cells](2026-11-20-emergencycall-reference.json) and [primary literature](../../../docs/developer/large-models.md#published-model-sizes-and-reproducible-comparisons) | Four original configurations, one and eight workers on Core and TLC. Every final count, depth and verdict matches the authors’ released run. The same record contains exact results for four closure-checked f3 partitions and a mutable-partition counterexample. |
| Can a new, noninductive invariant be checked across processes? | [Five-pair Sail measurements](https://github.com/Chelis-Lang/sail/blob/df6ffaca82b3aacdea301616411de1c0c726b12f/docs/validation/data/2026-11-20-symbolic-predicates.json) | Five-key KVS, 47.79× median paired speedup. Includes the false three-transaction control and real CCF revisions. |
| What changed with more workers or less buffering? | [Worker observations, prefixes and tradeoffs](2026-11-19-large-model-explicit.json) | `runs` contains the CoffeeCan and Huang worker matrix, PConProof and KVS Medium and the fixed KVS prefixes. `safety_pool_trials` includes the interleaved trial in which pooling ordinary safety workers cost time. |
| Which models compress under symbolic checking? | [Finite safety and edit samples](2026-11-19-large-model-symbolic.json) | Compare `runs.shape-grouped` and `runs.shape-path` for field order, `growth-5` and `growth-large3v` for coupling, `source-edits` for an invariant change, and `standalone-proof` for checking a candidate predicate independently. `c1cs_postdecision` covers only the specified reachable regions. |
| How variable were symbolic times across runs? | [Additional timed integration trials](2026-11-19-large-model-symbolic-controls.json) | Each trial lists the observed configuration names and measured steps in their original order. Configurations in different test cases can share a filename. Successful, violated and inconclusive cases have different semantics. |
| Did merged ordinary Core finish the unmodified KVS Large model? | [Complete eight-worker search](2026-11-20-kvs-large-main.json) | 6,596.232 s wall and 18.26 GiB peak sampled RSS plus swap on the shared workstation. The record binds the binary to the source, the exact input bytes and the original 24 GiB / 7,200 s budget. |
| Do the methods agree on one machine? | [Same-host KVS Large comparison](2026-11-19-large-model-ec2.json) | One EC2 m6id.4xlarge, one checker at a time, 600 s and 24 GiB. Core and TLC at one and eight workers reached the deadline. The source-driven symbolic search finished in 200.27 s. TLC counters are its periodic samples. |
| How much does Sail save when maintenance repeats a check? | [Sail S7 methods and results](https://github.com/Chelis-Lang/sail/blob/89e61512cbcce6be425fcff4959504db33d9eed3/docs/validation/temporal-reuse-s7.md) | The 1,330× mutex rerun and 3.34× mixed-loop speedups compare exact receipts with the v0.3.0 source on c7i.4xlarge. Substantive edits took the fresh route. |
| Which properties fail on the original KIP-996 election loop? | [Original CFG fairness, all properties and concrete replay](2026-11-21-kafka-liveness.json) | Two properties fail on a fair translating execution. Includes all 255 action instances, a starved control, and independent native replay through 16 laps. |
| Can a coordinator and several participant-indexed fields be counted together? | [TwoPhase product and direct checks](2026-11-21-two-phase-product.json) | Original exact Core and TLC through ten managers, labeled counts and depth at one million, three transition edits and a replayed unsafe commit. |
| How large is the reconstructed tunable-set FIFO history graph? | [Exact counts, explicit controls and published times](2026-11-21-tunable-set-history.json) | The three-replica, three-element reconstruction has 2.358 trillion symmetry representatives. Includes proofs of the five reconstructed invariants, edit diagnostics, 27.12-million-state exact controls and the separate published disk stop. |
| How many labeled states does a SlateDB symmetry representative cover? | [Orbit histograms and unreduced controls](2026-11-20-slatedb-wal.json) · [Method and domain assumptions](../../../docs/user/checking.md#count-labeled-states-under-symmetry) | Direct exact search confirms all 350,341,877 labeled states and 1,360,205,173 generated occurrences predicted from 30,114,878 representatives. |
| Can Dive reproduce Flint’s published three-writer storage checks? | [Archived model, oracle, commands and measurements](2026-11-21-flint-leanp1.json) | The 9.821-billion fingerprint result, exact and TLC controls at 39–48 million states, and 83,107,713 duplicate guard witnesses measured on the full three-writer graph. The weighted compiled traversal differs from Core by 26 generated occurrences and seven distinct fingerprints. |
| Which published ZooKeeper searches can be reproduced from the released module? | [Paper parameters, input recipe and paired measurements](2026-11-20-zookeeper-reference.json) | Generated and distinct counts, depth, worker count, wall time, maximum RSS and restart-bound discrepancies. |
| Which of the thirteen historical non-finishes closed, and where do the others stop? | [Thirteen model outcomes and isolated larger attempts](2026-11-20-large-model-outcomes.json) | Original CFGs, stated profile and budgets, external OOMs, retained frontiers, closed-input controls and source-bound solutions. |
| Which large published search has a full exact repeat? | [Pasture node](2026-11-21-pasture-node.json) · [PaxosLease](2026-11-20-paxoslease-reference.json) | Pasture reached 4,234,887,880 distinct states at depth 53. Core fingerprint took 9,463.54 s and 56.13 GiB, pinned TLC 10,153.63 s and 174.11 GiB, and Core exact 17,024.83 s and 225.38 GiB. |
| What makes large temporal cases tractable? | [EWD998 finite ring](2026-11-21-ewd998-fair-ring.json) · [Failure detector](2026-11-21-environment-detector.json) · [Blocking queue](2026-11-21-poison-queue.json) | Finite closure and fair rank, a safety counterexample on an infinite graph with two proved temporal properties, and a terminal deadlock. The grown five-node ring reaches 34,276,406,948 states, including 129,196 terminated states that require a fair continuation past the graph constraint. |
| Which consensus results are proofs or reachable lower bounds? | [Byzantine Paxos configured invariant and state lower bound](2026-11-21-bpcon-property.json) ([reproduce](../../../docs/developer/large-models.md#prove-the-configured-invariant)) · [Sailfish DAG](2026-11-21-sailfish-dag.json) · [c1cs](2026-11-21-c1cs-source-proof.json) · [etcd terms](2026-11-21-etcd-election-terms.json) · [Kafka](2026-11-21-kafka-liveness.json) | Source identities, proof or reachability argument, small explicit controls and the limits of each result. |
| What happens when a released input grows? | [EmergencyCall exact cells](2026-11-21-emergencycall-finite-values.json) · [Larger domains](2026-11-21-emergencycall-growth.json) · [LuPastry+ maintenance and size](2026-11-21-pastry-source-scale.json) | Each record identifies the changed constant or domain and the original properties. |
| What do all 13 models without a completed TLC run actually report? | [Bounded Core results on merged main](2026-11-20-no-golden-observations.json) | 26 one- and eight-worker attempts, five seconds and 2 GiB each. Filter `rows` by model and worker for final counters or the named feature refusal. All are ungraded. |
| Which substantive edits can reuse an exact native check? | [Invariant subset](../../studies/s7-invariant-projection.json), [ordered states](../../studies/edited-invariant-recheck.json), [labelled graph](../../studies/edited-temporal-recheck.json) | Original capture cost, current bind and analysis, matched-resource fresh comparisons and earlier controls. The [scope table](../../../docs/developer/large-models.md#which-evidence-can-decide-a-source-edit) distinguishes the three contracts. |
| When do temporal capture workers add overhead? | [54 fixed-frequency runs with reported worker widths](2026-11-20-temporal-worker-selection.json) | Same-host measurements at 1/4/8 workers on original CoffeeCan, Flash and Dijkstra configurations. |
| Can a transition edit reuse shortest-path layers? | [KVS Medium and Large edit sequences](2026-11-20-isolated-symbolic-growth.json) | The changed_transition_layers section compares a global reset, its removal and a shortcut against fresh runs. |
| What limits the commit of wide safety states? | [Flint, SlateDB, KVS and PConProof](2026-11-21-safety-worker-commit.json) | Worker scheduling and already-interned row reuse measured separately, with completed and bounded controls. |
| How does successor fanout affect peak memory? | [132 fixed-prefix trials and the 100M disk comparison](2026-11-20-high-fanout-waves.json) | Three cohorts compare resident, disk handles, independent chunks, batch width and reclamation frequency. All four prefixes have matched counts. |
| What did writing the frontier to disk buy at equal counts? | [Six fixed 300,000-state prefixes](2026-11-20-six-fixed-prefixes.json) | All six memory-stopped models, four resident/disk pairs with matching counters. Compare process peak RSS, wall time, queue size, spill bytes and the persistent-value-store lower bound. |
| Is the Large result reproducible with R2 CPU controls? | [Reference hardware, Core/TLC/symbolic](2026-11-20-kvs-large-reference.json) | Five sequential 600-second, 24 GiB cells on a dedicated c7i.metal-24xl. `cells` holds each wall time and peak RSS, exact completed symbolic counts, final Core prefixes and all TLC progress samples. |
| How did source changes, field growth and negative controls behave? | [Source-checker qualification](2026-11-20-symbolic-qualification.json) | `trials.final-large-growth` includes the 131.080 s Large fixed point and 35.952 ms holding edit. `resource-smoke` covers the final resource-status change. Rows preserve the driver order. |
| Can a completed native temporal proof survive an aggregate-state property removal? | [First-property timings and execution counts](../../studies/s7-temporal-first-projection.json) | Seven paired S7 trials compare an unchanged first temporal product against the fresh checker. Includes cold-process and previous-binding timings, plus the actual atom counts for 4,408 states. |
| How much of a real multi-module edit history can Sail reuse? | [Installed Disruptor and CCF trials](https://github.com/Chelis-Lang/sail/blob/b8f084052d08928f234684f26cc9b1fea4ad950c/docs/validation/data/2026-11-20-installed-public-edit-histories.json) and [two-writer trace controls](https://github.com/Chelis-Lang/sail/blob/f19082a2b07bd5ba3fe107cfd84033eb1cb42768/docs/validation/data/2026-11-20-model-valued-witness.json) | Nine chronological Disruptor checks, both unchanged MPMC roots, failed admissions, and an independently validated typed witness on the older source. |
| Can a later process prove a new KVS safety claim after changing the protocol or growing its domain? | [Installed six-key Sail session, changed actions and induction on larger domains](2026-11-21-sail-kvs-edits.json) | A 76,579-byte predicate proved a new claim in 0.40 s. An unsafe action produced an independently validated depth-two violation in 0.75 s. An explicit strengthening proved safety at 50 keys and two transactions in 2.35 s. |
| What do authenticated edits cost through the installed Sail CLI? | [Substantive edits on native Linux and Mac control](https://github.com/Chelis-Lang/sail/blob/e609cad17b45d6e8b3d1259d45561bd269a53041/docs/validation/data/2026-11-20-native-edit-session.json) and [integrated S7 samples](https://github.com/Chelis-Lang/sail/blob/e609cad17b45d6e8b3d1259d45561bd269a53041/docs/validation/data/2026-11-20-integrated-s7.json) | Five paired release trials include process startup, authenticated receipt lookup and witness validation. The 724,274-state mutex proof projected to its remaining invariants in 22.46 ms. The exact control repeats all 333 original S7 calls. |
| How much does Sail save after removing an actual temporal property on aggregate state? | [Final native edit trials](https://github.com/Chelis-Lang/sail/blob/main/docs/validation/data/2026-11-20-first-temporal-claim-native.json) and [333 exact-receipt calls](https://github.com/Chelis-Lang/sail/blob/main/docs/validation/data/2026-11-20-first-temporal-claim-s7.json) | Separate InternalMemory recheck took 15.565 ms versus 86.734 ms fresh. Mutex invariant removal took 20.347 ms versus 9.576 s. Each file identifies the native worker, every input, all five paired edit trials or all three repeat-workload trials. |
| How much does a resident safety frontier cost? | [All 16 matched runs and four handle distributions](2026-11-20-high-fanout-waves.json) | The `resident_rows` section includes both binaries, per-run wall time and peak RSS, admission queue sizes, and offline row layouts for SlateDB, DiskSynod, BPConProof and TLCSailfish2. |
| How much memory does each temporal edge need? | [Matched two-million-state prefixes](2026-11-20-temporal-edge-ordinals.json) | Three storage layouts, two models and three rotations. The count and queue boundary agree in all nine runs per model. Use the per-process RSS or the separately measured retained graph buffers. |
| What does exact equality cost at 242 million states? | [Complete resident store layout comparison](2026-11-20-slatedb-wal.json) | The `exact_membership_layout` trials report all three layouts, complete identity counts, peak RSS and elapsed time. |
| Which SlateDB WAL protocol was measured? | [Earlier model from TLAPS-Bench](2026-11-20-slatedb-wal.json) | `earlier_protocol_with_fix` records the separate, restart-free module and complete two- and three-writer controls with both engines. |
| How does the released SlateDB WAL configuration compare with its published estimate of 49 billion states? | [Pinned inputs, controls and storage comparisons](2026-11-20-slatedb-wal.json) · [Original TLC, exact Core and twelve matched counting runs](2026-11-21-slatedb-full-comparison.json) | Exact Core and pinned TLC exhausted 4,910,385,790 symmetry representatives, generated 23,680,348,760 occurrences and reached depth 139. Their times were 22,664.55 s and 49,123.76 s. Fingerprint Core took 14,122.37 s and counted one fewer representative. The author’s rounded count has no published terminal log. |
| How much work can a symbolic recheck reuse after deleting an action? | [PaxosLease deletion trials](2026-11-21-deleted-transitions.json) | Paired Base runs and a 76,749,192-state closure, with per-revision times, peak memory, rejected designs and differential controls. |
| How does Core compare on published consensus checks? | [PaxosLease reference runs and historical logs](2026-11-20-paxoslease-reference.json) | Original Base and quarantine (8 workers), Retry and RenewRetry (32 workers). Also includes source-edit and fresh comparisons, per-revision breadth-first layer reuse, explicit controls, one-core sparse symbolic runs and variable-order comparisons. |
| Can a closed CFG select a graph without ambient environment access? | [TestMCReachability graph controls and the 50-vertex liveness violation](2026-11-20-closed-graph-inputs.json) | Six matched closed CFGs, the graph 1a deadlock, and independently replayed Core and TLC counterexamples for graph 7. |
| Why did a labeled model exhaust memory exporting an unused permutation? | [Same-source export and observable-startup controls](2026-11-21-symbolic-source-binding.json) | Unused 12- and 16-transaction permutation domains exceeded a 1,100 MiB address cap before binding changed. |
| How stable are the large symbolic results on reference hardware? | [Six configurations, 18 whole-process runs](2026-11-20-isolated-symbolic-growth.json) | Exact arbitrary-precision counts and all wall, RSS and checker measurements for KVS and transaction commit on a fixed-frequency bare-metal host. |
| What did Sail's original 100-check experiment measure? | [All native S7 samples](https://github.com/Chelis-Lang/sail/blob/47b83af65fea95ffff6300dc4818a7c1d6016d6c/docs/validation/data/2026-11-19-native-s7.json) and [earlier exploratory samples](https://github.com/Chelis-Lang/sail/blob/47b83af65fea95ffff6300dc4818a7c1d6016d6c/docs/validation/data/2026-11-19-pre-release-s7.json) | Look up each closed input hash, fresh or replay status, foreground time and pairing for TLC, Sail 0.3.0 and the receipt build. |

### Reading the development records

The explicit record retains the actual binary digests. In the fixed-prefix runs, `base` used the original table, `shards` grew table shards independently, `packed` added packed parents, and `short` reduced candidate batches. `pred-spill` and `frontier-spill` forced the corresponding data to disk. `cliff-*` runs had a sampled whole-process memory limit. In the temporal matrix `packed` is the comparison build, `pool` reused workers, `claims` reused only the claim table, and `wave` reused both. `integrated` combines the retained changes, while `allpool` also pools ordinary safety expansion. The numeric suffix is the worker count on the named model. A `stopped` run records the exploration prefix and its queue.

`elapsed_ms` is the checker report's elapsed time. `sampled_rss_plus_swap_bytes` is the maximum of its process samples. `process_wall_s` and `max_rss_kib` come from the external process timer. In the symbolic record, `seconds` measures one operation, while the external timer can span multiple steps in one invocation. `final-analytic-*` and `kvs-symbolic-*` used a hand-translated KVS relation. Use `source-*` or `growth-*` for results from the TLA+ source. A completed fixed point reports generated and distinct counts and depth. An inductive safety proof establishes its obligations without enumerating the graph. The records retain both timing scopes where available.

For example, a documentation build can get the original explicit counts and the edited-property times directly with:

```sh
jq '.runs["local-original"] | {elapsed_ms, sampled_rss_plus_swap_bytes, stats}' \
  conformance/reports/perf/2026-11-19-large-model-explicit.json
jq '.runs["source-edits"].steps[] | {verdict, reused, seconds}' \
  conformance/reports/perf/2026-11-19-large-model-symbolic.json
```

## Simulation records

| File | Question | Source | Host |
|---|---|---|---|
| `2026-11-21-simulation-throughput-d04a3a88.json` | S4 (ADR 0162 section 14): `dive simulate` traces per second and startup against TLC `-simulate`, at 1, 4 and 8 workers, with and without temporal properties | `d04a3a88` | c7i.metal-24xl, fresh node, method as D1 to D6 |
| `2026-11-21-simulation-throughput-f685076b.json` | The S4 benches again after #1162 (temporal atoms on per-worker Core machines) | `f685076b` | c7i.metal-24xl, fresh node, same method |

Each bench samples the same number of traces of depth 100 at seed 0 in both
tools; TLC gets `num=N/W` because its count is per worker. Every run sampled
all its traces with exit 0. Without temporal properties, dive is 1.1x to 4.5x
faster than TLC at one worker and 1.4x to 3.8x faster at eight. dive scales
to 4.7x to 6.3x at eight workers, where TLC reaches 2.2x to 2.9x except on
`bosco` (6.3x). `MCBakery` does not scale in either tool: its 655,200
initial states take 4.5 s in dive and 6.0 s in TLC before the first trace.
Other startups for one trace are 9 to 100 ms against 370 to 610 ms. With
temporal properties, dive is 3.0x faster on `SimpleAllocator` and 1.8x on
`DijkstraMutex` at one worker, but 0.48x on `EWD840` (`TDSpec`), and the
`DijkstraMutex` and `EWD840` temporal runs scale only to 3.4x and 3.5x at
eight workers (TLC 5.9x and 4.2x). The record's `profile` names the cost:
temporal atoms are evaluated by the tree-walking `dive-eval` evaluator
rather than Core, and at eight workers clone and drop of shared evaluator
environments take 37 % of samples.

After #1162 (`f685076b`), every bench is ahead of TLC at 1, 4 and 8
workers. With temporal properties, `EWD840` is 3.6x, 4.6x and 5.4x faster
than TLC (21.4 s to 2.9 s at one worker), `DijkstraMutex` 8.0x, 7.4x and
5.7x, and `SimpleAllocator` 6.0x, 6.0x and 6.2x. Benches without temporal
properties are unchanged. dive's generated counts and per-property coverage
are those of the `d04a3a88` record and identical across worker counts. The
per-worker atom caches raise `DijkstraMutex`'s peak RSS at eight workers
from 173 to 350 MiB.

## Earlier records

Development-period measurements, kept as historical evidence. Their schemas
differ from file to file and from the release records, several were taken on
quieter or noisier hosts than the release nodes, and most are diagnostics
rather than acceptance. Read each file's own `purpose`, `scope` or `decision`
field before quoting a number.

| File | What it is |
|---|---|
| `2026-10-26-core-m0-x86.json` | M0 go/no-go: Core against the reference engine and TLC at one worker, with ablations |
| `2026-10-27-core-m1-x86.json` | M1 checkpoint: Core against the reference engine and TLC on the bench set at 1 and 8 workers |
| `2026-10-28-core-*.json`, `2026-10-28-core-tlc-guard-quiet-pair.json` | focused Core qualifications and A/B diagnostics (profiles, membership checks, memo tables, startup fold, Sailfish) |
| `2026-10-30-*.json`, `2026-10-31-*.json` | native temporal capacity observations and history-cost diagnostics |
| `2026-11-01-*.json` to `2026-11-03-*.json` | integrated performance diagnostics and single adoption experiments, including rejected ones |
| `2026-11-10-r2-procedure-e3835968.json` | the first complete R2 record (geomean 0.195x); the release record D1 repeats it on `08a14fba` |
