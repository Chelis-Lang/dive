//! Project a completed, exact ordinary safety proof onto a subset of the same
//! configured invariants. This is independent of row representation. A
//! relation between two programs alone never establishes a verdict.

pub(crate) mod identity;

use crate::{AnalyzeOptions, AnalyzeReport, ConfigReport, core_engine};
use dive_core::Opts;
/// Actual ordinary search counters.
pub use dive_explore::Stats as Counts;
use identity::Transitions;
use std::{path::Path, sync::Arc, time::Instant};
/// A named non-deciding admission miss.
#[derive(Debug)]
pub struct Miss(String);
impl std::fmt::Display for Miss {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for Miss {}
pub(crate) fn miss(reason: impl Into<String>) -> Miss {
    Miss(reason.into())
}

/// Deterministic state/depth limits from both the establishing and edited run.
/// The remaining policy is fixed to exact Dive-profile Core, one worker,
/// seed/fingerprint zero, no external grants and ordinary BFS.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Limits {
    /// Maximum admitted states.
    pub max_states: Option<u64>,
    /// Maximum admitted depth.
    pub max_depth: Option<u64>,
}

impl Limits {
    pub(crate) fn options(self) -> dive_runtime::CheckOptions {
        let mut options = dive_runtime::CheckOptions::dive(0);
        options.store = dive_store::StoreMode::Exact;
        options.max_states = self.max_states;
        options.max_depth = self.max_depth;
        options
    }
}

struct Bound {
    transitions: Transitions,
    invariants: Vec<String>,
}

impl Bound {
    fn subset<'a>(
        &self,
        current: &'a Self,
        previous_limits: Limits,
        current_limits: Limits,
    ) -> Result<Subset<'a>, Miss> {
        core_engine::edit_policy().map_err(miss)?;
        if previous_limits != current_limits || self.transitions != current.transitions {
            return Err(miss("transition inputs, scope or limits changed"));
        }
        let mut prior = self.invariants.iter();
        if !current
            .invariants
            .iter()
            .all(|name| prior.any(|old| old == name))
        {
            return Err(miss(
                "current invariants must be an ordered subsequence of the previous obligations",
            ));
        }
        Ok(Subset {
            invariants: &current.invariants,
        })
    }
}

/// Freshly bound ordinary safety inputs. All TLA source bytes, including
/// imports and assumptions, must match. Fixed pure state constraints and
/// unscoped constant substitutions remain part of the transition identity.
pub struct Program {
    bound: Bound,
    cfg: dive_config::ModelConfig,
    prepared: core_engine::Prepared,
}

impl Program {
    /// Analyze and bind the current closed module and configuration.
    ///
    /// # Errors
    /// Inputs outside the admitted pure safety contract return a miss.
    pub fn load(file: &Path, config: &Path) -> Result<Self, Miss> {
        let a = crate::analyze_file(file, &AnalyzeOptions::default())
            .map_err(|e| miss(e.to_string()))?;
        let cfg = crate::parse_config_file(config).map_err(|e| miss(e.to_string()))?;
        Self::from_reports(&a, &cfg)
    }
    /// Bind the immutable analysis and parsed configuration supplied by the caller.
    ///
    /// # Errors
    /// Refuses unsupported inputs and any obligation not bound as a pure ordinary invariant.
    pub fn from_reports(src: &AnalyzeReport, config: &ConfigReport) -> Result<Self, Miss> {
        if !src.accepted() || !config.accepted() {
            return Err(miss("source or configuration rejected"));
        }
        let cfg = config
            .config()
            .ok_or_else(|| miss("configuration rejected"))?;
        let analysis = src.analysis();
        let transitions = Transitions::new(analysis, cfg)?;
        let ir = Arc::new(dive_ir::lower(analysis).map_err(|e| miss(format!("lowering: {e:?}")))?);
        if let Some(reason) = core_engine::edit_input_refusal(&ir, cfg) {
            return Err(miss(reason));
        }
        if !dive_model::local_java_class_digests(&ir)
            .map_err(miss)?
            .is_empty()
        {
            return Err(miss("local native module override"));
        }
        let prepared = core_engine::prepare(
            &ir,
            cfg,
            &Limits::default().options(),
            false,
            Opts::default(),
            false,
        )
        .map_err(|r| miss(format!("current binding: {:?}", r.verdict)))?;
        let p = &prepared.plan;
        if !Arc::ptr_eq(&ir, &prepared.model.ir)
            || prepared.start.has_diagnostics()
            || p.effects.any
            || p.effects.level
            || p.view.is_some()
            || p.symmetry.is_some()
            || !p.implied_inits.is_empty()
            || !p.implied_actions.is_empty()
            || !p.action_constraints.is_empty()
            || !prepared.model.implied_temporals.is_empty()
            || !prepared.model.unsupported.is_empty()
        {
            return Err(miss(
                "requires pure ordinary safety with no external override, reduction, startup output or extra obligation",
            ));
        }
        let invariants: Vec<_> = cfg.invariants.iter().map(|n| n.text.clone()).collect();
        if p.invariants.iter().map(|i| &i.name).ne(&invariants)
            || prepared
                .model
                .invariants
                .iter()
                .map(|i| &i.name)
                .ne(&invariants)
            || p.constraints
                .iter()
                .map(|i| &i.name)
                .ne(cfg.constraints.iter().map(|n| &n.text))
        {
            return Err(miss(
                "configured names do not match the effective ordinary invariant or constraint plan",
            ));
        }
        Ok(Self {
            bound: Bound {
                transitions,
                invariants,
            },
            cfg: cfg.clone(),
            prepared,
        })
    }

    /// Prove an ordered subset relationship between these two freshly bound
    /// inputs. This establishes no exploration, holds verdict or counters.
    ///
    /// A caller with a prior receipt must independently authenticate a *complete
    /// successful ordinary safety search of this exact previous program*,
    /// build and policy and bind its deterministic limits to this call. A
    /// previous prefix, counterexample, temporal result or unchecked cache
    /// record confers no authority. Current request resource policy still applies.
    ///
    /// # Errors
    /// A different scope, transition input, limit, new or reordered obligation.
    pub fn subset<'a>(
        &self,
        current: &'a Self,
        previous_limits: Limits,
        current_limits: Limits,
    ) -> Result<Subset<'a>, Miss> {
        self.bound
            .subset(&current.bound, previous_limits, current_limits)
    }

    /// Run ordinary exact Core. Only an exhausted, successful search establishes
    /// a proof. State values and the exploration store are released on return.
    ///
    /// # Errors
    /// A stopped, failed, refuted, unsupported or non-exhausted search.
    pub fn capture(self, limits: Limits) -> Result<Complete, Miss> {
        core_engine::edit_policy().map_err(miss)?;
        let p = self.prepared;
        let options = limits.options();
        let started = Instant::now();
        let out = dive_explore::explore_with_storage(
            &p.plan,
            &p.store,
            dive_explore::Config {
                opts: Opts::default(),
                workers: 1,
                warn: options.eval.warnings,
                max_states: limits.max_states,
                max_depth: limits.max_depth,
                sample: None,
            },
            &|| false,
            None,
            None,
            &dive_explore::storage::Storage {
                exact: true,
                ..Default::default()
            },
        );
        if out.verdict != dive_explore::Verdict::Success
            || out.stats.left_on_queue != 0
            || !out.printed.is_empty()
            || !out.warnings.is_empty()
        {
            return Err(miss(format!(
                "requires completed safety holds; producer returned {:?}",
                out.verdict
            )));
        }
        let counts = out.stats;
        let search = core_engine::Search {
            ir: &p.model.ir,
            config: &self.cfg,
            options: &options,
            profile_tlc: false,
            workers: 1,
            explore_started: started,
        };
        let r = core_engine::safety_result(&search, &p.model, &p.store, &p.plan, p.start, out);
        if !matches!(r.verdict, dive_runtime::Verdict::Success) {
            return Err(miss("ordinary result did not report success"));
        }
        Ok(Complete {
            previous: self.bound,
            limits,
            counts,
        })
    }
}

/// A relation between source programs. Contains only the current inventory.
pub struct Subset<'a> {
    invariants: &'a [String],
}
impl Subset<'_> {
    /// The current, freshly bound ordinary invariant names in checker order.
    #[must_use]
    pub fn invariants(&self) -> &[String] {
        self.invariants
    }
}

/// In-process proof of the *actual* establishing successful ordinary search.
pub struct Complete {
    previous: Bound,
    limits: Limits,
    counts: Counts,
}
impl Complete {
    /// The establishing run's actual deterministic statistics.
    #[must_use]
    pub fn original_counts(&self) -> Counts {
        self.counts
    }
    /// Prove the edited inventory on the previously exhausted graph. Returns
    /// establishing counts; this operation visits no states.
    ///
    /// # Errors
    /// Any edit or policy outside the strict subset relation.
    pub fn project(&self, current: &Program, limits: Limits) -> Result<Counts, Miss> {
        self.previous.subset(&current.bound, self.limits, limits)?;
        Ok(self.counts)
    }
}

#[cfg(test)]
mod tests;
