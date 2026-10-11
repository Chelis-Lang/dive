# Inspect terminal composition in Rust

The bounded Rust periodic diagnostic now distinguishes an independently replayed
request witness, an independently replayed EOF witness, completed final graph
selections, an ordered source stop, a postcondition failure and an incomplete
transaction. It does **not** expose ordinary periodic TLC checking or a Holds
certificate. `model_proof()` remains false in every case.

Run the complete [Cargo example](../crates/dive-api/examples/terminal_classification.rs):

```sh
cargo run -p dive-api --example terminal_classification --locked
```

The example calls `periodic_candidate::check_optional_stopped` with TLC profile,
four requested workers, fingerprint polynomial zero, seed zero, existing capture,
Phase-L and graph limits, and an internally owned request timer with a 3600-second
interval and no initial force. The original configuration has no `_PERIODIC`;
absence skips that hook rather than inserting a synthetic TRUE predicate.

The first two models share `Init == x=0`, `Next == x'=1` and `Post == x=x`. With
`Live == <>(x=2)`, the actual EOF selection yields a word that violates Live;
both fresh memory and lossless JSON replays must accept it. The deciding temporal
candidate skips the normal success-path postcondition call. TLC also calls posts
while rendering the counterexample and ignores returned evaluation errors; the
corresponding nondetermining 2405 is retained separately in
`counterexample_postcondition_failure()`. With `Live == <>(x=0)`, final
selection passes, then Post fails with TLC code 2405: TLC evaluates postconditions
without a state, so the reference to x is undefined. Inspect `terminal()`,
`witness()`, `run().lifecycle().finalization()` and `postcondition_failure()`.
The original counters, workers and deciding class are retained separately.
A third model uses `Live == <>(x=0)` and `Post == TRUE \/ (x=x)`: its
state-level post short-circuits successfully after all final selections. The
terminal classification is FinalSelectionsComplete and model_proof is still
false, rather than Holds.

`FinalSelectionsComplete` means the actual retained safety graph was admitted,
every compiled OOS/PEM final selection passed and postconditions passed or were
absent. It does not prove ordinary timer, queue, stop-count or checkpoint
equivalence, nor temporal Holds. A missing witness alone establishes none of
those facts. `Incomplete` preserves an earlier request, input, resource or
execution stop. An actual stopped-worker transaction is distinct from EOF.

## Supported and refused scope

The producer admits its actual graph and statistics before the EOF callback:
rows and original concrete successors match, roots/targets are valid, each
completed source has its implicit stutter and the actual queue counter is zero.
Phase-L additionally checks its actual written records, product indices,
tableau done-fingerprints, per-node write coverage and complete source commits
matched against private original source-id/fingerprint stamps under the shared graph
inspection quota before final SCC selection. Supplied graphs or copied counters
cannot construct these private receipts. Deferred posts additionally require
the actual opaque admission identity and evaluator core; a separately admitted
public Model clone does not share the token even when its evaluator is shared. Editing an inspectable clone does not
recertify it.

Deferred postconditions currently admit direct Boolean/scalar literals, unprimed
variables, equality/inequality and Boolean conditional/junction syntax. The
specific recorded constant-role error 2233 is deferred without evaluating a
body and is reported through the existing GENERAL 1000 classification only after
final selection passes. If an actual candidate encounters that getter error,
the native path retains a named render refusal and admits no witness: the pin
exits 1 without returning from finalCheck. Request candidates with deferred posts
also remain refused until partial rendering authority is proved. Calls, configured constants, captures, quantifiers, constructors,
native operators (including TLCGet), effects and unknown metadata refuse. Missing
or malformed roles remain separately refused. No-action/no-temporal-OOS models
remain outside this diagnostic path; empty Init with an actual Next is separate.

The direct-post admission policy charges 50,000 visits/text units and depth 128.
Existing capture representative/generated/edge/scratch ceilings, Session value
limits and cumulative product/graph/SCC budgets remain unchanged. These are
entry/work policies, not aggregate heap, byte or instruction guarantees. The
ordinary engine may insert a deciding state before capture admission rejects it;
its stopping distinct count must not be mistaken for a complete retained graph.

Only TLC-profile production is admitted here. Default-profile independent replay
is a separate validator contract. Checkpoint, ordinary clock/queue equivalence,
Java after-error graph writes, no-action special exits and ordinary verdict
certification remain unfinished. See [the acceptance evidence](../conformance/probes/terminal-outcome/README.md)
and [ADR 0080](decisions/0080-owned-terminal-finalization.md).

## Complete runnable Rust example

```rust
//! Source-owned graph finalization and final-before-post diagnostic outcomes.
use dive_api::periodic_candidate::{self, Terminal};
use dive_liveness::{periodic_lifecycle, phase_graph};
use std::{sync::Arc, time::Duration};
fn main() -> Result<(), String> {
    for (live, post, expected) in [
        ("<>(x=2)", "x=x", Terminal::EofWitness),
        (
            "<>(x=0)",
            "x=x",
            Terminal::PostconditionFailure { code: Some(2405) },
        ),
        (
            "<>(x=0)",
            "TRUE \\/ (x=x)",
            Terminal::FinalSelectionsComplete,
        ),
    ] {
        let source = format!(
            "---- MODULE FinalGuide ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=1\nLive == {live}\nPost == {post}\n====\n"
        );
        let a =
            dive_sema::analyze_source("FinalGuide.tla", &source, &dive_sema::Options::default());
        let ir = Arc::new(dive_ir::lower(&a).map_err(|e| e.to_string())?);
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\nPOSTCONDITION Post\n")
            .into_config()
            .ok_or("configuration")?;
        let r = periodic_candidate::check_optional_stopped(
            ir,
            cfg,
            dive_check::CheckOptions {
                workers: 4,
                ..dive_check::CheckOptions::tlc(0, 0)
            },
            phase_graph::Limits::default(),
            periodic_lifecycle::Options::default(),
            dive_check::progress::Options {
                interval: Duration::from_secs(3600),
                force: false,
            },
        )
        .map_err(|e| e.to_string())?;
        if r.terminal() != &expected || r.model_proof() {
            return Err("terminal qualification".into());
        }
        let lifecycle = r.run().lifecycle();
        let f = lifecycle
            .finalization()
            .ok_or("no actual finalized graph")?;
        if let Some(w) = r.witness()
            && (w.memory_validation().validation != dive_trace::Validation::Valid
                || w.json_validation().validation != dive_trace::Validation::Valid)
        {
            return Err("independent witness replay".into());
        }
        println!(
            "terminal={:?} graph_states={} final_checks={}/{} post_passed={} model_proof={}",
            r.terminal(),
            f.capture().entries().0,
            f.checks().0,
            f.checks().1,
            lifecycle.postconditions_passed(),
            r.model_proof()
        );
    }
    Ok(())
}
```
