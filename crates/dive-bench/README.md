# dive-bench

Run manual benchmarks of `dive check` against the pinned TLC. The ten-model
set retains the existing eight workloads and adds bounded Disruptor MPMC and
BufferedRandomAccessFile. Benchmarks never run in the per-change gate.

```sh
cargo build --release --locked -p dive-cli -p dive-bench
cargo run --release --locked -p dive-bench -- --list
```

On a reserved quiet machine, with the pinned cache fetched by the conformance
harness, a campaign can use:

```sh
target/release/dive-bench --profile both --workers 1,4,8 --reps 5 --tlc \
  --dive-revision <candidate-commit> --timeout 1800 --xmx 3g \
  --runs <external-run-directory>/runs.jsonl \
  --summary <external-run-directory>/summary.json
```

Keep the raw rows and console report outside the repository. To compare a
previous frozen build in the same campaign, add both
`--baseline-bin <previous-executable>` and `--baseline-revision <previous-commit>`.
The supplied revision is recorded as a user label; it is not inferred from the
runner checkout or proved by the executable hash. The candidate revision is
unknown when its optional label is omitted. Build and record both binaries from
clean pinned sources before the campaign; engine optimization follows the
consolidated kernel baseline.

TLC runs at every selected worker count, with the same effective cfg as both
dive profiles and the previous binary. The default is one worker, three
repetitions, both profiles, a 1800-second per-process timeout and TLC heap 3g.
`--tlc` enables the oracle arm; without it the TLC comparisons remain missing
and no TLC speed-up is reported. The default cfg is the original pinned input.
`--set <file>` selects another set; duplicate model IDs/names and duplicate
worker counts are rejected. Replacement configs must use an explicit name ending
in `Safety`.

Within a model, each repetition runs worker blocks containing TLC, then each
profile's previous/current binary pair. Odd repetitions reverse the complete
worker/arm order. Processes run one at a time. The schedule is deterministic
and recorded, and every run captures load before and after execution. This
counterbalance does not make a shared or thermally changing machine quiet.
Reserve the same physical cores and resource limits for all arms, record their
SMT/affinity and memory limits, and rerun a declared block after interference
rather than selecting favorable samples.

Every run must match the successful golden on verdict, distinct states, depth,
error class, exit and property. Generated counts must match at one worker;
at multiple workers they remain diagnostics (ADR 0086). Dive additionally
must report verified TLC success. Corpus integrity and typed golden pin
provenance are checked before executing. Oracle id, upstream commit and jar SHA,
ordered classpath artifact ids/SHA pins and module-library ids/commits must match
the configuration. Historical Java/platform details and physical paths remain
recorded context rather than equality requirements. Only successful counted goldens are
admitted; violation/error benchmarks need a separate scope and trace validation. Timeouts, refusals and failed or
unverified runs remain in the raw report and make the command fail (exit 1).
Invalid options, missing inputs or ambiguous/nonpositive timing data fail with
exit 2. `--list` and successful complete runs exit 0.

Speed-up is the median **within-repetition baseline/candidate wall ratio**, at
matched requested workers, reported for each profile. Scaling pairs the same
profile's one-worker and N-worker runs before taking the median; it requires
actual use of the requested workers. Requested/used workers and sequential
reasons remain visible. Missing, censored, refused and failed pairs have explicit
counts. A timeout is an incomplete observation, never a completed timing or
ratio. Partial-pair medians are labeled descriptive, and the equal-model-weight
geometric mean is absent unless every selected model has every planned pair.
Historical survey/golden timings never enter these denominators. CPU seconds
(user + sys), whole-process wall time, state rates, spreads and RSS are separate
quantities; startup/parsing/reporting and the JVM remain included in wall time.

JSONL schema 2 binds the binary hashes and source labels, runner revision/dirt,
corpus/oracle/Java/module-library pins, effective cfg/module/local-source and
golden hashes, worker use, arguments, heap, timeout, seed/fp and schedule/load.
It retains each dive child's reported options. Timed dive runs remove inherited
`DIVE_COMPAT` and `DIVE_CHAOS`; the command selects compatibility explicitly.
The optional JSON summary reports all selected arms, comparisons and aggregate
population counts. It does not certify performance target acceptance. ADR 0006's
>=400,000-distinct-state population flag is reported for each model; the scaling
population is descriptive; this summary does not assess the scaling target. All ten model
rows are retained, including depth-one initial-state workloads and BRAF's small
but duplicate-heavy graph. The runner does not redefine the >=2×/3.5× targets.

`CoffeeCan1000BeansSafety` explicitly replaces the original config with Spec and
TypeInvariant, dropping four temporal properties. Its effective cfg is hashed
and the pinned TLC checks the identical safety workload. The original golden
provides counted-success expectations, not credit for the original temporal
properties. Its timings cannot be compared with the historical full-liveness
survey, and the original CoffeeCan remains part of liveness/conformance work.

This runner repair has only synthetic and list/option validation; no new
performance baseline or target credit is claimed. Startup, liveness, compiled
tier, memory and the remaining roadmap require their own measurements.

## Historical parallel scaling (WP-D3, 2026-10-09, indicative)

Results and caveats are in ADR 0015, "Implementation and measurements (WP-D3)".
In short:
- the machine was heavily shared (load average 20–44 on 8 cores);
- dive's results are byte-identical at every worker count;
- wall-clock gains from 1 to 8 workers were 1.9–2.4× (harness runs: 3.3–4.6×);
- CPU time grows about 2.8× from 1 to 8 workers, which is the main target for
  WP-G2.

### Method

For each model, one repetition, in this order:

1. `dive check <Module>.tla --config <cfg> --compat=tlc --seed 0 --fp 0
   --workers W --output json`, for W in 1, 2, 4, 8, with the harness's module
   path (the CommunityModules jar's modules, the Apalache and TLAPS module
   libraries).
2. The pinned TLC (`tlc-2026.10.06.014338`, Temurin 17.0.20.1+1):
   `java -Xss8m -XX:+UseParallelGC -cp tla2tools.jar:CommunityModules*.jar:<libraries>
   tlc2.TLC -workers W -fp 0 -seed 0 -metadir <tmp> -config <cfg> <Module>.tla`,
   for W in 1 and 8.

Wall time is the whole process: start-up, parsing and checking, and the JVM
for TLC. Every run's generated and distinct counts are checked against the
others. CPU time comes from `/usr/bin/time -p`, with the load average recorded
alongside each run.

Models: `TeachingConcurrency/SimpleRegular`, `lamport_mutex/MCLamportMutex`
and `btree/MCbtree` from the survey's benchmark set
(`docs/survey/competitive.md`), plus `Disruptor/MCDisruptor_MPMC` for a deep
state graph (Examples `6a7b26a`). The survey's other models are left out:
- `CoffeeCan1000Beans` checks liveness, which dive does not do yet;
- `MCBakery` and `GameOfLife` have depth 1: every state is an initial state,
  so the search is the inline initial-state phase and does not exercise the
  parallel engine.

### To repeat

- Use a quiet machine.
- Interleave the tools and repeat at least three times, as the survey's
  method does.
- Record the machine, the load and the commit.
