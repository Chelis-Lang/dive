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

For pure Core safety, `CheckRequest.store_compact_bytes = Some(0)` with
`check_file_engine(&request, Engine::Core)` reclaims expanded persistent values
after each completed level. Positive values trigger on measured Store bytes;
this is not a hard live-memory cap. `None` uses Core's default 256 MiB trigger
on pure fingerprint safety BFS, which does not change results. The same field may be enabled for
`check_file_checkpoint_engine` recovery-only requests. See the supported
combinations, retained owners and metrics in [storage and recovery](storage-recovery.md).

Explicit native worker roles use `prepare_core_worker`/`CoreWorker::serve`, and
coordinators use `CheckRequest.distributed` plus `DistributedOptions` under the
explicit Core engine. Participants independently prepare the same model with
the same executable. The [distributed safety guide](distributed-safety.md)
documents authentication, retries, current combination bounds and both roles.
The default is `None`; worker facts alone never certify a model result.

The repository includes an independent consumer Cargo project and executable
`#[test]` examples for ordinary public API workflows:

```sh
scripts/test-rust-consumer.sh
```

The script compiles that project with its own locked dependency graph, outside
the workspace's feature union. It then runs the compiled tests with an empty
`PATH`, so Java and the `dive` executable cannot satisfy the workflow. It uses
the checkout's own `target/` and needs Cargo and Python only while preparing the
test executable. Checking success and independently validated violations, seeded
constructive samples, DFID completion versus bounds, implementation trace
acceptance/rejection, translation followed by checking and real safety recovery
are exercised through public library calls on Core; the tests inspect actual
results rather than treating `Ok(report)` as a proof. The independent project is in
`crates/dive-api/tests/fixtures/rust-consumer/`; its tests are examples to adapt
for a Rust application's own specifications and recorded behaviors.

`CheckRequest.source_coverage` and `CheckRequest.tlc_coverage` are named
refusals (`ApiError::Unsupported`): source and TLC logical coverage ran only on
the removed reference engine (#791, #910). `dive-eval`'s collectors for them
(`SourceCoverage`, `TlcCoverageSnapshot` and `Evaluator::new_source_covered`)
were removed with it.

Low-level callers may supply predicate and action scopes. Scope ownership and
expression references are checked before backend startup. Parallel graph capture
and DFID also inspect actual captured callables for printing, randomness, I/O or
live checker reads; a caller-supplied constant-level annotation cannot hide those
effects. These scopes refuse before worker execution or output. Pure captured
operators remain usable, and `Evaluator::scope_has_effects` exposes this
non-evaluating effects check without forcing stored values or callbacks.

Low-level graph consumers implement `dive_runtime::capture::Observer`. The
reference capture drivers that called it (`dive_check::capture::explore*`) were
removed with the `dive-check` crate (issue #791); Core builds its behavior
graphs itself. Observation requires one worker; `--compat=tlc` uses the same
hook semantics. Methods are inert by default.
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

Completed graph consumers can retain original producer facts while releasing
graph storage with `capture::Complete::into_evidence`; `Prefix::into_evidence`
does the same for a successful committed prefix. `evidence()` borrows the
facts while keeping the graph. These immutable values preserve the original
safety result, options and counters independently of edits to public capture
fields. Complete facts retain the existing graph-completion receipt; prefix
facts retain their level and unexhausted queue and cannot become completion.
Neither kind proves a temporal property. `GraphBuilder::complete` and `prefix`
return the original verdict when supplied a failed or stopped safety result.
Existing graph-owning variants keep their graph owners; native reports retain
only sealed facts and any independently validated selected word.

Producers retaining their own state rows can supply borrowed adjacency through
`capture::CsrCaptureInput`. `CsrCapture::checked` checks bounds, roots, ordinals,
original row ids and implicit stutters under an actual `EvaluationControl`.
`CsrComplete::from_producer` additionally requires the producer's successful EOF
and matching statistics; `CsrPrefix::from_producer` requires a successful committed
boundary with an unfinished frontier. Both preserve original failures and budget
stops. Their `evidence()` and `into_evidence()` expose the same graph-independent
facts as reference capture. These low-level APIs do not evaluate the opaque rows
or certify temporal properties. Existing `capture::Limits` remain unchanged;
there is no new CLI option or TLC divergence. The ordinary core temporal route
uses native retained rows and checked CSR, without a retained reference value
graph. The `capture_csr` and `capture_evidence` tests build checked CSR and
producer evidence from actual captures; run
`cargo test -p dive-liveness --test capture_csr --test capture_evidence --locked`.

After temporal analysis, `Analysis::into_native()` and
`PeriodicAnalysis::into_native()` release that legacy graph storage. Their
results live in `analyze::periodic::external`: `NativeFinal` retains completed
producer facts and any `NativeWord`; `NativeAnalysis` distinguishes a final
result from `NativePrefix`. Words retain their property set, concrete states,
closing position and original edge metadata, and still require independent
validation. Prefix facts cannot become a holding result. Existing reference
analysis entrypoints keep their current return types.
The core now consumes these carriers directly for graph-neutral reports;
detaching a graph alone adds no capture capacity or proof authority.

The native producer uses `GraphCheck::native_level_due(stats)` before scanning
CSR, then `level_native` at due committed levels. `finish_native` consumes
`CsrComplete`; `finish_native_prefix` consumes `CsrPrefix`. Both return
`(NativeAnalysis, Checks)` through the existing failure channel. A `NativeRead`
callback receives a checked opaque row id and this run's `EvaluationControl`;
it returns all model variables in IR order or an explicit failure. Missing
values and typed stops cannot become FALSE truth or absence of acceptance.
The core converter reuses successful exact-handle closed materialized trees in a
bounded run-local cache: Bool/Int/run-owned-string leaves and recursive tuples,
records, general functions with scalar keys and fully stored enumerated sets.
Raw/symbolic sets, BigInt/model values, foreign names and unproved shapes still
reconvert.
Its allowance is `min(128 MiB, memory_budget / 64)` (128 MiB when unlimited),
covering actual table capacity, one-component row arrays and known private
function/set/boxed payload and possible scalar-domain TLC-order storage. Shared
descendants are conservatively charged per occurrence; work/depth exhaustion
bypasses reuse. One-variable model hits reuse the same shared row;
multi-variable models assemble rows from cached components. In-range opaque row
IDs select single-variable slots directly; exact Handle comparison still guards
reuse. Other reads retain the bounded handle-hash path. Conversion timing excludes
single-variable hits, as reported by `convert_ms_scope`. Run-owned
name payload is shared after pointer-identity checking; opaque Arc/allocator
metadata is not a complete RSS charge. Hits still check actual run control and
eager admission always converts. Only demanded values and the bounded cache are
retained; this is not an unbounded Value graph or a new capacity claim. Reference `GraphCheck::level`/`finish`/`finish_prefix` remain.

Core's `temporal::Report::outcome()` returns one of these boxed variants, or
`Safety` with the ordinary safety result:

| Native outcome | Accessible evidence |
| --- | --- |
| `NativeHolds` | `CompleteEvidence`: actual exhausted safety result, options, counters and completion receipt |
| `NativeViolation` | `ValidatedNativeCounterexample`: `word()`, complete `evidence()`, `document()`, `validation()` |
| `NativePrefixViolation` | `ValidatedNativePrefix`: `word()`, unfinished-prefix `evidence()`, `document()`, `validation()` |

Validated wrappers have private constructors. Each selected word passes fresh
independent typed and serialized replay after native graph/converter/store/plan
owners are dropped. `NativeWord` alone is unvalidated; a validation marker or
caller-supplied CSR cannot construct a validated outcome. Prefix facts never
become complete facts or a holding result. JSON retains schema 3, existing trace
endings and actual completion/worker/collision/output metadata. The core example
and report meanings are in the [temporal guide](temporal.md#native-graph-capture-with-the-core-engine).

| Entry | Meaning |
| --- | --- |
| `parse_source` / `parse_file` | Syntax/CST and source diagnostics |
| `analyze_source` / `analyze_file` | Resolved semantic analysis |
| `parse_config_source` / `parse_config_file` | Configuration reading |
| `check_file` | On Core, the only engine (`Engine::Core`; `check_file_engine` takes it explicitly): ordinary safety and ordinary temporal checking; `TemporalMode::FinalShort` is a named refusal; inspect actual route/completion |
| `simulate_file` | Seeded finite traces, never exhaustive proof |
| `dfid_file` | Iterations and explicit completion evidence |
| `CheckRequest::host_access` | Explicit env/files/exec permissions; a named refusal on Core (external inputs ran only on the removed reference engine) |
| `host_inputs::inputs_from_report` / `TraceOptions::inputs` | Explicit external assumptions for fresh offline source replay |
| `translate_source` / `translate_file` | Admitted in-memory PlusCal artifact |
| `prepare_translation_file` / `write_translation_file` | Explicit admitted module, backup and merged cfg publication; prepared publication checks concurrent edits |
| `original_ast_source` / `write_original_ast_file` | Separate original AST bytes/publication; a successful AST file write retains pinned exit 255 |
| `trace::SpecModel` | Independent implementation/witness replay |
| `CheckReport::temporal_result` / `temporal_failure` | Core temporal report with private validated violations, or its typed failure |
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

Low-level items that only the removed reference engine called, such as
`dive_model::bind_with_names` and `dive_explore::explore_checkpoint`, are gone;
`CHANGELOG.md` lists them (#791).

`dive-store` keeps what Core uses: `state::{StateFingerprinter, state_key}`,
`StoreMode`, the [disk seen-state set](disk-seen-library.md), the ordered
frontier spill and the predecessor log. The reference checker's in-memory
stores (`StateSet`, `FpSet`, `ExactSet`, `frontier`, `TraceLog`,
`encode_state`/`StateDecoder` and the `collision` estimates) were removed with
it (#791).

`Value::set_controlled`, `fcn_controlled` and `record_controlled` accept a
cooperative `proceed` callback. Returning `false` discards the partial result
with `ConstructionError::Interrupted`; scratch reservation failure and original
value errors stay distinct. Closed materialized normalization is cooperative.
Symbolic children use the existing constructor, and final shared allocations
retain Rust's allocation behavior. These helpers establish no memory ceiling.

For exact type signatures and compiled examples run `cargo doc -p dive-api --no-deps`
and consult [the API root](../../crates/dive-api/src/lib.rs),
[trace guide](traces.md) and [temporal guide](temporal.md).

`dive-liveness` has no public API for building a temporal product from a
supplied graph and labels. Temporal properties are checked through `CheckRequest`, which
explores the product on the fly.

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

`SimulateRequest` and `DfidRequest` also expose `BudgetSetting` fields
`time_limit` and `max_memory`, with the same defaults as checking. Their finite
reports distinguish `stopped` (exit 5) from sampled/incomplete/completed outcomes;
a stopped report never has `model_proved: true`. One owner covers execution,
retained-state serialization checkpoints and fresh finite-word validation.
See the mode guides for the after-loading scope and uninterruptible value units.

`simulate_file` and `dfid_file` run compiled finite execution on Core, the
only engine; `Engine::Reference` was removed with the reference checking
runtime (#791). A reached refusal is never retried. Reports name the engine
that ran. Seeded
reproduction includes that engine and the same request settings.
