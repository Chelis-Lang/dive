# Periodic-work backend controller

`dive_liveness::periodic::run` executes the pinned TLC periodic-work callback
order for a backend that implements `Hooks`. It is a library building block.
It does not enable ordinary temporal checking in `dive check --compat=tlc`,
and a successful invocation is not a safety or liveness proof.

The caller supplies the current liveness runtime fraction, ratio limit,
outstanding force request, the checkpoint request consumed for this invocation,
and whether a configured `_PERIODIC` expression exists. Backend callbacks provide
actual graph growth, queue suspension, partial SCC checking, clock samples,
PERIODIC evaluation, checkpointing and queue resumption. They must retain their
own source, graph and failure evidence.

The order matters. A nonzero SCC message code returns before clearing the force
request, evaluating PERIODIC, checkpointing or resuming. Successful SCC checking
clears the force request before the second clock sample and ratio update. A
FALSE PERIODIC result returns code 2104 before checkpointing or resuming. A failed
queue suspension leaves the force request untouched. Checkpoint callbacks own the
resume within their transaction; the controller does not resume twice.

At a runtime ratio exactly equal to the configured limit, an unforced invocation
can suspend because of graph growth, PERIODIC or checkpointing without running
SCC checking. A forced invocation can run SCC checking even at or above that
boundary. The implementation preserves floating-point comparisons, including
NaN, rather than rounding the boundary into a different branch.

Run the checked backend example with:

```sh
cargo run -p dive-api --example periodic_controller
```

The example deliberately supplies callback outcomes. It demonstrates an exact
ratio-boundary PERIODIC failure and prints `model_proof=false`. It does not
explore a model, create a checkpoint or select a real counterexample. The complete
Rust source below is compiled and executed by the Cargo-owned example.

```rust
use dive_liveness::periodic::{self, Hooks, Outcome, Policy};

struct Backend {
    calls: Vec<&'static str>,
}
impl Hooks for Backend {
    type Error = &'static str;
    fn growth_due(&mut self) -> Result<bool, Self::Error> {
        self.calls.push("growth");
        Ok(true)
    }
    fn suspend_workers(&mut self) -> Result<bool, Self::Error> {
        self.calls.push("suspend");
        Ok(true)
    }
    fn check_liveness(&mut self, _forced: bool) -> Result<i32, Self::Error> {
        Err("an unforced check must not run at the exact ratio boundary")
    }
    fn time_millis(&mut self) -> Result<i64, Self::Error> {
        Err("no SCC time sample is expected")
    }
    fn update_runtime_ratio(&mut self, _elapsed: i64) -> Result<f64, Self::Error> {
        Err("no ratio update is expected at the exact boundary")
    }
    fn periodic_is_false(&mut self) -> Result<bool, Self::Error> {
        self.calls.push("periodic");
        Ok(true)
    }
    fn checkpoint(&mut self) -> Result<(), Self::Error> {
        Err("a FALSE PERIODIC must stop before checkpointing")
    }
    fn resume_workers(&mut self) -> Result<(), Self::Error> {
        Err("a FALSE PERIODIC must stop before resuming")
    }
}
fn main() -> Result<(), String> {
    let mut policy = Policy {
        check_liveness: true,
        runtime_ratio: 0.2,
        ratio_limit: 0.2,
        force_liveness: false,
        checkpoint: true,
        periodic: true,
    };
    let mut backend = Backend { calls: Vec::new() };
    let outcome = periodic::run(&mut policy, &mut backend).map_err(|e| format!("{e:?}"))?;
    if outcome != Outcome::PeriodicAssumptionFalse
        || backend.calls != ["growth", "suspend", "periodic"]
    {
        return Err("unexpected periodic callback order or result".into());
    }
    println!("periodic_message_code=2104 model_proof=false");
    Ok(())
}
```

The backend must implement the actual FALSE-equality comparison on the evaluated
PERIODIC value, including TLC's non-boolean comparison failures. It must also
implement the real ratio update from elapsed time; supplying a fabricated ratio
does not establish clock conformance. Typed callback failures stop the controller.
A negative elapsed liveness time is rejected explicitly, without a completion
or model verdict. The usual default profile and TLC profile do not acquire a new
schedule option from this kernel.

The [closed reference study](../conformance/probes/ordinary-periodic-work/README.md)
contains all 296 selected pinned-Java observations, failed producer attempts and
independent readback. Those observations prove callback order, deciding codes,
force retention and PERIODIC effect position under controlled callback outcomes.
Clock-ratio arithmetic, actual queue quiescence, partial-graph authority,
checkpoint persistence, worker-width invariance and ordinary temporal verdicts
remain integration obligations. Growth must match the actual node-pointer table
and its recorded check-size snapshot; neither file bytes nor product/placeholder
identity counts establish that authority. Follow [the parity plan](design/tlc-parity-plan.md).
