# Inspect bounded partial temporal checks

The Rust diagnostic under #185 constructs actual ordered Phase-L records from a
bound model, then checks written product records after initial registrations
and entire source commits. Its public entry is
`dive_liveness::phase_execution::execute_partial`. The bounded delivery passed
the full 35-stage workspace gate (965 tests, zero skips), the standalone Cargo
example, and all 21 CM1 plus 16 validation-smoke models at 1/4/8 workers.
Ordinary `dive check --compat=tlc` periodic liveness remains
refused. Explicit final-only checking remains a separate mode.

A partial check can find a candidate counterexample. It cannot return `Holds`,
completed temporal exploration or a model certificate. No candidate may be used
as a violation proof until independently replayed against a freshly bound spec.
A check with no candidate says only that this invocation found none among the
written records; subsequent graph growth can reveal one. The diagnostic
continues after candidates deliberately to expose those later changes, unlike
TLC's ordinary deciding-stop path.

The producer accepts a bound `Model`, checking its IR/name/context authority
before planning and worker callbacks. Callers cannot provide graph nodes, a
done mask or a claimed validation flag to create source-produced checks. The
separate supplied-graph `check_partial` and `select_partial` entries grant only
diagnostic projection results. An unwritten record with outgoing adjacency is
rejected; incoming edges to an unfinished record remain legal and are skipped.
This also prevents a caller-invented unwritten path from shortening a witness.
For a null-tableau group, the producer registers an unfinished placeholder only
when a real stored successor slot first mentions that fingerprint. It retains
the original source index/fingerprint, actual table slot, slot-vector position
and generator ordinal (absent for implicit stutter). The original vertex and
shared work quotas are charged before retention. Placeholder adjacency remains
empty until an actual source write; final/full execution is unchanged. Registered
placeholder/product counts do not establish TLC node-pointer-table size, growth
thresholds or physical disk bytes. Those schedule authorities still need their
own source-produced evidence.

Requested/actual pool widths, original concrete graph/safety counts and logical
source prefixes remain separate from graph and SCC work. Partial projection,
retention and completed selection work share the single existing `graph.steps`
ceiling with Observer graph insertion/recursive inspections. Phase-L predicate
and value budgets remain separately bounded and unchanged. Failed selection
work can be unknown; a retained charge is not presented as its exact total.
These quotas are entry/work ceilings, not aggregate memory or byte limits.

The current scope is pure, bounded, admitted source contexts; opaque/effectful,
malformed ownership and unavailable concrete witness recovery remain named
failures. No new result-changing default, clock schedule, exact-store proof,
VIEW/symmetry recovery or host authority is introduced.

See [ADR0048](decisions/0048-bounded-partial-temporal-scc.md) and the
[complete source/capture family](../conformance/probes/tlc-partial-scc/README.md).
The next core step must integrate real progress-clock/quiescence decisions and
TLC's threshold/runtime/force/error/checkpoint order before admitting ordinary
strict temporal mode; it cannot silently substitute final-only or short lassos.

## Rust usage

Run the Cargo-owned example from the repository root:

```sh
cargo run -p dive-api --example partial_temporal_checks
```

It loads the real `PartialGrowthSafe` model, uses four workers, and asserts that
every retained check has no candidate and `model_proof() == false`. The output
prints `4 diagnostic checks; no model proof`; it does
not certify that the temporal property holds.

The complete example is compiled with the workspace and is shown here:

```rust
use std::{path::Path, sync::Arc};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = Path::new(
        "conformance/probes/tlc-partial-scc/models/PartialGrowthSafe/PartialGrowthSafe.tla",
    );
    let analysis = dive_sema::analyze_file(file, &dive_sema::Options::default())?;
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let cfg = dive_config::parse("SPECIFICATION Spec\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .ok_or("invalid configuration")?;
    let options = dive_check::CheckOptions {
        workers: 4,
        ..dive_check::CheckOptions::tlc(0, 0)
    };
    let model = dive_model::bind(
        ir,
        &cfg,
        &dive_model::BindOptions {
            eval: options.eval,
            no_deadlock: options.no_deadlock,
            host: None,
        },
    )?;
    let result = dive_liveness::phase_execution::execute_partial(
        &model,
        &options,
        dive_liveness::phase_graph::Limits::default(),
    )?;
    assert!(!result.model_proof());
    assert_eq!(result.execution().capture.workers.used, 4);
    assert!(
        result
            .checks()
            .iter()
            .all(|c| c.selection().candidate().is_none() && !c.model_proof())
    );
    assert!(result.inspections() <= result.graph_inspections());
    println!(
        "{} diagnostic checks; no model proof",
        result.checks().len()
    );
    Ok(())
}
```

The original [standalone caller](../conformance/probes/tlc-partial-scc/user-example.rs)
and its frozen Cargo execution remain historical producer evidence.

`PartialExecution::checks()` retains each original OOS/PEM, logical committed
source count, original registered product identities, actual write bits,
source-slot origins and SCC/candidate projection. `inspections()` is the charged
partial-work subtotal; `graph_inspections()` includes original insertion work
under the one graph quota. A `PartialFailure` retains previous finished checks
and the original deciding execution failure, actual statistics/worker facts and
bounded source prefix. Failed-stage work can be unknown. No callback-provided
proof bit, supplied graph or absence of an accepting component becomes `Holds`.

A candidate still needs fresh model/word validation, including its closing
edge, original property claim and a JSON replay. The API deliberately does not
package it as a certified `Violation`. All current reference prefix/witness
fields are compared only in the documented representation: Java `getPath`
exposes concrete fingerprint-only prefixes; native product identities remain
separate, never inferred from that vector. Exact temporal storage, clock timing,
quiescence, deciding-stop counters and ordinary schedule certification remain
pending under #66; this bounded diagnostic increment is tracked by #185.
