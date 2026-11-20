//! Checker options. Every option declares its TLC setting, the value
//! `--compat=tlc` selects.

use dive_eval::{Options, Profile};
use dive_store::StoreMode;

/// How to run a check.
#[derive(Clone, Debug)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent public checking options"
)]
pub struct CheckOptions {
    /// Optional owned external inputs for source binding. Runtime reads and
    /// checkpoint/recovery are not implemented; evaluator grants stay explicit.
    pub inputs: Option<std::sync::Arc<dyn dive_eval::inputs::InputSource>>,
    /// Opt-in transient fallible disk seen-state retention.
    /// `None` keeps the seen set resident in memory. Disk runs use an explicitly
    /// reported inline fallback; this is not a total-memory or recovery budget.
    pub seen_disk: Option<crate::SeenOptions>,
    /// Opt-in encoded resident budget per current/next frontier queue. TLC uses
    /// its disk queue; `None` keeps the in-memory equivalence reference. This
    /// bounds frontier bytes only, not seen sets, candidates, traces or graphs.
    pub frontier_memory_bytes: Option<usize>,
    /// Store independent DAG chunks on disk for ordinary fingerprint safety.
    /// This allows value reclamation between committed expansion batches.
    pub independent_frontier: bool,
    /// Parent directory for private transient frontier files; system temp by
    /// default. Files are explicitly cleaned and failures reported.
    pub frontier_directory: Option<std::path::PathBuf>,
    /// Encoded candidate bytes per worker under explicit Core safety BFS.
    /// None is the resident TLC-equivalent reference; zero immediately spills.
    /// Active evaluator roots, codec scratch and accepted Store are outside it.
    pub candidate_memory_bytes: Option<usize>,
    /// Parent for private transient candidate files; requires a candidate budget.
    pub candidate_directory: Option<std::path::PathBuf>,
    /// Completed-level measured Store-byte trigger for explicit pure Core safety.
    /// Zero forces each boundary; None is TLC-equivalent. This is not a hard cap.
    pub store_compact_bytes: Option<usize>,
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
    /// Whole-run symmetry policy for genuine temporal properties. TLC checks a
    /// symmetry-reduced graph, which can miss or invent liveness violations.
    /// TLC-equivalent: `Tlc`; unsupported quotient checking still refuses.
    pub symmetry_liveness: dive_model::SymmetryLiveness,
    /// Evaluation options (`Options::tlc()` under `--compat=tlc`).
    pub eval: Options,
    /// Fingerprint mode (the default, as in TLC) or exact mode
    /// (`--store exact`, full states with an unconditional exhaustiveness
    /// claim). TLC setting: fingerprint.
    pub store: StoreMode,
    /// TLC's fingerprint polynomial index (`-fp`), `0..131`. TLC setting:
    /// random per run unless given; the default profile fixes it at 0.
    pub fp_index: usize,
    /// Whether to key the fingerprint set on TLC's own state fingerprints in
    /// TLC's variable order (`StateFingerprinter::tlc`), or on dive's cheaper
    /// two-level fingerprints (`StateFingerprinter::new`: cached FP64 value
    /// fingerprints combined by a non-linear, position-keyed mixer chain).
    /// TLC setting: `true`, which `--compat=tlc` uses so that
    /// the fingerprint set and the "actual" collision figure are TLC's; the
    /// default profile uses dive's. Result-preserving: only a collision could
    /// tell them apart.
    pub tlc_fingerprints: bool,
    /// The run's seed (always reported, so `--seed` replays the run).
    pub seed: i64,
    /// TLC's `-deadlock` flag: do not check deadlock, whatever the
    /// configuration says.
    pub no_deadlock: bool,
    /// Worker threads (`--workers`). Results are identical at every count.
    /// TLC setting: 1, TLC's default; a spec that depends on
    /// TLC's sequential side effects runs with one logical worker whatever
    /// this says (sequential mode), and the result says so.
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
    /// The run's time and memory limits. A run arms an unarmed
    /// budget for itself; a caller that runs several stages
    /// (capture, then temporal analysis) arms it once with
    /// [`crate::budget::Budget::start`] so the stages share one clock.
    /// Unlimited here; `dive_api` resolves the profile's defaults. TLC
    /// setting: no time limit, memory bounded by its heap.
    pub budget: crate::budget::Budget,
    /// Maximum admitted distinct in-model identities; `None` is TLC-equivalent.
    /// Duplicate states do not consume this budget.
    pub max_states: Option<u64>,
    /// Maximum breadth-first discovery depth; initial states have depth one.
    /// `None` is TLC-equivalent; TLC's simulation `-depth` is unrelated.
    pub max_depth: Option<u64>,
}

impl CheckOptions {
    /// `--compat=tlc` with `-fp index` and `-seed seed`.
    #[must_use]
    pub const fn tlc(fp_index: usize, seed: i64) -> Self {
        Self {
            inputs: None,
            seen_disk: None,
            frontier_memory_bytes: None,
            independent_frontier: false,
            frontier_directory: None,
            candidate_memory_bytes: None,
            candidate_directory: None,
            store_compact_bytes: None,
            predecessor_memory_bytes: None,
            predecessor_directory: None,
            profile: Profile::Tlc,
            symmetry_liveness: dive_model::SymmetryLiveness::Tlc,
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
            max_states: None,
            max_depth: None,
        }
    }

    /// dive's default profile with `seed`.
    #[must_use]
    pub const fn dive(seed: i64) -> Self {
        Self {
            inputs: None,
            seen_disk: None,
            frontier_memory_bytes: None,
            independent_frontier: false,
            frontier_directory: None,
            candidate_memory_bytes: None,
            candidate_directory: None,
            store_compact_bytes: None,
            predecessor_memory_bytes: None,
            predecessor_directory: None,
            profile: Profile::Dive,
            symmetry_liveness: dive_model::SymmetryLiveness::Unreduced,
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
            max_states: None,
            max_depth: None,
        }
    }
}
