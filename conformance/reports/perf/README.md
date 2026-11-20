# Performance records

Compact JSON records of dive performance measurements. Each record identifies the experiment and its sources and hosts. Raw release runs (per-run JSON lines,
console output, reports) are not in the repository; they are tarballs in the
dive evidence archive bucket (us-east-1), and each record cites its tarball by
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

[Transaction-commit domain growth](2026-11-20-tcommit-domain-growth.json) records four complete searches, including 128 resource managers.

The [seven-key KVS fixed points](2026-11-20-seven-key-reachability.json) give arbitrary-precision counts for the same source with and without transaction symmetry.

The November 19 engineering runs separate the cost of storing more states, the cost of expanding more successor choices, and the cost of repeating a previous check. These records preserve the measurements behind [#799](https://github.com/Chelis-Lang/dive/issues/799), [#1028](https://github.com/Chelis-Lang/dive/issues/1028) and [#1034](https://github.com/Chelis-Lang/dive/issues/1034). They use development builds. Quote the model, checker, workload, units and host with any number.

| Question | Record | What to look up |
|---|---|---|
| Where did explicit Core run out of memory? | [Explicit storage and scheduling](2026-11-19-large-model-explicit.json) | `fingerprint_resize`, `predecessors` and `runs.local-original`. The resize probe inserted 402,653,185 keys. The original KVS Large run finished in 6,693.531 s with 594,616,815 distinct states and sampled 18.31 GiB RSS plus swap. |
| What does the five-piece Car Talk holdout actually compute? | [Expression, closed-form bound and pinned reproducer](../../../docs/developer/large-models.md#car-talk-evaluates-a-constant-expression) | All 243 signed weights, the unique solution, smaller TLC controls and why this configuration has no reachable-state count. |
| Which published workloads were reproduced on the same host? | [Sixteen EmergencyCall cells](2026-11-20-emergencycall-reference.json) and [primary literature](../../../docs/developer/large-models.md#published-model-sizes-and-reproducible-comparisons) | Four original configurations, one and eight workers on Core and TLC. Every final count, depth and verdict matches the authors’ released run. |
| Can a new, noninductive invariant be checked across processes? | [Five-pair Sail measurements](https://github.com/Chelis-Lang/sail/blob/df6ffaca82b3aacdea301616411de1c0c726b12f/docs/validation/data/2026-11-20-symbolic-predicates.json) | Five-key KVS, 47.79× median paired speedup. Includes the false three-transaction control and real CCF revisions. |
| What changed with more workers or less buffering? | [Worker observations, prefixes and tradeoffs](2026-11-19-large-model-explicit.json) | `runs` contains the CoffeeCan and Huang worker matrix, PConProof and KVS Medium and the fixed KVS prefixes. `safety_pool_trials` includes the interleaved trial in which pooling ordinary safety workers cost time. |
| Which models compress under symbolic checking? | [Finite safety and edit samples](2026-11-19-large-model-symbolic.json) | Compare `runs.shape-grouped` and `runs.shape-path` for field order, `growth-5` and `growth-large3v` for coupling, `source-edits` for an invariant change, and `standalone-proof` for checking a candidate predicate independently. `c1cs_postdecision` covers only the specified reachable regions. |
| How variable were symbolic times across runs? | [Additional timed integration trials](2026-11-19-large-model-symbolic-controls.json) | Each trial lists the observed configuration names and measured steps in their original order. Configurations in different test cases can share a filename. Successful, violated and inconclusive cases have different semantics. |
| Did merged ordinary Core finish the unmodified KVS Large model? | [Complete eight-worker search](2026-11-20-kvs-large-main.json) | 6,596.232 s wall and 18.26 GiB peak sampled RSS plus swap on the shared workstation. The record binds the binary to the source, the exact input bytes and the original 24 GiB / 7,200 s budget. |
| Do the methods agree on one machine? | [Same-host KVS Large comparison](2026-11-19-large-model-ec2.json) | One EC2 m6id.4xlarge, one checker at a time, 600 s and 24 GiB. Core and TLC at one and eight workers reached the deadline. The source-driven symbolic search finished in 200.27 s. TLC counters are its periodic samples. |
| How much does Sail save when maintenance repeats a check? | [Sail S7 methods and results](https://github.com/Chelis-Lang/sail/blob/89e61512cbcce6be425fcff4959504db33d9eed3/docs/validation/temporal-reuse-s7.md) | The 1,330× mutex rerun and 3.34× mixed-loop speedups compare exact receipts with the v0.3.0 source on c7i.4xlarge. Substantive edits took the fresh route. |
| What do all 13 models without a completed TLC run actually report? | [Bounded Core results on merged main](2026-11-20-no-golden-observations.json) | 26 one- and eight-worker attempts, five seconds and 2 GiB each. Filter `rows` by model and worker for final counters or the named feature refusal. All are ungraded. |
| Which substantive edits can reuse an exact native check? | [Invariant subset](../../studies/s7-invariant-projection.json), [ordered states](../../studies/edited-invariant-recheck.json), [labelled graph](../../studies/edited-temporal-recheck.json) | Original capture cost, current bind and analysis, matched-resource fresh comparisons and earlier controls. The [scope table](../../../docs/developer/large-models.md#which-evidence-can-decide-a-source-edit) distinguishes the three contracts. |
| When do temporal capture workers add overhead? | [54 fixed-frequency runs with reported worker widths](2026-11-20-temporal-worker-selection.json) | Same-host measurements at 1/4/8 workers on original CoffeeCan, Flash and Dijkstra configurations. |
| What did writing the frontier to disk buy at equal counts? | [Six fixed 300,000-state prefixes](2026-11-20-six-fixed-prefixes.json) | All six memory-stopped models, four resident/disk pairs with matching counters. Compare process peak RSS, wall time, queue size, spill bytes and the persistent-value-store lower bound. |
| Is the Large result reproducible with R2 CPU controls? | [Reference hardware, Core/TLC/symbolic](2026-11-20-kvs-large-reference.json) | Five sequential 600-second, 24 GiB cells on a dedicated c7i.metal-24xl. `cells` holds each wall time and peak RSS, exact completed symbolic counts, final Core prefixes and all TLC progress samples. |
| How did source changes, field growth and negative controls behave? | [Source-checker qualification](2026-11-20-symbolic-qualification.json) | `trials.final-large-growth` includes the 131.080 s Large fixed point and 35.952 ms holding edit. `resource-smoke` covers the final resource-status change. Rows preserve the driver order. |
| Can a completed native temporal proof survive an aggregate-state property removal? | [First-property timings and execution counts](../../studies/s7-temporal-first-projection.json) | Seven paired S7 trials compare an unchanged first temporal product against the fresh checker. Includes cold-process and previous-binding timings, plus the actual atom counts for 4,408 states. |
| What do authenticated edits cost through the installed Sail CLI? | [Substantive edits on native Linux and Mac control](https://github.com/Chelis-Lang/sail/blob/e609cad17b45d6e8b3d1259d45561bd269a53041/docs/validation/data/2026-11-20-native-edit-session.json) and [integrated S7 samples](https://github.com/Chelis-Lang/sail/blob/e609cad17b45d6e8b3d1259d45561bd269a53041/docs/validation/data/2026-11-20-integrated-s7.json) | Five paired release trials include process startup, authenticated receipt lookup and witness validation. The 724,274-state mutex proof projected to its remaining invariants in 22.46 ms. The exact control repeats all 333 original S7 calls. |
| How much does Sail save after removing an actual temporal property on aggregate state? | [Final native edit trials](https://github.com/Chelis-Lang/sail/blob/main/docs/validation/data/2026-11-20-first-temporal-claim-native.json) and [333 exact-receipt calls](https://github.com/Chelis-Lang/sail/blob/main/docs/validation/data/2026-11-20-first-temporal-claim-s7.json) | Separate InternalMemory recheck took 15.565 ms versus 86.734 ms fresh. Mutex invariant removal took 20.347 ms versus 9.576 s. Each file identifies the native worker, every input, all five paired edit trials or all three repeat-workload trials. |
| How does Core compare on published consensus checks? | [PaxosLease reference runs and historical logs](2026-11-20-paxoslease-reference.json) | Original Base and QuarantineEqualsProposer, eight workers and four completed processes. Includes matching input hashes, counts, wall time and RSS. |
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
