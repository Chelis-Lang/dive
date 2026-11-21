//! The run budget during temporal analysis.
//!
//! Graph capture polls the budget through the shared safety engine. After
//! capture, the analysis thread polls it between units of labelling,
//! tableau expansion and product construction. Those stages report an
//! exhausted budget as their own named limit; [`crate::analyze`] then turns
//! any failure into [`crate::analyze::AnalysisError::Stopped`] when the budget
//! is the cause, so a stop is never a refusal, a holding result or a
//! violation. Polling is one thread-local read and one relaxed atomic load.

use std::cell::RefCell;

use dive_runtime::budget::Budget;

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

/// Each optional temporal buffer (WF columns or repeated product rows)
/// can retain up to one 256th of the requested memory budget.
pub(crate) fn optional_buffer_limit() -> usize {
    const MAX: usize = 16 * 1024 * 1024;
    BUDGET.with(|budget| {
        budget.borrow().memory.map_or(MAX, |bytes| {
            usize::try_from(bytes / 256).unwrap_or(MAX).min(MAX)
        })
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "an armed, expired test budget")]
pub(crate) fn expired_for_test() -> dive_runtime::budget::Watchdog {
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

#[cfg(test)]
mod reuse_limit_tests {
    #[test]
    fn optional_columns_use_requested_memory_fraction_and_restore_scope() {
        let original = super::optional_buffer_limit();
        for (memory, expected) in [
            (None, 16 * 1024 * 1024),
            (Some(0), 0),
            (Some(256 * 1024), 1024),
            (Some(16 * 1024 * 1024 * 1024), 16 * 1024 * 1024),
        ] {
            let budget = dive_runtime::budget::Budget::new(None, memory);
            let _scope = super::enter(&budget);
            assert_eq!(super::optional_buffer_limit(), expected);
        }
        assert_eq!(super::optional_buffer_limit(), original);
    }
}
