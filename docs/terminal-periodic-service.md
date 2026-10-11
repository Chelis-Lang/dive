# Exhausted-queue terminal diagnostics in Rust

`periodic_candidate::check_optional_terminal` owns the original configuration,
actual completed-source graph and terminal queue facts. It services genuine
frontier exhaustion before timer/pool joins, then performs bounded final
selection. Every deciding word is separately rebound/re-homed/replayed in memory
and lossless JSON. This is diagnostic: no result asserts temporal Holds, ordinary
TLC scheduling/count equality or model completion. Ordinary `dive check` retains
its unsupported route for this scheduling contract.

Run the workspace-owned example:

```sh
cargo run -p dive-api --example terminal_periodic_candidate --locked
```

The checkout uses its pinned Rust toolchain and workspace dependencies. Native
execution needs no Java. Exact example source:

```rust
//! Admit only an actual producer-selected word; never an ordinary model proof.
use dive_api::periodic_candidate;
use dive_check::{CheckOptions, progress};
use dive_liveness::{periodic_lifecycle, phase_graph};
use std::{sync::Arc, time::Duration};
fn main() -> Result<(), String> {
    let source = "---- MODULE TerminalGuide ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == x'=IF x=0 THEN 1 ELSE 1\nLive == <>(x=2)\n====\n";
    let analysis =
        dive_sema::analyze_source("TerminalGuide.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|e| e.to_string())?);
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .ok_or("configuration")?;
    let report = periodic_candidate::check_optional_terminal(
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
    if report.run().configured_name().is_some()
        || report.run().definition().is_some()
        || report.run().expression().is_some()
    {
        return Err("original configuration must remain hook-free".into());
    }
    let terminal = report
        .run()
        .lifecycle()
        .terminal_service()
        .ok_or("no actual terminal service")?;
    if !terminal.joins_confirmed() || terminal.stats().left_on_queue != 0 {
        return Err("missing post-service timer/pool joins".into());
    }
    let witness = report.witness().ok_or("no admitted word")?;
    if !matches!(witness.origin(), periodic_candidate::Origin::Eof(_)) || report.model_proof() {
        return Err("EOF witness qualification".into());
    }
    println!(
        "origin=eof memory={:?} json={:?} model_proof={}",
        witness.memory_validation().validation,
        witness.json_validation().validation,
        report.model_proof()
    );
    Ok(())
}
```

The API is additive; `check`, `check_optional`, mandatory snapshot APIs and
ordinary checking retain their existing contracts. `Ok(report)` means a diagnostic
report returned. Inspect actual backend stopping error, terminal receipt, final
selection, witness and both validation outcomes. No witness is never Holds.

A finished queue differs from a dispatch pause. Pinned TLC attempts periodic work
before consulting done, but `suspendAll` returns false after queue finish. The
configured hook is then skipped, even if its body is FALSE or would fail. For
`Init == x=0`, `Next == x'=1`, `Live == <>(x=0)`, the full primary succeeds even
with `_PERIODIC Hook` and `Hook == FALSE`. A real earlier request on a nonfinished
queue still evaluates that hook and stops before EOF. A zero counter or an empty
request alone cannot choose the terminal path. Any actual pending request is
retained as unserviced provenance; EOF never invents a request identity.

`terminal_service()` reports actual initialization/exhaustion counters, graph
source, pool width/generation, real pending request, internally sampled time,
IEEE ratio/force, whether suspension was attempted and the actual controller
outcome. `joins_confirmed` comes only from the matching successful postjoin
callback. False does not assert a leak; native teardown still joins on failure.
Complete controller service does not imply temporal completion. Final selected
words retain `Origin::Eof`; earlier request candidates retain `Origin::Request`.

Admission requires matching TLC evaluator/checker options, fresh source/context
ownership and bounded callback-free startup/hook grammar. Default profile,
opaque/effectful/unproved state/action hooks, unknown callbacks and checkpoint
remain named refused. Existing graph/material/partial/witness quotas are unchanged
and cumulative; inspection/policy charges are not aggregate bytes or exact
evaluator complexity bounds. Every bound or deciding safety/worker/hook error
prevents a terminal/final success claim. Fresh replay can also explicitly refuse.

The [complete reference evidence](../conformance/probes/terminal-periodic-service/README.md)
keeps natural full runs separate from controlled reflection observation. The
original misleading `<>(x=1)` fixture is retained and independently validated:
stuttering at initial x=0 is legal without fairness. Direct Init method errors
and actual outer deciding errors remain distinct. See [ADR0076](decisions/0076-exhausted-queue-terminal-service.md)
and the [temporal guide](user/temporal.md). Ordinary asynchronous clock/queue,
terminal error/worker-alive behavior and stopping-count composition remain open;
this increment grants no family certification or broad corpus credit.

The same increment repairs request-lane setup/join error statistics: initial queue
and seen-set facts are saved before the set is moved into pool ownership; a
postcallback timer-join failure keeps the actual returned source-prefix counters.
The source-owned terminal callback's typed failure likewise preserves its real
completed counters and suppresses final selection. No injected timer panic is
credited as a primary reference match; invalid-interval and scoped unwind tests
verify the actual setup and teardown authorities.


### Distinct partial-stop and full-CLI observations (#222)

The separately controlled 12-row queue study holds a genuinely dequeued state:
physical queue size 0 is not finished while a Worker owns outstanding work.
Actual suspension succeeds, partial checking precedes the hook, and FALSE or
division stops without final selection; a deciding temporal2116 precedes FALSE.
These bounded scheduler observations are separate from the natural full CLI.
The void `ModelChecker.modelCheck` observer can return with reflected errorCode 0
and observer process 0; neither is a temporal success or the inner periodic code.

The separate unmodified full CLI FALSE-hook counter exits 10, with 3,913,590
states generated/distinct and one queued state after 4.2456s; it prints no success
and no 2104 message. The declared 512MiB heap and 30s observer bound remain recorded.
This is an intentional-boundary observation, not a newly declared TLC bug. A
native early hook/service stop remains incomplete or refused and cannot be Holds.
All original selections, cleanup events and raw command/timing facts are retained
in [the evidence](../conformance/probes/terminal-periodic-service/README.md).


### Qualified delivery evidence

The closed source epoch passes all 47 local gate stages and 1,078 tests with no
skips. Its complete 70-row default-library selection retains early stops and
preexecution refusals. Twelve actual EOF words pass two fresh independent routes;
48 deliberate mutants fail both. The executable Cargo example and required 37
safety models pass at 1, 4 and 8 workers. These are bounded diagnostic and safety
regression controls, not an ordinary scheduling or temporal-absence certificate.
The [delivery index](../conformance/probes/terminal-periodic-service/delivery-index.json)
identifies the compiled source, closed raw payloads and immutable binaries.
