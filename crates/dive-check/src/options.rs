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
    /// Worker threads (`--workers`). Results are identical at every count
    /// (ADR 0015). TLC setting: 1, TLC's default; a spec that depends on
    /// TLC's sequential side effects runs with one logical worker whatever
    /// this says (sequential mode, ADR 0015 §6), and the result says so.
    /// Final/short graph capture expands at most
    /// [`crate::capture::MAX_EXPANSION_UNITS`] source units concurrently even
    /// with a wider pool; its scratch admission bound is reported separately.
    pub workers: usize,
    /// Run every level inline on one thread, as sequential mode does, even
    /// when the specification does not need it. For tests and diagnosis
    /// (`--sequential`); results are identical either way.
    pub sequential: bool,
    /// Test-only chaos (`--chaos SEED`, hidden): randomize block sizes, the
    /// order in which workers claim blocks and shards, the split of narrow
    /// levels, whether small phases run inline, and insert random yields.
    /// Results must not change; any change is a determinism bug.
    pub chaos: Option<u64>,
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
            sequential: false,
            chaos: None,
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
            sequential: false,
            chaos: None,
        }
    }
}
