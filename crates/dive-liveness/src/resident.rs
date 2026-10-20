//! A resident-memory ceiling for temporal analysis.
//!
//! TLC bounds liveness checking by its JVM heap, not by fixed counts. When the
//! counts are unbounded ([`crate::analyze::Limits::unbounded`]), this ceiling
//! is the remaining stop. Graph capture samples it per retained state and edge
//! ([`dive_check::capture::Limits::resident_bytes`]). The analysis thread
//! samples it while it plans and builds the tableau, labels each captured
//! state and builds the product. Above the ceiling each stage refuses with a
//! named resource limit; a refusal is never a holding result.
//!
//! Samples are taken between units of work, never inside one evaluation: an
//! atom evaluated for one state, or one successor computation, is not
//! interrupted. Sampling reads `/proc/self/status`; where that is not
//! available (not Linux), no sample is taken and no ceiling applies.

use std::cell::Cell;

use dive_check::resident::Sampler;
pub use dive_check::resident::{default_ceiling, physical_bytes, resident_bytes};

thread_local! {
    static SAMPLER: Cell<Sampler> = const { Cell::new(Sampler::new(None)) };
}

/// Restores the previous ceiling of this thread when dropped.
#[derive(Debug)]
pub(crate) struct Scope {
    previous: Sampler,
}

impl Drop for Scope {
    fn drop(&mut self) {
        SAMPLER.with(|c| c.set(self.previous));
    }
}

/// Installs `ceiling` (bytes of resident memory) for analysis work on this
/// thread until the returned scope is dropped. `None` leaves no ceiling.
pub(crate) fn enter(ceiling: Option<u64>) -> Scope {
    Scope {
        previous: SAMPLER.with(|c| c.replace(Sampler::new(ceiling))),
    }
}

/// Counts one unit of guarded work and, every
/// [`dive_check::resident::SAMPLE_EVERY`] units, checks the resident set size
/// against this thread's ceiling.
///
/// # Errors
/// The process's resident memory exceeds the installed ceiling.
pub(crate) fn tick() -> Result<(), ()> {
    SAMPLER.with(|c| {
        let mut sampler = c.get();
        let result = sampler.tick().map_err(|_| ());
        c.set(sampler);
        result
    })
}

/// Checks the resident set size against this thread's ceiling now, at a stage
/// boundary after bulk work that has no periodic sample of its own.
///
/// # Errors
/// The process's resident memory exceeds the installed ceiling.
pub(crate) fn check() -> Result<(), ()> {
    SAMPLER.with(|c| c.get().sample().map_err(|_| ()))
}

/// The ceiling installed on this thread, if any.
#[cfg(test)]
fn ceiling() -> Option<u64> {
    SAMPLER.with(|c| c.get().ceiling())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dive_check::resident::SAMPLE_EVERY;

    #[test]
    fn a_ceiling_below_the_current_footprint_refuses_and_scopes_restore() {
        if resident_bytes().is_none() {
            return;
        }
        assert_eq!(ceiling(), None);
        {
            let _outer = enter(Some(1));
            assert_eq!(ceiling(), Some(1));
            let refused = (0..SAMPLE_EVERY).any(|_| tick().is_err());
            assert!(
                refused,
                "one sample per {SAMPLE_EVERY} ticks must see the ceiling"
            );
            {
                let _inner = enter(None);
                assert!((0..2 * SAMPLE_EVERY).all(|_| tick().is_ok()));
            }
            assert_eq!(ceiling(), Some(1));
        }
        assert_eq!(ceiling(), None);
        assert!((0..2 * SAMPLE_EVERY).all(|_| tick().is_ok()));
    }

    #[test]
    fn a_generous_ceiling_never_refuses() {
        let _scope = enter(Some(u64::MAX));
        assert!((0..2 * SAMPLE_EVERY).all(|_| tick().is_ok()));
    }
}
