# Committed source prefixes at request pauses

This separate bounded Rust diagnostic extends the [producer request lane](producer-progress-requests.md)
under issue #204. A request closes new claims, the actual admitted expansion
workers join, and their real dispatch mutex stays held. The producer publishes
only the maximal returned contiguous source prefix through the same safety,
counter, constraint, output and Phase-L path as ordinary whole-wave commits.
Later returned and unclaimed source work remains retained for resume, without
regenerating successors. It never resolves a later duplicate key first.

This is not ordinary TLC queue suspension or an ordinary temporal verdict.
`model_proof()` remains false; ordinary CLI `_PERIODIC` checking and checkpoint
transactions remain unsupported. The current producer requires the TLC value
profile and existing pure snapshot/startup admission. This changes no semantic
default, accepted ordinary CLI flag or resource policy.

## Run the maintained Cargo example

```sh
cargo run -p dive-api --example committed_prefix --locked
```

The example checks actual finite safety counters and receipt consistency:

```text
generated=4234 distinct=65 safety_eof=true model_proof=false
```

Clock timing determines the number of request pauses and published prefixes;
this example does not assert their number or require a particular pause instant.
The bounded regression campaign separately requires a genuine `RequestPrefix`
publication, rather than accepting only `OrderedWave` observations.

```rust
//! Actual ordered source publication at producer-owned request pauses.
use dive_check::{CheckOptions, progress};
use dive_liveness::{periodic_lifecycle, periodic_snapshot, phase_graph};
use std::sync::Arc;
use std::time::Duration;
fn main() -> Result<(), String> {
    let source = "---- MODULE PrefixGuide ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..8\nNext == x' \\in 0..64\nInv == x>=0\nLive == []<>(x>=0)\nHook == TRUE\n====\n";
    let analysis =
        dive_sema::analyze_source("PrefixGuide.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|e| e.to_string())?);
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n")
        .into_config().ok_or("configuration")?;
    let snapshot = periodic_snapshot::bind(ir, &cfg).map_err(|e| e.to_string())?;
    let run = snapshot
        .execute_requests(
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
                interval: Duration::from_millis(1),
                force: true,
            },
        )
        .map_err(|e| e.to_string())?;
    let lifecycle = run.lifecycle();
    let complete = lifecycle
        .backend()
        .complete()
        .ok_or("incomplete safety graph")?;
    let stats = complete.capture.stats;
    if stats.generated != 4234
        || stats.distinct != 65
        || stats.left_on_queue != 0
        || lifecycle.model_proof()
    {
        return Err("actual safety counters/nonproof contract".into());
    }
    for pause in lifecycle.expansion_pauses() {
        let receipt = pause.receipt();
        if receipt.frontier_appended() != receipt.committed()
            || receipt
                .committed()
                .any(|id| !receipt.completed().contains(&id))
            || receipt.predicate_workers() > 1
            || receipt.model_proof()
        {
            return Err("actual returned/committed/append contract".into());
        }
    }
    println!(
        "generated={} distinct={} safety_eof=true model_proof=false",
        stats.generated, stats.distinct
    );
    Ok(())
}
```

## Inspect the producer facts

`expansion_pauses()` retains actual pending request identity, submitted range,
claimed source IDs, returned source IDs, failed outputs and speculative generated
successors. `completed()` includes earlier published sources from the same wave;
returned output alone is not invariant success or a graph write.

`receipt.committed()` identifies the new contiguous source range published by
that successful request transaction. `committed_stats()` retains its actual
post-publication counters. `frontier_appended()` records source transactions
whose winning states have reached the physical next frontier; these are source
IDs, not the IDs of newly discovered states. `predicate_workers()` is zero when
no fresh-state invariant work was required, otherwise one for pause-owned inline
work. Actual expansion-pool width remains separately available from `pause.pool()`.

A controller invocation exposes `publication().publication()` as either
`RequestPrefix` or `OrderedWave`. For a request prefix,
`published_prefix()` identifies its actual range and
`prefix_predicate_workers()` identifies inline predicate width. The submitted
range may still contain unclaimed/unreturned outputs: all pool threads have
joined, but the whole submitted wave is not necessarily complete.

Source/Phase-L publication happens before physical frontier append, preserving
the established deciding-error order. `frontier_append_pending()` is therefore
true at the callback. A candidate, FALSE hook, hook error or observer failure can
stop at that stage. It does not earn a successful expansion-pause append receipt.
A callback failure after a successful prefix append preserves the actual remaining
queue `(submitted_len - committed_cursor) + next_frontier_len` and counters.
Graph writes, logical statistics and physical queue append must never be conflated.

Known expansion cuts shut down timer requests but still drain earlier unclaimed
sources to the original ordered error winner. No pending request fabricates a
hook or SCC receipt in place of that error. Failed Init creates no successful
request pause or EOF. Actual EOF retains mandatory final checking. Unwind/error
cleanup joins the timer and resumes/wakes pool threads through the existing guard.

## Limits, witnesses and remaining scope

Fixed eight-source scratch waves, shared `graph.steps` bookkeeping, original
Session value-work/depth policies and snapshot inspection/startup/hook policies
remain unchanged. Inline prefix predicates avoid reentering `Pool.run` while the
pause mutex is held. These are structural/work policy ceilings, not aggregate
heap/byte or complete evaluator complexity bounds.

A partial candidate is not a validated public violation. Reconstruct its actual
word and independently rebind/replay both memory and lossless JSON. The complete
controls include smaller valid words and wrong Init/property/closing/ending
mutants. The original 8192-binding word retains the validator's explicit
`quantifier domain materialization work` refusal; no candidate or graph result
can override that refusal. Natural timers may select either publication path;
no timing/count identity is inferred from word validity.

Effects/Print, opaque providers, unsupported capture forms and checkpoints stay
named refusals in this facade. The ordinary safety output path is shared, but
this diagnostic does not authorize side effects to demonstrate output order.
Independent memory/JSON replay validates a particular word/property, never the
ordinary schedule or absence of violations.

The remaining ordinary contract must compose asynchronous queue/clock triggering,
stop/error/count boundaries, configured hook and checkpoint effects, and complete
independent deciding witness admission. It cannot silently substitute final-only
checking or these request diagnostics. See [ADR 0062](decisions/0062-committed-periodic-prefix.md),
[complete source/reference evidence](../conformance/probes/committed-prefix/README.md)
and [the parity plan](design/tlc-parity-plan.md).
