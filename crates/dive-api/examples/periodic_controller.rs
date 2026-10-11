//! Exercise a periodic backend boundary without claiming a model proof.
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
