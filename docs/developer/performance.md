# Performance measurement

Use the pinned inputs and runner described in
[`crates/dive-bench/README.md`](../../crates/dive-bench/README.md).
[ADR 0104](../decisions/0104-evaluator-performance-plan.md) orders the remaining
evaluator work. Instruction counts assess individual optimizations; wall time
against the pinned TLC assesses the release goal. Neither substitutes for the
other, and incomplete or refused runs earn no credit.

## Controlled AWS baseline, 2026-10-21

A dedicated bare-metal campaign at clean source
`39f4e483031464031664f341b65e9fe2ef164b55` completed all **300 planned runs**:
ten models, five repetitions, TLC and both dive profiles, at requested worker
counts 1 and 8. Every run passed the pinned-golden checks. All arm summaries, including actual
worker use, remain in the compact
[controlled summary](../survey/data/aws-benchmark-controlled-2026-10-21.json).
All 300 individual raw rows are retained in the external archive identified by
the summary's raw-row and archive hashes.
This is a historical source baseline; later parity and budget changes are not
measured by this campaign.

Paired TLC/dive wall-time ratios, including process startup, parsing and
reporting, are **0.321× / 0.150×** in the TLC profile and **0.328× / 0.234×**
in the default profile at 1 / requested 8 workers. Each entry below is the
median of five within-repetition ratios. Every model remains slower than TLC.

| Model | TLC profile, 1 | TLC profile, requested 8 | Default, 1 | Default, 8 |
|-------|---------------:|------------------------:|-----------:|-----------:|
| GameOfLife | 0.234 | 0.124 | 0.225 | 0.152 |
| TLCSailfish1 | 0.355 | 0.056 | 0.352 | 0.265 |
| EWD998PCal | 0.241 | 0.175 | 0.246 | 0.175 |
| MCBakery | 0.338 | 0.354 | 0.360 | 0.364 |
| MCLamportMutex | 0.322 | 0.082 | 0.309 | 0.211 |
| SimpleRegular | 0.300 | 0.399 | 0.311 | 0.398 |
| MCbtree | 0.435 | 0.083 | 0.612 | 0.573 |
| CoffeeCan1000BeansSafety | 0.427 | 0.353 | 0.358 | 0.314 |
| MCDisruptor_MPMC | 0.388 | 0.355 | 0.389 | 0.338 |
| BufferedRandomAccessFile | 0.249 | 0.040 | 0.251 | 0.037 |

TLCSailfish1, MCLamportMutex, MCbtree and BufferedRandomAccessFile still used
one worker under the TLC profile when eight were requested. Those observations
receive no scaling credit, and the all-ten-model TLC scaling aggregate remains
absent. Default-profile scaling is **2.479×** across all ten models. These are
descriptive aggregates, including small and initial-state-only graphs; the
ADR 0006 acceptance population and targets remain unchanged. CoffeeCan's
explicit safety configuration earns no original-liveness parity credit.

The c7i.metal-24xl host used a pinned AMI, Xeon Platinum 8488C, Ubuntu 24.04.5,
Linux 7.0.0-1014-aws and Rust 1.99.0. SMT and turbo were disabled. Physical
CPUs 0–15 on NUMA node 0 used the performance governor with minimum and maximum
set to 2.4 GHz; memory was bound to node 0. Housekeeping ran on CPUs 32–47,
and no gate, build or profiling workload ran on this host during measurement.
The runner executed one process at a time with seed/fingerprint zero and
counterbalanced worker/arm blocks. All selected-core and package hardware
throttle counters were zero during and after the campaign; controls were
unchanged at completion.

Independent five-second telemetry covers 14,042 seconds in 2,788 samples,
with a maximum gap of 5.043 seconds, no selected-core steal time and a maximum
CPU sensor temperature of 52 °C. Observed unrelated processes consumed 138.23
CPU seconds across all cores, including monitoring and housekeeping. This is
an all-core bound for observed processes; sampling cannot individually attribute
every short-lived process. The runner's original
`quiet_window_certified=false` remains intact. The separate control audit
supports this controlled baseline without claiming absolute absence of noise
or release-target acceptance.

The external per-process backstop was 900 seconds. The frozen runner did not
forward a native time-limit flag: dive-profile runs retained their 30-minute
default, TLC-profile runs retained no native time limit, and native memory
ceilings were half the host RAM. TLC's heap was 3 GiB. The summary records each
arm's actual native budget, rather than treating the TLC heap as a native
whole-process ceiling. Different turbo settings and source pins prevent
attributing differences from the older campaign to a code optimization.

The native binary SHA-256 is
`3b9859b2fe99b77a3393b74a460b573547e5cde61c9410ca1e9cdaa8ea9131ce`.
All paired medians and geometric means were independently recomputed. Complete
raw evidence and frozen binaries were downloaded and hash-verified before the
instance was terminated. Their archive hashes, exact workload/oracle/runtime
pins and control-artifact hashes are retained with the compact summary. No raw
run outputs are committed.

## AWS baseline, 2026-10-20

The recovered campaign at clean source `73259029b7689a55ddfe77ed648cb5dd14519274`
completed all **300 planned runs**: ten models, five repetitions, TLC and both
dive profiles, at requested worker counts 1 and 8. TLC's observations are shared
by the two comparisons. All runs passed the runner's pinned-golden checks;
there were no censored, refused or failed runs. Single-worker counts match,
and counterexample validation is unnecessary because these are success models.

The descriptive equal-model geometric means of paired TLC/dive wall time are
**0.329× / 0.149×** in the TLC profile and **0.335× / 0.213×** in the default
profile, at 1 / requested 8 workers respectively. A value below 1 means dive
is slower. Every individual model remains below 1× TLC in this campaign.

These are **provisional measurements with no release-target acceptance credit**.
The bare-metal host was dedicated, but the campaign did not independently
certify its quiet window or thermal stability (`quiet_window_certified=false`).
Turbo remained enabled. Recorded before/after load samples are context rather
than proof of a quiet run. Repeat a certified campaign at the performance
milestones, preserving the same population and all failed observations.

| Model | TLC profile, 1 | TLC profile, requested 8 | Default, 1 | Default, 8 |
|-------|---------------:|------------------------:|-----------:|-----------:|
| GameOfLife | 0.237 | 0.091 | 0.227 | 0.119 |
| TLCSailfish1 | 0.362 | 0.060 | 0.360 | 0.238 |
| EWD998PCal | 0.244 | 0.171 | 0.249 | 0.182 |
| MCBakery | 0.351 | 0.355 | 0.376 | 0.372 |
| MCLamportMutex | 0.326 | 0.113 | 0.300 | 0.204 |
| SimpleRegular | 0.299 | 0.417 | 0.309 | 0.421 |
| MCbtree | 0.440 | 0.092 | 0.614 | 0.536 |
| CoffeeCan1000BeansSafety | 0.462 | 0.305 | 0.388 | 0.294 |
| MCDisruptor_MPMC | 0.406 | 0.311 | 0.407 | 0.282 |
| BufferedRandomAccessFile | 0.253 | 0.040 | 0.254 | 0.026 |

Ratios are medians of the five within-repetition wall-time ratios, including
startup, parsing and reporting; they are not ratios of historical golden times
or separately selected median runs. The full summary retains arm timing ranges,
RSS medians, config/module hashes and every comparison's pair counts.

In the TLC profile, TLCSailfish1, MCLamportMutex, MCbtree and
BufferedRandomAccessFile requested eight workers but actually used one because
of runtime-name encounter-order checks. Their wall-time comparisons remain
visible; they receive **no scaling credit**, and the all-model TLC-profile
scaling aggregate is absent. Default-profile 1-to-8-worker scaling is 1.986×
across all ten models. This does not assess ADR 0006's distinct-state population
or 3.5× scaling target; small and initial-state-only models remain in the table.
CoffeeCan1000BeansSafety drops temporal properties in its explicit safety
configuration and earns no original-liveness parity credit.

The compact [machine-readable summary](../survey/data/aws-benchmark-2026-10-20.json)
records the source/binary, benchmark set, oracle, Java, corpus and module pins.
The release binary SHA-256 is
`2987ec6480544a3d69de2e7f3de794cc9035678611c03bb85ae217bedfdd4ca5`.
Raw rows and console output are retained externally, never committed. Recovery
independently recomputed all paired medians and geometric means from the raw
rows and verified archive hashes before terminating the completed instance.

Machine: c7i.metal-24xl, Intel Xeon Platinum 8488C, 188 GiB RAM, Ubuntu 24.04.5,
Linux 7.0.0-1014-aws, pinned AMI `ami-02a9cdd4e98bc3c78`, rustc 1.99.0. SMT was
off, governor `performance`, affinity CPUs 0–15 and memory NUMA node 0. The
runner executed one process at a time, counterbalancing worker/arm blocks on odd
repetitions. Seed and fingerprint polynomial were zero, timeout 1,800 seconds,
TLC heap 3g. The node was terminated after evidence recovery.

Reproduce the workload from that clean commit after oracle/corpus fetch, on a
reserved Linux host with the recorded CPU/memory controls:

```sh
cargo build --release --locked -p dive-cli -p dive-bench
taskset -c 0-15 numactl --membind=0 target/release/dive-bench \
  --profile both --workers 1,8 --reps 5 --tlc \
  --dive-revision 73259029b7689a55ddfe77ed648cb5dd14519274 \
  --timeout 1800 --xmx 3g \
  --runs <external-directory>/runs.jsonl \
  --summary <external-directory>/summary.json
```

Do not certify a quiet window by changing metadata after the run. Record the
reservation, CPU/thermal controls and absence of interfering work for the next
campaign; retain all original measurements when repeating a declared block.

## Substitution-cache probe

On 2026-10-21, a small paired probe confirmed that caching every forced
`INSTANCE` substitution would change the pinned TLC's evaluation behavior.
`Core::force` already reuses unprimed substitution thunks. Primed references
pass a null successor and bypass the cache, matching the explicitly retained
branch in pinned `Tool.java` at commit `94d0c507`.

Use these two modules in an external scratch directory:

```tla
---- MODULE Inner ----
VARIABLE a
Repeated == a = a
PrimedRepeated == a' = a'
====
```

```tla
---- MODULE Outer ----
EXTENDS Naturals, TLC
VARIABLE x
I == INSTANCE Inner WITH a <- Print(<<"force",x>>,x)
Init == x = 0
Next == x' = IF x < 1 THEN x+1 ELSE x
Unprimed == [][I!Repeated]_x
Primed == [][I!PrimedRepeated]_x
====
```

Each config selects `INIT Init`, `NEXT Next`, and either `PROPERTY Unprimed`
or `PROPERTY Primed`. Run both with pinned TLC (`-workers 1 -lncheck final
-seed 0 -fp 0`) and `dive check --compat=tlc --workers 1 --seed 0 --fp 0`.
Both implementations succeed with two distinct, three generated and depth two.
Unprimed emits two `Print` lines, while Primed emits four. This is a control on
cache semantics, not a timing benchmark. The current paired dive executable
was a debug binary (SHA-256
`3b0cff81cd88dfb68a198fd4f52e97e92fd78f8c3242912387180a4381400eb8`)
with recorded source context `d1eb36e3912a88032c24de025e9b11f125a72943`;
a complete clean-build attestation was not captured. The relevant cache source
blobs are identical at that context and integrated main `39f4e483`:
`eval.rs` blob `022d85c523c1de4ca6696a411365673de0db627d`, and `env.rs`
blob `2bad80a1ae4d265e5d0c27ab0b56bb1e0b984a4e`. No evaluator change
or performance improvement follows from the probe. Investigate cache misses within the existing TLC rule before
claiming that ADR 0104 item 6 offers new memoization savings.


## Discarded evaluator prototypes

A factorized instruction experiment on 2026-10-21 compared the run-budget
source `d1eb36e3912a88032c24de025e9b11f125a72943` with two local candidates:
stack arrays for strict unary/binary native operands (`a2a9461f`), and those
arrays plus evaluator-private compact dispatch (`48d323e6`). Both preserve the
original expression error wrapper, left-to-right forcing, host checks and lazy
semantics. The compact candidate retains IR fallback and differential controls;
it does not change `INSTANCE` caching.

| Reduced model / profile | Baseline instructions | Stack arrays | Arrays + compact dispatch |
|---|---:|---:|---:|
| GameOfLife / dive | 1,224,031,470 | −1.697% | −0.834% |
| GameOfLife / TLC | 1,179,946,797 | −1.759% | −0.864% |
| SimpleRegular / dive | 2,793,329,298 | −0.985% | −1.141% |
| SimpleRegular / TLC | 2,872,622,761 | −0.908% | −1.079% |

All twelve measurements completed successfully with identical counts and
depth: GameOfLife 512 distinct / 1,024 generated / depth 1; SimpleRegular
12,097 / 47,584 / depth 19. Measurements used the ADR 0104 reduced inputs,
Rust 1.99.0's profiling build, Valgrind 3.22.0
`--tool=cachegrind --cache-sim=no`, one worker, seed/fingerprint zero and a
330-second external backstop. The c7i.8xlarge Xeon 8488C host also ran gates;
these are instruction comparisons, with no quiet-host wall-time claim.

**Neither candidate met the 5% bar, and neither runtime change was landed.**
The experiment stopped after these two models; it is not evidence for an
improvement across the complete benchmark population. In particular, a second
plan representation added little beyond the smaller operand-storage change.
The shared IR already provides typed variable IDs and resolved locals, and
`Core::eval_inner` borrows its expression. Further work should profile remaining
activation and context ownership costs before introducing more dispatch
scaffolding. Raw observations, source overlays, binary hashes and callgrind
profiles are retained externally; the quick comparison archive SHA-256 is
`d90b2ea46d7f95761cfa8b96521af44ee0a9012f15dbafdf17d80632f7677665`.

Boxing `ValueError`'s private cold details reduced its size from 32 to 8 bytes,
and reduced `Result<Value, ValueError>` and `Result<bool, ValueError>` from
32 to 16 bytes. The complete four-model, two-profile instruction matrix still
increased by **0.301% geometrically** (individual deltas −0.314% to +0.910%).
All sixteen arms completed with the expected verdict, distinct/generated counts
and depth. The experiment was rejected; no runtime change was landed.

The paired source pins, binary hashes, all eight instruction pairs and exact
reduced-input hashes are in the compact
[experiment summary](../survey/data/performance-prototypes-2026-10-21.json).
Both arms included CLI startup and used the same profiling build, Valgrind
3.22.0 Cachegrind with cache simulation disabled, one worker, seed/fingerprint
zero, a 900-second native budget, an 8 GiB memory ceiling and a 930-second
external backstop. The shared c7i.8xlarge host also ran gates; these are
instruction comparisons with no quiet-host wall-time claim. Smaller Rust
layouts alone do not establish fewer instructions across the measured workload.

A separate four-byte GF(2) fingerprint extension, combined with skipping the
initial traversal-stack allocation for scalars, also failed the complete matrix:
**0.473% more instructions geometrically**, with individual deltas −0.368% to
+1.214%. All sixteen arms completed with the expected counts and depth. The
larger private table grew from 2 to 8 KiB, added 768 construction reductions,
and grew an `Fp64` clone from 2,056 to 8,200 bytes (12 KiB more for the two
keyer copies). Those construction and clone costs remained inside the measured
CLI process. The compact summary records the exact `90174cdf` / `e38cf395`
source and binary pins; no runtime change was landed.

The hot `TlcFingerprint::extend` symbol includes iterative value traversal and
inlined byte arithmetic. Its whole exclusive share does not measure the byte
arithmetic alone. Correct algebra and smaller or faster-looking inner loops
therefore do not establish a workload improvement; the complete paired matrix
remains the acceptance evidence.

A complete Callgrind profile of reduced MCbtree at `d1eb36e3` also corrects the
older cost estimates in ADR 0104. Direct `normalize_set` costs only 0.495%
inclusive in the default profile and 0.421% in the TLC profile. Default-profile
membership costs 14.208% inclusive, with the function-domain comparison edge
accounting for 9.314%; TLC function-set membership costs 6.019%. The compact
summary retains top self costs, direct inclusive costs and call edges, with
source locations. Recursive inclusive costs overlap and must not be added.
These single-model measurements guide investigation; they do not establish a
saving across the eight-cell acceptance population.

Inlining already settled literal, typed-state and local-value leaves ahead of
the large evaluator dispatch also failed the complete matrix: **2.263% more
instructions geometrically**, with all eight cells increasing (0.202–4.390%).
All sixteen arms completed with the expected counts and depth. The compact
summary records the frozen `773490db` / `b7c22ccd` pair and full matrix. The
experiment was rejected; its function-level appearance did not establish a
whole-process saving.

A separately approved GameOfLife/default Callgrind pair retained exactly
512 distinct states, 1,024 generated states and depth 1. The expanded
`#[inline]` wrapper remained out of line: its symbol grew from 57 to 595 bytes,
with a larger register-save prologue and a call to the original interpreter
on fallback. Hot interpreter calls fell from 2,170,366 to 1,415,827, while hot
wrapper calls rose from 394,540 to 2,170,366. Wrapper exclusive instructions
rose from 4.734 million to 105.639 million; interpreter exclusive instructions
fell from 152.516 million to 104.930 million. Extra local-slot checks added
7.456 million instructions.

Across all wrapper/interpreter variants, exclusive instructions increased by
53.152 million, exceeding the whole-process increase of 51.792 million; other
small reductions offset part of it. Each arm's exclusive function costs sum
to its whole-process count. Assembly also shows repeated IR-node retrieval
and discriminant checks on fallback. These observations explain this failed
pair; they do not establish that forced inlining or another dispatch layer
would improve the complete matrix. The compact summary retains the exact
causal counts and archive identity.

The complete earlier eight-cell Callgrind profiles bound another proposed
cache: `callee_env` costs only 0.108–2.879% inclusive, and parentless empty-frame
allocation is a smaller subset. The dominant GameOfLife `Sum`/`score` frames
have arguments and captures. That cache was not implemented. Remaining
ownership and allocation changes need current evidence across the full matrix.

A later prototype reused uniquely owned inline activation frames through a
bounded thread-local pool. Captured or weakly referenced frames kept the existing
copy-on-write behavior, and released frames dropped their contents before reuse.
The complete four-model, two-profile screen increased instructions by
**0.795% geometrically**: all eight cells were worse, ranging from +0.510% to
+1.422%. All sixteen arms completed with identical verdicts, distinct/generated
counts, depth, initial counts and empty final queues. The +1.422% cell also
exceeded the per-cell regression ceiling. The prototype was rejected; its
conditional full gate was correctly skipped and no runtime change was landed.

The compact [frame-reuse screen](../survey/data/activation-frame-reuse-rejected-2026-10-22.json)
records the exact `537f8388` / `37488e87` source and binary pins, all eight pairs,
input hashes and retained raw-archive identity. It used Rust 1.99.0 profiling
builds, Valgrind 3.22.0 Cachegrind with cache simulation disabled, one worker,
seed/fingerprint zero, a 900-second native budget, an 8 GiB ceiling and a
930-second external backstop. The c7i.8xlarge campaign ran its measured arms
sequentially; no quiet-host wall-clock, TLC-throughput or scaling claim follows.
The campaign's instance, root volume, temporary security group and bucket were
removed after all producer evidence was preserved and verified.

A separate prototype read settled leaf operands at their consuming operations,
used an inline buffer for two operands, and handled successful bounded integer
arithmetic before falling back to the original kernels. Its complete reduced
four-model, two-profile screen lowered instructions by **1.787% geometrically**.
Every cell improved, from 0.078% to 4.872%, with identical verdicts, counts,
depth, initial counts and empty final queues in all sixteen arms. It missed the
required 5% geometric reduction, so the prototype was rejected and its
conditional full gate was skipped. No runtime change was landed.

The compact [operand screen](../survey/data/fused-operands-rejected-2026-10-22.json)
retains the actual `4e219ea8` / `1faf4905` source and profiling binary pins,
all eight pairs and sixteen profile hashes. Each arm included the complete CLI
under Cachegrind with cache simulation disabled, one worker, seed/fingerprint
zero, a 900-second native budget, an 8 GiB ceiling and a 930-second external
backstop. This is an instruction screen on reduced inputs; it establishes no
quiet wall-clock, TLC-throughput, scaling or full-gate qualification. The
campaign instance, root volume, temporary security group and bucket were removed
after all original producer evidence was preserved and verified.

An evaluator-private predicate-region prototype replaced nested logical and native
expression dispatch with an explicit continuation loop and a Boolean result
channel. Its complete four-model, two-profile screen increased instructions by
**10.234% geometrically**, with every cell worse (+7.741% to +12.958%). All
sixteen arms preserved success verdicts, exact distinct/generated counts, depth,
initial counts and empty queues. The runtime candidate was rejected, and the
frozen runner correctly skipped its conditional full gate. No runtime change
was landed.

The compact [predicate-region screen](../survey/data/predicate-regions-rejected-2026-10-22.json)
records actual `6badeb20` / `561e6ff4` source and profiling binary pins, all eight
pairs and sixteen original profile hashes. The complete CLI ran under Cachegrind
with cache simulation disabled, one worker, seed/fingerprint zero, a 900-second
native budget, an 8 GiB ceiling and a 930-second external backstop. The same
115 pinned reduced-input and library files were used by both isolated builds.
This is instruction evidence, with no quiet wall-clock, throughput, scaling or
TLC-speedup claim. Raw profiles and actual binaries remain externally preserved.
The instance, root volume, temporary bucket and security group were removed after
all original producer evidence was preserved and hash-verified.

The original exclusive profiles charge `Work::push` another 1.745–3.409% of
baseline whole-process instructions and `region_operand` 1.271–2.093%. Region
code also receives attribution inside `eval_inner` after inlining, so a named
function's change does not isolate dispatch cost. Function and source-line
views overlap and must not be added. These observations identify added work;
they are neither inclusive costs nor removable-savings bounds.

A later prototype used evaluator-private recursive Boolean-result consumers
with a borrowed-node fallback to the unchanged generic dispatch body. Its
complete four-model, two-profile screen increased instructions by **0.501%
geometrically**; all eight cells were worse (+0.230% to +0.811%). All sixteen
arms preserved success verdicts, exact distinct/generated counts, depth, initial
counts and empty queues. The screen missed the required 5% geometric reduction,
so the runner rejected the prototype and skipped its conditional full gate.
No runtime change was landed.

The compact [Boolean-consumer screen](../survey/data/typed-boolean-consumers-rejected-2026-10-22.json)
records actual `4459812b` / `8448506c` source and profiling binary pins, all eight
pairs and sixteen original profile hashes. It used the same 115 pinned inputs,
Valgrind 3.22.0 Cachegrind with cache simulation disabled, one worker,
seed/fingerprint zero, a 900-second native budget, an 8 GiB ceiling and a
930-second external backstop. These are whole-CLI instruction observations on
reduced models, with no quiet wall-clock, throughput, scaling or TLC-speedup
claim. All original producer evidence was preserved and verified before the
instance, root volume, temporary bucket and security group were removed.

A private action-context continuation prototype retained borrowed action segments
and exported captured contexts through immutable frame and thunk ownership. Its
complete four-model, two-profile screen lowered whole-CLI instructions by
**2.414% geometrically**, below the required 5% reduction. GameOfLife regressed
by 2.150% in the default profile and 1.941% in TLC; EWD998PCal regressed by
1.662% and 1.569%, respectively. Those four cells also exceeded the individual
1% regression ceiling. SimpleRegular and MCbtree improved in both profiles,
but individual improvements do not qualify the complete matrix. All sixteen
arms completed with native exit zero and no external censor, preserving success
verdicts, exact distinct/generated/initial counts, depth and empty queues.

The [compact continuation screen](../survey/data/action-context-continuations-rejected-2026-10-23.json)
records exact `1892ddc7` / `b30b2d4d` sources and trees, both physically frozen
profiling ELF identities and modes, all eight instruction pairs, sixteen original
profile/output hashes and both integer acceptance tests. It used the same 115
pinned workload/library inputs, one worker, seed/fingerprint zero, Valgrind
3.22.0 Cachegrind with cache simulation disabled, a 900-second native budget,
an 8 GiB ceiling and a 930-second external backstop. The runner's terminal
threshold assertion rejected a completed screen; its preserved generic HOLD and
progress-prefix marker do not mean that model arms were incomplete. No full
gate ran, no runtime change was delivered, and no wall-clock, throughput,
scaling or TLC-speedup credit follows. Original evidence and the rejected source
were preserved before the campaign resources were removed.

A retained-profile investigation of boxing only the unused foreign-domain
`Thunk` cache also found no credible 5% matrix case. Even deleting all named
`cached`, `store`, `callee_env` and thunk-drop exclusive work, plus direct
thunk frees, gives only a 1.987% geometric reduction; much of that work is
required for ordinary evaluation. This deliberately generous scenario is not
a rigorous global bound: constructor code is inlined into several callers and
allocator/source positions, so the profiles do not isolate its total cost.
No implementation or additional profiling workload was attempted. The compact
summary retains the narrow cost inventory and its attribution limitations.

### Optional value entrypoints, 2026-10-23

A complete screen compared main `e78ffdb1` with candidate `bbf71ff3`, adding
optional value-returning entrypoints to the existing resolved evaluator plan.
Two isolated profiling builds and all sixteen sequential Cachegrind arms
completed normally. All eight reduced model/profile pairs improved and retained
identical success verdicts, distinct/generated counts, depths and queue counters.
The geometric instruction reduction was **2.5763943%**, below the fixed **5%**
bar. An exact integer-product comparison confirms rejection without rounding.
The runtime change remains unmerged; no full checker gate was run for it.

The runner's threshold assertion followed the complete matrix. It is a negative
performance result, not a failed or censored native model run. The separate
`02854597` / `6b7baa57` raw-handler experiment's 4.5943593% rejection remains
unchanged; neither result earns a threshold waiver or a layout retry.

The [compact screen](../survey/data/prepared-value-entry-screen-2026-10-23.json)
retains each integer instruction pair, equal semantic counters, actual source
and binary pins, input/tool/library hashes and raw evidence hashes. The host
was c7i.8xlarge with Rust 1.99.0 and Valgrind 3.22.0. Explicit native budgets
were 900 seconds and 8 GiB, with a 930-second external backstop. The scope is
four reduced safety models in both profiles at one worker, including startup:
there is no quiet wall-time, TLC-throughput, scaling, new agreement or release
milestone claim. Original evidence was verified before resource cleanup.

### Transient activations and raw outlining, 2026-10-24

Two further evaluator experiments completed the same fixed instruction screen:
four reduced ADR 0104 models, both profiles, one worker, two profiling builds
and sixteen sequential Cachegrind arms. Every native stage completed normally;
each baseline/candidate pair retained identical success results and all five
reported counters. The baseline in each screen is dive, not Java TLC.

| Experiment | Baseline / candidate | Geometric instruction change | Decision |
| --- | --- | --- | --- |
| Transient function activations | `771178b5` / `25850c0d` | +0.5727351% | All eight pairs slower; rejected |
| Compact raw evaluator outlining | `68c09a1b` / `9be70ddf` | -0.4409831% | All eight pairs improve, below 5%; rejected |

The fixed bar requires at least 5% fewer instructions geometrically and no more
than 1% regression in any cell. Both experiments pass the per-cell bound and
fail the geometric requirement. Their runtime branches remain unmerged; neither
received a full checker gate or a threshold waiver. The outline producer's
terminal HOLD follows a complete matrix and records the threshold failure;
its earlier zero-build, zero-model startup failure is separate evidence.

The compact [activation record](../survey/data/transient-activations-rejected-2026-10-24.json)
and [outline record](../survey/data/compact-raw-outline-rejected-2026-10-24.json)
retain integer instruction pairs, equal counters, source/binary/input identities,
tool and machine provenance, review hashes and retained S3 origins. Both used
900-second native budgets, 8 GiB ceilings and 930-second external backstops.
Symbol or source-line attribution does not establish removable work. These
screens establish no quiet wall-time, TLC throughput, scaling, new agreement or
release-capacity result.

## Packed temporal atom truth

The native temporal label store keeps one bit per actual state/action observation.
It retains global atom IDs and original-edge order while omitting false data for
unused atom domains. Public owned labels remain supported, and existing temporal
count limits are unchanged.

A diagnostic campaign on ElevatorLivenessMedium compared baseline e2b9d9f8 with
candidate c4fa21d2, using five alternating pairs per profile. Every run completed
with 4,122 distinct states, 14,296 generated states and depth 36, under identical
native 391,180 ms and 4 GiB limits. Median sampled resident memory changed from
41,172,992 to 34,947,072 bytes in the TLC profile (-15.1%), and 40,878,080 to
34,865,152 bytes in the default profile (-14.7%).

These are sampled resident pages on a shared Mac15,13 host, polled every 500 ms;
they are not an exact high-water mark or a quiet wall-clock benchmark. The compact
summary records source/binary pins, all repetition ranges, host/kernel/toolchain
and command settings. No temporal count limit is removed on this evidence.

A separate single-pair profiling diagnostic on the same source pins used
Valgrind 3.22.0 on a shared Xeon 8488C host, with native 900-second/4 GiB
budgets. Whole-CLI instructions changed by -1.031% in the default profile and
-1.017% in the TLC profile, with the same success, counts and depth. This is
not the evaluator performance acceptance campaign and earns no throughput
credit. Both measurements are preserved in the compact
[machine-readable summary](../survey/data/packed-atom-truth-2026-10-21.json).

## Search-budget overhead qualification

Optional state/depth budgets were measured with both ceilings unset, comparing
`9be2ba41` against `c82dd72c`. All eight ADR 0104 model/profile instruction cells
retain their distinct/generated counts and depth. Whole-CLI Cachegrind deltas
range from −0.0155% to +0.0953%, with geometric mean +0.0225%.

A separate genuine eight-worker `SimpleRegular` Callgrind run used three
interleaved baseline/candidate pairs per profile. Both arms report eight workers
and execute the parallel candidate-publication path. Paired-median instruction
deltas are −0.0679% (default) and +0.0144% (TLC); every pair lies between
−0.1355% and +0.0833%. Counts and depth remain identical. These satisfy the
one-percent normal-input instruction overhead bar. Cachegrind and Callgrind
totals are not pooled. The shared host and reduced diagnostic workloads make
no wall-clock, quiet-host or TLC throughput claim; this campaign executes no
TLC oracle.

The [compact evidence](../survey/data/search-budget-overhead-2026-10-21.json)
retains all cells/pairs, exact source/binary and workload hashes, reductions,
machine/toolchain, constraints and archive identity. Raw outputs stay external.

## Interval-streaming overhead qualification

Comparing `c82dd72c` against `f299eb07`, all eight ADR 0104 model/profile
Cachegrind cells retain their distinct/generated counts and depth. Whole-CLI
instruction deltas range from −0.0023% to +0.0462%, geometric mean +0.0112%.
Separate genuine eight-worker SimpleRegular Callgrind runs use three interleaved
pairs per profile, actual eight-worker reports and positive parallel-publication
call evidence. Their paired-median deltas are −0.0209% default / −0.0428% TLC,
with identical counts/depth. Every measured cell and pair satisfies the
one-percent normal-input overhead bar. The two instrument totals are not pooled.

The [compact evidence](../survey/data/interval-streaming-overhead-2026-10-21.json)
retains all rows/pairs, exact source/binary/workload/library identities, machine,
toolchain, constraints and archive hash. The shared host and reduced diagnostic
workloads earn no wall-clock or TLC throughput credit; no TLC oracle executes
in this paired instruction campaign. Raw output remains external.

## Native memory-budget sampling qualification

Initial and terminal process-memory observations were compared at frozen sources
`52485957` and `ca2d6e96`. All eight ADR 0104 single-worker cells retained their
verdict/counts/depth; instruction deltas ranged from −0.091353% to +0.113696%,
with geometric mean +0.007820%. Three interleaved pairs per profile on
SimpleRegular's genuine eight-worker path gave paired median deltas +0.084346%
(default) and −0.118612% (TLC). Every pair remained within one percent, with
actual eight-worker use and nonzero `parallel::commit_sources` evidence.
Cachegrind single-worker and Callgrind parallel totals are separate; these are
whole-command instruction observations on a shared host, not throughput claims.

The runner's original provenance block accidentally named older source and
binary paths. That block is preserved and invalidated in the
[compact evidence](../survey/data/native-memory-overhead-2026-10-21.json).
The original staging script checks and builds the intended clean source heads,
and every original Valgrind header names the corresponding executable path.
Corrected hashes and clean-head observations were collected afterward. Retained
file timestamps support ordinary filesystem continuity; they are not an
original pre-execution hash attestation or a continuous tamper audit.
The final sampling-comparison tests change only test code; their explicit
source transfer preserves the measured runtime/build/harness tree.

A separate macOS startup diagnostic used fresh release binaries and twenty
interleaved Counter pairs per profile after two warmup pairs. Default CLI
medians were 3.735→3.761 ms (default) and 3.795→3.812 ms (TLC). Warm same-process
API diagnostics and nine initial one-byte memory-stop controls are recorded
alongside them. Shared-host sub-millisecond differences are diagnostic; they
do not establish quiet-machine speedups or replace instruction qualification.


## Cooperative evaluator interruption overhead

The [compact 2026-10-21 campaign](../survey/data/evaluator-loop-overhead-2026-10-21.json)
qualifies normal-input instruction overhead for typed interruption in nested
quantifier, CHOOSE and filter loops. On the four reduced ADR 0104 workloads in
both semantic profiles, whole-CLI Cachegrind instructions at one worker rose
0.255% geometrically; all eight cells were between 0.049% and 0.536%. Counts
and depth were identical. Three paired genuine eight-worker SimpleRegular
observations per profile used Callgrind and retained nonzero parallel commit
work: paired median instruction changes were 0.223% in the default profile
and 0.131% in the TLC profile. Both medians and every single-worker cell stay
within the 1% overhead bar. Cachegrind and Callgrind totals are not pooled.

Measured sources are `ca2d6e96` and `4255be6b`; full-gate source `1338630b`
differs only by the reviewed parent test normalizer for nondeterministic memory
observations. The summary records exact source-transfer proof, binary/input
hashes, raw archive identity, all 28 arms and native three-second stop controls.
The original campaign identity text contained a stale block, retained externally
unchanged and invalid. Clean source heads, binary hashes and original filesystem
metadata preceding the first arm support the separately recorded collection-time
attestation; it is not an original before-run or continuous tamper audit.

The shared c7i.8xlarge host used Valgrind 3.22.0 and rustc 1.99.0, explicit
900-second/8 GiB native budgets and 930-second external backstops. Startup is
included. This establishes neither quiet wall time nor an ADR 0104 optimization
win, corpus agreement increase or release throughput acceptance. The bounded
MCInnerSerial probes stop with actual incomplete capture in both profiles;
their distinct outer report and stopped elapsed fields are retained separately.
Binding/ASSUME, diagnostic reconstruction and quiet/bulk/native value units
remain interruption limitations; the fresh-validation follow-up is separate.


## Controlled native validation overhead

The [compact validation-control campaign](../survey/data/validation-control-overhead-2026-10-21.json)
compares exact sources `1338630b` and `8f0731ac`. All eight reduced ADR 0104
model/profile cells retain success, counts and depth. Whole-CLI Cachegrind
instructions at one worker have geometric mean change −0.000512%, with cells
from −0.03655% to +0.03359%. Three paired genuine eight-worker SimpleRegular
observations per profile use Callgrind, actual eight-worker reports and nonzero
parallel commit work. Paired-median deltas are +0.002206% default / −0.070326%
TLC. Every single-worker cell and parallel median stays within the 1% overhead
bar; Cachegrind and Callgrind totals are not pooled.

The correct source/binary/bundle/runner identity was captured after locked builds
before executing any measured arm. Final clean-source, original timestamp and
frozen-binary hash checks retain that chain, without claiming a continuous
filesystem audit. Workload/library hashes, all 28 arms and semantic controls are
recorded in the compact summary. The shared host and successful reduced safety
models establish normal-input overhead only: they do not measure validation-heavy
workload performance, quiet wall time, TLC throughput or a new agreement. Raw
outputs remain external. Generic control covers supplied native words; binding,
serialization, quiet/bulk/native units, foreign deferred providers and legacy
TLC/session/standalone validation retain their documented limitations.


## Persistent function domains

ADR 0110 shares immutable general-function keys and conservative
facts across same-domain EXCEPT updates. Borrowed checked views and exact dense
integer/Cartesian indices reuse those facts; tuple/record kinds, literal flags,
eager native hashes, fresh fingerprint caches and unknown-key/error-order
fallbacks stay intact. Auxiliary metadata adds no range or evaluator ownership
beyond the original keys, which may themselves retain provider contexts. Such
keys keep the original fallbacks, and deep domain keys use iterative destruction.

A fresh complete screen measured **5.365455% fewer instructions**, geometric
mean over all eight cells, against main `510dce00` using candidate `239db53f`.
All 16 native arms completed with identical verdicts, distinct/generated counts
and depths. Every cell improved; no historical arm or favorable repetition was
substituted. The exact `239db53f` full checker gate passed in 35m16s with all
2,188 tests, cm1 and independent validation at 1/4/8 workers, 147 TLC-profile
and 142 default agreements, 458 SANY agreements and no disagreements, timeouts
or baseline changes. ADR 0110 is accepted; final delivery review and required
PR checks are separate from this exact-source gate.

| Reduced model | Default Ir reduction | TLC-profile Ir reduction |
|---|---:|---:|
| SimpleRegular | 7.472831% | 7.816672% |
| MCbtree | 12.868712% | 2.627899% |
| GameOfLife | 6.478926% | 3.658053% |
| EWD998PCal | 0.769651% | 0.574490% |

[Compact evidence](../survey/data/persistent-function-domains-2026-10-21.json)
records actual source trees, profiling binaries, tool/input identities, every
arm and raw hash. This dedicated c7i.8xlarge campaign ran two sequential locked
profiling builds and 16 sequential Cachegrind arms with explicit 900-second,
8-GiB native budgets and a 930-second external backstop. Original source/build/
library/manifest receipts were uploaded before the first arm; each arm retained
before/after identities. Collection timestamps are recorded separately.

The delivery retains the measured owned runtime and tests exactly. Main's later
PR #399 changes only benchmark fake-child test headroom and corresponding expected
values; its production prefix is unchanged and its fast gate qualified separately.
The whole later delivery tree is therefore not claimed identical to measured
`239db53f`. The full gate remains labelled with its actual measured pin.

These are instruction counts for four reduced safety models at one worker,
including CLI startup. They establish no quiet wall-clock gain, full-model or
eight-worker scaling, TLC throughput, new corpus agreement or release milestone.
Construction and metadata acquisition still add costs. The complete checker
gate and both small-profile baseline comparisons qualify the actual measured
source; later source transfer and PR checks are recorded independently.

## Controlled fresh model binding

Owning run control now reaches cooperative evaluator loops during fresh model
binding, with typed interruption and original output/capture evidence. Existing
uncontrolled Rust binding APIs retain their behavior. Structural/bulk startup,
assumptions, post-binding symmetry, quiet value units, serialization and legacy
validation retain their documented limitations. This does not establish a hard
wall-clock deadline for every binding operation.

The exact `8e650dba` candidate against main `44c1fd97` passed all 2,243 tests and
13 full-gate stages, including both small baselines without changes: 149 TLC
agreements, 143 default agreements plus six expected divergences, seven refusals
each and 458 SANY agreements. The gate's reported total is 35m40s; its outer
process observation is 2,143.013 seconds. These are distinct observations.

All 16 normal single-worker Cachegrind arms completed with identical verdicts,
counts and depth. Every paired cell stays within 1% instruction overhead; the
geometric mean is +0.014098%, with individual deltas from -0.082696% to +0.137646%.
The campaign used explicit 900-second/8-GiB native budgets and a 930-second OS
backstop, original before-run source/binary/tool/input receipts and matching
final identities. This measures whole-CLI normal-input overhead, not binding-heavy
throughput, quiet wall time, TLC speedup, universal interruption coverage or new
corpus agreement. [Compact evidence](../survey/data/controlled-binding-overhead-2026-10-22.json)
records the actual source pins, binaries, raw hashes and all eight pairs. Later
original delivery on `6badeb20` retained the entire measured runtime tree.

After integration with SelectSeq on main `0ec2d0f8`, exact `f5db2f53` passed
all 2,249 tests, the full gate and both unchanged small baselines. Its reported
total is 21m55s; the outer process took 1,320.523 seconds. The integration retains
all 18 binding-owned runtime/test files exactly from `8e650dba` and preserves
SelectSeq's separately qualified behavior. The normal instruction comparison
remains the original `44c1fd97` / `8e650dba` campaign; it is not relabelled as a
measurement of the integrated tree.

## Incremental ENABLED interval enumeration

Candidate `c2730906` against main `1d49aa76` passed all eight normal-input
instruction comparisons: each used at most 1% more instructions, with identical
success verdicts, counts and depths. The whole-CLI geometric mean was
**+0.033933%**, descriptive rather than an acceptance substitute for individual
cells. Two isolated profiling builds and 16 sequential Cachegrind arms used the
four reduced models, both profiles, one worker, seed/fingerprint zero, native
900-second/8-GiB limits and a 930-second external backstop. This establishes
normal-input overhead, with no performance speedup, quiet wall-time, throughput
or scaling claim.

The same source then completed all 13 full-gate phases: native exit zero,
2,292 unique tests passed with no skips, cm1 and validation at 1/4/8 workers,
149 TLC-profile agreements, 143 default agreements plus seven expected
divergences, and 458 SANY agreements. All 312 model identities and typed baseline
facts remained unchanged. Separate interval controls agree on the first witness;
the first-candidate error remains pending independent trace validation under
[#434](https://github.com/Chelis-Lang/dive/issues/434), without established
population agreement credit.

[Compact evidence](../survey/data/enabled-interval-qualification-2026-10-23.json)
records every integer instruction pair, actual source/tool/input identities,
profile and release binary hashes, gate reports and original archive hashes.
Release binaries were captured after the gate and remain distinct from the
profiling binaries frozen before the matrix. Raw artifacts remain external;
the campaign instance, root volume, bucket and security group were independently
verified removed after preserving all original objects.

## Assumption control and interval context cursors, 2026-10-23

Controlled producer `ASSUME` evaluation now uses the existing cooperative run
budget. Primitive interval contexts avoid a bulk domain vector while retaining
the original size checks and binding order. Eight loop controls stopped during
startup with their earlier output retained; four scalar false/error controls kept
their original classes. At compiled source `1ba776da`, the full gate passed
2,308 tests and all 13 phases, with the existing 150 TLC-profile and 144
default-profile agreements unchanged.

The complete eight-pair instruction screen kept all reported state, depth and
queue counters identical and every pair within the 1% overhead limit
(geometric mean −0.5968%). This qualifies the capacity change; it establishes no wall-clock or throughput milestone.
Structural, quiet/native/bulk and independent-validation exclusions remain.
The [compact record](../survey/data/assumption-control-overhead-2026-10-23.json)
distinguishes the compiled source from its later delivery, retains both earlier
setup HOLDs, and records preservation plus independent resource cleanup.
