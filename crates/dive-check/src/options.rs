//! Checker options (ADR 0010: every option declares its TLC setting).

use dive_eval::{Options, Profile};
use dive_store::StoreMode;

/// How to run a check.
#[derive(Clone, Debug)]
pub struct CheckOptions {
    /// Opt-in transient fallible disk seen-state retention (ADR 0068).
    /// None preserves the resident reference. Disk runs use an explicitly
    /// reported inline fallback; this is not a total-memory or recovery budget.
    pub seen_disk: Option<crate::SeenOptions>,
    /// Opt-in encoded resident budget per current/next frontier queue. TLC uses
    /// its disk queue; `None` keeps the in-memory equivalence reference. This
    /// bounds frontier bytes only, not seen sets, candidates, traces or graphs.
    pub frontier_memory_bytes: Option<usize>,
    /// Parent directory for private transient frontier files; system temp by
    /// default. Files are explicitly cleaned and failures reported.
    pub frontier_directory: Option<std::path::PathBuf>,
    /// Separate retained predecessor capacity budget. `None` preserves the
    /// in-memory TLC counterpart; zero immediately spills native records.
    /// Seen sets, frontiers, candidates, graphs and reconstructed paths are
    /// outside this limit. Only ordinary safety BFS is admitted.
    pub predecessor_memory_bytes: Option<usize>,
    /// Parent of private transient predecessor directories. Requires a budget;
    /// omitted uses the system temporary directory. Not a checkpoint location.
    pub predecessor_directory: Option<std::path::PathBuf>,
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
    /// The run's time and memory limits (ADR 0107). [`crate::check`] arms an
    /// unarmed budget for its own run; a caller that runs several stages
    /// (capture, then temporal analysis) arms it once with
    /// [`crate::budget::Budget::start`] so the stages share one clock.
    /// Unlimited here; `dive_api` resolves the profile's defaults. TLC
    /// setting: no time limit, memory bounded by its heap.
    pub budget: crate::budget::Budget,
}

impl CheckOptions {
    /// `--compat=tlc` with `-fp index` and `-seed seed`.
    #[must_use]
    pub const fn tlc(fp_index: usize, seed: i64) -> Self {
        Self {
            seen_disk: None,
            frontier_memory_bytes: None,
            frontier_directory: None,
            predecessor_memory_bytes: None,
            predecessor_directory: None,
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
            budget: crate::budget::Budget::unlimited(),
        }
    }

    /// dive's default profile with `seed`.
    #[must_use]
    pub const fn dive(seed: i64) -> Self {
        Self {
            seen_disk: None,
            frontier_memory_bytes: None,
            frontier_directory: None,
            predecessor_memory_bytes: None,
            predecessor_directory: None,
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
            budget: crate::budget::Budget::unlimited(),
        }
    }
}
