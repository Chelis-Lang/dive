//! Process resident-memory sampling for the unbounded temporal setting.
//!
//! TLC bounds liveness checking by its JVM heap, not by fixed counts. When
//! graph capture runs without fixed counts ([`crate::capture::Limits::unbounded`]),
//! a resident-memory ceiling is its remaining stop: capture samples the
//! process's resident set size while it retains states and edges, and refuses
//! with a named resource limit above the ceiling. `dive-liveness` uses the
//! same readings for the tableau, labels and product. Sampling reads
//! `/proc/self/status`; where that is not available (not Linux), no sample is
//! taken and no ceiling applies.

/// Units of guarded work between two samples. Reading `/proc` costs a few
/// microseconds; this keeps sampling well below the cost of the work it guards.
pub const SAMPLE_EVERY: u32 = 4096;

/// Counts guarded work and samples resident memory every [`SAMPLE_EVERY`]
/// units against a fixed ceiling. A sampler without a ceiling never samples.
///
/// ```
/// let mut sampler = dive_check::resident::Sampler::new(Some(u64::MAX));
/// assert!((0..10_000).all(|_| sampler.tick().is_ok()));
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct Sampler {
    ceiling: Option<u64>,
    ticks: u32,
}

impl Sampler {
    /// A sampler for `ceiling` bytes of resident memory; `None` samples nothing.
    #[must_use]
    pub const fn new(ceiling: Option<u64>) -> Self {
        Self { ceiling, ticks: 0 }
    }

    /// The ceiling this sampler enforces.
    #[must_use]
    pub fn ceiling(&self) -> Option<u64> {
        self.ceiling
    }

    /// Counts one unit of guarded work and, every [`SAMPLE_EVERY`] units,
    /// checks the process's resident set size against the ceiling.
    ///
    /// # Errors
    /// The process's resident memory exceeds the ceiling.
    pub fn tick(&mut self) -> Result<(), Exceeded> {
        let Some(ceiling) = self.ceiling else {
            return Ok(());
        };
        self.ticks = (self.ticks + 1) % SAMPLE_EVERY;
        if self.ticks == 0 && resident_bytes().is_some_and(|rss| rss > ceiling) {
            return Err(Exceeded);
        }
        Ok(())
    }
}

/// The process's resident memory exceeded a [`Sampler`]'s ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exceeded;

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
/// if let Some(bytes) = dive_check::resident::default_ceiling() {
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
    fn a_ceiling_below_the_current_footprint_refuses_within_one_period() {
        if resident_bytes().is_none() {
            return;
        }
        let mut sampler = Sampler::new(Some(1));
        assert!((0..SAMPLE_EVERY).any(|_| sampler.tick().is_err()));
        let mut none = Sampler::new(None);
        assert!((0..2 * SAMPLE_EVERY).all(|_| none.tick().is_ok()));
    }
}
