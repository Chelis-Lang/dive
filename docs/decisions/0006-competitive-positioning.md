# 0006: Competitive positioning, adopted designs and performance targets

- **Status:** proposed
- **Date:** 2026-10-08

## Context

Five other implementations of TLA+ model checking appeared between 2026-01 and 2026-10: tla2, tla-rs, tla-mc and tlaplusplus in Rust, and tlago in Go. The owner has decided dive is built regardless (charter appendix item 2). This ADR records where dive is positioned against them and which of their designs it adopts or rejects. It also sets the performance numbers dive must beat. The evidence is in `docs/survey/competitive.md` and `docs/survey/data/`. All of it was measured in Docker against the pinned TLC (`tlc-2026.10.06.014338`) on tlaplus/Examples `6a7b26a`.

The evidence that shapes the decision:

1. **Correctness.** On 140 small exhaustive Examples models, measured:

   | Tool | Agree | Wrong answers |
   |---|---|---|
   | tla-mc | 99 | 0 |
   | tlago | 110 | 0 |
   | tla2 | 108 | 2 false alarms (`MCbtree` safety, `SingleLaneBridge` liveness) |
   | tlaplusplus | 96 | 12 success-with-wrong-count (6 under-explored, down to 1% of TLC's states), 1 missed safety violation, 16 evaluation errors reported as violations |
   | tla-rs | 44 | 2 wrong counts under SYMMETRY |

   No competitor is both broad and exact.
2. **Defaults.** Silent divergence from TLC defaults is the norm. Measured on probes:
   - **32-bit overflow.** tla-mc, tla-rs, tlaplusplus and tla2 accept `2147483647 + 1` and report success, where TLC errors.
   - **Randomness.** `RandomElement` is fixed or non-random in tla-rs, tlaplusplus and tla2.
   - **Ignored cfg directives.** tla-rs ignores `ACTION_CONSTRAINT` and reports success over 11 states, where TLC has 5.
   - **Parallel search.** tlaplusplus returns non-shortest counterexamples at 8 workers. tla2 drops the counterexample entirely at 8 workers.
3. **Better-than-TLC options.** Where a competitor offers one (wider integers, fixed seeds, exact storage, JSON), it does so by changing the default, never as an option beside a TLC-identical default. ADR 0010 makes that combination dive's rule; no competitor has it.
4. **TLC's own surprise.** `CHOOSE` over strings follows interning order. An unused definition mentioning `"alpha"` above `Init` changes `CHOOSE e \in {"zeta","alpha","mid"} : TRUE` from `"zeta"` to `"alpha"` (probes P2 and P7). tla-mc, tlago and tla2 reproduce this; tla-rs and tlaplusplus do not.
5. **Performance.** This axis is contested. tla-mc reproduces its claimed speed-up: about 3.9× TLC compiled and 1.9× interpreted at 8 workers, geometric mean over 4 shared models. It also has 2 ms startup and 3–10× less memory than TLC. Its interpreter is 3–4× *slower* than TLC on `GameOfLife`. tla2, tlaplusplus and tlago are slower than TLC on every safety spec measured (0.04–0.87×). TLC's own 1→8 worker scaling on these 0.4–0.7 M-state specs is only 0.9–3.5×.

## Decision

### Positioning

"Under `--compat=tlc`, results identical to TLC; in the default profile, strictly better, and never silently different." This follows ADR 0010 as revised.

- **Under `--compat=tlc`**, dive's observable results equal the pinned TLC, proven over the full corpus: verdict, counts, depth, counterexample length, error class, the 32-bit overflow error and `CHOOSE` order.
- **In the default profile**, dive is strictly better on the axes where TLC has a known limitation, where the owner has approved it:
  - deterministic parallel workers on by default, giving the same counts, depth and shortest counterexample at any worker count;
  - big integers;
  - refusing symmetry with liveness.

  Further options:
  - replayable seeds, always reported;
  - `--choose-order=canonical` (open question 8);
  - exact storage;
  - versioned JSON with counterexamples as data;
  - an in-process API.
- **Every active option is reported** in the output's `options` block. When a default-profile result would differ under `--compat=tlc`, the output says so (ADR 0010). This is the property no competitor has: each one that improves on TLC changes results silently and has no TLC mode (survey, "Better than TLC without silent changes").
- **Unsupported is a verdict.** It has its own exit code and JSON value, and is never folded into an error or violation code.

### Designs adopted (with evidence)

| Design | From | Evidence | dive component |
|---|---|---|---|
| Intern strings and model values in TLC's UniqueString order; value ordering identical to TLC's `compareTo` | TLC (MIT), as reproduced by tla-mc and tlago | P2/P7: only these orderings match TLC | `dive-value` |
| Level-synchronous parallel BFS with a deterministic merge | tla-mc (MIT), extended | P8: tla-mc keeps shortest traces at 8 workers, but its counts vary; determinism needs the merge | `dive-check` |
| Fingerprint set spilling to sorted runs with Bloom and sparse index; checkpoint at level boundaries; refuse resume on a changed spec | tla-mc (MIT) | 9.8-billion-state run reported by tla-mc (claimed) | `dive-store` |
| Compile spec to Rust, build with cargo, cache by content hash; interpreter for short runs | tla-mc (MIT) | Compiled tier measured fastest in our benchmarks (see survey) | `dive-compile` |
| JSON with explicit `soundness`/`completeness` blocks, plus dive's `options`, seed, collision bound and `unsupported` verdict | tla2 (Apache-2.0), concept only | Best structured output among competitors, but drops traces in parallel mode | `dive-cli`, `dive-api` |
| Shadow-compare evaluator tiers; swarm chaos tests of store and spill; state-graph digest snapshots; mutation audits | tlaplusplus (GPL-3.0), ideas only, independent implementation | Its own history shows divergent evaluators hid missed violations | `dive-eval`, `dive-compile`, `dive-store` tests |
| TLC `-tool`-compatible output mode | tlago (MIT) | Drop-in for TLC tooling | `dive-cli` (later) |

### Designs rejected (with evidence)

| Design | Seen in | Why rejected |
|---|---|---|
| Wider integers *silently*, with no TLC mode | tla-mc, tla-rs, tlaplusplus, tla2 | P1: silent success where TLC errors. dive's default is also big integers (owner-approved), but `--int=tlc` reproduces TLC's error and the default-profile output flags any result that would differ under `--compat=tlc` |
| Swallow evaluation errors and count them | tla2 (`suppressed_guard_errors`), tlaplusplus ("swallowed eval errors") | tla2 warns that the "state count may be incorrect" but still reports. dive stops with an error verdict, as TLC does |
| Warning-only handling of unimplemented cfg directives or symmetry sets | tla-rs | P3: 11 states instead of 5, with "No errors found" |
| Bloom or probabilistic store switched on silently | tlaplusplus (`--bloom-auto-switch` default true) | The verdict does not disclose it. dive allows a degraded mode only as an explicit, reported option |
| Work-stealing search that is not level-synchronous | tlaplusplus, tla2 | P8: non-shortest traces (tlaplusplus) and missing traces (tla2) at 8 workers |
| Auto-tuning the worker count at run time | tlaplusplus | Breaks reproducibility of reported inputs |
| A text-heuristic front end without a syntax tree | tlaplusplus | It hangs on malformed specs (M1 and M2) and is the source of its own recorded soundness fixes |
| A faithful structural port of TLC's Java classes, including global state | tlago | Not embeddable or re-entrant. dive ports semantics, not structure (charter) |
| Vendoring hundreds of forked crates | tla2 (~345 forks) | Build fragility (its published repo does not build) and audit cost. dive keeps `deny.toml` and a small dependency set |
| Fingerprint hash not compatible with TLC | tla-mc, tla2, tlaplusplus | Not a soundness bug in itself, but dive keeps FP64 available so fingerprint-level comparisons with TLC remain possible. Decided in `dive-value`'s ADR |

### Performance targets dive must beat

All targets are measured as in `docs/survey/competitive.md`: the same container limits, interleaved reps, median of within-rep ratios against the pinned TLC, and correctness checked on every run. They apply to **both profiles**. Every target is result-preserving, so the default profile and `--compat=tlc` must meet the same numbers, with identical results under `--compat=tlc`.

| Metric | Must beat (measured baseline) | Target |
|---|---|---|
| Startup, trivial spec | TLC 335 ms; tla-mc 2 ms | **≤ 10 ms** for parse, check and report of a 1-state spec |
| Peak RSS, `CoffeeCan1000Beans` at 8 workers | TLC 1.3 GB; tla-mc 277 MB | **≤ 300 MB** in fingerprint mode |
| Interpreted throughput, 1 worker | TLC (1.0×); tla-mc interpreter 0.24–4.9× | **≥ 1.0× TLC on every benchmark spec**, including `GameOfLife`, where tla-mc loses; **≥ 2× TLC geometric mean** |
| Interpreted throughput, 8 workers | tla-mc interpreter ≈1.9× TLC geometric mean | **≥ 2× TLC geometric mean**, with deterministic counts and shortest counterexamples (tla-mc's counts at a violation vary with workers) |
| Compiled throughput (`dive-compile`), 8 workers | tla-mc compiled ≈3.9× TLC geometric mean | **≥ 4× TLC geometric mean** on the same specs, passing the same conformance suite as the interpreter |
| Liveness, `ElevatorLivenessMedium` | TLC 5.9 s at 8 workers; tla-mc 0.14 s | **≤ 0.5 s** with a full tableau (no shape restrictions) |
| Scaling 1→8 workers, ≥ 0.4 M-state specs | TLC 0.9–3.5×; tla-mc 1.2–4.5× | **≥ 3.5×**, with identical results at every worker count |
| Build footprint | tla-mc: 3 dependencies, 12 s release build | `dive-cli` release build ≤ 60 s on 8 cores; dependencies kept within `deny.toml` policy |

These targets are revisited when `dive` first produces verdicts. The quiet-machine re-measurements listed in the survey refresh the baselines.

## Consequences

- **Harness.** The conformance harness must classify `wrong-count-claimed-success` and `wrong-verdict-claimed-success` separately from other disagreements. Those are the categories that exposed competitors' unsoundness. The probe specs in `docs/survey/competitive.md` (P1–P9, M1–M2) should be imported as corpus entries with TLC goldens.
- **Registry.** The divergence registry gains the `CHOOSE` order row, now characterised: strings by interning order, sets by size first, tuples by length first (measured). Its option is `--choose-order=canonical`.
- **Revisiting.** The targets are revisited when dive first produces verdicts (M2/M3), and whenever a competitor publishes a release that changes them. The benchmark commands in the survey are the reproducible baseline.
- **Licences.** Ideas taken from tlaplusplus are recorded as independent implementations of published techniques. No code or text is taken from it. Code consulted from MIT or Apache-2.0 competitors is recorded in `docs/provenance.md` when used.
