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

| Entry | Meaning |
| --- | --- |
| `parse_source` / `parse_file` | Syntax/CST and source diagnostics |
| `analyze_source` / `analyze_file` | Resolved semantic analysis |
| `parse_config_source` / `parse_config_file` | Configuration reading |
| `check_file` | Ordinary safety, `TemporalMode::FinalShort`, or explicit `TemporalMode::FinalTlc`; inspect actual route/completion |
| `tlc_temporal::check_final` | Bounded pinned final-only checking; empty implied temporal obligations route actual bounded safety, never a temporal product certificate |
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
model result. The independent trace crate builds without the successor enumerator.
For exact type signatures and compiled examples run `cargo doc -p dive-api --no-deps`
and consult [the API root](../../crates/dive-api/src/lib.rs),
[trace guide](traces.md), [temporal guide](temporal.md) and
[the final TLC API/CLI contract](../tlc-final-api.md).

For pure conditional predicates through `FinalTlc`/`tlc_temporal::check_final`,
see [the conditional API example](../tlc-state-if.md). Ordinary periodic
scheduling, exact temporal identity and unproved captured bindings remain
explicitly unavailable in this bounded path.


The [fair procedure guide](../pluscal-procedure-fairness.md) includes a compiled
Rust caller using `TranslationRequest.limits.fairness`. Shared finite process
procedures retain per-ID ownership; their admitted translation reports still
carry `model_proved: false`. Read [PlusCal boundaries](pluscal.md) before
interpreting a generated Spec as a checker result.

For pure operator formals through `tlc_temporal::check_final`, use the runnable
[operator example](../temporal-operators.md). The low-level
`Evaluator::scope_operator_analysis` exposes a callback-free known user scope for
analysis only; its unknown formals cannot be forced or certify a model. Actual
applications use real arguments and fresh independent witness replay.

For stored concrete values and original claim names in final checking, use the
compiled examples in [material captures](../material-capture.md) and
[property origins](../tlc-property-origins.md). Their ownership, ordering and
work/depth admission checks are separate from finite graph construction.
