//! A checkable relationship between two ordinary temporal inputs. Removing
//! every claim after the first can preserve a prior complete Holds on a
//! closed, pure model even when its states contain sets, records or functions.
//! This module never issues a verdict or consumes an unauthenticated receipt.

mod binding;
mod config;
mod execution;

use crate::safety_projection::miss;
pub use crate::safety_projection::{Limits, Miss};
use crate::{
    AnalyzeOptions, AnalyzeReport, ConfigReport, core_engine,
    safety_projection::identity::{Module, closed_modules},
};
use binding::Binding;
use config::Config;
use dive_core::Opts;
use dive_liveness::analyze::periodic::{Schedule, external::GraphCheck};
use dive_runtime::budget::{EvaluationControl, Phase};
use std::{collections::HashSet, path::Path, sync::Arc};

/// A freshly bound ordinary first claim with its complete behavior and policy.
pub struct Program {
    modules: Vec<Module>,
    root: String,
    config: Config,
    claims: Vec<String>,
    limits: Limits,
    first: Binding,
    atom_count: usize,
}
fn policy(limits: Limits) -> Result<(), Miss> {
    core_engine::edit_policy().map_err(miss)?;
    if std::env::var_os("DIVE_NATIVE_ATOM_AUDIT").is_some() {
        return Err(miss("requires the ordinary native atom policy"));
    }
    state_cap(limits)
}

fn state_cap(limits: Limits) -> Result<(), Miss> {
    if !limits
        .max_states
        .is_some_and(|n| n > 0 && n < Schedule::DEFAULT.min_states)
    {
        return Err(miss(
            "requires a positive state cap below the first periodic check",
        ));
    }
    Ok(())
}

/// Check public native report fields and deterministic limits without binding
/// either program. This checks the reported worker policy independently of
/// the consuming process’s environment. Callers must authenticate the worker and
/// exact input. Only [`Program::first_claim`] checks the real first product and
/// the previous execution's atom inventory against the freshly bound programs.
///
/// # Errors
/// A missing or malformed field, non-default policy, incomplete search,
/// earlier failure, diagnostic or actual reference fallback returns a miss.
pub fn qualify_execution(report: &serde_json::Value, limits: Limits) -> Result<(), Miss> {
    state_cap(limits)?;
    execution::qualify(report, limits).map(|_| ())
}

/// Lexical ordering can still create runtime strings. Include declarations
/// because native configuration targets need not appear in a call expression.
fn creates_runtime_names(ir: &dive_ir::Ir) -> bool {
    ir.defs()
        .iter()
        .any(|d| d.std.is_some_and(dive_ir::StdOp::creates_runtime_names))
        || ir.nodes().iter().any(|n| {
            n.expr
                .op_refs()
                .iter()
                .any(|op| matches!(op, dive_ir::OpRef::Std(s) if s.creates_runtime_names()))
        })
}

impl Program {
    /// Load both the complete module closure and its configuration.
    ///
    /// # Errors
    /// Pure ordinary binding and all admission checks must succeed.
    pub fn load(file: &Path, config: &Path, limits: Limits) -> Result<Self, Miss> {
        let analysis = crate::analyze_file(file, &AnalyzeOptions::default())
            .map_err(|e| miss(e.to_string()))?;
        let cfg = crate::parse_config_file(config).map_err(|e| miss(e.to_string()))?;
        Self::from_reports(&analysis, &cfg, limits)
    }

    /// Prepare the actual Core transition plan and ordinary native temporal
    /// planning, including the full startup normalization of this inventory.
    ///
    /// # Errors
    /// Returns a non-deciding miss on rejection, unsupported input, source-row
    /// demand, host effects, native policy changes or unqualified bindings.
    pub fn from_reports(
        src: &AnalyzeReport,
        cfg: &ConfigReport,
        limits: Limits,
    ) -> Result<Self, Miss> {
        policy(limits)?;
        if !src.accepted() || !cfg.parse().diagnostics().is_empty() {
            return Err(miss("source or configuration diagnostics"));
        }
        let c = cfg.config().ok_or_else(|| miss("configuration rejected"))?;
        let config = Config::read(cfg, c)?;
        let a = src.analysis();
        let modules = closed_modules(a)?;
        let ir = Arc::new(dive_ir::lower(a).map_err(|e| miss(format!("lowering: {e:?}")))?);
        let root = ir.module(ir.root()).name.clone();
        if let Some(reason) = core_engine::edit_input_refusal(&ir, c) {
            return Err(miss(reason));
        }
        if !dive_model::local_java_class_digests(&ir)
            .map_err(miss)?
            .is_empty()
        {
            return Err(miss("local native module override"));
        }
        if creates_runtime_names(&ir) {
            return Err(miss("native operation can create runtime names"));
        }
        let options = limits.options();
        let opt = Opts::default();
        let p = core_engine::prepare(&ir, c, &options, false, opt, true)
            .map_err(|r| miss(format!("current binding: {:?}", r.verdict)))?;
        let m = &p.model;
        if !Arc::ptr_eq(&ir, &m.ir)
            || p.start.has_diagnostics()
            || !m.sequential.is_empty()
            || m.runtime_name_order
            || m.implied_temporals.is_empty()
            || m.symmetry.is_some()
            || m.view.is_some()
            || p.plan.effects.any
            || p.plan.effects.level
        {
            return Err(miss(
                "requires pure native temporal behavior without startup diagnostics or reductions",
            ));
        }
        let claims = c
            .properties
            .iter()
            .map(|n| n.text.clone())
            .collect::<Vec<_>>();
        if m.property_claims().iter().map(|n| &n.name).ne(&claims)
            || claims.iter().collect::<HashSet<_>>().len() != claims.len()
        {
            return Err(miss("ambiguous or mixed property inventory"));
        }
        if let Some(reason) = crate::core_temporal::static_refusal(&ir, c, &options, &p) {
            return Err(miss(reason));
        }
        let config_limits = crate::temporal::core_file_configuration();
        let graph = GraphCheck::prepare(
            m,
            &options,
            config_limits.limits,
            config_limits.acceptance,
            config_limits.graph_labels,
            Some(Schedule::DEFAULT),
        )
        .map_err(|e| miss(format!("ordinary temporal planning: {:?}", e.failure)))?;
        if graph
            .source_rows_possible()
            .map_err(|e| miss(format!("source-row planning: {e:?}")))?
        {
            return Err(miss("temporal source-row evaluation"));
        }
        let ctl = EvaluationControl::new(&options.budget, Phase::Capture)
            .map_err(|e| miss(e.to_string()))?;
        let atoms = crate::core_native_atoms::NativeAtoms::prepare(m, &p.store, &graph, &ctl, opt)
            .map_err(|e| miss(format!("native temporal atom planning: {e:?}")))?;
        if !atoms.all_native() {
            return Err(miss("requires the all-native temporal atom path"));
        }
        let first = Binding::collect(
            &p,
            graph
                .first_plan()
                .ok_or_else(|| miss("missing ordinary temporal product"))?,
        )?;
        Ok(Self {
            modules,
            root,
            config,
            limits,
            claims,
            first,
            atom_count: graph.native_predicates().count(),
        })
    }

    /// Establish that the current singleton inventory is the actual first
    /// product already searched by the previous program. Returns only its
    /// property name. The supplied prior native check report must document a
    /// completed, diagnostic-free, unaudited, all-native run with zero actual
    /// fallbacks or reference atom evaluations. Missing or malformed evidence
    /// returns a miss. This validates report fields, never their provenance.
    ///
    /// To publish a result, independently authenticate a successful ordinary
    /// Core `NativeHolds` of *this exact previous source and configuration*,
    /// same executable/build, Dive profile, one worker, exact store, seed and
    /// fingerprint zero, default periodic schedule, identical state/depth
    /// limits and unlimited internal time and memory. Require exhausted final
    /// analysis and no completed-level checks or startup/search diagnostics.
    /// Preserve only counters actually supplied by the establishing result.
    /// Enforce the current request deadline on preparation and dispatch.
    ///
    /// # Errors
    /// Any other edit, policy or role returns a miss requiring an ordinary check.
    pub fn first_claim<'a>(
        &self,
        current: &'a Self,
        previous_native_report: &serde_json::Value,
    ) -> Result<&'a str, Miss> {
        policy(current.limits)?;
        if self.limits != current.limits
            || self.root != current.root
            || self.modules != current.modules
            || self.config != current.config
        {
            return Err(miss(
                "closed behavior source, configuration or structural limits changed",
            ));
        }
        let [name] = current.claims.as_slice() else {
            return Err(miss("requires exactly one retained temporal claim"));
        };
        if self.claims.len() < 2 || self.claims.first() != Some(name) || self.first != current.first
        {
            return Err(miss(
                "requires the identical first bound temporal product and transition program",
            ));
        }
        if execution::qualify(previous_native_report, self.limits)? != self.atom_count {
            return Err(miss(
                "previous native atom count differs from the actual previous plan",
            ));
        }
        Ok(name)
    }
}

#[cfg(test)]
mod tests;
