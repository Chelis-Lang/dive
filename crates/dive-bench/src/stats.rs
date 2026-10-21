//! Current-run pairing; an incomplete population never earns an aggregate.

use std::collections::BTreeSet;

use serde::Serialize;

use super::Run;
use dive_bench::median;

#[derive(Debug, Default, Serialize)]
pub(crate) struct Pairs {
    pub(crate) median: Option<f64>,
    pub(crate) paired: usize,
    pub(crate) intended: usize,
    pub(crate) missing: usize,
    pub(crate) censored: usize,
    pub(crate) refused: usize,
    pub(crate) failed: usize,
    pub(crate) unused_workers: usize,
}

impl Pairs {
    pub(crate) fn complete(&self) -> bool {
        self.paired == self.intended && self.intended > 0
    }
}

pub(crate) fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

/// Native resource stops and external timeouts are incomplete observations.
pub(crate) fn incomplete(run: &Run) -> bool {
    matches!(run.verdict.as_str(), "timeout" | "stopped")
}

/// Duplicate keys are errors even if only one duplicate succeeded.
pub(crate) fn validate(runs: &[Run]) -> Result<(), String> {
    let mut keys = BTreeSet::new();
    for r in runs {
        if !keys.insert((&r.bench, &r.implementation, r.rep)) {
            return Err(format!(
                "ambiguous duplicate run: {} {} rep {}",
                r.bench,
                r.implementation,
                r.rep + 1
            ));
        }
        if !positive(r.wall_s) {
            return Err(format!(
                "nonpositive or nonfinite wall time: {} {}",
                r.bench, r.implementation
            ));
        }
    }
    Ok(())
}

/// Scaling allows distinct requested workers, but requires actual worker use.
pub(crate) fn pair(
    runs: &[Run],
    bench: &str,
    baseline: &str,
    candidate: &str,
    repetitions: usize,
    scaling: bool,
) -> Result<Pairs, String> {
    validate(runs)?;
    let mut p = Pairs {
        intended: repetitions,
        ..Pairs::default()
    };
    let mut ratios = Vec::new();
    for rep in 0..repetitions {
        let find = |implementation| {
            runs.iter()
                .find(|r| r.bench == bench && r.implementation == implementation && r.rep == rep)
        };
        let (Some(a), Some(b)) = (find(baseline), find(candidate)) else {
            p.missing += 1;
            continue;
        };
        if !scaling && a.workers != b.workers {
            return Err("throughput pairing requires matching requested workers".to_owned());
        }
        if incomplete(a) || incomplete(b) {
            p.censored += 1;
        } else if a.verdict == "unsupported" || b.verdict == "unsupported" {
            p.refused += 1;
        } else if !a.correct || !b.correct {
            p.failed += 1;
        } else if scaling
            && (a.used_workers != Some(a.workers) || b.used_workers != Some(b.workers))
        {
            p.unused_workers += 1;
        } else {
            let ratio = a.wall_s / b.wall_s;
            if !positive(ratio) {
                return Err("wall ratio overflowed or underflowed".to_owned());
            }
            ratios.push(ratio);
        }
    }
    p.paired = ratios.len();
    p.median = median(&ratios);
    Ok(p)
}

/// Equal model weights, and all selected models must have all planned pairs.
#[allow(
    clippy::cast_precision_loss,
    reason = "model count in a display statistic"
)]
pub(crate) fn geometric_mean(pairs: &[Pairs]) -> Option<f64> {
    if pairs.is_empty() || pairs.iter().any(|p| !p.complete()) {
        return None;
    }
    let logs: Option<Vec<f64>> = pairs
        .iter()
        .map(|p| p.median.filter(|&x| positive(x)).map(f64::ln))
        .collect();
    logs.map(|v| (v.iter().sum::<f64>() / v.len() as f64).exp())
        .filter(|&x| positive(x))
}
