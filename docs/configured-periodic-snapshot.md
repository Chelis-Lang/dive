# Configuration-owned periodic snapshot diagnostic

The Rust API `dive_liveness::periodic_snapshot::bind` selects a pure `_PERIODIC`
body from the configuration's actual pre-constant definition snapshot. Its
`Snapshot::execute` connects that private selection to real native dispatch
pauses. This is a bounded TLC-profile diagnostic: it does **not** enable ordinary
CLI scheduling, return temporal Holds or certify a model-checking violation.

Run the complete Cargo-owned example from the repository root:

```sh
cargo run -p dive-api --example periodic_snapshot --locked
```

Expected output:

```text
configured=Hook selected=Hook actual_pool=4 false_equality=true model_proof=false
```

```rust
//! Inspect a configuration-owned hook under bounded native diagnostic pauses.
use dive_check::CheckOptions;
use dive_liveness::{periodic_lifecycle::Options, periodic_snapshot, phase_graph::Limits};
use std::sync::Arc;
fn main() -> Result<(), String> {
    let source = "---- MODULE SnapshotGuide ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x>=0)\nHook == FALSE\n====\n";
    let analysis =
        dive_sema::analyze_source("SnapshotGuide.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|e| e.to_string())?);
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .ok_or("configuration")?;
    // Admission occurs before constant startup evaluation. The selection comes
    // from the configuration's pre-constant definition snapshot, not a Pred.
    let snapshot = periodic_snapshot::bind(ir, &cfg).map_err(|e| e.to_string())?;
    let options = CheckOptions {
        workers: 4,
        ..CheckOptions::tlc(0, 0)
    };
    let run = snapshot
        .execute(
            &options,
            Limits::default(),
            Options {
                progress_interval: 0,
                ratio_limit: 0.0,
                ..Options::default()
            },
        )
        .map_err(|e| e.to_string())?;
    let lifecycle = run.lifecycle();
    let call = lifecycle.invocations().first().ok_or("actual invocation")?;
    let hook = lifecycle.hook_evaluations().first().ok_or("actual hook")?;
    if !*hook.result().as_ref().map_err(ToString::to_string)?
        || call.resume_requested()
        || lifecycle.safety_eof().is_some()
        || lifecycle.model_proof()
    {
        return Err("expected diagnostic false hook stop".into());
    }
    println!(
        "configured={} selected={} actual_pool={} false_equality=true model_proof=false",
        run.configured_name(),
        snapshot.model().ir.def(run.definition()).name,
        call.workers()
    );
    Ok(())
}
```

## Selection and outcomes

`configured_name()` preserves `_PERIODIC` spelling. `definition()` identifies
the selected original definition; `predicate()` retains its original body and
owned scope. For `CONSTANT Hook <- Replacement`, configured `Hook` and selected
`Replacement` remain distinct. `Run` privately attaches those identities to the
actual lifecycle receipt. Callers cannot substitute their own `Pred`, claimed
boolean or validation flag for this token.

A false hook stops diagnostically without resuming dispatch; a true hook can
resume but does not mean temporal success. Nonboolean FALSE comparisons retain
TLC error 1000; admitted integer division by zero retains 2179. An earlier
partial temporal candidate suppresses the hook, preserving TLC's ordering.
Inspect `run.lifecycle()` for actual worker, pause, clock, stop and EOF evidence.
Candidate replay against freshly bound models is separate evidence. No receipt
sets `model_proof=true`, and native commit-boundary polling does not establish
TLC asynchronous queue/timer schedule equality.

Ordinary `dive check --compat=tlc` still reports `_PERIODIC` unsupported. Even the
exposed snapshot Model contains a producer-added ordinary-refusal marker; it
cannot complete ordinary graph capture. The private execution clone removes
only that exact marker after revalidation and preserves unrelated refusals.
The global compatibility switch does not imply this unfinished schedule exists.

## Admission and limits

Use the liveness factory for callback-free admission. The lower-level
`dive_model::bind_periodic_snapshot` shares ordinary startup behavior and **may
execute startup definitions**; it is not a sandbox or pre-effect guard.

The guarded factory validates public IR before inspection and refuses host
effects/runtime-name operators throughout inspected model/startup roots before
binding. It includes unused user definitions, standard top-level zero-arity
startup bodies and actual configuration context aliases. Constant precomputation
must satisfy a small grammar: scalar literals/assignments, explicit finite
stored constructors, boolean forms, selected integer arithmetic and nonrecursive
zero-arity calls/LET. Symbolic domains, quantifiers, general function application,
opaque callbacks and recursion refuse even when a small example would finish in
TLC. Replacement-erased state witnesses must pass this startup grammar too;
edited level annotations cannot excuse startup work.

The selected body must also pass the pure zero-state hook grammar. State/action/
temporal references, applied opaque operators, configured constant-definition
captures and INSTANCE substitutions remain named refusals. Pure simple wrappers
through an INSTANCE with no substitution are supported. `_PERIODIC S!Probe` is
rejected by TLC's configuration parser (5006); name lookup cannot replace it.

Source and capture inspection retain 50,000-unit/depth-128 policies, including
text bytes charged before scans. Startup construction reserves a separate
50,000-unit cubic **policy charge**, while repeated hook evaluations reserve
against the existing shared graph quota. These are not proved evaluator
instruction, comparison-work, heap or byte bounds. Existing graph-retention,
generation, source scratch, Session and cumulative partial-check limits remain
unchanged. Limits and named refusals never produce completion credit.

## Errors and evidence

Missing definition 2229 and nonzero arity 2228 retain their original binding
classes where admitted. Constant-role `ClassCastException` is retained as a real
Java primary observation and a named native refusal, not an invented TLC code.
Prebinding effects and unsupported startup work yield `DIVE-L0058` with no output;
unproved zero-state bodies yield `DIVE-L0057`. Actual earlier binding warnings
are preserved when a postbinding body admission refuses.

The [complete primary selection](../conformance/probes/periodic-snapshot/README.md)
retains 26 original rows and 12 additive precedence/startup rows, including all
exceptions and conservative refusals. [ADR 0057](decisions/0057-configured-periodic-snapshot.md)
and [the supplied-expression guide](periodic-hook.md) distinguish the binding
and execution contracts. This increment grants no ordinary-schedule or feature
family certification.
