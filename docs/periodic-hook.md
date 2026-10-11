# Pure supplied periodic expression diagnostics

`dive_liveness::periodic_hook::execute` composes a supplied pure expression with
the internally clocked native lifecycle. It evaluates only under the actual
borrowed dispatch pause, after successful partial liveness and before resume.
This API grants no ordinary model-checking verdict. It is **not** binding of the
configuration's `_PERIODIC` snapshot or a substitute for the ordinary scheduler.

Run the Cargo-owned complete example:

```sh
cargo run -p dive-api --example periodic_hook --locked
```

It prints:

```text
actual_pool=4 false_equality=true resume=false eof=false configured_hook=false model_proof=false
```

The full [Rust example](../crates/dive-api/examples/periodic_hook.rs) binds a pure
TLC-profile model, selects `Hook == FALSE` with its evaluator-owned scope and
supplies that predicate separately. The configuration deliberately has no
`_PERIODIC` declaration. `Ok(run)` means diagnostic evidence was returned: inspect
its invocation outcomes, original hook errors, safety stop and real EOF before
using any result. `model_proof()` remains false in every case.

## Complete Rust example

```rust
//! Compose a supplied pure expression, without configured-hook or model proof.
use dive_check::CheckOptions;
use dive_liveness::{periodic_hook, periodic_lifecycle::Options, phase_graph::Limits};
use std::sync::Arc;
fn main() -> Result<(), String> {
    let source = "---- MODULE HookGuide ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x>=0)\nHook == FALSE\n====\n";
    let analysis =
        dive_sema::analyze_source("HookGuide.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|e| e.to_string())?);
    let def = dive_ir::DefId(
        u32::try_from(
            ir.defs()
                .iter()
                .position(|d| d.name == "Hook")
                .ok_or("Hook definition")?,
        )
        .map_err(|e| e.to_string())?,
    );
    // This config deliberately contains no _PERIODIC: configured hook binding
    // remains refused. The expression is supplied separately, with its scope.
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .ok_or("config")?;
    let model =
        dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).map_err(|e| e.to_string())?;
    let pred = dive_model::Pred::new(
        model.ir.def(def).body,
        model.evaluator.scope(def).map_err(|e| e.to_string())?,
    );
    let options = CheckOptions {
        workers: 4,
        ..CheckOptions::tlc(0, 0)
    };
    let run = periodic_hook::execute(
        &model,
        &pred,
        &options,
        Limits::default(),
        Options {
            progress_interval: 0,
            ratio_limit: 0.0,
            ..Options::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let call = run.invocations().first().ok_or("actual invocation")?;
    let hook = run
        .hook_evaluations()
        .first()
        .ok_or("actual expression call")?;
    if !*hook.result().as_ref().map_err(ToString::to_string)?
        || call.resume_requested()
        || run.safety_eof().is_some()
        || run.model_proof()
    {
        return Err("expected false hook diagnostic stop".into());
    }
    println!(
        "actual_pool={} false_equality=true resume=false eof=false configured_hook=false model_proof=false",
        call.workers()
    );
    Ok(())
}
```

## Scope and outcomes

The producer is the existing bounded TLC-profile phase-L lifecycle. The default
profile retains its zero-work named refusal. A temporal OOS is currently needed;
safety-only models are not silently routed through a fabricated temporal graph.
The supplied predicate must belong to the exact admitted model IR/name table and
evaluator scope. State references refuse even in unreachable branches such as
`FALSE /\ x=0`, because this slice does not establish their empty-state contract.

Admission accepts stored scalar values, finite explicit set/tuple/record syntax,
boolean combinations and a small TLC integer arithmetic whitelist. It
inspects known nonrecursive user definitions and actual value parameters without
evaluation. Quantifiers, general function constructors, powersets, recursion,
configured operator replacements, opaque/higher-order operator captures, host effects,
runtime-name creation and state/action/temporal references refuse. Unknown leaves
cannot pass through an empty child list. Providers remain refused even if their
materialization cache was filled; stored sets/functions must carry the run's
order, with tuple domains' existing exemption. Startup callbacks may already have
occurred before this API receives a bound model; it cannot undo them. Do not
evaluate this model or evaluator-sharing clones concurrently with execution.

One callback-free admission uses the existing 50,000-unit/128-depth context
ceilings. Each visit and inspected text byte consumes a unit before text scans
or equality. Before each evaluation and receipt, the checked cubic weight of
these expanded inspection units, plus one receipt unit, is reserved against the
existing cumulative `graph.steps` quota. Overflow or insufficient remaining
quota stops before hook evaluation and gives no EOF or absence-of-error credit.
Repeated calls share that quota with graph and partial-SCC work.

The cubic weight is a conservative policy charge, not an independently proved
evaluator-complexity upper bound, exact instruction count or aggregate byte/heap
bound. Nested tuple/set/record construction, sorting and equality can consume
more work than linear structural visits. No default resource limit is raised.
Already-proved pure native operators can remain in unused captured slots; the
hook grammar refuses every application through an operator parameter.

TLC evaluates its configured hook with two empty states and Clear, then calls
`FALSE.equals(value)`. The native supplied-expression call retains the same value
comparison in the admitted TLC profile:

| Value/result | Diagnostic result |
|---|---|
| TRUE or an untyped model atom | Resume; no model-proof credit |
| FALSE | Stop with `PeriodicAssumptionFalse`, TLC counterpart2104 |
| Integer, string or stored non-boolean composite | Original comparison error1000 |
| Division by zero | Original evaluation error2179 |
| Earlier liveness candidate | Skip expression; force remains pending |

`Run::hook_evaluations()` exposes actual source/expression identities and the
original typed evaluation/comparison result. `PeriodicEvaluation` retains the
source-bearing error; it is not a model verdict. False/error stops do not request
resume, do not produce safety EOF and do not run final checking. Successful calls
can resume actual dispatch and eventually reach the separate actual EOF checks.
The four width-specific deciding candidates are reconstructed from actual graph
receipts and replayed with a fresh independent word interpreter and in-memory/JSON
validator. Wrong property, Init, concrete closing Next and malformed ending are
separate negative controls. The original draft closing-index mutant was a valid
stuttering behavior; its failed expectation is retained, and this valid behavior
stays a positive control. Candidates still require fresh independent word/in-memory/JSON replay; a hook's
TRUE result never certifies the model or the schedule.

## Ordinary CLI remains refused

For a model that actually configures the hook:

```tla
Hook == FALSE
```

```text
_PERIODIC Hook
```

`dive check Model.tla --compat=tlc` retains the named `_PERIODIC` binding refusal.
There is no CLI switch that turns this supplied-predicate diagnostic into ordinary
hook binding or strict temporal scheduling. Host access, checkpoint transactions,
TLC asynchronous queue timing and deciding stop-count equivalence remain pending.

## Reference and developer evidence

The [complete59 primary controls](../conformance/probes/periodic-hook/README.md)
use the actual pinned `ModelChecker.doPeriodicWork` and real configured expression;
only queue/liveness/checkpoint callback outcomes are supplied. They retain all
return/error/setup cases, including state/action/temporal, effects, wrong name,
wrong arity and constant-role ClassCastException. Those unsupported observations
are not claimed native matches. See [ADR0054](decisions/0054-bounded-pure-periodic-hook.md),
[the lifecycle guide](native-periodic-lifecycle.md) and
[the parity plan](design/tlc-parity-plan.md). F-023 remains missing/uncertified.
