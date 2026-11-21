//! Recheck temporal property edits on a complete, exact, labelled Core graph.
//!
//! The caller gets a named miss for source-row evaluation and for any
//! terminal completed-level analysis before exhaustion. A prior prefix cannot be
//! promoted to a complete temporal graph.

use crate::{
    AnalyzeOptions, AnalyzeReport, ConfigReport, PeriodicSchedule, core_engine, core_native_atoms,
    core_native_graph::{NativeGraph, NativeSink},
    core_native_values::NativeValues,
    safety_reuse::{self, Miss, identity::Identity, miss},
    temporal,
};
use dive_core::Opts;
use dive_liveness::analyze::periodic::external::GraphCheck;
use dive_runtime::{
    CheckOptions, CheckResult, Verdict,
    budget::{EvaluationControl, Phase},
    capture::{CsrCapture, CsrComplete, CsrTargetStorage},
};
use std::{path::Path, sync::Arc, time::Instant};

/// State, edge, generated-state and per-source graph retention ceilings.
pub use dive_runtime::capture::Limits as GraphLimits;

/// The fixed ordinary single-worker search policy plus graph retention ceilings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Limits {
    /// Ordinary state admission limit.
    pub max_states: Option<u64>,
    /// Ordinary depth admission limit.
    pub max_depth: Option<u64>,
    /// Maximum number of states, edges, generated and per-source successors.
    pub graph: GraphLimits,
    /// The ordinary completed-level schedule. Replay every due level in order.
    pub periodic: PeriodicSchedule,
}

impl Limits {
    fn options(self) -> CheckOptions {
        let mut options = CheckOptions::dive(0);
        options.store = dive_store::StoreMode::Exact;
        options.max_states = self.max_states;
        options.max_depth = self.max_depth;
        options
    }
}

/// Freshly lowered and compiled ordinary temporal program, without a host grant.
pub struct Program {
    identity: Arc<Identity>,
    cfg: dive_config::ModelConfig,
    prepared: core_engine::Prepared,
}

impl Program {
    /// Load the ordinary module closure and cfg from disk.
    ///
    /// # Errors
    /// Unsupported source, syntax, binding or plan returns a non-deciding miss.
    pub fn load(file: &Path, config: &Path) -> Result<Self, Miss> {
        let analysis = crate::analyze_file(file, &AnalyzeOptions::default())
            .map_err(|e| miss(e.to_string()))?;
        let config = crate::parse_config_file(config).map_err(|e| miss(e.to_string()))?;
        Self::from_reports(&analysis, &config)
    }
    /// Bind the actual current source and property configuration.
    ///
    /// # Errors
    /// Requires a pure, unreduced genuine temporal property set without
    /// additional state or action safety obligations.
    pub fn from_reports(analysis: &AnalyzeReport, config: &ConfigReport) -> Result<Self, Miss> {
        let (identity, cfg, prepared) = safety_reuse::bind(analysis, config, true)?;
        let p = &prepared.plan;
        if p.vars.is_empty()
            || p.effects.any
            || p.effects.level
            || p.symmetry.is_some()
            || p.view.is_some()
            || !p.invariants.is_empty()
            || !p.constraints.is_empty()
            || !p.action_constraints.is_empty()
            || !p.implied_actions.is_empty()
            || !p.implied_inits.is_empty()
            || prepared.model.implied_temporals.is_empty()
        {
            return Err(miss(
                "requires pure temporal properties with no safety obligations or reductions",
            ));
        }
        Ok(Self {
            identity,
            cfg,
            prepared,
        })
    }

    fn driver(&self, limits: Limits, options: &CheckOptions) -> Result<GraphCheck<'_>, Miss> {
        let model = &self.prepared.model;
        if let Some(reason) =
            crate::core_temporal::static_refusal(&model.ir, &self.cfg, options, &self.prepared)
        {
            return Err(miss(reason));
        }
        let mut config = temporal::core_file_configuration();
        config.limits.capture = limits.graph;
        let check = GraphCheck::prepare(
            model,
            options,
            config.limits,
            config.acceptance,
            config.graph_labels,
            Some(limits.periodic),
        )
        .map_err(|e| miss(format!("temporal planning: {:?}", e.failure)))?;
        if check
            .source_rows_possible()
            .map_err(|e| miss(format!("source row planning: {e:?}")))?
        {
            return Err(miss("requires evaluation during source expansion"));
        }
        Ok(check)
    }

    /// Explore all reachable states and actual labelled edges, including
    /// implicit stutters, then evaluate the original temporal properties.
    ///
    /// # Errors
    /// Incomplete searches, unsupported policies or unvalidated words return a miss.
    pub fn capture(&self, limits: Limits) -> Result<(Artifact, temporal::Report), Miss> {
        core_engine::edit_policy().map_err(miss)?;
        let options = limits.options();
        // Reject early/special analysis routes before paying for a graph.
        drop(self.driver(limits, &options)?);
        let prepared = &self.prepared;
        let model = &prepared.model;
        let started = Instant::now();
        let mut boundaries = Vec::new();
        let mut level = |level, graph: &NativeGraph, _: &mut NativeValues<'_>, stats| {
            Boundary::observe(&mut boundaries, limits.periodic, level, graph, stats)
        };
        let mut sink = NativeSink::new(
            model,
            prepared.store.clone(),
            limits.graph,
            &options,
            Some(&mut level),
        )
        .map_err(|e| miss(format!("graph admission: {e:?}")))?;
        sink.use_exact_identities();
        let out = dive_explore::explore_with_storage(
            &prepared.plan,
            &prepared.store,
            dive_explore::Config {
                opts: Opts::default(),
                workers: 1,
                warn: options.eval.warnings,
                max_states: limits.max_states,
                max_depth: limits.max_depth,
                sample: None,
            },
            &|| false,
            Some(&mut sink),
            None,
            &dive_explore::storage::Storage {
                exact: true,
                ..Default::default()
            },
        );
        if let Some(e) = sink.take_error() {
            return Err(miss(format!("graph capture: {e:?}")));
        }
        let (graph, values) = sink.finish();
        drop(values);
        if out.verdict != dive_explore::Verdict::Success
            || out.stats.left_on_queue != 0
            || !out.warnings.is_empty()
            || !out.printed.is_empty()
        {
            return Err(miss(format!(
                "ineligible temporal graph ending: {:?}",
                out.verdict
            )));
        }
        for id in 0..graph.input().retained_rows {
            if graph
                .row(id)
                .map_err(|e| miss(format!("state row: {e:?}")))?
                .iter()
                .any(|h| h.as_int().is_none() && h.as_bool().is_none())
            {
                return Err(miss("temporal reuse requires scalar integer/Boolean rows"));
            }
        }
        let search = core_engine::Search {
            ir: &model.ir,
            config: &self.cfg,
            options: &options,
            profile_tlc: false,
            workers: 1,
            explore_started: started,
        };
        let safety = core_engine::safety_result(
            &search,
            model,
            &prepared.store,
            &prepared.plan,
            prepared.start.clone(),
            out,
        );
        if !matches!(safety.verdict, Verdict::Success) {
            return Err(miss("producer did not complete safety"));
        }
        let artifact = Artifact {
            identity: self.identity.clone(),
            limits,
            graph,
            boundaries,
            safety,
        };
        let result = artifact.recheck(self, limits)?;
        Ok((artifact, result))
    }
}

/// Observed at the real producer callback. Native CSR arrays append edges,
/// original rows and representatives. Offsets of closed source rows never
/// change. Every unfinished source had an empty row at this boundary.
struct Boundary {
    level: u64,
    stats: dive_runtime::Stats,
    vertices: usize,
    closed: usize,
    edges: usize,
    roots: usize,
    retained: usize,
}

impl Boundary {
    fn observe(
        boundaries: &mut Vec<Self>,
        schedule: PeriodicSchedule,
        level: u64,
        graph: &NativeGraph,
        stats: dive_runtime::Stats,
    ) -> Result<bool, Verdict> {
        let previous = boundaries.last().map_or(0, |b| b.stats.distinct);
        if stats.left_on_queue != 0 && schedule.due(stats.distinct, previous) {
            let input = graph.input();
            boundaries
                .try_reserve(1)
                .map_err(|_| Verdict::Unsupported {
                    feature: "temporal boundary retention".into(),
                })?;
            boundaries.push(Self {
                level,
                stats,
                vertices: input.representatives.len(),
                closed: input.closed_rows,
                edges: input.targets.len(),
                roots: input.roots.len(),
                retained: input.retained_rows,
            });
        }
        Ok(false)
    }

    fn input<'a>(
        &self,
        full: &CsrTargetStorage<'a>,
        offsets: &'a mut Vec<usize>,
    ) -> Result<CsrTargetStorage<'a>, Miss> {
        let bad = || miss("invalid captured level boundary");
        let closed = self.closed.checked_add(1).ok_or_else(bad)?;
        let len = self.vertices.checked_add(1).ok_or_else(bad)?;
        let fixed = full.offsets.get(..closed).ok_or_else(bad)?;
        if closed > len || fixed.last() != Some(&self.edges) || self.retained > full.retained_rows {
            return Err(bad());
        }
        offsets.clear();
        offsets
            .try_reserve(len)
            .map_err(|_| miss("level offsets allocation"))?;
        offsets.extend_from_slice(fixed);
        offsets.resize(len, self.edges);
        Ok(CsrTargetStorage {
            offsets,
            targets: full.targets.prefix(self.edges).ok_or_else(bad)?,
            ordinals: full.ordinals.prefix(self.edges).ok_or_else(bad)?,
            actions: full.actions.prefix(self.edges).ok_or_else(bad)?,
            edge_rows: full.edge_rows.prefix(self.edges).ok_or_else(bad)?,
            representatives: full.representatives.get(..self.vertices).ok_or_else(bad)?,
            roots: full.roots.get(..self.roots).ok_or_else(bad)?,
            action_labels: full.action_labels,
            retained_rows: self.retained,
            closed_rows: self.closed,
        })
    }
}

/// Captured edges index the unchanged transition plan. A preceding property
/// edit can move Next's source location without changing that plan.
fn current_labels<'a>(
    old: &'a [String],
    model: &dive_model::Model,
) -> Result<std::borrow::Cow<'a, [String]>, Miss> {
    if model.actions.len() != old.len() {
        return Err(miss("current action inventory differs from captured graph"));
    }
    if model.actions.iter().zip(old).all(|(a, l)| a.label() == l) {
        return Ok(std::borrow::Cow::Borrowed(old));
    }
    let mut current = Vec::new();
    current
        .try_reserve_exact(old.len())
        .map_err(|_| miss("current action labels allocation"))?;
    current.extend(model.actions.iter().map(|a| a.label().to_owned()));
    Ok(std::borrow::Cow::Owned(current))
}

/// Complete graph from its exact producer. No serialization or reconstruction
/// from counts, an arbitrary caller-supplied graph, or a previous word.
pub struct Artifact {
    identity: Arc<Identity>,
    limits: Limits,
    graph: NativeGraph,
    boundaries: Vec<Boundary>,
    safety: CheckResult,
}
impl Artifact {
    /// Original establishing safety exploration, with its counts and settings.
    #[must_use]
    pub fn original_safety(&self) -> &CheckResult {
        &self.safety
    }

    /// Evaluate the current named properties on the stored labelled edges.
    /// The ordinary analyzer runs its SCC and attribution passes and rebinds
    /// any emitted word for independent validation.
    ///
    /// # Errors
    /// A changed transition input/policy, unsupported analysis or word-validation
    /// failure returns a non-deciding miss.
    pub fn recheck(&self, current: &Program, limits: Limits) -> Result<temporal::Report, Miss> {
        core_engine::edit_policy().map_err(miss)?;
        if self.limits != limits || !self.identity.compatible(&current.identity) {
            return Err(miss("transition inputs or limits changed"));
        }
        let started = Instant::now();
        let options = limits.options();
        let model = &current.prepared.model;
        let mut driver = current.driver(limits, &options)?;
        let control = EvaluationControl::new(&options.budget, Phase::Capture)
            .map_err(|e| miss(e.to_string()))?;
        let guarded_names = crate::core_temporal::fixed_name_domain(&current.prepared);
        let start_names = model.names.len();
        let mut values = NativeValues::new(model, current.prepared.store.clone(), &options.budget)
            .map_err(|e| miss(format!("graph values: {e:?}")))?;
        let mut atoms = core_native_atoms::NativeAtoms::prepare(
            model,
            &current.prepared.store,
            &driver,
            &control,
            Opts::default(),
        )
        .map_err(|e| miss(format!("current native atoms: {e:?}")))?;
        let mut reader = core_native_atoms::Reader::new(&self.graph, &mut values, &mut atoms);
        reader.allow_inline_rows();
        let input = self.graph.input();
        let labels = current_labels(input.action_labels, model)?;
        let input = CsrTargetStorage {
            action_labels: &labels,
            ..input
        };
        let mut offsets = Vec::new();
        for b in &self.boundaries {
            let prefix = b.input(&input, &mut offsets)?;
            let csr = CsrCapture::checked_targets(prefix, limits.graph, &control)
                .map_err(|e| miss(format!("captured level: {e:?}")))?;
            if !driver.native_level_due(b.stats) {
                return Err(miss(
                    "captured schedule boundary differs from current checker",
                ));
            }
            if driver.level_native_with_reader(b.level, &csr, b.stats, &mut reader) {
                return Err(miss(
                    "current temporal check stopped on an earlier completed level",
                ));
            }
        }
        let complete = CsrComplete::from_target_producer(
            input,
            self.safety.clone(),
            options.clone(),
            limits.graph,
            &control,
        )
        .map_err(|e| miss(format!("graph completion: {e:?}")))?;
        let analysis = driver
            .finish_native_with_reader(complete, &mut reader)
            .map_err(|e| miss(format!("temporal analysis: {:?}", e.failure)))?;
        let report = temporal::core_native_report(
            &model.ir,
            &current.cfg,
            &options,
            Ok(analysis),
            true,
            model.symmetry_liveness().clone(),
            started,
        )
        .map_err(|e| miss(format!("independent validation: {e:?}")))?;
        if !matches!(
            report.outcome(),
            temporal::Outcome::NativeHolds(_) | temporal::Outcome::NativeViolation(_)
        ) {
            return Err(miss("unproved final temporal result"));
        }
        if guarded_names && model.names.len() != start_names {
            return Err(miss("reference evaluation extended the proved name domain"));
        }
        if let temporal::Outcome::NativeViolation(v) = report.outcome()
            && !dive_trace::validate_doc(model, v.document(), &current.cfg.model_values)
                .0
                .validation
                .is_valid()
        {
            return Err(miss(
                "word did not validate against immutable current binding",
            ));
        }
        Ok(report.with_graph_limits(limits.graph))
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::panic,
        reason = "executable temporal agreement controls"
    )]
    use super::*;
    fn program(a: &str, b: &str, clauses: &str, cfg: &str) -> Program {
        let text = format!(
            "---- MODULE Test ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = 1\nSpec == Init /\\ [][Next]_x {clauses}\nA == {a}\nB == {b}\n====\n"
        );
        let src = crate::analyze_source("Test.tla", &text, &AnalyzeOptions::default());
        let cfg = crate::parse_config_source("Test.cfg", cfg);
        Program::from_reports(&src, &cfg).expect("bound temporal properties")
    }
    const CFG: &str = "SPECIFICATION Spec\nPROPERTY A B\n";
    #[test]
    fn eventualities_on_original_edges_and_implicit_stutters() {
        let l = Limits::default();
        let p = program("<>(x = 0)", "<>(x < 2)", "", CFG);
        let (graph, original) = p.capture(l).expect("completed graph with held properties");
        assert_eq!(
            original
                .configuration()
                .expect("effective graph limits")
                .limits
                .capture,
            l.graph
        );
        assert!(matches!(
            original.outcome(),
            temporal::Outcome::NativeHolds(_)
        ));
        let new = program("<>(x = 1)", "<>(x = 2)", "", CFG);
        let checked = graph.recheck(&new, l).expect("new word");
        let temporal::Outcome::NativeViolation(v) = checked.outcome() else {
            panic!("expected a concrete word")
        };
        assert_eq!(v.validation().validation, dive_trace::Validation::Valid);
        assert!(v.document().properties.iter().any(|name| name == "B"));
        let fair = program("<>(x = 1)", "<>(x = 2)", "/\\ WF_x(Next)", CFG);
        assert!(graph.recheck(&fair, l).is_err());
    }
    #[test]
    fn ordinary_periodic_or_incomplete_graph_never_yields_final_proof() {
        let p = program("<>(x = 0)", "<>(x < 2)", "", CFG);
        let limits = Limits::default();
        let tiny = Limits {
            periodic: PeriodicSchedule {
                min_states: 1,
                growth_factor: 2,
            },
            ..limits
        };
        let (periodic, held) = p.capture(tiny).expect("actual scheduled prefix replay");
        assert!(held.periodic().is_some_and(|checks| checks.prefixes > 0));
        let false_at_prefix = program("<>(x=4)", "<>(x=8)", "", CFG);
        assert!(
            periodic
                .recheck(&false_at_prefix, tiny)
                .is_err_and(|e| e.to_string().contains("earlier completed level"))
        );
        assert!(
            p.capture(Limits {
                max_states: Some(1),
                ..limits
            })
            .is_err()
        );
        assert!(
            p.capture(Limits {
                max_depth: Some(1),
                ..limits
            })
            .is_err()
        );
        let (g, _) = p.capture(limits).expect("baseline");
        assert!(g.recheck(&p, tiny).is_err());
        let smaller = Limits {
            graph: GraphLimits {
                edges: 1,
                ..limits.graph
            },
            ..limits
        };
        assert!(p.capture(smaller).is_err());
        assert!(g.recheck(&p, smaller).is_err());
    }

    #[test]
    fn unchanged_fairness_is_evaluated_on_every_current_word() {
        let fair = "/\\ WF_x(Next)";
        let p = program("<>(x = 0)", "<>(x < 2)", fair, CFG);
        let limits = Limits::default();
        let (graph, _) = p.capture(limits).expect("fair completed product");
        let edited = program("<>(x = 1)", "<>(x = 1)", fair, CFG);
        assert!(matches!(
            graph
                .recheck(&edited, limits)
                .expect("fair recheck")
                .outcome(),
            temporal::Outcome::NativeHolds(_)
        ));
        let edited = program("<>(x = 2)", "<>(x > 3)", fair, CFG);
        let r = graph
            .recheck(&edited, limits)
            .expect("new fair false properties");
        let temporal::Outcome::NativeViolation(v) = r.outcome() else {
            panic!("expected fair counterexample")
        };
        assert_eq!(v.document().properties, ["A", "B"]);
        assert_eq!(v.validation().validation, dive_trace::Validation::Valid);
    }
    #[test]
    fn changed_property_error_and_temporal_to_safety_role_miss() {
        let p = program("<>(x = 0)", "<>(x < 2)", "", CFG);
        let l = Limits::default();
        let (closed, _) = p.capture(l).expect("complete");
        let error = program("<>(IF x = 0 THEN 1 ELSE TRUE)", "<>(x < 2)", "", CFG);
        assert!(closed.recheck(&error, l).is_err());
        let source = "---- MODULE Test ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=1\nSpec == Init /\\ [][Next]_x\nA == [](x=0)\nB == <>(x=0)\n====\n";
        let src = crate::analyze_source("Test.tla", source, &AnalyzeOptions::default());
        let cfg = crate::parse_config_source("Test.cfg", CFG);
        if let Ok(safety) = Program::from_reports(&src, &cfg) {
            assert!(closed.recheck(&safety, l).is_err());
        }
    }
    #[test]
    fn file_supplied_native_module_cannot_establish_a_temporal_graph() {
        let src = r"---- MODULE TLC ----
EXTENDS Naturals
RandomElement(S) == 999
VARIABLE x
Q == <>(RandomElement(0..1) \in 0..1)
Draw == RandomElement(0..1)
Init == x=Draw
Next == UNCHANGED x
Spec == Init /\ [][Next]_x
P == <>(x=1)
====";
        let a = crate::analyze_source("TLC.tla", src, &AnalyzeOptions::default());
        let cfg = crate::parse_config_source("TLC.cfg", "SPECIFICATION Spec\nPROPERTY Q P\n");
        assert!(Program::from_reports(&a, &cfg).is_err_and(|e| e.to_string().contains("shadows")));
    }
    #[test]
    fn typed_word_edges_use_current_action_name_module_and_source_span() {
        fn moved(property: &str) -> Program {
            let source = format!(
                "---- MODULE Move ----\nEXTENDS Naturals\nVARIABLE x\nP == {property}\nB == <>(x<2)\nInit == x=0\nNext == x'=1\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\n====\n"
            );
            let a = crate::analyze_source("Move.tla", &source, &AnalyzeOptions::default());
            let c = crate::parse_config_source("Move.cfg", "SPECIFICATION Spec\nPROPERTY P B\n");
            Program::from_reports(&a, &c).expect("valid model")
        }
        let old = moved("<>(x=0)");
        let l = Limits::default();
        let (captured, _) = old.capture(l).expect("complete graph");
        for (edit, line) in [("<>(x=2)", 7), ("<>\n (x=2)", 8)] {
            let now = moved(edit);
            let reused = captured.recheck(&now, l).expect("current word");
            let (_, fresh) = now.capture(l).expect("current fresh graph");
            let (temporal::Outcome::NativeViolation(a), temporal::Outcome::NativeViolation(b)) =
                (reused.outcome(), fresh.outcome())
            else {
                panic!("fair word required")
            };
            assert_eq!(a.word().edges, b.word().edges);
            assert_eq!(a.document(), b.document());
            assert_eq!(
                a.word().edges[0].label.as_deref(),
                Some(
                    format!("<Next line {line}, col 9 to line {line}, col 12 of module Move>")
                        .as_str()
                )
            );
            assert_eq!(a.word().edges[0].ordinal, Some(0));
        }
    }
}
