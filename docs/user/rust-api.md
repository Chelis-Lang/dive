# Embedding dive in Rust

`dive-api` is published on crates.io together with the crates it builds on:

```toml
[dependencies]
dive-api = "0.1"
serde_json = "1"
```

All dive crates are released together at one version, and each requires the
exact versions of the others, so depend on `dive-api` alone. To build against
an unreleased checkout instead, use a path dependency such as
`dive-api = { path = "../dive/crates/dive-api" }`.

The caller needs Rust 1.99 or later, and no JVM.

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

Run it from a function that returns `Result`, with your own source and
configuration paths. `Ok(report)` means only that the operation produced a
report: check its verdict, refusal, completion and validation before you
treat the model as checked. API errors also distinguish rejected inputs, I/O, named unsupported
requests and internal failures.

## Recheck edited safety invariants

The source-checkout API in `dive_api::safety_reuse` lets a long-lived caller
keep a Core search while editing its configured invariants. Bind each
revision with `Program::load` or `Program::from_reports`. The latter uses the
same analyzed transitive module closure for eligibility and compilation.

```rust
use dive_api::safety_reuse::{Limits, Program};
use std::path::Path;

fn recheck(before: &Path, after: &Path, cfg: &Path)
    -> Result<(), Box<dyn std::error::Error>>
{
    let policy = Limits::default();
    let (artifact, original) = Program::load(before, cfg)?.capture(policy)?;
    let edited = Program::load(after, cfg)?;
    let current = artifact.recheck(&edited, policy)?;
    println!("original {:?}, current {:?}", original.counts(), current.counts());
    Ok(())
}
```

The artifact owns original concrete rows, discovery parents, generating actions
and search counters. A successful full exploration can decide an edited true
or false invariant. An original refutation can decide a still-reachable
failure at or before its stopping point. Each false result includes the current
property name and a finite trace validated independently against the current
program. The reported counts describe where fresh Core would have decided it.
The scan generates zero successors.

The policy uses exact storage, the Dive profile, one worker, seed/fingerprint
zero, and the same state, depth and retained-row limits. The default row cap
is one million. Callers with wall-clock or process-memory requirements must
enforce them externally. The initial subset admits integer/Boolean state
variables, pure ordinary invariants, literal constants, and an explicit
Init/Next pair or equivalent safety specification. It compares all loaded
module sources and their origin and resolution relative to the root. It can
change the list/order of configured invariants, or the body of an isolated
zero-argument invariant whose identifier occurs only in its own declaration.
Other edits, constraints, overrides, symmetry, views, unsupported module
origins and temporal checks produce a miss. Admission runs before binding
can evaluate an assumption or constant. A file shadowing a native standard
module, an impure native operation or an explicit `DIVE_CORE_OPT` override
also misses. The artifact expires with the
process.

On a miss, capture the current model afresh or use `check_file` to obtain its
ordinary result, including stops and evaluation errors. The example processes
successive snapshots and implements both fallbacks. It emits one JSON record
per revision with timings, counts and any independently validated witness.
Keep each snapshot's module filename the same.

```sh
cargo run -p dive-api --release --example recheck_invariants -- \
  before/Model.tla before/Model.cfg after/Model.tla after/Model.cfg
```

For measurements and the reproduction command see [edited-invariant
benchmarks](../developer/performance.md#edited-invariants).

For pure safety checks, `CheckRequest.store_compact_bytes = Some(0)` with
`check_file(&request)` reclaims expanded persistent values
after each completed level. A positive value triggers reclamation when the
measured Store bytes exceed it; it is not a hard cap on live memory. `None` uses the default 256 MiB trigger
on pure fingerprint safety BFS, which does not change results. The same field may be enabled for
`check_file_checkpoint` recovery-only requests. See the supported
combinations, retained values and metrics in [storage and recovery](storage-recovery.md).

Distributed workers use `prepare_core_worker` and `CoreWorker::serve`, and the
coordinator sets `CheckRequest.distributed` with `DistributedOptions`. Every
participant prepares the same model with the same executable. The
[distributed safety guide](distributed-safety.md) covers authentication,
retries, supported combinations and both roles. The default is `None`. A
worker's own report says nothing about the model; only the coordinator's
result does.

The repository includes an independent consumer Cargo project and executable
`#[test]` examples for public API workflows:

```sh
scripts/test-rust-consumer.sh
```

The script compiles that project with its own locked dependency graph, outside
the workspace's feature union. It then runs the compiled tests with an empty
`PATH`, so Java and the `dive` executable cannot satisfy the workflow. It uses
the checkout's own `target/` and needs Cargo and Python only while preparing the
test executable. The tests call the public library for:

- successful checks and independently validated violations;
- seeded constructive samples;
- DFID completion and depth bounds;
- accepted and rejected implementation traces;
- PlusCal translation followed by checking;
- safety recovery from a checkpoint.

Each test inspects the report it gets back. The project is in
`crates/dive-api/tests/fixtures/rust-consumer/`; its tests are examples to adapt
for a Rust application's own specifications and recorded behaviors.

`CheckRequest.source_coverage` and `CheckRequest.tlc_coverage` are named
refusals (`ApiError::Unsupported`): dive does not collect source or TLC
logical coverage.

Low-level callers may supply predicate and action scopes. Scope ownership and
expression references are checked before backend startup. Parallel graph capture
and DFID also inspect captured callables for printing, randomness, I/O or
live checker reads; a caller-supplied constant-level annotation cannot hide those
effects. These scopes refuse before worker execution or output. Pure captured
operators remain usable, and `Evaluator::scope_has_effects` exposes this
non-evaluating effects check without forcing stored values or callbacks.

Low-level graph consumers implement `dive_runtime::capture::Observer`. dive
builds its behavior graphs itself. Observation requires one worker; `--compat=tlc` uses the same
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

A callback error or panic stops exploration with the boundary counters
and no completed graph. A failed Init or partial level supplies no completion
event, and an earlier safety failure retains its verdict and counterexample.
Prefix graphs can support independently validated violations; absence of a
violation proves nothing before exhaustion. The `Observer` rustdoc contains a
runnable example. Parallel graph capture without an observer uses the
unobserved entry points.

Completed graph consumers can retain original producer facts while releasing
graph storage with `capture::Complete::into_evidence`; `Prefix::into_evidence`
does the same for a successful committed prefix. `evidence()` borrows the
facts while keeping the graph. These immutable values preserve the original
safety result, options and counters independently of edits to public capture
fields. Complete facts keep the record that the graph was completed; prefix
facts keep their level and unexhausted queue and can never become complete.
Neither kind proves a temporal property. `GraphBuilder::complete` and `prefix`
return the original verdict when supplied a failed or stopped safety result.
Graph-owning variants keep their graphs; native reports retain
only sealed facts and any independently validated selected word.

Producers retaining their own state rows can supply borrowed adjacency through
`capture::CsrCaptureInput`. `CsrCapture::checked` checks bounds, roots, ordinals,
original row ids and implicit stutters under the run's `EvaluationControl`.
`CsrComplete::from_producer` additionally requires the producer's successful EOF
and matching statistics; `CsrPrefix::from_producer` requires a successful committed
boundary with an unfinished frontier. Both preserve original failures and budget
stops. Their `evidence()` and `into_evidence()` expose the same graph-independent
facts. These low-level APIs do not evaluate the opaque rows
or certify temporal properties. The limits are the usual `capture::Limits`.
dive's temporal checking
uses native retained rows and checked CSR, without a retained value graph. The `capture_csr` and `capture_evidence` tests build checked CSR and
producer evidence from captures; run
`cargo test -p dive-liveness --test capture_csr --test capture_evidence --locked`.

After temporal analysis, `Analysis::into_native()` and
`PeriodicAnalysis::into_native()` release that legacy graph storage. Their
results live in `analyze::periodic::external`: `NativeFinal` retains completed
producer facts and any `NativeWord`; `NativeAnalysis` distinguishes a final
result from `NativePrefix`. Words retain their property set, concrete states,
closing position and original edge metadata, and still require independent
validation. Prefix facts cannot become a holding result. The other analysis
entrypoints keep their return types.
dive reads these results directly when it builds its reports. Releasing the
graph frees memory; it does not let a capture grow larger or prove more.

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
metadata is not a complete RSS charge. Cache hits still check the run budget,
and eager admission always converts. Only the values a check asks for and the
bounded cache are kept in memory.
`GraphCheck::level`, `finish` and `finish_prefix` are also available.

dive's `temporal::Report::outcome()` returns one of these boxed variants, or
`Safety` with the ordinary safety result:

| Native outcome | Accessible evidence |
| --- | --- |
| `NativeHolds` | `CompleteEvidence`: exhausted safety result, options, counters and completion receipt |
| `NativeViolation` | `ValidatedNativeCounterexample`: `word()`, complete `evidence()`, `document()`, `validation()` |
| `NativePrefixViolation` | `ValidatedNativePrefix`: `word()`, unfinished-prefix `evidence()`, `document()`, `validation()` |

Validated wrappers have private constructors. Each selected word passes fresh
independent typed and serialized replay after the native graph, converter, store and plan
are dropped. `NativeWord` alone is unvalidated; a validation marker or
caller-supplied CSR cannot construct a validated outcome. JSON retains schema 3, existing trace
endings and completion/worker/collision/output metadata. The core example
and report meanings are in the [temporal guide](temporal.md#metrics).

| Entry | Meaning |
| --- | --- |
| `parse_source` / `parse_file` | Syntax/CST and source diagnostics |
| `analyze_source` / `analyze_file` | Resolved semantic analysis |
| `parse_config_source` / `parse_config_file` | Configuration reading |
| `check_file` | Safety and temporal checking (`TemporalMode::Ordinary`, the only mode); read the route and completion in the report |
| `simulate_file` | Seeded finite traces, never exhaustive proof |
| `dfid_file` | Iterations and explicit completion evidence |
| `CheckRequest::host_access` | Explicit env/files/exec permissions; a named refusal |
| `host_inputs::inputs_from_report` / `TraceOptions::inputs` | Explicit external assumptions for fresh offline source replay |
| `translate_source` / `translate_file` | Admitted in-memory PlusCal artifact |
| `prepare_translation_file` / `write_translation_file` | Explicit admitted module, backup and merged cfg publication; prepared publication checks concurrent edits |
| `original_ast_source` / `write_original_ast_file` | Separate original AST bytes/publication; a successful AST file write retains pinned exit 255 |
| `trace::SpecModel` | Independent implementation/witness replay |
| `CheckReport::temporal_result` / `temporal_failure` | Temporal report with private validated violations, or its typed failure |
| `check_file_checkpoint` | Bounded safety pause and recovery |

Request fields make profile and module resolution explicit. Rust APIs do not
implicitly adopt the CLI's environment-variable parsing. Reports preserve worker widths, seeds, limits and mode qualifications; retain these alongside the
source/configuration in a `cargo test` failure artifact.

Low-level crates expose syntax, IR, values, evaluation, stores and staged temporal
graph studies. Public IR/model metadata is validated before execution; manually
constructing a graph or marking a validation boolean does not supply a certified
model result. The trace crate decides `ENABLED` with the core evaluator and
shares nothing else with the checker.

`dive-store` keeps what dive uses: `state::{StateFingerprinter, state_key}`,
`StoreMode`, the [disk seen-state set](disk-seen-library.md), the ordered
frontier spill and the predecessor log.

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
procedures keep per-ID ownership; their translation reports carry
`model_proved: false`, because translation checks nothing. Read [PlusCal boundaries](pluscal.md) before
interpreting a generated Spec as a checker result.


The TLC-internal liveness diagnostics (`tlc_temporal`, `periodic_candidate`,
`phase_*`, `periodic_*` and `tlc_*` projections other than `tlc_plan::classify`)
were removed. Dive does not reproduce TLC's exact lasso or periodic timing.
A configured `_PERIODIC` entry binds with TLC's
errors and is then refused by name.

`SimulateRequest` and `DfidRequest` also expose `BudgetSetting` fields
`time_limit` and `max_memory`, with the same defaults as checking. Their finite
reports distinguish `stopped` (exit 5) from sampled/incomplete/completed outcomes;
a stopped report never has `model_proved: true`. One budget covers execution,
retained-state serialization checkpoints and fresh finite-word validation.
See the mode guides for the after-loading scope and uninterruptible value units.

`simulate_file` and `dfid_file` run compiled finite execution. A reached refusal is never retried. Reports name the engine that
ran. Seeded reproduction needs the same request settings.
