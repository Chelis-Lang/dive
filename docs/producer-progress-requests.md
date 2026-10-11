# Producer-owned progress requests (bounded Rust diagnostic)

Issue #201 adds an internally sampled request lane to the
[configuration-owned snapshot](configured-periodic-snapshot.md). A real timer
request stops new expansion claims. Claimed work returns, the pool joins, and a
borrowed pause holds its actual dispatch mutex before unclaimed work resumes.
The lane accepts a timing policy, not a caller clock, callback or claimed pause.

This does **not** enable ordinary `dive check` periodic checking or produce a
model verdict. `model_proof()` stays false. Completed expansion output is not a
successful safety check, logical source commit or written temporal graph. The
new pause records those facts separately. Only the existing ordered source
commit path evaluates the actual configured hook and temporal graph; this first
slice never performs mid-wave SCC checks on uncommitted output.

## Run the maintained Cargo example

```sh
cargo run -p dive-api --example progress_requests --locked
```

The example prints:

```text
configured=Hook request=1 actual_pool=4 uncommitted=0 model_proof=false
```

Its initial forced **request** is serviced before claims. The distinct existing
liveness-controller force option remains off. Once source publication happens,
the genuine configured `Hook == FALSE` stops the diagnostic before safety EOF.

```rust
//! Configuration-owned diagnostic requests; no ordinary temporal proof.
use dive_check::{CheckOptions, progress};
use dive_liveness::{periodic_lifecycle, periodic_snapshot, phase_graph};
use std::sync::Arc;
use std::time::Duration;
fn main() -> Result<(), String> {
    let source = "---- MODULE ProgressGuide ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x>=0)\nHook == FALSE\n====\n";
    let analysis =
        dive_sema::analyze_source("ProgressGuide.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|e| e.to_string())?);
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .ok_or("configuration")?;
    let snapshot = periodic_snapshot::bind(ir, &cfg).map_err(|e| e.to_string())?;
    let run = snapshot
        .execute_requests(
            &CheckOptions {
                workers: 4,
                ..CheckOptions::tlc(0, 0)
            },
            phase_graph::Limits::default(),
            periodic_lifecycle::Options {
                progress_interval: 0,
                ratio_limit: 0.0,
                ..periodic_lifecycle::Options::default()
            },
            progress::Options {
                interval: Duration::from_secs(1),
                force: true,
            },
        )
        .map_err(|e| e.to_string())?;
    let lifecycle = run.lifecycle();
    let pause = lifecycle
        .expansion_pauses()
        .first()
        .ok_or("actual request pause")?;
    let receipt = pause.receipt();
    if !receipt.request().forced()
        || !receipt.claimed().is_empty()
        || !receipt.completed().is_empty()
        || lifecycle.model_proof()
        || lifecycle.hook_evaluations().is_empty()
        || lifecycle.safety_eof().is_some()
    {
        return Err("request/commit distinction".into());
    }
    println!(
        "configured={} request={} actual_pool={} uncommitted={} model_proof=false",
        run.configured_name(),
        receipt.request().sequence(),
        pause.pool().0,
        receipt.completed().len()
    );
    Ok(())
}
```

## Inspect facts and failures

`run.lifecycle().expansion_pauses()` exposes privately produced request identity,
wall-clock sample, initial-force reason, actual pool width/generation, submitted
wave range, actual claimed IDs, retained completed outputs, error-bearing outputs,
speculative generated successors and prior logical counters. A cut-skipped claim
can be claimed without completed output; later invariant checks can still reject
an otherwise returned expansion. None grants Phase-L write or stopping-count
agreement. Request IDs increase; timing and receipt counts are not deterministic.

Pending requests coalesce. Consuming a request starts another timer interval;
a new request may accumulate while a pause callback is still running. The timer
closes and joins on normal return, callback error and unwind. An actual failed
Init creates no successful request pause or EOF. A coordinator unwind wakes
waiting pool workers before joining them. Caller observation failure stops by
name; it cannot create successful graph or temporal evidence. Once an actual
expansion cut is observed, `requests_cancelled()` reports timer shutdown, and
`aborted_request()` retains at most the one pending request. That is distinct
from closed claim admission: earlier unclaimed sources still drain through the
original ordered error path. No hook or SCC check is fabricated for the aborted
request. This prevents repeated timer publication from starving error drainage.
A later quota failure can retain an actual safety EOF while refusing temporal
completion; EOF alone never establishes a model proof.

Use `request_options()` for the actual additional interval/initial-force policy.
Intervals outside 1..=2,147,483,647 ms refuse before producer setup in this facade.
The fixed maximum of eight expanded source units remains unchanged. Retained
pause metadata shares the cumulative `graph.steps` quota; source/predicate
inspection and Phase-L value-work/depth limits remain separate. These ceilings
are entry/inspection policies, not aggregate heap/byte or evaluator complexity
bounds. Requests cannot raise or bypass them.

The factory revalidates the owned IR/names, actual configuration selection and
bounded pure zero-state hook. Effects, runtime names, unproved startup work and
opaque captures retain the snapshot refusals. Default-profile production remains
refused; independently bound default-profile witness replay is a separate check.
Do not evaluate the snapshot or evaluator-sharing clones concurrently with this
run: the evaluator has mutable run state. The timer itself evaluates no model.

Ordinary CLI `_PERIODIC` binding/scheduling, checkpoint transactions, distributed
workers and ordinary temporal verdicts remain unsupported. An independently
valid candidate proves its word/property only, not this diagnostic's ordinary
schedule or complete-model absence of violations.

## Reference evidence and next implementation

[The complete reference selection](../conformance/probes/progress-queue/README.md)
contains 33 actual local `Worker`, `AbstractChecker.runTLC`,
`ModelChecker.doPeriodicWork` and real-queue observations at widths 1/4/8.
Controlled request injection through a deliberately instrumented Next gate is
labelled separately from natural-clock execution. Early 2116 and 2104 returns
leave workers alive; probe-owned finish/join is visibly separate cleanup.
Worker division-by-zero retains actual 2179 and `keepCallStack=true` while
`runTLC` returns zero, so zero is not a successful model-check certificate.
Failed driver epochs and every selected error are preserved.

The next production contract is ordered publication of a maximal completed
contiguous source prefix under genuine quiescence, preserving the same safety,
output, counter and graph rules while retaining the out-of-order remainder.
Only that actually written prefix may enter periodic checking. Queue/clock,
stop/error/count, checkpoint and complete deciding-witness contracts must still
compose before ordinary checking can be admitted. See
[ADR 0059](decisions/0059-producer-progress-requests.md) and the
[parity roadmap](design/tlc-parity-plan.md).
