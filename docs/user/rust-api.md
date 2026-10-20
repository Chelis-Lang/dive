# Embedding dive in Rust

The workspace is unpublished. Until releases are published, use a checkout/path
dependency; for a caller checked out beside this repository:

```toml
[dependencies]
dive-api = { path = "../dive/crates/dive-api" }
serde_json = "1"
```

The caller needs the repository's pinned Rust/MSRV. Normal checking is native;
the Java conformance harness is a separate developer tool.

```rust
use dive_api::{CheckRequest, check_file};

let request = CheckRequest {
    file: "Counter.tla".into(),
    compat_tlc: true,
    seed: Some(7),
    fp: Some(0),
    workers: Some(4),
    ..CheckRequest::default()
};
let report = check_file(&request)?;
let result = report.to_json();
assert_eq!(result["verdict"], "success");
```

Use a `Result`-returning test/application and provide the actual source/config
paths. An API call returning `Ok(report)` means the operation returned a report;
inspect that report's verdict, refusal, completion and validation before asserting
success. API errors also distinguish rejected inputs, I/O, named unsupported
requests and internal failures.

Low-level callers may supply predicate and action scopes. Scope ownership and
expression references are checked before backend startup. Parallel graph capture
and DFID also inspect actual captured callables for printing, randomness, I/O or
live checker reads; a caller-supplied constant-level annotation cannot hide those
effects. These scopes refuse before worker execution or output. Pure captured
operators remain usable, and `Evaluator::scope_has_effects` exposes this
non-evaluating effects check without forcing stored values or callbacks.

Low-level graph consumers implement `dive_check::capture::Observer` and pass it to
`capture::explore_observed_with_evidence`. Observation currently requires one
worker; `--compat=tlc` uses the same hook semantics. Methods are inert by default.
`init` receives accepted roots after Init succeeds. `edge` receives original
concrete successors, including duplicate and seen-state transitions, followed by
implicit stutters. `level_done` borrows the graph after every source at that BFS
level has passed safety checks and the next frontier has been appended. Counters
include excluded and duplicate generated candidates, independently of the edge
count. Frontier states can still have empty outgoing rows.

With `--action-composition=tlc`, pure composition capture uses the existing
inline fallback. Generated counts include intermediate states, while graph and
observer edges contain only emitted successors and implicit stutters. Other
stateful checking reasons remain refused. The default `tla` composition mode
counts emitted successors and can use the ordered parallel executor.

A callback error or panic stops exploration with the actual boundary counters
and no completed graph. A failed Init or partial level supplies no completion
event, and an earlier safety failure retains its verdict and counterexample.
Prefix graphs can support independently validated violations; absence of a
violation proves nothing before exhaustion. The `Observer` rustdoc contains a
runnable example. Producer-clock requests and the former dispatch, stopped and
terminal service APIs have been removed. Ordinary parallel graph capture remains
available through the unobserved entry points.

| Entry | Meaning |
| --- | --- |
| `parse_source` / `parse_file` | Syntax/CST and source diagnostics |
| `analyze_source` / `analyze_file` | Resolved semantic analysis |
| `parse_config_source` / `parse_config_file` | Configuration reading |
| `check_file` | Ordinary safety, eligible ordinary dive-profile deterministic level checks, TLC-profile final analysis, or explicit `TemporalMode::FinalShort`; inspect actual route/completion |
| `simulate_file` | Seeded finite traces, never exhaustive proof |
| `dfid_file` | Iterations and explicit completion evidence |
| `translate_source` / `translate_file` | Admitted in-memory PlusCal artifact |
| `trace::SpecModel` | Independent implementation/witness replay |
| `temporal::check` | Explicit bounded final/short backend with private validated violations |
| `check_file_checkpoint` | Bounded actual native safety pause/recovery |

Request fields make profile and module resolution explicit. Rust APIs do not
implicitly adopt the CLI's environment-variable parsing. Reports preserve actual
worker widths, seeds, limits and mode qualifications; retain these alongside the
source/configuration in a `cargo test` failure artifact.

Low-level crates expose syntax, IR, values, evaluation, stores and staged temporal
graph studies. Public IR/model metadata is validated before execution; manually
constructing a graph or marking a validation boolean does not supply a certified
model result. The trace crate decides `ENABLED` with the core evaluator and
shares nothing else with the checker.
For exact type signatures and compiled examples run `cargo doc -p dive-api --no-deps`
and consult [the API root](../../crates/dive-api/src/lib.rs),
[trace guide](traces.md) and [temporal guide](temporal.md).

The [fair procedure guide](../pluscal-translation.md) includes a compiled
Rust caller using `TranslationRequest.limits.fairness`. Shared finite process
procedures retain per-ID ownership; their admitted translation reports still
carry `model_proved: false`. Read [PlusCal boundaries](pluscal.md) before
interpreting a generated Spec as a checker result.


The TLC-internal liveness diagnostics (`tlc_temporal`, `periodic_candidate`,
`phase_*`, `periodic_*` and `tlc_*` projections other than `tlc_plan::classify`)
were removed on 2026-10-12 (ADR 0082). TLC's exact lasso and periodic timing
are not parity obligations. A configured `_PERIODIC` entry binds with TLC's
errors and is then refused by name.
