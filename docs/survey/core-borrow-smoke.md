# Evaluator-core borrow smoke measurement

The change borrows the evaluator core during recursive successor traversal,
action splitting and enabledness, removing ten temporary shared-pointer
clones. It preserves values, scopes, evaluation order and random streams.

Three paired repetitions on SimpleRegular alternated the baseline and
changed binaries. Every repetition matched the golden: 277,726 distinct
and 1,454,776 generated states. The [raw runs](../../conformance/reports/benchmarks/20261009-core-borrow-smoke.jsonl)
contain binary hashes, runner revision, host/load, pins, seed, CPU, wall
time and peak RSS. Each runner invocation used `--reps 1`; the file order
is baseline/changed, changed/baseline, baseline/changed.

- Baseline: main `7ceb59f`, binary SHA-256
  `715ddf1bc54f7c85d2aa0c7d4926751e8fa93c1900f2761d2ec3552b94a10366`.
- Changed: `72c15df9ce8f059349fbf4cab2ee852561740421`, binary SHA-256
  `059a2e94748d35969d79664ac105edd4d8cc21227da0dbcffcc90ac2f661aee7`.
  Both builds use the pinned release profile. Only the changelog was dirty
  while measuring; the source change is committed at that revision.
- Machine: Apple M3, eight CPUs, macOS/Darwin arm64, with other active
  workloads. Load averages ranged from 26 to 54.

| Pair | Baseline CPU s | Changed CPU s | Baseline wall s | Changed wall s |
|---|---:|---:|---:|---:|
| 1 | 15.02 | 15.67 | 98.15 | 114.07 |
| 2 | 16.80 | 15.27 | 159.76 | 105.42 |
| 3 | 14.94 | 15.13 | 116.27 | 122.47 |

These measurements show correctness and substantial wall-time noise. They
provide no credible speed-up claim; the M5 throughput and scaling targets
remain open. No TLC historical wall time is compared with current CPU time.

Reproduce with the two binaries built from the revisions above. Alternate
this command's `--dive-bin` between them, reversing the pair order on the
second repetition:

```sh
DIVE_CACHE_DIR=<verified-cache> cargo run --release -p dive-bench -- \
  --dive-bin <baseline-or-changed-binary> --filter SimpleRegular \
  --reps 1 --profile tlc --workers 1 --runs <raw-jsonl>
```

The complete standard-tier strict run also passed: 154 selected models,
83 agree and 71 explicit unsupported results, with no disagreements,
pending validations, timeouts or oracle errors. The generated
[report](../../conformance/reports/20261009T134501007-dive-small.json)
records the same changed binary hash above, two concurrent jobs and a fixed
2,400-second timeout. This run includes the slower EWD998 and TLCSailfish
models. It validates the cleanup; it does not establish a performance gain.

```sh
DIVE_CACHE_DIR=<verified-cache> cargo run --release -p dive-conformance -- \
  run --impl dive --dive-bin target/release/dive --impl-profile tlc \
  --tier small --workers 1 --jobs 2 --timeout 2400 --no-sany --no-summary
```
