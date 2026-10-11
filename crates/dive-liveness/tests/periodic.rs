//! Actual pinned periodic callback order and fail-closed backend controls.
#![allow(clippy::expect_used, reason = "test assertions")]
use dive_liveness::periodic::{self, Error, Hooks, Outcome, Policy};

#[allow(
    clippy::struct_excessive_bools,
    reason = "lossless projection of independent pinned primary-control inputs"
)]
struct Case {
    liveness: bool,
    ratio: f64,
    growth: bool,
    force: bool,
    checkpoint: bool,
    periodic: &'static str,
    suspended: bool,
    check_code: i32,
    expected: &'static [&'static str],
}
include!("fixtures/periodic_cases.rs");

struct Backend<'a> {
    case: &'a Case,
    events: Vec<String>,
    clocks: [i64; 2],
    clock_index: usize,
    calls: usize,
    fail_at: Option<usize>,
}
impl<'a> Backend<'a> {
    fn new(case: &'a Case) -> Self {
        Self {
            case,
            events: vec!["start".into()],
            clocks: [100, 103],
            clock_index: 0,
            calls: 0,
            fail_at: None,
        }
    }
    fn call(&mut self) -> Result<(), &'static str> {
        let index = self.calls;
        self.calls += 1;
        if self.fail_at == Some(index) {
            Err("backend failure")
        } else {
            Ok(())
        }
    }
}
impl Hooks for Backend<'_> {
    type Error = &'static str;
    fn growth_due(&mut self) -> Result<bool, Self::Error> {
        self.call()?;
        self.events.push("growth".into());
        Ok(self.case.growth)
    }
    fn suspend_workers(&mut self) -> Result<bool, Self::Error> {
        self.call()?;
        self.events.push("suspend".into());
        Ok(self.case.suspended)
    }
    fn check_liveness(&mut self, forced: bool) -> Result<i32, Self::Error> {
        self.call()?;
        self.events.push(format!("check-force:{forced}"));
        Ok(self.case.check_code)
    }
    fn time_millis(&mut self) -> Result<i64, Self::Error> {
        self.call()?;
        let sample = self.clocks[self.clock_index];
        self.clock_index += 1;
        Ok(sample)
    }
    fn update_runtime_ratio(&mut self, elapsed: i64) -> Result<f64, Self::Error> {
        self.call()?;
        assert!(elapsed == 0 || elapsed == 3);
        self.events.push(if elapsed == 0 {
            "ratio-update:0".into()
        } else {
            "ratio-update:elapsed".into()
        });
        Ok(0.01)
    }
    fn periodic_is_false(&mut self) -> Result<bool, Self::Error> {
        self.call()?;
        self.events.push("periodic".into());
        Ok(self.case.periodic == "false")
    }
    fn checkpoint(&mut self) -> Result<(), Self::Error> {
        self.call()?;
        self.events.push("checkpoint".into());
        Ok(())
    }
    fn resume_workers(&mut self) -> Result<(), Self::Error> {
        self.call()?;
        self.events.push("resume".into());
        Ok(())
    }
}
fn policy(case: &Case) -> Policy {
    Policy {
        check_liveness: case.liveness,
        runtime_ratio: case.ratio,
        ratio_limit: 0.2,
        force_liveness: case.force,
        checkpoint: case.checkpoint,
        periodic: case.periodic != "none",
    }
}

#[test]
fn all_296_actual_reference_orders_codes_and_force_states_match() {
    assert_eq!(CASES.len(), 296);
    for (index, case) in CASES.iter().enumerate() {
        let mut state = policy(case);
        let mut backend = Backend::new(case);
        let outcome = periodic::run(&mut state, &mut backend).expect("callbacks");
        let code = match outcome {
            Outcome::Complete => 0,
            Outcome::LivenessStopped(code) => code,
            Outcome::PeriodicAssumptionFalse => 2104,
        };
        // Java's actual elapsed millisecond sample is intentionally abstracted,
        // but its position, nonnegative observation and every other field remain.
        backend.events.push(format!("result:{code}"));
        backend
            .events
            .push(format!("force-retained:{}", state.force_liveness));
        assert_eq!(
            backend.events, case.expected,
            "actual reference row {index}"
        );
    }
}

#[test]
fn every_backend_failure_stops_without_unrequested_work() {
    let case = Case {
        liveness: true,
        ratio: 0.0,
        growth: true,
        force: true,
        checkpoint: true,
        periodic: "true",
        suspended: true,
        check_code: 0,
        expected: &[],
    };
    let mut complete = Backend::new(&case);
    let mut state = policy(&case);
    assert_eq!(
        periodic::run(&mut state, &mut complete),
        Ok(Outcome::Complete)
    );
    assert_eq!(complete.calls, 8);
    for fail_at in 0..complete.calls {
        let mut backend = Backend::new(&case);
        backend.fail_at = Some(fail_at);
        let mut state = policy(&case);
        assert_eq!(
            periodic::run(&mut state, &mut backend),
            Err(Error::Callback("backend failure"))
        );
        assert_eq!(backend.calls, fail_at + 1);
        assert!(!backend.events.iter().any(|e| e == "resume"));
        assert_eq!(state.force_liveness, fail_at < 4);
    }
    let mut backend = Backend::new(&case);
    backend.clocks = [103, 100];
    let mut state = policy(&case);
    assert_eq!(
        periodic::run(&mut state, &mut backend),
        Err(Error::ClockWentBackwards)
    );
    assert_eq!(backend.calls, 5);
    assert!(!state.force_liveness);
    assert!(!backend.events.iter().any(|e| e == "periodic"));
}
