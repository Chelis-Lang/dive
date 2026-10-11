//! `dive-model`: binding a specification to its TLC model configuration
//! (ADR 0013 §8).
//!
//! This is a port of TLC's `tlc2.tool.impl.SpecProcessor` (tlaplus
//! `94d0c50`), with the parts of `Spec` and `Tool` that finish setting up a
//! model. [`bind`] takes the lowered specification and the parsed
//! configuration and does what TLC does before it checks the assumptions, in
//! TLC's order:
//!
//! 1. **Literals** (`processConstants`): a numeral too big for `--int=tlc` or
//!    a decimal stops TLC first.
//! 2. **Names** ([`mod@intern`]): the run's name table is filled in TLC's
//!    encounter order, so that strings and model values order and
//!    fingerprint as in TLC.
//! 3. **Constants and overrides** (`processSpec`): every root constant must be
//!    assigned (`C = v`) or substituted (`C <- D`); `Op = v` and `Op <- D`
//!    replace definitions; `[M]` entries replace definitions of module `M`.
//!    Errors carry TLC's codes ([`Ec`]).
//! 4. **Constant definitions** (`processConstantDefns`): every zero-arity
//!    definition TLC's level bound ([`mod@level`]) finds constant is
//!    evaluated at start-up, module by module, and its value is used from
//!    then on. `Print` output appears here, as in TLC.
//! 5. **Roles** (`processConfig`): invariants, the behavior specification
//!    (`INIT`/`NEXT` or `SPECIFICATION`, split into initial predicates, the
//!    next-state relation and temporal formulas), properties (split into
//!    invariants, implied initial predicates, implied actions and temporal
//!    properties), constraints, `VIEW`.
//! 6. **Actions** (`Tool.getActions`): the next-state relation split into
//!    TLC's actions, with their labels, as `dive_eval::next` actions (with
//!    the default `enumerate` feature).
//!
//! The result is a [`Model`]: an [`dive_eval::Evaluator`] with everything
//! bound, and every role as a [`Pred`] (an expression and the scope TLC
//! evaluates it in). Features dive does not check yet (liveness, `SYMMETRY`,
//! operators it does not implement) are listed in [`Model::unsupported`];
//! the checker refuses such a model by name. Anything this stage cannot do
//! exactly is an [`ErrorKind::Unsupported`] error.
//!
//! # Java overrides and loaded modules
//!
//! Which Java implementation TLC uses for a standard operator depends on the
//! modules loaded: `Integers`' methods replace `Naturals`' when both are
//! loaded. The IR records the active override of every definition, computed
//! from the loaded modules (`dive_ir::std_op`). TLC's `-dumpTrace json`
//! loads `_JsonTrace`, and with it `Integers`, without extending the root
//! module; `dive_sema::Options::extra_root_extendees` reproduces that for
//! oracle comparison (see [`DUMP_TRACE_JSON_MODULES`]).
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//! use dive_model::{bind, BindOptions};
//!
//! let src = "---- MODULE M ----\nEXTENDS Naturals, TLC\nCONSTANT N\nVARIABLE x\n\
//!            Max == Print(N, N * 2)\nInit == x = 0\nNext == x < Max /\\ x' = x + 1\n\
//!            Inv == x <= Max\n====\n";
//! let analysis = dive_sema::analyze_source("M.tla", src, &dive_sema::Options::default());
//! let ir = Arc::new(dive_ir::lower(&analysis).unwrap());
//! let cfg = dive_config::parse("CONSTANT N = 3\nINIT Init\nNEXT Next\nINVARIANT Inv\n")
//!     .into_config()
//!     .unwrap();
//! let model = bind(ir, &cfg, &BindOptions::tlc()).unwrap();
//! // `Max` is evaluated once, at start-up, as TLC does.
//! assert_eq!(model.output, [dive_model::Message::Print("3  6".to_owned())]);
//! assert_eq!(model.invariants[0].name, "Inv");
//! assert_eq!(model.actions.len(), 1);
//! assert!(model.actions[0].label().starts_with("<Next line 7, col 9 to line 7, col"));
//! ```
#![forbid(unsafe_code)]

#[cfg(feature = "enumerate")]
mod actions;
mod bind;
mod error;
pub mod intern;
pub mod level;
mod model;
pub mod random_view;
mod periodic;
mod reach;
mod spec;
pub mod symmetry;
mod tables;
mod tlc_names;
mod walk;

use std::sync::Arc;

use dive_config::ModelConfig;
use dive_ir::Ir;

pub use bind::BindOptions;
pub use error::{BindError, Ec, ErrorKind, Failure, Message};
pub use model::{Init, Model, Named, Pred, Settings, Unsupported, UnsupportedKind};
pub use periodic::{PeriodicSnapshot, bind_periodic_snapshot};

/// The modules TLC's `-dumpTrace json <file>` loads in addition to the
/// specification's own (`TLC.java` adds the `_JsonTrace` post-condition, and
/// `ParameterizedSpecObj` makes SANY load `_JsonTrace` as if the root module
/// extended it). Pass it as `dive_sema::Options::extra_root_extendees` to
/// reproduce the conformance harness's TLC runs, which use `-dumpTrace json`:
/// it changes which Java overrides are active (`Integers` replaces
/// `Naturals`' arithmetic) and the encounter order of names. TLC as users
/// run it, and so `--compat=tlc`, does not load them. Oracle reproduction
/// only, until the goldens are regenerated without `-dumpTrace` (issue #35).
pub const DUMP_TRACE_JSON_MODULES: &[&str] = &["_JsonTrace"];

/// Binds `ir` to `cfg` as TLC does (see the crate documentation).
///
/// # Errors
/// TLC's error for the model, with what TLC printed before it; or an
/// unsupported feature or an internal error, which are never verdicts.
pub fn bind(ir: Arc<Ir>, cfg: &ModelConfig, opts: &BindOptions) -> Result<Model, Box<Failure>> {
    bind_in(ir, cfg, opts, None, false).map(|(model, _)| model)
}

/// Binds a trusted native recovered ordered name table before evaluating constants.
/// The complete startup feed must be an identical prefix; missing, duplicate or
/// reordered names refuse before any model callback. This is not an authenticity
/// check and does not admit runtime state absent from the native checkpoint.
///
/// # Errors
/// The ordinary binding errors, or a named unsupported restored-table mismatch.
pub fn bind_with_names(
    ir: Arc<Ir>,
    cfg: &ModelConfig,
    opts: &BindOptions,
    restored: &[String],
) -> Result<Model, Box<Failure>> {
    bind_in(ir, cfg, opts, Some(restored), false).map(|(model, _)| model)
}

fn bind_in(
    ir: Arc<Ir>,
    cfg: &ModelConfig,
    opts: &BindOptions,
    restored: Option<&[String]>,
    periodic_snapshot: bool,
) -> Result<(Model, Option<periodic::Role>), Box<Failure>> {
    dive_eval::validate_ir(&ir).map_err(|error| {
        Box::new(Failure {
            error: BindError::unsupported(error.to_string()),
            output: Vec::new(),
        })
    })?;
    let walk = {
        let t = tables::Tables::new(ir.clone()).map_err(|error| {
            Box::new(Failure {
                error,
                output: Vec::new(),
            })
        })?;
        walk::process_constants(&t, opts.eval.ctx.int_mode)
    };
    let ir: Arc<Ir> = ir;
    if let Some(error) = walk.error {
        return Err(Box::new(Failure {
            error,
            output: Vec::new(),
        }));
    }
    let feed = intern::feed(&ir, cfg);
    let names = if let Some(restored) = restored {
        let refuse = || {
            Box::new(Failure {
                error: BindError {
                    kind: ErrorKind::Unsupported {
                        feature: "native checkpoint ordered name table/startup-prefix mismatch"
                            .into(),
                    },
                    message: "native checkpoint ordered name table/startup-prefix mismatch".into(),
                },
                output: Vec::new(),
            })
        };
        if restored.len() < feed.len()
            || restored.len() > 65_536
            || restored.iter().any(|s| s.len() > 1 << 20)
            || restored
                .iter()
                .try_fold(0usize, |n, s| n.checked_add(s.len()))
                .is_none_or(|n| n > 64 << 20)
        {
            return Err(refuse());
        }
        let names = dive_value::Interner::new();
        for (i, text) in restored.iter().enumerate() {
            if (i < feed.len()
                && feed
                    .resolve(u32::try_from(i).unwrap_or(u32::MAX))
                    .is_none_or(|a| a.as_str() != text))
                || names.get(text).is_some()
            {
                return Err(refuse());
            }
            names.intern(text);
        }
        Arc::new(names)
    } else {
        Arc::new(feed)
    };
    let mut b = bind::Binder::new(ir, cfg, opts, names).map_err(|error| {
        Box::new(Failure {
            error,
            output: Vec::new(),
        })
    })?;
    b.periodic_snapshot = periodic_snapshot;
    let result = run(&mut b);
    b.drain_prints();
    match result {
        Ok(model) => Ok(model),
        Err(error) => Err(Box::new(Failure {
            error,
            output: std::mem::take(&mut b.output),
        })),
    }
}

fn run(b: &mut bind::Binder<'_>) -> Result<(Model, Option<periodic::Role>), BindError> {
    b.process_spec()?;
    // SpecProcessor snapshots defns immediately after processSpec, before
    // constant definitions replace their entries with values.
    let periodic_definition = b
        .cfg
        .periodic
        .as_ref()
        .and_then(|n| b.defns.get(&n.text).cloned());
    // Tool.getSymmetryPerms uses the definition table before constant
    // definitions become values (SpecProcessor.snapshot).
    let symmetry_definition = b
        .cfg
        .symmetry
        .as_ref()
        .and_then(|n| b.defns.get(&n.text).cloned());
    b.build_evaluator()?;
    b.process_constant_defns()?;
    b.process_config()?;
    let periodic = if b.periodic_snapshot {
        Some(periodic::resolve(b, periodic_definition.as_ref())?)
    } else {
        None
    };
    if b.periodic_snapshot && !b.cfg.possible.is_empty() {
        return Err(BindError::unsupported("_POSSIBLE"));
    }
    let view = b.process_view()?;
    #[cfg(feature = "enumerate")]
    let (init_actions, actions) = (b.init_actions()?, b.expand_actions()?);
    b.drain_prints();
    let ir = b.t.ir.clone();
    let root = ir.module(ir.root());
    let assumptions = root
        .assumptions
        .iter()
        .zip(root.axioms.iter().chain(std::iter::repeat(&false)))
        .filter(|(_, ax)| !**ax)
        .map(|(d, _)| *d)
        .collect();
    let mut model = Model {
        ir: ir.clone(),
        evaluator: b.ev().clone(),
        names: b.names.clone(),
        output: std::mem::take(&mut b.output),
        init: std::mem::take(&mut b.init),
        next: b.next.clone(),
        #[cfg(feature = "enumerate")]
        init_actions,
        #[cfg(feature = "enumerate")]
        actions,
        invariants: std::mem::take(&mut b.invariants),
        implied_inits: std::mem::take(&mut b.implied_inits),
        implied_actions: std::mem::take(&mut b.implied_actions),
        temporals: std::mem::take(&mut b.temporals),
        implied_temporals: std::mem::take(&mut b.implied_temporals),
        implied_temporal_action_names: std::mem::take(&mut b.implied_temporal_action_names),
        configured_properties: std::mem::take(&mut b.configured_properties),
        constraints: std::mem::take(&mut b.constraints),
        action_constraints: std::mem::take(&mut b.action_constraints),
        view,
        alias: b.process_alias(),
        postconditions: b.process_postconditions(),
        symmetry: b.cfg.symmetry.as_ref().map(|n| n.text.clone()),
        symmetry_definition,
        symmetry_reduction: None,
        settings: model::Settings {
            check_deadlock: b.cfg.checks_deadlock() && !b.opts.no_deadlock,
        },
        assumptions,
        unsupported: Vec::new(),
        sequential: Vec::new(),
        sequential_names_only: false,
        runtime_name_order: false,
    };
    model.unsupported = reach::unsupported(b, &model);
    let sequential = reach::sequential(b, &model);
    model.sequential = sequential.reasons;
    model.sequential_names_only = sequential.names_only;
    model.runtime_name_order = sequential.runtime_names;
    Ok((model, periodic))
}
