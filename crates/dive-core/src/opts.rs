//! Strategy switches for ablation.
//!
//! Every switch turns one strategy off (`-name`) or on (`+name`) and must
//! leave every observable result unchanged. They are parsed from a list
//! such as `-fold,-delta-hash` (`DIVE_CORE_OPT`).

/// Core strategies and optional diagnostic measurements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one independent switch per strategy"
)]
pub struct Opts {
    /// Incremental state and `EXCEPT` hashes from cached node
    /// hashes. Off (`-delta-hash`): every successor's key is recomputed from
    /// the leaves of every variable, and `EXCEPT` re-sums every entry.
    pub delta_hash: bool,
    /// Probe the seen set before materializing. Off (`-early-probe`):
    /// every successor is interned first.
    pub early_probe: bool,
    /// Hash-consing and equality by handle. Off (`-hashcons`).
    pub hashcons: bool,
    /// Symbolic membership in `a..b`, `[D -> R]`, `SUBSET S`, record
    /// sets and `Seq(S)`. Off (`-symbolic-member`): the set is materialized
    /// (up to 2^24 elements; beyond that the run is refused).
    pub symbolic_member: bool,
    /// Bounded hash indexes for repeated default normalized membership.
    /// Off (`-member-index`); TLC comparisons always use the original search.
    pub member_index: bool,
    /// Restrict finite function assignments to their proved writable coordinates.
    pub framed_function_choices: bool,
    /// Keyed result caches. Off (`-memo`).
    pub memo: bool,
    /// Keyed result caches for the cached bodies of step predicates
    /// (implied actions, action constraints), keyed on the primed and
    /// unprimed variables they read. Off (`-step-memo`).
    pub step_memo: bool,
    /// Keyed result caches for zero-arity definitions that a
    /// next-state action reads primed, keyed on the primed and unprimed
    /// variables they read, used only once those primed variables are
    /// assigned. Off (`-primed-memo`).
    pub primed_memo: bool,
    /// Parent-delta skip of invariant and constraint conjuncts. Off
    /// (`-parent-skip`).
    pub parent_skip: bool,
    /// The pooled arena. Off (`-arena`): one heap allocation per
    /// transient value.
    pub arena: bool,
    /// Arguments substituted or evaluated eagerly where the demand-order
    /// rule allows. Off (`-eager`): every argument is a thunk.
    pub eager: bool,
    /// Constant folding of closed constant-level subexpressions. Off
    /// (`-fold`).
    pub fold: bool,
    /// Collect an orbit-size histogram on complete exact safety searches (opt-in).
    pub orbit_census: bool,
    /// The byte cap of each worker's memo tables (not a switch; the engine
    /// derives it from the run's memory budget).
    pub memo_cap: usize,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            delta_hash: true,
            early_probe: true,
            hashcons: true,
            symbolic_member: true,
            member_index: true,
            framed_function_choices: true,
            memo: true,
            step_memo: true,
            primed_memo: true,
            parent_skip: true,
            arena: true,
            eager: true,
            fold: true,
            orbit_census: false,
            memo_cap: crate::memo::DEFAULT_CAP,
        }
    }
}

/// Every switch name, as `DIVE_CORE_OPT` spells it.
pub const SWITCHES: &[&str] = &[
    "delta-hash",
    "early-probe",
    "hashcons",
    "symbolic-member",
    "member-index",
    "framed-function-choices",
    "memo",
    "step-memo",
    "primed-memo",
    "parent-skip",
    "arena",
    "eager",
    "fold",
    "orbit-census",
];

impl Opts {
    /// Parses a comma-separated list of `-name` (off) and `+name` (on).
    ///
    /// ```
    /// let o = dive_core::Opts::parse("-memo,+delta-hash").unwrap();
    /// assert!(!o.memo && o.delta_hash && o.arena);
    /// assert!(dive_core::Opts::default().memo);
    /// assert!(dive_core::Opts::parse("-inline-subst").is_err());
    /// ```
    ///
    /// # Errors
    /// An unknown switch. `-inline-subst` is reported as not applicable:
    /// the core binds substitutions dynamically, never by inlining.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut o = Self::default();
        for item in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let (on, name) = match item.as_bytes().first() {
                Some(b'-') => (false, &item[1..]),
                Some(b'+') => (true, &item[1..]),
                _ => (false, item),
            };
            let flag = match name {
                "delta-hash" => &mut o.delta_hash,
                "early-probe" => &mut o.early_probe,
                "hashcons" => &mut o.hashcons,
                "symbolic-member" => &mut o.symbolic_member,
                "member-index" => &mut o.member_index,
                "framed-function-choices" => &mut o.framed_function_choices,
                "memo" => &mut o.memo,
                "step-memo" => &mut o.step_memo,
                "primed-memo" => &mut o.primed_memo,
                "parent-skip" => &mut o.parent_skip,
                "arena" => &mut o.arena,
                "eager" => &mut o.eager,
                "fold" => &mut o.fold,
                "orbit-census" => &mut o.orbit_census,
                "inline-subst" => {
                    return Err(
                        "`inline-subst` does not apply: the core binds INSTANCE substitutions dynamically (`04-plan-compiler.md` §3)".into(),
                    );
                }
                other => {
                    return Err(format!(
                        "unknown core switch `{other}` (known: {})",
                        SWITCHES.join(", ")
                    ));
                }
            };
            *flag = on;
        }
        Ok(o)
    }

    /// The switches that differ from the default, as `-name` (off) and
    /// `+name` (on) items, or `default`.
    #[must_use]
    pub fn describe(&self) -> String {
        let flags = [
            ("delta-hash", self.delta_hash),
            ("early-probe", self.early_probe),
            ("hashcons", self.hashcons),
            ("symbolic-member", self.symbolic_member),
            ("member-index", self.member_index),
            ("framed-function-choices", self.framed_function_choices),
            ("memo", self.memo),
            ("step-memo", self.step_memo),
            ("primed-memo", self.primed_memo),
            ("parent-skip", self.parent_skip),
            ("arena", self.arena),
            ("eager", self.eager),
            ("fold", self.fold),
            ("orbit-census", self.orbit_census),
        ];
        let d = Self::default();
        let defaults = [
            d.delta_hash,
            d.early_probe,
            d.hashcons,
            d.symbolic_member,
            d.member_index,
            d.memo,
            d.step_memo,
            d.primed_memo,
            d.parent_skip,
            d.arena,
            d.eager,
            d.fold,
            d.orbit_census,
        ];
        let changed: Vec<String> = flags
            .iter()
            .zip(defaults)
            .filter(|((_, on), def)| on != def)
            .map(|((n, on), _)| format!("{}{n}", if *on { '+' } else { '-' }))
            .collect();
        if changed.is_empty() {
            "default".into()
        } else {
            changed.join(",")
        }
    }
}
