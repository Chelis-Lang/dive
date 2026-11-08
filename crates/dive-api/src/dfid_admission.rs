//! The DFID request, outcome and report, and its static admission (ADR 0021).
//!
//! Core's iterative deepening (`core_modes::dfid`) runs these requests. The
//! admission checks a bound model without evaluating it.

use dive_eval::next::StateVals;
use dive_model::Model;

use dive_runtime::CheckOptions;

/// A bounded DFID request. Resource limits never imply a model verdict.
#[derive(Clone, Debug)]
pub struct Options {
    /// Shared evaluation, fingerprint, deadlock, and profile settings.
    pub check: CheckOptions,
    /// Maximum iterative level, matching TLC's `-dfid N`.
    pub max_level: u32,
    /// Explicit worker seeds, one per iteration. This replay override is
    /// labelled mixed in TLC mode because TLC does not seed workers from CLI.
    pub worker_seeds: Option<Vec<i64>>,
    /// Maximum retained distinct states and candidate states per action.
    pub max_states: usize,
    /// Maximum eager/candidate states across the retained DFS frame stack.
    pub max_retained_candidates: usize,
    /// Maximum total generated states across all iterations.
    pub max_generated: u64,
    /// Maximum retained witness states.
    pub max_trace: usize,
    /// Restricts fingerprint bits for collision controls. Normal runs use 64.
    pub fingerprint_bits: u32,
}

impl Options {
    /// Shared checker settings with explicit iterative depth.
    #[must_use]
    pub const fn new(check: CheckOptions, max_level: u32) -> Self {
        Self {
            check,
            max_level,
            worker_seeds: None,
            max_states: 1_000_000,
            max_retained_candidates: 1_000_000,
            max_generated: 10_000_000,
            max_trace: 1_000_000,
            fingerprint_bits: 64,
        }
    }
}

/// A deciding result. Only `Complete` certifies full pure safety exploration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Visitation completion certificate, with all configured checks passing.
    Complete,
    /// Maximum iterative depth reached before a completion certificate.
    Incomplete,
    /// A reachable safety violation, pending independent certification.
    Violation {
        /// Invariant, property, or deadlock.
        kind: &'static str,
        /// Configured property name.
        name: Option<String>,
        /// Pinned TLC deciding code.
        code: u32,
    },
    /// A measured evaluation/configuration failure.
    Error {
        /// Pinned TLC deciding code.
        code: u32,
        /// Diagnostic detail.
        message: String,
    },
    /// Unimplemented mode interaction, explicitly named.
    Unsupported(String),
    /// Resource or collision bound reached, without a conclusion.
    Limit(String),
    /// The armed run budget stopped execution, without a conclusion.
    Cancelled,
    /// Internal invariant failure, without a conclusion.
    Internal(String),
}

/// Run evidence, including partial initialization and actual worker streams.
#[derive(Clone, Debug)]
pub struct Report {
    /// Last reached phase.
    pub stage: &'static str,
    /// Result; a violation still requires independent validation.
    pub outcome: Outcome,
    /// TLC's generated count (eager vectors included).
    pub generated: u64,
    /// Initial enumeration prefix, including failed initialization.
    pub initial_candidates: u64,
    /// Distinct constrained states retained by the visitation map.
    pub distinct: usize,
    /// Last iterative level started, or zero before any iteration.
    pub level: u32,
    /// Actual selection seed used in every started iteration.
    pub worker_seeds: Vec<i64>,
    /// Concrete witness; never reproduces TLC's stale cross-root stack bug.
    pub trace: Vec<StateVals>,
    /// Whether TLC would emit its completion message at this depth bound.
    pub tlc_completion_message: bool,
    /// First deciding failure, retained if a later resource bound interrupts draining.
    pub deciding: Option<Outcome>,
}

fn sequential_for_composition(model: &Model) -> bool {
    model
        .sequential
        .iter()
        .all(|reason| reason == dive_model::TLC_CDOT)
}

/// Checks DFID request admission without evaluating the model.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "one static preflight enumerates unsupported mode combinations"
)]
pub fn preflight(model: &Model, opts: &Options) -> Option<Outcome> {
    if opts.check.max_states.is_some() || opts.check.max_depth.is_some() {
        return Some(Outcome::Unsupported(
            "BFS discovery budgets --max-states/--max-depth with DFID are not implemented; use DFID-specific limits".into(),
        ));
    }
    if opts.check.candidate_memory_bytes.is_some()
        || opts.check.candidate_directory.is_some()
        || opts.check.store_compact_bytes.is_some()
    {
        return Some(Outcome::Unsupported(
            "candidate spill or Store reclamation with DFID is not implemented".into(),
        ));
    }
    if opts.check.seen_disk.is_some() {
        return Some(Outcome::Unsupported(
            "disk seen-state retention with DFID is not implemented".into(),
        ));
    }
    if opts.check.predecessor_memory_bytes.is_some() || opts.check.predecessor_directory.is_some() {
        return Some(Outcome::Unsupported(
            "predecessor spill with DFID is not implemented".into(),
        ));
    }
    let refusal = |s: &str| Some(Outcome::Unsupported(s.to_owned()));
    if opts.check.workers == 0 {
        return Some(Outcome::Error {
            code: 1102,
            message: "Error: at least one worker required.".to_owned(),
        });
    }
    // TLC rejects these with a general error (exit 255); dive refuses them by
    // name with the refusal exit in both profiles (D-018 in docs/divergences.md).
    if opts.check.workers != 1 {
        return refusal("DFID multiple workers");
    }
    if opts.max_level > 10_000 {
        return Some(Outcome::Limit("DFID iterative level bound".to_owned()));
    }
    if opts.fingerprint_bits == 0 || opts.fingerprint_bits > 64 {
        return refusal("DFID fingerprint bit width outside 1..64");
    }
    if !model.temporals.is_empty() || !model.implied_temporals.is_empty() {
        return refusal("DFID temporal properties and fairness");
    }
    if let Some(u) = model.unsupported.first() {
        return refusal(&u.feature);
    }
    if model.symmetry.is_some() {
        return refusal("DFID SYMMETRY");
    }
    if model.view.is_some() {
        return refusal("DFID VIEW");
    }
    if model.alias.is_some() {
        return refusal("DFID ALIAS");
    }
    if !matches!(&model.postconditions, Ok(p) if p.is_empty()) {
        return refusal("DFID POSTCONDITION");
    }
    if !model.implied_inits.is_empty() && !model.constraints.is_empty() {
        return refusal(
            "DFID initial properties with state constraints require separate witness validation contract",
        );
    }
    if !model.implied_actions.is_empty()
        && (!model.constraints.is_empty() || !model.action_constraints.is_empty())
    {
        return refusal(
            "DFID action properties with constraints require separate witness validation contract",
        );
    }
    // Runtime names need ordered execution, which this inline backend already
    // provides. Independent witness validation still refuses history it cannot
    // reconstruct; effects and live introspection remain prohibited here.
    // TLC DFID counts only emitted successors: composition accounting does not
    // introduce live state or effects and does not require a refusal.
    if !model.sequential_names_only && !sequential_for_composition(model) {
        return refusal("DFID live checker state/effect reads");
    }
    let predicates = model
        .init
        .iter()
        .map(|i| &i.pred)
        .chain(model.next.iter())
        .chain(model.invariants.iter().map(|n| &n.pred))
        .chain(model.implied_inits.iter().map(|n| &n.pred))
        .chain(model.implied_actions.iter().map(|n| &n.pred))
        .chain(model.constraints.iter().map(|n| &n.pred))
        .chain(model.action_constraints.iter().map(|n| &n.pred));
    if dive_runtime::admission::effectful(model)
        || predicates
            .into_iter()
            .any(|p| model.evaluator.scope_shares_side_effects(&p.scope))
    {
        return refusal("DFID effects or unresolved constant-operator effects");
    }
    None
}
