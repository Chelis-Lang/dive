//! The run host's `TLCGet("config")` and live counters around the search,
//! shared by every engine.

use dive_eval::{Counter, RunConfig};
use dive_stdlib::host::RunHost;

use crate::CheckOptions;

/// `TLCGet("config")` (`AbstractChecker.createConfig`). The worker count is
/// the logical one, 1, by default and the requested one under
/// `--compat=tlc`, as TLC reports `-workers`; the install location
/// is dive's executable.
#[must_use]
pub fn run_config(opts: &CheckOptions, deadlock: bool) -> RunConfig {
    #[allow(clippy::cast_possible_wrap, reason = "Java's long: same bits")]
    let fingerprint = dive_value::fp64::POLYS
        .get(opts.fp_index)
        .copied()
        .unwrap_or(0) as i64;
    RunConfig {
        deadlock,
        workers: match opts.profile {
            dive_eval::Profile::Tlc => i64::try_from(opts.workers).unwrap_or(i64::MAX),
            dive_eval::Profile::Dive => 1,
        },
        seed: opts.seed,
        fingerprint,
        install: std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    }
}

/// The counters while TLC checks assumptions: no state yet, diameter 1
/// (measured on the pinned TLC, fixture `TgAssume`).
pub fn startup_counters(host: &RunHost) {
    for c in [
        Counter::Generated,
        Counter::Distinct,
        Counter::Queue,
        Counter::Initial,
    ] {
        host.set_counter(c, Some(0));
    }
    host.set_counter(Counter::Diameter, Some(1));
}

/// During the search the live counters are not provided: reading one is
/// refused rather than answered with a count TLC may not report at that
/// point.
pub fn search_counters(host: &RunHost) {
    for c in [
        Counter::Generated,
        Counter::Distinct,
        Counter::Queue,
        Counter::Initial,
        Counter::Diameter,
        Counter::DurationSecs,
    ] {
        host.set_counter(c, None);
    }
}
