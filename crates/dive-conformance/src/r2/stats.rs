//! The arithmetic of the R2 procedure (`docs/developer/performance.md`):
//! medians that may hold lower bounds, the re-run bands, size classes,
//! repetition order, geometric means and the canary rule.
//!
//! ```
//! use dive_conformance::r2::stats::{Ratio, median_bound, needs_pairs};
//!
//! let m = median_bound(&[Ratio::exact(0.5), Ratio::exact(0.7), Ratio::exact(0.6)]);
//! assert_eq!(m, Some(Ratio::exact(0.6)));
//! assert!(needs_pairs(Ratio::exact(1.05)));
//! assert!(!needs_pairs(Ratio::exact(0.5)));
//! ```

use serde::{Deserialize, Serialize};

/// A model whose TLC time is below this is in the small class.
pub const SMALL_BELOW_MS: u64 = 10_000;

/// Repetitions of the small class.
pub const SMALL_REPETITIONS: usize = 6;

/// Fresh TLC/Core pairs of a large model whose first pass is banded.
pub const BAND_PAIRS: usize = 3;

/// The closed ratio bands whose large-model first pass is re-run.
pub const BANDS: [(f64, f64); 2] = [(0.9, 1.1), (1.8, 2.0)];

/// The ratio that excluded rows count as in the fixed-set geomean, and the
/// least that a budget stop counts as.
pub const FIXED_SET_PENALTY: f64 = 2.0;

/// A canary whose median differs from its cached time by more than this
/// fraction invalidates the cache.
pub const CANARY_MEDIAN_TOLERANCE: f64 = 0.05;

/// The canaries' mean signed difference beyond which the cache is invalid.
pub const CANARY_MEAN_TOLERANCE: f64 = 0.02;

/// Canary models (the shortest large models) and TLC runs of each.
pub const CANARIES: usize = 3;

/// TLC runs of each canary.
pub const CANARY_RUNS: usize = 3;

/// A Core/TLC ratio, or a lower bound on one when Core stopped at its budget.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Ratio {
    /// The ratio, or its lower bound.
    pub value: f64,
    /// Core stopped at its budget: the true ratio is at least `value`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lower_bound: bool,
}

impl Ratio {
    /// An exact ratio.
    #[must_use]
    pub const fn exact(value: f64) -> Self {
        Self {
            value,
            lower_bound: false,
        }
    }

    /// A lower bound.
    #[must_use]
    pub const fn at_least(value: f64) -> Self {
        Self {
            value,
            lower_bound: true,
        }
    }

    /// `core_ms / tlc_ms` (TLC at least 1 ms).
    #[must_use]
    pub fn of(core_ms: u64, tlc_ms: u64, lower_bound: bool) -> Self {
        #[allow(
            clippy::cast_precision_loss,
            reason = "millisecond wall times are far below 2^52"
        )]
        let value = core_ms as f64 / tlc_ms.max(1) as f64;
        Self { value, lower_bound }
    }
}

/// The median of `values` (the mean of the middle two for an even count).
#[must_use]
pub fn median(values: &[f64]) -> Option<f64> {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => Some(v[n / 2]),
        _ => Some(f64::midpoint(v[n / 2 - 1], v[n / 2])),
    }
}

/// The median of ratios some of which may be lower bounds. The result is a
/// lower bound unless every lower bound sorts above the middle: a lower
/// bound at or below it could hide a larger true value that moves the
/// median.
#[must_use]
pub fn median_bound(values: &[Ratio]) -> Option<Ratio> {
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.value.total_cmp(&b.value));
    let n = v.len();
    if n == 0 {
        return None;
    }
    let upper = n / 2;
    let value = if n % 2 == 1 {
        v[upper].value
    } else {
        f64::midpoint(v[upper - 1].value, v[upper].value)
    };
    let lower_bound = v[..=upper].iter().any(|r| r.lower_bound);
    Some(Ratio { value, lower_bound })
}

/// Whether `ratio` lies in one of the closed [`BANDS`].
#[must_use]
pub fn in_band(ratio: f64) -> bool {
    BANDS.iter().any(|&(lo, hi)| (lo..=hi).contains(&ratio))
}

/// Whether a large model's first-pass result gets [`BAND_PAIRS`] re-run
/// pairs: a budget stop, or a ratio in a band.
#[must_use]
pub fn needs_pairs(first: Ratio) -> bool {
    first.lower_bound || in_band(first.value)
}

/// The size class of a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SizeClass {
    /// TLC under [`SMALL_BELOW_MS`]: six repetitions.
    Small,
    /// TLC [`SMALL_BELOW_MS`] or more (or stopped by the classification
    /// pass's timeout): one pass and band re-runs.
    Large,
}

impl SizeClass {
    /// `small` or `large`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Large => "large",
        }
    }
}

/// The size class from the classification pass's TLC run: a completed
/// agreeing time, or `None` when TLC hit the classification timeout.
#[must_use]
pub fn size_class(tlc_ms: Option<u64>) -> SizeClass {
    match tlc_ms {
        Some(ms) if ms < SMALL_BELOW_MS => SizeClass::Small,
        _ => SizeClass::Large,
    }
}

/// Which checker a pass times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Side {
    /// Pinned TLC.
    Tlc,
    /// dive's Core engine.
    Core,
}

/// The order of repetition or pair `index` (from 0): TLC first, reversed in
/// alternate ones.
#[must_use]
pub const fn order(index: usize) -> [Side; 2] {
    if index.is_multiple_of(2) {
        [Side::Tlc, Side::Core]
    } else {
        [Side::Core, Side::Tlc]
    }
}

/// Splits `(model, estimated ms)` into `n` shards by longest processing
/// time first: each model, longest first (ties by name), goes to the shard
/// with the least load so far (ties to the lowest index).
#[must_use]
pub fn lpt_shards(models: &[(String, u64)], n: usize) -> Vec<Vec<String>> {
    let mut sorted = models.to_vec();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut shards = vec![Vec::new(); n.max(1)];
    let mut load = vec![0_u64; n.max(1)];
    for (m, ms) in sorted {
        let i = (0..load.len()).min_by_key(|&i| (load[i], i)).unwrap_or(0);
        load[i] = load[i].saturating_add(ms);
        shards[i].push(m);
    }
    for s in &mut shards {
        s.sort();
    }
    shards
}

/// The geometric mean of positive values.
#[must_use]
pub fn geomean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "the model count is far below 2^52"
    )]
    let n = values.len() as f64;
    Some(
        (values
            .iter()
            .map(|v| v.max(f64::MIN_POSITIVE).ln())
            .sum::<f64>()
            / n)
            .exp(),
    )
}

/// A model's value in the fixed-set geomean: its ratio when it agreed, at
/// least [`FIXED_SET_PENALTY`] when it stopped, and the penalty when it was
/// excluded.
#[must_use]
pub fn fixed_set_value(ratio: Option<Ratio>) -> f64 {
    match ratio {
        Some(r) if r.lower_bound => r.value.max(FIXED_SET_PENALTY),
        Some(r) => r.value,
        None => FIXED_SET_PENALTY,
    }
}

/// One canary's re-timing against its cached time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Canary {
    /// Corpus id.
    pub corpus: String,
    /// Model id.
    pub model: String,
    /// The cached TLC milliseconds.
    pub cached_ms: u64,
    /// The fresh TLC milliseconds, in run order.
    pub runs_ms: Vec<u64>,
    /// The median of `runs_ms`.
    pub median_ms: f64,
    /// `(median - cached) / cached`.
    pub difference: f64,
}

/// The canary rule's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanaryVerdict {
    /// Every canary.
    pub canaries: Vec<Canary>,
    /// The mean of the signed differences.
    pub mean_difference: f64,
    /// Whether the cache stays valid.
    pub valid: bool,
    /// Why not, when invalid.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
}

/// Applies the 5 % / 2 % rule to `(corpus, model, cached ms, fresh runs)`.
/// Fewer than [`CANARIES`] canaries, or a canary without a fresh time,
/// invalidate the cache.
#[must_use]
pub fn canary_verdict(runs: Vec<(String, String, u64, Vec<u64>)>) -> CanaryVerdict {
    let mut reasons = Vec::new();
    if runs.len() < CANARIES {
        reasons.push(format!(
            "{} canary model(s) with a cached time; the rule needs {CANARIES}",
            runs.len()
        ));
    }
    let mut canaries = Vec::new();
    for (corpus, model, cached_ms, runs_ms) in runs {
        #[allow(
            clippy::cast_precision_loss,
            reason = "millisecond wall times are far below 2^52"
        )]
        let fresh: Vec<f64> = runs_ms.iter().map(|&ms| ms as f64).collect();
        let Some(median_ms) = median(&fresh) else {
            reasons.push(format!("{model}: no completed canary run"));
            continue;
        };
        if runs_ms.len() < CANARY_RUNS {
            reasons.push(format!(
                "{model}: {} of {CANARY_RUNS} canary runs completed",
                runs_ms.len()
            ));
        }
        #[allow(
            clippy::cast_precision_loss,
            reason = "millisecond wall times are far below 2^52"
        )]
        let difference = (median_ms - cached_ms as f64) / cached_ms.max(1) as f64;
        if difference.abs() > CANARY_MEDIAN_TOLERANCE {
            reasons.push(format!(
                "{model}: median {median_ms:.0} ms differs from the cached {cached_ms} ms by {:+.1} % (limit 5 %)",
                difference * 100.0
            ));
        }
        canaries.push(Canary {
            corpus,
            model,
            cached_ms,
            runs_ms,
            median_ms,
            difference,
        });
    }
    #[allow(clippy::cast_precision_loss, reason = "the canary count is tiny")]
    let mean_difference = if canaries.is_empty() {
        0.0
    } else {
        canaries.iter().map(|c| c.difference).sum::<f64>() / canaries.len() as f64
    };
    if mean_difference.abs() > CANARY_MEAN_TOLERANCE {
        reasons.push(format!(
            "mean signed difference {:+.1} % (limit 2 %)",
            mean_difference * 100.0
        ));
    }
    CanaryVerdict {
        canaries,
        mean_difference,
        valid: reasons.is_empty(),
        reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medians_of_odd_even_and_empty_sets() {
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[0.4, 0.1, 0.3, 0.2, 0.6, 0.5]), Some(0.35));
    }

    #[test]
    fn a_median_is_a_lower_bound_only_when_a_bound_can_move_it() {
        let m = |v: &[Ratio]| median_bound(v);
        assert_eq!(m(&[]), None);
        // The stop sorts above the median: the median is exact.
        assert_eq!(
            m(&[Ratio::exact(1.0), Ratio::exact(1.2), Ratio::at_least(2.5)]),
            Some(Ratio::exact(1.2))
        );
        // Two stops: the median is itself a bound.
        assert_eq!(
            m(&[
                Ratio::exact(1.0),
                Ratio::at_least(2.5),
                Ratio::at_least(2.6)
            ]),
            Some(Ratio::at_least(2.5))
        );
        // A low bound could hide a larger value.
        assert_eq!(
            m(&[Ratio::at_least(0.5), Ratio::exact(1.5), Ratio::exact(2.0)]),
            Some(Ratio::at_least(1.5))
        );
        let six = [0.2, 0.25, 0.3, 0.35, 0.4, 0.45].map(Ratio::exact);
        let r = m(&six).unwrap_or(Ratio::at_least(0.0));
        assert!((r.value - 0.325).abs() < 1e-12 && !r.lower_bound, "{r:?}");
    }

    #[test]
    fn bands_are_closed_and_stops_always_rerun() {
        for r in [0.9, 1.0, 1.1, 1.8, 1.9, 2.0] {
            assert!(in_band(r), "{r}");
            assert!(needs_pairs(Ratio::exact(r)));
        }
        for r in [0.0, 0.899, 1.1001, 1.5, 1.799, 2.001, 3.0] {
            assert!(!in_band(r), "{r}");
            assert!(!needs_pairs(Ratio::exact(r)));
        }
        assert!(needs_pairs(Ratio::at_least(0.3)));
        assert!(needs_pairs(Ratio::at_least(4.0)));
    }

    #[test]
    fn size_classes_split_at_ten_seconds() {
        assert_eq!(size_class(Some(0)), SizeClass::Small);
        assert_eq!(size_class(Some(9_999)), SizeClass::Small);
        assert_eq!(size_class(Some(10_000)), SizeClass::Large);
        assert_eq!(size_class(None), SizeClass::Large, "classification timeout");
    }

    #[test]
    fn repetitions_alternate_starting_with_tlc() {
        assert_eq!(order(0), [Side::Tlc, Side::Core]);
        assert_eq!(order(1), [Side::Core, Side::Tlc]);
        assert_eq!(order(4), [Side::Tlc, Side::Core]);
        assert_eq!(order(5), [Side::Core, Side::Tlc]);
    }

    #[test]
    fn geomeans_and_the_fixed_set() {
        assert_eq!(geomean(&[]), None);
        let g = geomean(&[0.25, 4.0]).unwrap_or_default();
        assert!((g - 1.0).abs() < 1e-12);
        assert!((fixed_set_value(Some(Ratio::exact(0.3))) - 0.3).abs() < 1e-12);
        assert!((fixed_set_value(Some(Ratio::at_least(1.2))) - 2.0).abs() < 1e-12);
        assert!((fixed_set_value(Some(Ratio::at_least(3.5))) - 3.5).abs() < 1e-12);
        assert!((fixed_set_value(None) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn lpt_shards_balance_the_longest_first() {
        let m = |v: &[(&str, u64)]| {
            v.iter()
                .map(|(a, b)| ((*a).to_owned(), *b))
                .collect::<Vec<_>>()
        };
        let shards = lpt_shards(
            &m(&[("a", 100), ("b", 60), ("c", 50), ("d", 40), ("e", 10)]),
            2,
        );
        assert_eq!(shards, [vec!["a", "d"], vec!["b", "c", "e"]]);
        assert_eq!(lpt_shards(&m(&[("a", 1)]), 3), [vec!["a"], vec![], vec![]]);
    }

    fn canary(model: &str, cached: u64, runs: &[u64]) -> (String, String, u64, Vec<u64>) {
        ("examples".into(), model.into(), cached, runs.to_vec())
    }

    #[test]
    fn canaries_apply_the_five_and_two_percent_rule() {
        let ok = canary_verdict(vec![
            canary("A", 10_000, &[10_100, 9_900, 10_050]),
            canary("B", 20_000, &[19_800, 20_300, 20_000]),
            canary("C", 30_000, &[30_200, 29_900, 30_100]),
        ]);
        assert!(ok.valid, "{:?}", ok.reasons);
        // One canary 6 % off.
        let one = canary_verdict(vec![
            canary("A", 10_000, &[10_600, 10_600, 10_600]),
            canary("B", 20_000, &[20_000; 3]),
            canary("C", 30_000, &[30_000; 3]),
        ]);
        assert!(!one.valid);
        assert!(one.reasons[0].contains("A: median"), "{:?}", one.reasons);
        // Each within 5 %, but all 3 % slower: the mean rule fires.
        let drift = canary_verdict(vec![
            canary("A", 10_000, &[10_300; 3]),
            canary("B", 20_000, &[20_600; 3]),
            canary("C", 30_000, &[30_900; 3]),
        ]);
        assert!(!drift.valid);
        assert!((drift.mean_difference - 0.03).abs() < 1e-9);
        assert!(drift.reasons.iter().any(|r| r.contains("mean signed")));
        // Too few canaries or runs.
        assert!(!canary_verdict(vec![canary("A", 10_000, &[10_000; 3])]).valid);
        let short = canary_verdict(vec![
            canary("A", 10_000, &[10_000]),
            canary("B", 20_000, &[20_000; 3]),
            canary("C", 30_000, &[]),
        ]);
        assert!(!short.valid);
        assert_eq!(short.reasons.len(), 2, "{:?}", short.reasons);
    }
}
