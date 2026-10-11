//! Source-owned pre-startup admission through the real configured role splitter.

use std::collections::BTreeSet;
use std::sync::Arc;

use dive_config::ModelConfig;
use dive_ir::{DefId, DefScope, Expr, ExprId, Ir, OpRef, StdOp};

use crate::bind::Binder;
use crate::tables::Look;
use crate::{BindError, BindOptions, Failure};

const MAX_WORK: usize = 16_384;
const MAX_DEPTH: usize = 128;

/// Admits the bounded pure-safety PROPERTY/SPECIFICATION slice before startup.
///
/// This prepares the actual configuration/tool-object tables, then reuses the
/// shared binder's role decomposition without evaluating constant definitions,
/// assumptions, predicates or quantified domains. It does not create a `RunHost`,
/// run the model, or certify a trace. The normal binder must still run afterward.
/// Potential startup bodies are conservative roots, including unused user
/// definitions and standard top-level zero-argument definitions; effectful,
/// opaque, recursive, over-budget or evaluation-demanding contexts refuse.
/// Configured redirects and native replacements use actual table lookup, not
/// propagated source levels/effect flags. Work is limited to 16,384 traversal visits
/// to nodes/definitions (including repeated shared paths) and depth 128. Temporal role vectors are never admitted.
///
/// # Errors
/// A typed binding diagnostic at its actual pre-startup stage, or a named
/// unsupported admission boundary. No startup Print output is produced.
pub fn admit_pure_safety_roles(
    ir: Arc<Ir>,
    cfg: &ModelConfig,
    opts: &BindOptions,
) -> Result<(), Box<Failure>> {
    let failure = |error| {
        Box::new(Failure {
            error,
            output: Vec::new(),
        })
    };
    dive_eval::validate_ir(&ir).map_err(|e| failure(BindError::unsupported(e.to_string())))?;
    let tables = crate::tables::Tables::new(ir.clone()).map_err(failure)?;
    let constants = crate::walk::process_constants(&tables, opts.eval.ctx.int_mode);
    if let Some(error) = constants.error {
        return Err(failure(error));
    }
    let names = Arc::new(crate::intern::feed(&ir, cfg));
    let local = BindOptions {
        host: None,
        ..opts.clone()
    };
    let mut binder = Binder::new(ir, cfg, &local, names).map_err(failure)?;
    binder.process_spec().map_err(failure)?;
    SourceWalk::new(&binder).check().map_err(failure)?;
    binder.build_evaluator().map_err(failure)?;
    binder.structural_admission = true;
    binder.process_config().map_err(failure)?;
    if !binder.temporals.is_empty() || !binder.implied_temporals.is_empty() {
        return Err(failure(BindError::unsupported(
            "pure safety role admission found temporal assumptions or obligations",
        )));
    }
    Ok(())
}

struct SourceWalk<'a, 'b> {
    binder: &'a Binder<'b>,
    active_definitions: BTreeSet<DefId>,
    active_expressions: BTreeSet<ExprId>,
    remaining: usize,
}

impl<'a, 'b> SourceWalk<'a, 'b> {
    fn new(binder: &'a Binder<'b>) -> Self {
        Self {
            binder,
            active_definitions: BTreeSet::new(),
            active_expressions: BTreeSet::new(),
            remaining: MAX_WORK,
        }
    }

    fn step(&mut self, depth: usize) -> Result<(), BindError> {
        if depth > MAX_DEPTH {
            return Err(BindError::unsupported(
                "pure safety role admission depth bound",
            ));
        }
        self.remaining = self.remaining.checked_sub(1).ok_or_else(|| {
            BindError::unsupported("pure safety role admission source-work bound")
        })?;
        Ok(())
    }

    fn check(mut self) -> Result<(), BindError> {
        let ir = self.binder.ir();
        for (index, def) in ir.defs().iter().enumerate() {
            let startup = !ir.module(def.module).standard
                || (def.params.is_empty() && matches!(def.scope, DefScope::TopLevel));
            if startup {
                let id = DefId(u32::try_from(index).unwrap_or(u32::MAX));
                // The real startup loop skips a plain native tool object.
                // Metadata alone cannot skip an actually configured redirect.
                if def.std.is_none() || !matches!(self.binder.t.lookup_def(id), Look::Java(..)) {
                    self.definition(id, 0)?;
                }
            }
        }
        // Native definitions selected directly as roles or configured targets
        // are real table entries even if their source bodies were not roots.
        let cfg = self.binder.cfg;
        let roles = cfg
            .invariants
            .iter()
            .chain(&cfg.properties)
            .chain(&cfg.constraints)
            .chain(&cfg.action_constraints)
            .chain(&cfg.postconditions)
            .chain(cfg.init.iter())
            .chain(cfg.next.iter())
            .chain(cfg.specification.iter())
            .chain(cfg.view.iter())
            .chain(cfg.symmetry.iter())
            .chain(cfg.alias.iter());
        let mut selected: BTreeSet<_> = roles.map(|n| n.text.as_str()).collect();
        selected.extend(cfg.constants.iter().filter_map(|entry| match entry {
            dive_config::ConstantEntry::Substitution { target, .. } => Some(target.text.as_str()),
            dive_config::ConstantEntry::Assignment { .. } => None,
        }));
        for name in selected {
            let Some(value) = self.binder.defns.get(name) else {
                continue;
            };
            match value {
                Look::Java(op, _) => Self::native(*op)?,
                Look::Def(def) => self.definition(*def, 0)?,
                Look::Value(_) | Look::Literal(_) => {}
            }
        }
        Ok(())
    }

    fn native(op: StdOp) -> Result<(), BindError> {
        if op.impure()
            || matches!(
                op,
                StdOp::TlcTLCGet | StdOp::TlcTLCSet | StdOp::TlcExtTLCGetOrDefault
            )
        {
            Err(BindError::unsupported(format!(
                "pure safety role admission found runtime effects/checker reads in {}",
                op.qualified_name()
            )))
        } else {
            Ok(())
        }
    }

    fn definition(&mut self, def: DefId, depth: usize) -> Result<(), BindError> {
        self.step(depth)?;
        if self.active_definitions.contains(&def) {
            return Err(BindError::unsupported(
                "pure safety role admission of cyclic definition/redirect context",
            ));
        }
        self.active_definitions.insert(def);
        let result = self.definition_inner(def, depth);
        self.active_definitions.remove(&def);
        result
    }

    fn definition_inner(&mut self, def: DefId, depth: usize) -> Result<(), BindError> {
        match self.binder.t.lookup_def(def) {
            Look::Java(op, _) => Self::native(op),
            Look::Value(_) | Look::Literal(_) => Ok(()),
            Look::Def(selected) => {
                if selected != def {
                    return self.definition(selected, depth + 1);
                }
                let definition = self.binder.ir().def(selected);
                if definition.recursive {
                    return Err(BindError::unsupported(format!(
                        "pure safety role admission of recursive definition {}",
                        definition.name
                    )));
                }
                self.expression(definition.body, depth + 1)
            }
        }
    }

    fn expression(&mut self, id: ExprId, depth: usize) -> Result<(), BindError> {
        self.step(depth)?;
        if !self.active_expressions.insert(id) {
            return Err(BindError::unsupported(
                "pure safety role admission of cyclic expression context",
            ));
        }
        let result = self.expression_inner(id, depth);
        self.active_expressions.remove(&id);
        result
    }

    fn expression_inner(&mut self, id: ExprId, depth: usize) -> Result<(), BindError> {
        let expression = self.binder.ir().expr(id);
        // Subscript inspection turns scope errors into warnings. Reject the
        // unproved strict context here, before that warning-only traversal.
        if let Expr::Subst { bindings, .. } = expression
            && bindings.iter().any(|binding| {
                matches!(
                    binding.value,
                    dive_ir::Arg::Expr {
                        pass: dive_ir::Pass::Strict,
                        ..
                    }
                )
            })
        {
            return Err(BindError::unsupported(
                "pure safety role admission needs strict instance-substitution evaluation",
            ));
        }
        if let Expr::Def(def) = expression {
            self.definition(*def, depth + 1)?;
        }
        if let Expr::Let { defs, .. } = expression {
            for &def in defs {
                self.definition(def, depth + 1)?;
            }
        }
        if let Expr::Const(constant) = expression
            && let Some(value) = self.binder.t.const_look(*constant)
        {
            match value {
                Look::Def(def) => self.definition(def, depth + 1)?,
                Look::Java(op, _) => Self::native(op)?,
                Look::Value(_) | Look::Literal(_) => {}
            }
        }
        for reference in expression.op_refs() {
            match reference {
                OpRef::Std(op) => Self::native(*op)?,
                OpRef::Def(def) => self.definition(*def, depth + 1)?,
                OpRef::Const(constant) => match self.binder.t.const_look(*constant) {
                    Some(Look::Def(def)) => self.definition(def, depth + 1)?,
                    Some(Look::Java(op, _)) => Self::native(op)?,
                    Some(Look::Value(_) | Look::Literal(_)) => {}
                    None => {
                        return Err(BindError::unsupported(
                            "pure safety role admission of unresolved constant operator",
                        ));
                    }
                },
                OpRef::Builtin(_) => {}
                OpRef::Param(_) | OpRef::Subst(_) | OpRef::Unbound(_) => {
                    return Err(BindError::unsupported(
                        "pure safety role admission of opaque operator context",
                    ));
                }
            }
        }
        let children = expression.children();
        if children.len() > self.remaining {
            return Err(BindError::unsupported(
                "pure safety role admission source-work bound",
            ));
        }
        for child in children {
            self.expression(child, depth + 1)?;
        }
        Ok(())
    }
}
