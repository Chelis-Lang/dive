# Request-owned periodic service

`periodic_snapshot::Snapshot::execute_service` is a separate bounded Rust diagnostic
for issue #208. It commits the actual completed contiguous source prefix through
ordered safety checks, counters, graph publication and physical frontier append,
then services the genuine request once while holding the real dispatch pause.
Source callbacks publish graph data only in this mode. The old ordinal `execute`
and `execute_requests` APIs and their before-append receipts remain available.

No ordinary CLI route is enabled. Ordinary `_PERIODIC` binding, checkpoints,
host effects and ordinary temporal verdicts remain named refusals. Every returned
run and service has `model_proof() == false`. A completed safety exploration or
successful partial SCC check is not a temporal absence-of-error certificate.

## Run the maintained Cargo example

```sh
cargo run -p dive-api --example request_owned_service --locked
```

Expected output:

```text
distinct=1 source=none resumed=false safety_eof=false model_proof=false
```

The example deliberately services a genuine initial empty request with a FALSE
configured hook. It checks real initialization counters and the absence of source,
resume and EOF evidence. It does not manufacture source zero or a temporal record.

```rust
//! One genuine empty request with a configuration-owned hook, never a model proof.
use dive_check::{CheckOptions, progress};
use dive_liveness::{periodic_lifecycle, periodic_snapshot, phase_graph};
use std::{sync::Arc, time::Duration};
fn main() -> Result<(), String> {
    let source = "---- MODULE ServiceGuide ----\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x=0)\nHook == FALSE\n====\n";
    let analysis =
        dive_sema::analyze_source("ServiceGuide.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|e| e.to_string())?);
    let cfg = dive_config::parse(
        "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
    )
    .into_config()
    .ok_or("configuration")?;
    let snapshot = periodic_snapshot::bind(ir, &cfg).map_err(|e| e.to_string())?;
    let run = snapshot
        .execute_service(
            &CheckOptions {
                workers: 4,
                ..CheckOptions::tlc(0, 0)
            },
            phase_graph::Limits::default(),
            periodic_lifecycle::Options {
                progress_interval: i32::MAX,
                ..periodic_lifecycle::Options::default()
            },
            progress::Options {
                interval: Duration::from_secs(3600),
                force: true,
            },
        )
        .map_err(|e| e.to_string())?;
    let lifecycle = run.lifecycle();
    let [service] = lifecycle.request_services() else {
        return Err("one genuine initial request required".into());
    };
    if service.graph_source().is_some()
        || !service.committed().is_empty()
        || service.stats().distinct != 1
        || service.initial_stats().distinct != 1
        || service.resume_requested()
        || lifecycle.safety_eof().is_some()
        || lifecycle.model_proof()
        || !matches!(
            service.outcome(),
            Ok(dive_liveness::periodic::Outcome::PeriodicAssumptionFalse)
        )
    {
        return Err("actual empty-source stop contract".into());
    }
    println!("distinct=1 source=none resumed=false safety_eof=false model_proof=false");
    Ok(())
}
```

## Options and evidence

The entry requires `CheckOptions::tlc` and evaluator options exactly matching the
retained snapshot evaluator, including integer/order/comparison, warning and depth
settings. The snapshot currently binds the TLC value mode. Default profile and
mixed overrides refuse before clock/worker setup; independent default-profile
word replay is separate from producer admission. Requested expansion widths up to
the unchanged 64-worker structural ceiling are supported, with actual pool width
recorded separately from inline predicate width.

The producer timer owns request identity and timestamp. Caller clocks, booleans,
predicates or supplied graphs cannot establish a request. `progress::Options`
requires 1..=2147483647 milliseconds. Its `force` requests one initial service;
this is distinct from the lifecycle policy's liveness `force`. An admitted pending
request is not suppressed again by the old per-source `progress_interval` filter.
Neither natural timer instant nor request count is deterministic under load.

`request_services()` reports the actual request, full submitted source range,
newly committed/frontier-appended range, last real graph source discovery ID
(distinct from the transaction range, `None` before
any source), initial and current counters, real paused pool generation/width,
inline predicate width, service time, ratio/force transitions, controller outcome
and whether resume was reached. `service_evaluations()` retains the actual owned
configured body and original FALSE-equality result or typed error. Neither list
fabricates the legacy source-bound `Invocation` or hook evaluation.

An empty range is not EOF. It may service initialization registrations or a
previously committed graph without inventing another graph write. Successful
service resumes retained scratch and unclaimed jobs. FALSE hooks, candidate
selection and errors prevent resume. A deciding candidate precedes the hook.
Earlier source/predicate/safety/frontier-append failures prevent this later service.
Actual EOF requires successful safety completion and closed workers; mandatory
final SCC checks remain distinct from request-driven partial checks.

## Bounds, witnesses and remaining authority

VIEW/symmetry representatives remain named Session admission refusals; no request
service or worker evidence is produced for them. Duplicate successor occurrences
retain original source discovery IDs while committed source ranges report actual
transactions, not newly inserted state counts.

Original fixed eight-source waves, capture limits, graph entries/edges/cells/steps,
Session value visits/depth, snapshot startup inspection and hook policy charges
remain unchanged. Request metadata, service records, SCC/witness inspections and
hook reservations consume the existing shared cumulative graph work budget before
retention. These are policy/structural ceilings, not aggregate memory/byte or proved
evaluator instruction-complexity bounds. Exhaustion returns a named stop, never Holds.

Freshly reconstruct each deciding word from actual committed receipts; rebind the
original spec and independently validate memory and lossless JSON. The controls
retain wrong Init, wrong property, an actual Next-failing closing edge and a malformed
ending as separate negative cases. A valid word does not prove clock/suspension,
stop-count or ordinary schedule parity; validator resource refusal remains a refusal.

The complete controlled reference selection is 18 empty-request and 9 written-graph
observations at widths 1/4/8. Failed adapter setup, invalid tableau queries, genuine
EOF races and old native test assumptions remain preserved. These selected real
methods use explicit start/force/Next gates, not natural ordinary scheduling. Probe
cleanup after an early return is observer cleanup, not TLC's original run behavior.
Clean final-only property controls separately establish the violating and safe fixture.

Next ordinary work must compose true asynchronous request/queue timing, stop/error
and count boundaries, checkpoint authority and independently admitted deciding
witnesses before any CLI verdict is enabled. See [ADR 0065](decisions/0065-request-owned-periodic-service.md),
[reference and native evidence](../conformance/probes/request-owned-service/README.md)
and [the parity plan](design/tlc-parity-plan.md).

The candidate-before-hook regression uses a small finite `x=0 -> 1 -> 1` model,
a selected +Infinity ratio limit and an explicit test-only returned-prefix/timer
rendezvous. That scheduler is a dev feature excluded from default release builds;
it cannot certify natural timing. The original infinite experiment returned a
shared ResourceLimit after 12,337 requests. Larger finite controls selected
candidates but independent replay refused its unchanged 4,096-work/temporal-purity
bounds; those failures remain evidence, not successful validation. Current
Java-compatible zero-duration ratio arithmetic can yield NaN, suppressing later
unforced liveness checks. See [D-014](tlc-differences.md#zero-duration-ratio-edge-case-controlled-component-evidence).
