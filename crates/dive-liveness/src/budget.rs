//! The run budget during temporal analysis (ADR 0107).
//!
//! Graph capture polls the budget through the shared safety engine. After
//! capture, the analysis thread polls it between units of labelling,
//! tableau expansion and product construction. Those stages report an
//! exhausted budget as their own named limit; [`crate::analyze`] then turns
//! any failure into [`crate::analyze::AnalysisError::Stopped`] when the budget
//! is the cause, so a stop is never a refusal, a holding result or a
//! violation. Polling is one thread-local read and one relaxed atomic load.

use std::cell::RefCell;

use dive_check::budget::Budget;

/// The named limit stages report for an exhausted budget.
pub(crate) const EXHAUSTED: &str = "run budget";

thread_local! {
    static BUDGET: RefCell<Budget> = const { RefCell::new(Budget::unlimited()) };
}

/// Restores the previous budget of this thread when dropped.
#[derive(Debug)]
pub(crate) struct Scope {
    previous: Budget,
}

impl Drop for Scope {
    fn drop(&mut self) {
        let previous = std::mem::take(&mut self.previous);
        BUDGET.with(|b| *b.borrow_mut() = previous);
    }
}

/// Installs `budget` for analysis work on this thread until the scope drops.
pub(crate) fn enter(budget: &Budget) -> Scope {
    Scope {
        previous: BUDGET.with(|b| b.replace(budget.clone())),
    }
}

/// Whether this thread's budget is exhausted.
#[inline]
pub(crate) fn exhausted() -> bool {
    BUDGET.with(|b| !matches!(b.borrow().poll(), Ok(None)))
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "an armed, expired test budget")]
pub(crate) fn expired_for_test() -> dive_check::budget::Watchdog {
    use std::time::{Duration, Instant};
    let watchdog = Budget::new(Some(Duration::ZERO), None)
        .start()
        .expect("watchdog starts")
        .expect("limited budget");
    let started = Instant::now();
    while watchdog.budget().poll().expect("measurement").is_none()
        && started.elapsed() < Duration::from_secs(10)
    {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        watchdog.budget().poll().expect("measurement").is_some(),
        "watchdog must expire"
    );
    watchdog
}
