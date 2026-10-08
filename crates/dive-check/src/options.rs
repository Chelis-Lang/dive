//! Checker options (ADR 0010: every option declares its TLC setting).

use dive_eval::{Options, Profile};
use dive_store::StoreMode;

/// How to run a check.
#[derive(Clone, Debug)]
pub struct CheckOptions {
    /// The profile: dive's default or `--compat=tlc`.
    pub profile: Profile,
    /// Evaluation options (`Options::tlc()` under `--compat=tlc`).
    pub eval: Options,
    /// Fingerprint mode (the default, as in TLC) or exact mode
    /// (`--store exact`, D-007). TLC setting: fingerprint.
    pub store: StoreMode,
    /// TLC's fingerprint polynomial index (`-fp`), `0..131`. TLC setting:
    /// random per run unless given; the default profile fixes it at 0.
    pub fp_index: usize,
    /// Whether to key the fingerprint set on TLC's own state fingerprints in
    /// TLC's variable order (`StateFingerprinter::tlc`), or on dive's cheaper
    /// two-level fingerprints (`StateFingerprinter::new`: cached FP64 value
    /// fingerprints combined by a non-linear, position-keyed mixer chain,
    /// ADR 0015 §5). TLC setting: `true`, which `--compat=tlc` uses so that
    /// the fingerprint set and the "actual" collision figure are TLC's; the
    /// default profile uses dive's. Result-preserving: only a collision could
    /// tell them apart.
    pub tlc_fingerprints: bool,
    /// The run's seed (always reported, D-002).
    pub seed: i64,
    /// TLC's `-deadlock` flag: do not check deadlock, whatever the
    /// configuration says.
    pub no_deadlock: bool,
    /// Workers requested. The engine is sequential until the parallel
    /// engine (WP-D3) lands; it runs every level inline and says so.
    pub workers: usize,
}

impl CheckOptions {
    /// `--compat=tlc` with `-fp index` and `-seed seed`.
    #[must_use]
    pub const fn tlc(fp_index: usize, seed: i64) -> Self {
        Self {
            profile: Profile::Tlc,
            eval: Options::tlc(),
            store: StoreMode::Fingerprint,
            fp_index,
            tlc_fingerprints: true,
            seed,
            no_deadlock: false,
            workers: 1,
        }
    }

    /// dive's default profile with `seed`.
    #[must_use]
    pub const fn dive(seed: i64) -> Self {
        Self {
            profile: Profile::Dive,
            eval: Options::dive(),
            store: StoreMode::Fingerprint,
            fp_index: 0,
            tlc_fingerprints: false,
            seed,
            no_deadlock: false,
            workers: 1,
        }
    }
}
