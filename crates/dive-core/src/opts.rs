//! Strategy switches for ablation (`09-performance-method.md` §5).
//!
//! Every switch turns one strategy off (`-name`) or on (`+name`) and must
//! leave every observable result unchanged. They are parsed from a list
//! such as `-fold,-delta-hash` (`DIVE_CORE_OPT`).

/// Which strategies are on. The default has all of them on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "one independent switch per strategy"
)]
pub struct Opts {
    /// S1, S2: incremental state and `EXCEPT` hashes from cached node
    /// hashes. Off (`-delta-hash`): every successor's key is recomputed from
    /// the leaves of every variable, and `EXCEPT` re-sums every entry.
    pub delta_hash: bool,
    /// S3: probe the seen set before materializing. Off (`-early-probe`):
    /// every successor is interned first.
    pub early_probe: bool,
    /// S4, S5: hash-consing and equality by handle. Off (`-hashcons`).
    pub hashcons: bool,
    /// S5: symbolic membership in `a..b`, `[D -> R]`, `SUBSET S`, record
    /// sets and `Seq(S)`. Off (`-symbolic-member`): the set is materialized
    /// (up to 2^24 elements; beyond that the run is refused).
    pub symbolic_member: bool,
    /// S6: keyed result caches. Off (`-memo`; `06-memoization.md` §6).
    pub memo: bool,
    /// S6: keyed result caches for the cached bodies of step predicates
    /// (implied actions, action constraints), keyed on the primed and
    /// unprimed variables they read. Off (`-step-memo`).
    pub step_memo: bool,
    /// S6: parent-delta skip of invariant and constraint conjuncts. Off
    /// (`-parent-skip`).
    pub parent_skip: bool,
    /// S7: the pooled arena. Off (`-arena`): one heap allocation per
    /// transient value.
    pub arena: bool,
    /// S8: arguments substituted or evaluated eagerly where the demand-order
    /// rule allows. Off (`-eager`): every argument is a thunk.
    pub eager: bool,
    /// S8: constant folding of closed constant-level subexpressions. Off
    /// (`-fold`).
    pub fold: bool,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            delta_hash: true,
            early_probe: true,
            hashcons: true,
            symbolic_member: true,
            memo: true,
            step_memo: true,
            parent_skip: true,
            arena: true,
            eager: true,
            fold: true,
        }
    }
}

/// Every switch name, as `DIVE_CORE_OPT` spells it.
pub const SWITCHES: &[&str] = &[
    "delta-hash",
    "early-probe",
    "hashcons",
    "symbolic-member",
    "memo",
    "step-memo",
    "parent-skip",
    "arena",
    "eager",
    "fold",
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
                "memo" => &mut o.memo,
                "step-memo" => &mut o.step_memo,
                "parent-skip" => &mut o.parent_skip,
                "arena" => &mut o.arena,
                "eager" => &mut o.eager,
                "fold" => &mut o.fold,
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
            ("memo", self.memo),
            ("step-memo", self.step_memo),
            ("parent-skip", self.parent_skip),
            ("arena", self.arena),
            ("eager", self.eager),
            ("fold", self.fold),
        ];
        let d = Self::default();
        let defaults = [
            d.delta_hash,
            d.early_probe,
            d.hashcons,
            d.symbolic_member,
            d.memo,
            d.step_memo,
            d.parent_skip,
            d.arena,
            d.eager,
            d.fold,
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
