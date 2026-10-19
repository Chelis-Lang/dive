//! A resident-memory ceiling for temporal analysis.
//!
//! TLC bounds liveness checking by its JVM heap, not by fixed counts. When the
//! counts are unbounded ([`crate::analyze::Limits::unbounded`]), this ceiling
//! is the remaining stop: the analysis samples the process's resident set
//! size while it builds the tableau, labels and product, and refuses with a
//! named resource limit once the ceiling is exceeded. A refusal is never a
//! holding result. Sampling reads `/proc/self/status`; where that is not
//! available (not Linux), no sample is taken and no ceiling applies.

use std::cell::Cell;

/// Calls between two samples. Reading `/proc` costs a few microseconds; this
/// keeps sampling well below the cost of the work it guards.
const SAMPLE_EVERY: u32 = 4096;

thread_local! {
    static CEILING: Cell<Option<u64>> = const { Cell::new(None) };
    static TICKS: Cell<u32> = const { Cell::new(0) };
}

/// Restores the previous ceiling of this thread when dropped.
#[derive(Debug)]
pub(crate) struct Scope {
    previous: Option<u64>,
}

impl Drop for Scope {
    fn drop(&mut self) {
        CEILING.with(|c| c.set(self.previous));
    }
}

/// Installs `ceiling` (bytes of resident memory) for analysis work on this
/// thread until the returned scope is dropped. `None` leaves no ceiling.
pub(crate) fn enter(ceiling: Option<u64>) -> Scope {
    TICKS.with(|t| t.set(0));
    Scope {
        previous: CEILING.with(|c| c.replace(ceiling)),
    }
}

/// Counts one unit of guarded work and, every [`SAMPLE_EVERY`] units, checks
/// the resident set size against this thread's ceiling.
///
/// # Errors
/// The process's resident memory exceeds the installed ceiling.
pub(crate) fn tick() -> Result<(), ()> {
    let Some(ceiling) = CEILING.with(Cell::get) else {
        return Ok(());
    };
    let due = TICKS.with(|t| {
        let n = t.get().wrapping_add(1);
        t.set(n % SAMPLE_EVERY);
        n % SAMPLE_EVERY == 0
    });
    if due && resident_bytes().is_some_and(|rss| rss > ceiling) {
        return Err(());
    }
    Ok(())
}

/// The ceiling installed on this thread, if any.
#[cfg(test)]
fn ceiling() -> Option<u64> {
    CEILING.with(Cell::get)
}

/// The process's current resident set size, where the platform reports it.
#[must_use]
pub fn resident_bytes() -> Option<u64> {
    proc_kib("/proc/self/status", "VmRSS:")
}

/// The machine's physical memory, where the platform reports it.
#[must_use]
pub fn physical_bytes() -> Option<u64> {
    proc_kib("/proc/meminfo", "MemTotal:")
}

/// TLC's counterpart of the ceiling: the JVM's default maximum heap, a
/// quarter of physical memory. `None` where physical memory is unknown.
///
/// ```
/// if let Some(bytes) = dive_liveness::resident::default_ceiling() {
///     assert!(bytes > 0);
/// }
/// ```
#[must_use]
pub fn default_ceiling() -> Option<u64> {
    physical_bytes().map(|bytes| bytes / 4)
}

fn proc_kib(path: &str, key: &str) -> Option<u64> {
    let text = std::fs::read_to_string(path).ok()?;
    let line = text.lines().find(|line| line.starts_with(key))?;
    let kib: u64 = line
        .strip_prefix(key)?
        .trim()
        .strip_suffix("kB")?
        .trim()
        .parse()
        .ok()?;
    kib.checked_mul(1024)
}

#[cfg(test)]
mod tests {
    use super::*;

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
