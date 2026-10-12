//! Growth-gated scheduling for liveness checks over a growing state graph.
//!
//! TLC's `LiveCheck.check` re-runs its liveness check only when the behavior
//! graph has grown by more than `-livenessThreshold` (default 0.1) since the
//! last check. That trigger is size-based and deterministic, so dive keeps it;
//! what grows here is the captured distinct-state count. TLC's wall-clock
//! cadence is not reproduced (D-014).

/// When unforced liveness checks run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Schedule {
    /// Check whenever [`Growth::due`] reports enough growth, and at the end.
    Default,
    /// Check only once, after exploration completes (`-lncheck final`).
    Final,
}

/// Graph size now and at the last completed check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Growth {
    /// Current captured distinct-state count.
    pub size: u64,
    /// Distinct-state count at the last completed check (initially one).
    pub last: u64,
}

impl Growth {
    /// Default growth threshold, TLC's `livenessThreshold`.
    pub const DEFAULT_THRESHOLD: f64 = 0.1;

    /// Whether `(size - last) / last > threshold`, compared in `f64` as TLC
    /// does. Zero over zero is NaN and is never due.
    ///
    /// ```
    /// use dive_liveness::periodic_backend::Growth;
    /// assert!(Growth { size: 12, last: 10 }.due(Growth::DEFAULT_THRESHOLD));
    /// assert!(!Growth { size: 11, last: 10 }.due(Growth::DEFAULT_THRESHOLD));
    /// assert!(!Growth { size: 0, last: 0 }.due(0.0));
    /// ```
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "TLC compares these counts as doubles"
    )]
    pub fn due(&self, threshold: f64) -> bool {
        (self.size as f64 - self.last as f64) / self.last as f64 > threshold
    }
}

#[cfg(test)]
mod tests {
    use super::Growth;

    #[test]
    fn growth_threshold_edges() {
        let due = |size, last, threshold| Growth { size, last }.due(threshold);
        assert!(!due(11, 10, 0.1), "strict comparison");
        assert!(due(12, 10, 0.1));
        assert!(due(1, 0, 0.1), "positive over zero is infinite");
        assert!(!due(0, 0, f64::NEG_INFINITY), "NaN is never due");
        assert!(!due(5, 10, 0.0), "shrinkage is never due");
        assert!(due(10, 10, -1.0), "negative threshold admits no growth");
        assert!(!due(u64::MAX, 1, f64::INFINITY));
        assert!(!due(2, 1, f64::NAN));
    }
}
