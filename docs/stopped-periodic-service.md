# Stopped-worker periodic diagnostics in Rust

`periodic_candidate::check_optional_stopped` retains the original configuration,
actual ordered source winner and joined dispatch authority. It adds a bounded
service before timer/pool closure for successor invariant/action-property
violations, deadlocks, incomplete successors and worker evaluation errors.
The original error remains in `run().lifecycle().backend().stop()`; service faults
are separate. This API is diagnostic. It never reports temporal Holds or proves
ordinary TLC clock/queue/count equivalence. Ordinary `dive check` keeps refusing
this scheduling contract.

Run the Cargo-owned example with the pinned workspace toolchain:

```sh
cargo run -p dive-api --example stopped_periodic_candidate --locked
```

Native execution needs no Java or host grants. Exact source:

```rust
//! Keep an actual source winner separate from the stopped-queue diagnostic.
use dive_api::periodic_candidate;
use dive_check::{CheckOptions, capture::WorkerStop, progress};
use dive_liveness::{periodic_lifecycle, phase_graph};
use std::{sync::Arc, time::Duration};
fn main() -> Result<(), String> {
    let source = "---- MODULE StoppedGuide ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == x'=1\nInv == x=0\nLive == <>(x=0)\nHook == FALSE\n====\n";
    let a = dive_sema::analyze_source("StoppedGuide.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&a).map_err(|e| e.to_string())?);
    let cfg =
        dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY Live\n_PERIODIC Hook\n")
            .into_config()
            .ok_or("configuration")?;
    let r = periodic_candidate::check_optional_stopped(
        ir,
        cfg,
        CheckOptions {
            workers: 4,
            ..CheckOptions::tlc(0, 0)
        },
        phase_graph::Limits::default(),
        periodic_lifecycle::Options::default(),
        progress::Options {
            interval: Duration::from_secs(3600),
            force: false,
        },
    )
    .map_err(|e| e.to_string())?;
    let run = r.run().lifecycle();
    let stopped = run.stopped_service().ok_or("no actual stopped queue")?;
    if stopped.kind() != WorkerStop::Invariant
        || stopped.code() != 2110
        || stopped.pool().0 != 4
        || stopped.joined_stats().is_none()
        || run.safety_eof().is_some()
        || !run.final_checks().is_empty()
        || !run.service_evaluations().is_empty()
        || r.witness().is_some()
        || r.model_proof()
    {
        return Err("source-stop qualification".into());
    }
    let phase_graph::Error::Capture(original) =
        &run.backend().stop().ok_or("no original winner")?.error
    else {
        return Err("wrong original stop".into());
    };
    println!(
        "original={} code={} source={} workers={} joined={} hook_calls={} model_proof={}",
        original.verdict.kind(),
        stopped.code(),
        stopped.source(),
        stopped.pool().0,
        stopped.joined_stats().is_some(),
        run.service_evaluations().len(),
        r.model_proof()
    );
    Ok(())
}
```

It prints:

```text
original=invariant-violation code=2110 source=0 workers=4 joined=true hook_calls=0 model_proof=false
```

The configured FALSE hook is genuinely bound from `_PERIODIC Hook`; it is skipped
because the already-stopped queue cannot suspend. It cannot replace the original
invariant error. A configuration without `_PERIODIC` retains genuine absence,
not a caller-supplied TRUE expression. The exposed diagnostic model still refuses
ordinary checking. Existing mandatory, optional and exhausted-terminal APIs keep
their signatures and behavior.

## Outputs and admission

Use `stopped_service()` to inspect the actual original source/discovery identity,
worker family, deciding code, prejoin counters, last published graph source,
pending request, paused pool width/generation and controller result. `joined_stats()`
contains the real postjoin result counters. Ordinary error replay may change them;
it does not rewrite the prejoin stopping snapshot. `stopped_faults()` names lane
close/join, dispatch, callback and unwind failures separately. Callback failures
never replace the original source winner. Catching Rust unwind does not promise
survival after process abort.

`Complete` in a controller receipt means that invocation returned; it does not
mean successful exploration or completed temporal checking. A hook-free unforced
call with no graph growth can return without suspension and preserve TLC's IEEE
ratio update, including NaN. A selected forced call attempts failed suspension
without clearing its force flag. These policy choices are reported; defaults
remain interval 3000 ms, ratio limit 0.2, initial force false. The request interval
in the example is an explicit one-hour diagnostic selection, not a claim about
TLC's natural progress schedule.

The service is produced from the actual source stop; copied counters, caller words
or caller EOF/suspension flags cannot authorize it. Its private admission and
postjoin matching rules are documented in [ADR 0078](decisions/0078-stopped-queue-worker-errors.md).

## Supported and refused combinations

Execution currently requires matching TLC-profile checker/evaluator options and
a private configuration-owned optional snapshot with admitted temporal OOS groups.
A safety-only model without those groups currently refuses this diagnostic
endpoint; use ordinary safety checking for safety-only specifications. Prestartup ownership, bounded
pure grammar, configured alias/replacement and zero-state hook guards remain.
Unknown/effectful/opaque callbacks, state-variable hooks, malformed/foreign public
IR, unknown ownership and configured unsupported features refuse before this
service. Default-profile execution refuses before work; independent validator
profiles remain a separate qualified obligation.

Failed Init, Init invariant failures, resource limits, library/internal failures,
unsupported operations and earlier deciding hook/candidate stops never enter the
worker-stop transaction. A shared-meter service refusal preserves the source
winner, with a fault and no invented completed service/EOF receipt. Successful
empty Init is real exhaustion and follows the existing terminal/final path;
an Init type/evaluation error is not empty success.

The endpoint retains the existing capture/Phase-L/graph/source-material and
independent replay ceilings. There is no new quota, aggregate-memory bound or
exact evaluator-complexity guarantee. Successful EOF still performs bounded
final selection. Every deciding temporal candidate uses the one shared private
reconstruction and freshly bound/re-homed independent memory and lossless-JSON
replay path. A refused replay remains a refusal; no caller Valid flag is trusted.

## Evidence and remaining work

Selected pinned worker errors agree on their deciding code/outer CLI class;
complete raw selections, failures and current-source accounting live in the
[acceptance README](../conformance/probes/stopped-periodic-service/README.md).
A method return, CLI exit and diagnostic controller result are separate facts.

Java workers can still call `doNextCheckLiveness`/`addNextState` after an accepted
error. Native evidence contains only the actual earlier committed graph prefix.
This gap prevents ordinary Java queue/count/graph credit. Real pool joins and
internally sampled clock values do not close natural-clock timing or the full
ordinary runTLC/stop/selection contract. Before an ordinary driver can be admitted, it must prove the genuine clock/queue
service cadence, Java after-error graph and ordered stopping-count behavior,
final-selection/error precedence and checkpoint composition. The current
ordinary route stays refused rather than substituting these diagnostics.

See [the complete probe selection](../conformance/probes/stopped-periodic-service/README.md),
[ADR 0078](decisions/0078-stopped-queue-worker-errors.md),
[successful EOF diagnostics](terminal-periodic-service.md) and
[the parity plan](design/tlc-parity-plan.md).
