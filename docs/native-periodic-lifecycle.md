# Native clock and dispatch lifecycle diagnostics

`dive_liveness::periodic_lifecycle::execute` connects real native pool dispatch
pauses, internally sampled wall-clock polling, source-owned graph growth and
actual EOF final SCC selection. It returns diagnostic evidence. It does not
enable ordinary temporal checking in `dive check --compat=tlc`, and no outcome
means that the model's temporal properties hold.

Run the complete Cargo-owned example:

```sh
cargo run -p dive-api --example periodic_lifecycle --locked
```

Its pure model remains at `x=0` while `Live` requires eventually reaching `x=1`.
The internally sampled timer checks every source commit (`progress_interval=0`),
with one initial forced request. It stops at an unvalidated candidate and prints:

```text
actual_pool=4 timer_invocations=1 eof=false candidate_requires_replay=true model_proof=false
```

The [complete Rust example](../crates/dive-api/examples/periodic_lifecycle.rs)
binds and executes the model. For an already bound model the central call is:

```rust
use dive_check::CheckOptions;
use dive_liveness::{periodic_lifecycle::{self, Options}, phase_graph::Limits};
let search = CheckOptions { workers: 4, ..CheckOptions::tlc(0, 0) };
let run = periodic_lifecycle::execute(&model, &search, Limits::default(), Options::default());
assert!(!run.model_proof());
```

Binding remains a separate caller operation. This endpoint cannot undo startup
callbacks the caller already allowed during binding. The bound model/roles must
pass current ownership/scoped purity admission; unknown captures and effects
remain explicit refusals. It accepts no caller graph, clock hook, growth count,
completed marker or claimed validation flag.

The default interval is 3000 milliseconds, runtime-ratio limit 0.2 and growth
threshold 0.1. A zero interval requests polling at each source commit; negative
intervals refuse before execution. The initial `force` request is cleared only
after the real controller's successful liveness callback. `backend.force=true`
is refused: use the lifecycle request rather than the old backend's force at
every checkpoint. TLC-profile producer execution is currently required; the default profile yields a named zero-work Phase-L refusal. Fresh default-profile independent validator replay is separately exercised on selected numeric witnesses and does not enable default producer execution. Default/Final backend schedules retain their original strict
growth/force branches. No default or compatibility-profile behavior changes.

Native timer polling occurs after each complete logical source, while its
fixed-wave generation/check jobs have actually joined. The borrowed pause token
is supplied under the real pool mutex and cannot be copied, constructed or
retained. A copied SourceBarrier does not grant pause authority. Dispatch and
nested pauses refuse inside that callback; unwinding releases authority. Existing
observers keep their old route. Workers do not independently dequeue states from
TLC's disk queue: this is native dispatch quiescence, not a proof of Java's
asynchronous progress-thread/queue suspension or timing equivalence.

`run.invocations()` retains actual source/counters, pool width/job generation,
internally sampled UNIX milliseconds, runtime ratio, forced-request state and
controller outcome. `resume_requested()` records reaching the resume hook; the
actual mutex pause ends when the callback returns. Candidate/error stops admit
no subsequent source dispatch. Logical stopping counters exclude speculative
later sources already expanded within the at-most-eight-source wave.

A timer candidate retains diagnostic `DIVE-L0054`, requiring fresh independent
word, memory and JSON replay before any future violation report. A missing timer
candidate never implies Holds. At genuine successful safety EOF, after the pool
closes, final SCC/witness checks execute in OOS/PEM order even if no timer ran.
`run.safety_eof()` records that actual safety fact separately from final checks.
It remains present if a final diagnostic fails; no complete graph is promoted by
that failure. Source, Init, assumption, safety, worker and resource stops cannot
synthesize EOF from a zero queue counter. Final candidates also remain unvalidated.

Existing graph-step limits are shared by graph, pointer projection, timer
receipt, SCC and witness work. Session value/predicate ceilings remain separate
and unchanged. Metadata is reserved before projection/retention. Clock range or
backwards elapsed samples produce typed execution failures; errors/bounds never
become completed exploration. These limits are entry/work ceilings, not total
heap/byte budgets. `backend()` retains actual options, bounds, error and worker
facts.

`_PERIODIC` and checkpoint transactions remain named unsupported paths; their
hooks cannot return a successful placeholder. Ordinary scheduling/stop-count
matching, Java queue EOF/races, mode-specific runtime error replay and deciding
witness certification remain pending. The 96 selected real Java queue controls
and separate controller/ratio controls are narrow evidence, not a whole-model
or ordinary-schedule proof. The ordinary CLI remains refused.

See [ADR0052](decisions/0052-native-periodic-lifecycle.md),
[the complete source controls](../conformance/probes/native-periodic-lifecycle/README.md),
[explicit backend checkpoints](periodic-backend.md) and
[remaining parity work](design/tlc-parity-plan.md).

The full Cargo-owned example:

```rust
use dive_check::CheckOptions;
use dive_liveness::{
    periodic_lifecycle::{self, Options},
    phase_graph::Limits,
};
use std::sync::Arc;
fn main() -> Result<(), String> {
    let source = "---- MODULE Backend ----\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x=1)\n====\n";
    let analysis = dive_sema::analyze_source("Backend.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).map_err(|e| e.to_string())?);
    let config = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .ok_or("configuration")?;
    let options = CheckOptions {
        workers: 4,
        ..CheckOptions::tlc(0, 0)
    };
    let model = dive_model::bind(
        ir,
        &config,
        &dive_model::BindOptions {
            eval: options.eval,
            no_deadlock: options.no_deadlock,
            host: None,
        },
    )
    .map_err(|e| e.to_string())?;
    let run = periodic_lifecycle::execute(
        &model,
        &options,
        Limits::default(),
        Options {
            progress_interval: 0,
            force: true,
            ..Options::default()
        },
    );
    let invocation = run
        .invocations()
        .last()
        .ok_or("no actual timer invocation")?;
    if run.backend().stop().is_none() || run.safety_eof().is_some() || run.model_proof() {
        return Err("expected a stopped diagnostic candidate".into());
    }
    println!(
        "actual_pool={} timer_invocations={} eof=false candidate_requires_replay=true model_proof=false",
        invocation.workers(),
        run.invocations().len()
    );
    Ok(())
}
```

Do not evaluate the supplied model or clones sharing its evaluator concurrently
with this diagnostic. Its bound evaluator carries mutable run state.

EOF replay controls reconstruct each candidate from the actual committed concrete
representatives at 1, 3, 4 and 8 workers. Fresh TLC and default-profile validators
check its temporal word and its in-memory and JSON trace independently. Wrong Init,
a concrete closing edge for which Next is false, a wrong property claim, and a
malformed ending remain separate negative controls. These selected replay results
do not certify ordinary scheduling or automatically validate production candidates.
