//! The configuration's roles (`SpecProcessor.processConfig`,
//! `processConfigSpec`, `processConfigProps`, the constraints, and `Spec`'s
//! `VIEW`, `ALIAS`, `POSTCONDITION` lookups), in TLC's order.

use dive_eval::{Scope, SlotView, St};
use dive_ir::{DeclId, DefId, Expr, ExprId, JunctionKind, Level, OpRef, QuantKind};
use dive_value::Value;

use crate::bind::{Binder, R};
use crate::error::{BindError, Ec, Message};
use crate::level::LevelCx;
use crate::model::{Init, Named, Pred};
use crate::tables::Look;

/// What TLC's `lookup(opNode, c)` gives for the operator of a zero-argument
/// application while it walks a specification.
enum Found {
    /// A definition, with the scope its body is evaluated in.
    Def(DefId, Scope),
    /// A value.
    Value(Value),
    /// A `LazyValue`: an argument TLC bound when it inlined an operator.
    Lazy(ExprId, Scope),
    /// Something TLC's code does not expect here (a declaration, a bound
    /// variable, a Java override): TLC prints the node or method in its
    /// message, which dive does not reproduce.
    Other(String),
}

impl Binder<'_> {
    /// `SpecProcessor.processConfig`.
    #[allow(clippy::too_many_lines, reason = "TLC's sequence, step by step")]
    pub(crate) fn process_config(&mut self) -> R<()> {
        let cfg = self.cfg;
        for n in cfg
            .invariants
            .iter()
            .chain(&cfg.properties)
            .chain(&cfg.constraints)
            .chain(&cfg.action_constraints)
            .chain(cfg.specification.iter())
            .chain(cfg.init.iter())
            .chain(cfg.next.iter())
            .chain(cfg.view.iter())
        {
            self.check_refused(&n.text)?;
        }
        self.process_invariants()?;
        let spec = self.cfg.specification.as_ref().map(|n| n.text.clone());
        match spec {
            None => self.process_init_next()?,
            Some(name) => {
                if self.cfg.init.is_some() || self.cfg.next.is_some() {
                    return Err(BindError::tlc(Ec::TlcConfigNotBothSpecAndInit, &[]));
                }
                match self.defn(&name) {
                    Some(Look::Def(d)) => {
                        if !self.ir().def(d).params.is_empty() {
                            return Err(BindError::tlc(Ec::TlcConfigIdRequiresNoArg, &[&name]));
                        }
                        let scope = self.ev().scope(d);
                        let body = self.ir().def(d).body;
                        self.spec(body, scope, None)?;
                    }
                    None => {
                        return Err(BindError::tlc(
                            Ec::TlcConfigSpecifiedNotDefined,
                            &["name", &name],
                        ));
                    }
                    Some(other) => {
                        let shown = self.show_look(&other)?;
                        return Err(BindError::tlc(
                            Ec::TlcConfigIdHasValue,
                            &["value", &name, &shown],
                        ));
                    }
                }
            }
        }
        let props: Vec<String> = self.cfg.properties.iter().map(|n| n.text.clone()).collect();
        for name in props {
            match self.defn(&name) {
                Some(Look::Def(d)) => {
                    if !self.ir().def(d).params.is_empty() {
                        return Err(BindError::tlc(Ec::TlcConfigIdRequiresNoArg, &[&name]));
                    }
                    if self.ir().def(d).level == Level::State {
                        self.output
                            .push(Message::warning(Ec::TlcLiveFormulaStateLevel, &[&name]));
                    }
                    let scope = self.ev().scope(d);
                    let body = self.ir().def(d).body;
                    self.props(name.clone(), body, scope.clone(), None)?;
                    self.configured_properties.push(Named {
                        name,
                        pred: Pred::new(body, scope),
                    });
                }
                None => {
                    return Err(BindError::tlc(
                        Ec::TlcConfigSpecifiedNotDefined,
                        &["property", &name],
                    ));
                }
                Some(Look::Value(Value::Bool(true))) => {}
                Some(other) => {
                    let shown = self.show_look(&other)?;
                    return Err(BindError::tlc(
                        Ec::TlcConfigIdHasValue,
                        &["property", &name, &shown],
                    ));
                }
            }
        }
        let has_vars = !self.ir().vars().is_empty();
        if self.init.is_empty()
            && (!self.implied_inits.is_empty()
                || !self.implied_actions.is_empty()
                || has_vars
                || !self.invariants.is_empty()
                || !self.implied_temporals.is_empty())
        {
            return Err(BindError::tlc(Ec::TlcConfigMissingInit, &[]));
        }
        if self.next.is_none()
            && (!self.implied_actions.is_empty()
                || !self.invariants.is_empty()
                || !self.implied_temporals.is_empty())
        {
            return Err(BindError::tlc(Ec::TlcConfigMissingNext, &[]));
        }
        let constraints: Vec<String> = self
            .cfg
            .constraints
            .iter()
            .map(|n| n.text.clone())
            .collect();
        self.constraints = self.process_constraints(&constraints, "constraint")?;
        let actions: Vec<String> = self
            .cfg
            .action_constraints
            .iter()
            .map(|n| n.text.clone())
            .collect();
        self.action_constraints = self.process_constraints(&actions, "action constraint")?;
        if self.cfg.rl_reward.is_some() {
            return Err(BindError::unsupported("_RL_REWARD"));
        }
        if self.cfg.periodic.is_some() {
            return Err(BindError::unsupported("_PERIODIC"));
        }
        if !self.cfg.possible.is_empty() {
            return Err(BindError::unsupported("_POSSIBLE"));
        }
        Ok(())
    }

    fn show_look(&self, l: &Look) -> R<String> {
        match l {
            Look::Value(v) => Ok(self.show(v)),
            Look::Literal(e) => Ok(self.ev().location(*e)),
            Look::Def(d) => Ok(self.ev().location(self.ir().def(*d).body)),
            Look::Java(op, _) => Err(BindError::unsupported(format!(
                "the message TLC prints for the Java override of {} used as a configuration name",
                op.qualified_name()
            ))),
        }
    }

    /// `processConfigInvariants`.
    fn process_invariants(&mut self) -> R<()> {
        let names: Vec<String> = self.cfg.invariants.iter().map(|n| n.text.clone()).collect();
        for name in names {
            match self.defn(&name) {
                Some(Look::Def(d)) => {
                    let def = self.ir().def(d);
                    if !def.params.is_empty() {
                        return Err(BindError::tlc(
                            Ec::TlcConfigIdRequiresNoArg,
                            &["invariant", &name],
                        ));
                    }
                    if def.level >= Level::Action {
                        // TLC adds a note when the body's `levelParams` are not
                        // empty (`processConfigInvariants`).
                        let params: &[&str] = if self.has_level_params(def.body) {
                            &[&def.name, "includeWarning"]
                        } else {
                            &[&def.name]
                        };
                        return Err(BindError::tlc(Ec::TlcInvariantViolatedLevel, params));
                    }
                    let pred = Pred::new(def.body, self.ev().scope(d));
                    self.invariants.push(Named { name, pred });
                }
                None => {
                    return Err(BindError::tlc(
                        Ec::TlcConfigSpecifiedNotDefined,
                        &["invariant", &name],
                    ));
                }
                Some(Look::Value(Value::Bool(true))) => {
                    self.output.push(Message::warning(
                        Ec::TlcInvariantConstantLevel,
                        &[&name, "TRUE"],
                    ));
                }
                Some(other) => {
                    let shown = self.show_look(&other)?;
                    return Err(BindError::tlc(
                        Ec::TlcConfigIdHasValue,
                        &["invariant", &name, &shown],
                    ));
                }
            }
        }
        Ok(())
    }

    /// `processConfigInitAndNext`.
    fn process_init_next(&mut self) -> R<()> {
        for (which, name) in [
            (
                "initial predicate",
                self.cfg.init.as_ref().map(|n| n.text.clone()),
            ),
            (
                "next state action",
                self.cfg.next.as_ref().map(|n| n.text.clone()),
            ),
        ] {
            let Some(name) = name else { continue };
            let d = match self.defn(&name) {
                None => {
                    return Err(BindError::tlc(
                        Ec::TlcConfigSpecifiedNotDefined,
                        &[which, &name],
                    ));
                }
                Some(Look::Def(d)) => d,
                Some(_) => {
                    return Err(BindError::tlc(
                        Ec::TlcConfigIdMustNotBeConstant,
                        &[which, &name],
                    ));
                }
            };
            if !self.ir().def(d).params.is_empty() {
                return Err(BindError::tlc(
                    Ec::TlcConfigIdRequiresNoArg,
                    &[which, &name],
                ));
            }
            let pred = Pred::new(self.ir().def(d).body, self.ev().scope(d));
            if which == "initial predicate" {
                self.init.push(Init { pred, def: Some(d) });
            } else {
                self.next_def = Some(d);
                self.next = Some(pred);
            }
        }
        Ok(())
    }

    /// `processModelConstraints`, `processActionConstraints`.
    fn process_constraints(&mut self, names: &[String], what: &str) -> R<Vec<Named>> {
        let mut out = Vec::new();
        for name in names {
            match self.defn(name) {
                Some(Look::Def(d)) => {
                    if !self.ir().def(d).params.is_empty() {
                        return Err(BindError::tlc(Ec::TlcConfigIdRequiresNoArg, &[what, name]));
                    }
                    let pred = Pred::new(self.ir().def(d).body, self.ev().scope(d));
                    out.push(Named {
                        name: name.clone(),
                        pred,
                    });
                }
                Some(Look::Value(Value::Bool(true))) => {}
                Some(other) => {
                    let shown = self.show_look(&other)?;
                    return Err(BindError::tlc(
                        Ec::TlcConfigIdHasValue,
                        &[what, name, &shown],
                    ));
                }
                None => {
                    return Err(BindError::tlc(
                        Ec::TlcConfigSpecifiedNotDefined,
                        &[what, name],
                    ));
                }
            }
        }
        Ok(out)
    }

    /// The view, alias, post-condition and symmetry lookups of `Spec`, which
    /// TLC makes when it sets up the state class (view, symmetry) or later
    /// (alias, post-conditions).
    pub(crate) fn process_view(&mut self) -> R<Option<Named>> {
        let Some(name) = self.cfg.view.as_ref().map(|n| n.text.clone()) else {
            return Ok(None);
        };
        match self.defn(&name) {
            None => Err(BindError::tlc(
                Ec::TlcConfigSpecifiedNotDefined,
                &["view function", &name],
            )),
            Some(Look::Def(d)) => {
                if !self.ir().def(d).params.is_empty() {
                    return Err(BindError::tlc(
                        Ec::TlcConfigIdRequiresNoArg,
                        &["view function", &name],
                    ));
                }
                let pred = Pred::new(self.ir().def(d).body, self.ev().scope(d));
                Ok(Some(Named { name, pred }))
            }
            Some(_) => Err(BindError::tlc(
                Ec::TlcConfigIdMustNotBeConstant,
                &["view function", &name],
            )),
        }
    }

    /// `Spec.getAliasSpec`.
    pub(crate) fn process_alias(&self) -> Option<R<Named>> {
        let name = self.cfg.alias.as_ref()?.text.clone();
        Some(self.named_def(&name, "alias"))
    }

    /// `Spec.getPostConditionSpecs`.
    pub(crate) fn process_postconditions(&self) -> R<Vec<Named>> {
        self.cfg
            .postconditions
            .iter()
            .map(|n| self.named_def(&n.text, "post condition"))
            .collect()
    }

    /// A configuration name that must name a definition without arguments
    /// (`TLC_CONFIG_SPECIFIED_NOT_DEFINED`, `TLC_CONFIG_ID_MUST_NOT_BE_CONSTANT`,
    /// `TLC_CONFIG_ID_REQUIRES_NO_ARG`, in that order).
    fn named_def(&self, name: &str, what: &str) -> R<Named> {
        match self.defn(name) {
            None => Err(BindError::tlc(
                Ec::TlcConfigSpecifiedNotDefined,
                &[what, name],
            )),
            Some(Look::Def(d)) => {
                if !self.ir().def(d).params.is_empty() {
                    return Err(BindError::tlc(Ec::TlcConfigIdRequiresNoArg, &[what, name]));
                }
                Ok(Named {
                    name: name.to_owned(),
                    pred: Pred::new(self.ir().def(d).body, self.ev().scope(d)),
                })
            }
            Some(_) => Err(BindError::tlc(
                Ec::TlcConfigIdMustNotBeConstant,
                &[what, name],
            )),
        }
    }

    // -------------------------------------------------------------------
    // processConfigSpec
    // -------------------------------------------------------------------

    /// The scope an instance substitution wrapper starts, and its location.
    fn subst_scope(&self, scope: &Scope, e: ExprId) -> R<Scope> {
        self.ev()
            .scope_subst(scope, e)
            .map_err(|err| BindError::unsupported(format!("binding instance substitutions: {err}")))
    }

    fn let_scope(&self, scope: &Scope, e: ExprId) -> R<Scope> {
        self.ev()
            .scope_let(scope, e)
            .map_err(|err| BindError::bug(format!("binding a LET: {err}")))
    }

    fn level(&self, e: ExprId, scope: &Scope) -> u8 {
        LevelCx::new(&self.t).level(e, scope)
    }

    /// TLC's `toString()` of a node: its location (or a numeral's image).
    pub(crate) fn node_string(&self, e: ExprId) -> String {
        self.ev().location(e)
    }

    /// `lookup(opNode, c)` for a zero-argument application, or `None` if the
    /// expression is not one.
    fn lookup_zero(&self, e: ExprId, scope: &Scope) -> Option<(String, Found)> {
        let ir = self.ir();
        Some(match ir.expr(e) {
            Expr::Def(d) => {
                let name = ir.def(*d).name.clone();
                let found = match self.t.lookup_def(*d) {
                    Look::Def(r) if self.refused.contains_key(&r) => {
                        Found::Other(self.refused.get(&r).cloned().unwrap_or_default())
                    }
                    Look::Def(r) => match self.ev().scope_enter(scope, r) {
                        Ok(s) => Found::Def(r, s),
                        Err(err) => Found::Other(err.to_string()),
                    },
                    Look::Value(v) => Found::Value(v),
                    Look::Literal(lit) => {
                        match self
                            .ev()
                            .eval_scoped(&self.ev().scope(*d), lit, &St::constant())
                        {
                            Ok(v) => Found::Value(v),
                            Err(err) => Found::Other(err.to_string()),
                        }
                    }
                    Look::Java(op, _) => Found::Other(op.qualified_name()),
                };
                (name, found)
            }
            Expr::Const(c) => {
                let decl = ir.consts()[c.index()];
                (ir.decl(decl).name.clone(), self.const_found(decl, scope))
            }
            Expr::SubstRef {
                decl,
                primed: false,
            } => (ir.decl(*decl).name.clone(), self.const_found(*decl, scope)),
            Expr::Local(r) => {
                let (name, kind) = self
                    .t
                    .slot_info(scope.def(), *r)
                    .unwrap_or_else(|| ("?".to_owned(), dive_ir::SlotKind::Bound));
                let found = match self.ev().scope_slot(scope, *r) {
                    SlotView::Lazy(x, s) => Found::Lazy(x, s),
                    SlotView::Value(v) => Found::Value(v),
                    _ => match kind {
                        dive_ir::SlotKind::LetValue(d) => Found::Def(d, scope.clone()),
                        _ => Found::Other(format!("the bound name {name}")),
                    },
                };
                (name, found)
            }
            Expr::Var { primed: false, var } => {
                let name = ir.decl(ir.vars()[var.index()]).name.clone();
                (name.clone(), Found::Other(format!("the variable {name}")))
            }
            Expr::Bool(b) => (
                if *b { "TRUE" } else { "FALSE" }.to_owned(),
                Found::Value(Value::Bool(*b)),
            ),
            _ => return None,
        })
    }

    /// A declaration's tool object (TLC's lookup does not see instance
    /// substitutions while it walks a specification).
    fn const_found(&self, decl: DeclId, scope: &Scope) -> Found {
        let ir = self.ir();
        match ir.decl(decl).constant.and_then(|c| self.t.const_look(c)) {
            Some(Look::Value(v)) => Found::Value(v),
            Some(Look::Def(d)) => match self.ev().scope_enter(scope, d) {
                Ok(s) => Found::Def(d, s),
                Err(err) => Found::Other(err.to_string()),
            },
            Some(Look::Java(op, _)) => Found::Other(op.qualified_name()),
            Some(Look::Literal(_)) | None => {
                Found::Other(format!("the declaration {}", ir.decl(decl).name))
            }
        }
    }

    /// `processConfigSpec(pred, c, subs)`. `wrap` is the outermost
    /// substitution TLC wraps around what it finds (`Specs.addSubsts`), whose
    /// location TLC reports.
    #[allow(clippy::too_many_lines, reason = "one arm per case of TLC's method")]
    fn spec(&mut self, e: ExprId, scope: Scope, wrap: Option<ExprId>) -> R<()> {
        let ir = self.t.ir.clone();
        match ir.expr(e) {
            Expr::Subst { body, .. } => {
                let s = self.subst_scope(&scope, e)?;
                return self.spec(*body, s, Some(wrap.unwrap_or(e)));
            }
            Expr::Let { body, .. } => {
                let s = self.let_scope(&scope, e)?;
                return self.spec(*body, s, wrap);
            }
            _ => {}
        }
        if let Some((name, found)) = self.lookup_zero(e, &scope) {
            match found {
                Found::Def(d, s) => {
                    if !ir.def(d).params.is_empty() {
                        return Err(BindError::tlc(Ec::TlcConfigOpNoArgs, &[&name]));
                    }
                    let body = ir.def(d).body;
                    if self.level(body, &s) == 1 {
                        let mut pred = Pred::new(body, s);
                        if let Some(w) = wrap {
                            pred.location = w;
                        }
                        self.init.push(Init { pred, def: Some(d) });
                    } else {
                        self.spec(body, s, wrap)?;
                    }
                }
                Found::Value(Value::Bool(true)) => {}
                Found::Value(Value::Bool(false)) => {
                    return Err(BindError::tlc(Ec::TlcConfigSpecIsTrivial, &[&name]));
                }
                Found::Lazy(x, s) => self.spec(x, s, wrap)?,
                Found::Value(v) => {
                    let shown = self.show(&v);
                    return Err(BindError::tlc(
                        Ec::TlcConfigOpIsEqual,
                        &[&name, &shown, "spec"],
                    ));
                }
                Found::Other(what) => {
                    return Err(BindError::unsupported(format!(
                        "the message TLC prints for {what} used as a conjunct of the behavior specification"
                    )));
                }
            }
            return Ok(());
        }
        match ir.expr(e) {
            Expr::Apply { callee, args } if !args.is_empty() => {
                if wrap.is_none()
                    && let Some((d, s, _)) = self.inline_scope(callee, args, &scope)?
                {
                    return self.spec(ir.def(d).body, s, wrap);
                }
            }
            Expr::TemporalQuant { .. } => {
                return Err(BindError::tlc(
                    Ec::TlcSpecificationFeaturesTemporalQuantifier,
                    &[],
                ));
            }
            Expr::Junction {
                kind: JunctionKind::And,
                items,
                ..
            } => {
                for &i in items {
                    self.spec(i, scope.clone(), wrap)?;
                }
                return Ok(());
            }
            Expr::Always(a) => {
                let a = *a;
                if is_op_appl(ir.expr(a)) && ir.node(a).level <= Level::State {
                    return Err(BindError::tlc(
                        Ec::TlcLiveCannotHandleFormula,
                        &[&self.node_string(a)],
                    ));
                }
                if let Expr::StepBox { action, sub } = ir.expr(a) {
                    let (action, sub) = (*action, *sub);
                    self.subscript_warnings(sub, &scope);
                    if self.next.is_some() {
                        return Err(BindError::tlc(Ec::TlcCantHandleTooManyNextStateRels, &[]));
                    }
                    let mut pred = Pred::new(action, scope);
                    if let Some(w) = wrap {
                        pred.location = w;
                    }
                    self.next = Some(pred);
                    self.next_def = None;
                } else {
                    let name = self.node_string(e);
                    let mut pred = Pred::new(e, scope);
                    if let Some(w) = wrap {
                        pred.location = w;
                    }
                    self.temporals.push(Named { name, pred });
                }
                return Ok(());
            }
            Expr::Selected(a) => return self.spec(*a, scope, wrap),
            _ => {}
        }
        let level = self.level(e, &scope);
        let mut pred = Pred::new(e, scope);
        if let Some(w) = wrap {
            pred.location = w;
        }
        match level {
            0 | 1 => self.init.push(Init { pred, def: None }),
            3 => {
                let name = self.node_string(e);
                self.temporals.push(Named { name, pred });
            }
            _ => {
                return Err(BindError::tlc(
                    Ec::TlcCantHandleConjunct,
                    &[&self.node_string(e)],
                ));
            }
        }
        Ok(())
    }

    /// The definition TLC inlines for an application with arguments, entered
    /// with its arguments bound (`getOpContext`), and the operator's name:
    /// the operator's lookup is a non-recursive definition with a body
    /// and as many parameters as there are arguments (`processConfigSpec`,
    /// `processConfigProps`).
    fn inline_scope(
        &self,
        callee: &OpRef,
        args: &[dive_ir::Arg],
        scope: &Scope,
    ) -> R<Option<(DefId, Scope, String)>> {
        let ir = self.ir();
        let (look, name) = match callee {
            OpRef::Def(d) => (self.t.lookup_def(*d), ir.def(*d).name.clone()),
            OpRef::Const(c) => {
                let decl = ir.consts()[c.index()];
                match self.t.const_look(*c) {
                    Some(l) => (l, ir.decl(decl).name.clone()),
                    None => return Ok(None),
                }
            }
            OpRef::Subst(decl) => {
                match ir.decl(*decl).constant.and_then(|c| self.t.const_look(c)) {
                    Some(l) => (l, ir.decl(*decl).name.clone()),
                    None => return Ok(None),
                }
            }
            OpRef::Param(r) => {
                let name = self
                    .t
                    .slot_info(scope.def(), *r)
                    .map_or_else(|| "?".to_owned(), |(n, _)| n);
                return match self.ev().scope_apply_param(scope, *r, args) {
                    None => Ok(None),
                    Some(Err(err)) => Err(BindError::bug(err.to_string())),
                    Some(Ok((d, s))) => {
                        if ir.def(d).recursive {
                            Ok(None)
                        } else {
                            Ok(Some((d, s, name)))
                        }
                    }
                };
            }
            _ => return Ok(None),
        };
        match look {
            Look::Def(d) if !ir.def(d).recursive && ir.def(d).params.len() == args.len() => {
                let s = self
                    .ev()
                    .scope_apply(scope, d, args)
                    .map_err(|err| BindError::bug(err.to_string()))?;
                Ok(Some((d, s, name)))
            }
            _ => Ok(None),
        }
    }

    /// TLC's check that the subscript of `[][Next]_v` contains every
    /// variable (`SubscriptCollector`): warnings only.
    fn subscript_warnings(&mut self, sub: ExprId, scope: &Scope) {
        let mut found: Vec<dive_ir::VarId> = Vec::new();
        let ok = self.collect_subscript(sub, scope, &mut found, 0);
        if !ok {
            self.output
                .push(Message::warning(Ec::TlcCouldNotDetermineSubscript, &[]));
            return;
        }
        let ir = self.t.ir.clone();
        for &v in ir.tlc_var_order() {
            if !found.contains(&v) {
                let name = ir.decl(ir.vars()[v.index()]).name.clone();
                self.output.push(Message::warning(
                    Ec::TlcSubscriptContainNoStateVar,
                    &[&name],
                ));
            }
        }
    }

    /// `SubscriptCollector.enter`; `false` where TLC gives up (an exception,
    /// caught as "could not determine").
    fn collect_subscript(
        &self,
        e: ExprId,
        scope: &Scope,
        out: &mut Vec<dive_ir::VarId>,
        depth: u32,
    ) -> bool {
        if depth > 200 {
            return false;
        }
        let ir = self.ir();
        match ir.expr(e) {
            Expr::Var { var, primed: false } => {
                out.push(*var);
                true
            }
            Expr::SubstRef {
                decl,
                primed: false,
            } => match self.ev().scope_subst_slot(scope, *decl) {
                SlotView::Lazy(x, s) => self.collect_subscript(x, &s, out, depth + 1),
                _ => true,
            },
            Expr::Local(r) => match self.ev().scope_slot(scope, *r) {
                SlotView::Lazy(x, s) => self.collect_subscript(x, &s, out, depth + 1),
                _ => true,
            },
            Expr::Def(d) => match self.t.lookup_def(*d) {
                Look::Def(r) => match self.ev().scope_enter(scope, r) {
                    Ok(s) => self.collect_subscript(ir.def(r).body, &s, out, depth + 1),
                    Err(_) => false,
                },
                _ => true,
            },
            Expr::Tuple(items) => items
                .iter()
                .all(|&i| self.collect_subscript(i, scope, out, depth + 1)),
            Expr::Apply {
                callee: OpRef::Def(d),
                args,
            } => match self.t.lookup_def(*d) {
                Look::Def(r) => match self.ev().scope_apply(scope, r, args) {
                    Ok(s) => self.collect_subscript(ir.def(r).body, &s, out, depth + 1),
                    Err(_) => false,
                },
                _ => true,
            },
            Expr::Subst { body, .. } => match self.ev().scope_subst(scope, e) {
                Ok(s) => self.collect_subscript(*body, &s, out, depth + 1),
                Err(_) => false,
            },
            Expr::Let { body, .. } => match self.ev().scope_let(scope, e) {
                Ok(s) => self.collect_subscript(*body, &s, out, depth + 1),
                Err(_) => false,
            },
            Expr::Label { body, .. } => self.collect_subscript(*body, scope, out, depth + 1),
            // Other built-in operators are ignored; numerals, strings and
            // decimals make TLC give up.
            Expr::Num(_) | Expr::Str(_) | Expr::Decimal { .. } | Expr::ExceptAt => false,
            _ => true,
        }
    }

    // -------------------------------------------------------------------
    // processConfigProps
    // -------------------------------------------------------------------

    /// `processConfigProps(name, configName, pred, c, subs)`. TLC also names
    /// the `Action`s it creates after `configName`; dive keeps the names TLC
    /// reports (`name`).
    #[allow(clippy::too_many_lines, reason = "one arm per case of TLC's method")]
    fn props(&mut self, name: String, e: ExprId, scope: Scope, wrap: Option<ExprId>) -> R<()> {
        let ir = self.t.ir.clone();
        match ir.expr(e) {
            Expr::Subst { body, .. } => {
                let s = self.subst_scope(&scope, e)?;
                return self.props(name, *body, s, Some(wrap.unwrap_or(e)));
            }
            Expr::Let { body, .. } => {
                let s = self.let_scope(&scope, e)?;
                return self.props(name, *body, s, wrap);
            }
            _ => {}
        }
        let is_var = matches!(ir.expr(e), Expr::Var { .. });
        if !is_var && let Some((op_name, found)) = self.lookup_zero(e, &scope) {
            match found {
                Found::Def(d, s) => {
                    if !ir.def(d).params.is_empty() {
                        return Err(BindError::tlc(Ec::TlcConfigOpNoArgs, &[&op_name]));
                    }
                    return self.props(op_name, ir.def(d).body, s, wrap);
                }
                Found::Value(Value::Bool(true)) => return Ok(()),
                Found::Value(Value::Bool(false)) => {
                    return Err(BindError::tlc(Ec::TlcConfigSpecIsTrivial, &[&op_name]));
                }
                Found::Lazy(x, s) => return self.props(name, x, s, wrap),
                Found::Value(v) => {
                    let shown = self.show(&v);
                    return Err(BindError::tlc(
                        Ec::TlcConfigOpIsEqual,
                        &[&op_name, &shown, "property"],
                    ));
                }
                Found::Other(what) => {
                    return Err(BindError::unsupported(format!(
                        "the message TLC prints for {what} used in a PROPERTY"
                    )));
                }
            }
        }
        match ir.expr(e) {
            Expr::Apply { callee, args } if !args.is_empty() => {
                if wrap.is_none()
                    && let Some((d, s, op_name)) = self.inline_scope(callee, args, &scope)?
                {
                    return self.props(op_name, ir.def(d).body, s, wrap);
                }
            }
            Expr::Quant {
                kind: QuantKind::Forall,
                bounds,
                body,
            } if bounds.iter().all(|b| b.domain.is_some()) => {
                let body = *body;
                // `opDefEvaluator.contexts(pred1, c)`: the domains in TLC's
                // context, which does not hold the substitutions; a
                // `TLCRuntimeException` makes TLC handle the formula below.
                let plain = self.ev().scope_without_substs(&scope);
                match self.ev().scope_bindings(&plain, e, &St::constant()) {
                    Ok(scopes) => {
                        if scopes.is_empty() {
                            return Err(BindError::tlc(Ec::TlcLiveFormulaTautology, &[]));
                        }
                        let n = self.node_string(body);
                        for s in scopes {
                            // The bindings, over the scope with its substitutions.
                            let s = self.rebase_bindings(&scope, &s, e);
                            self.props(n.clone(), body, s, wrap)?;
                        }
                        return Ok(());
                    }
                    Err(err) if err.class() == dive_eval::ErrorClass::Eval => {}
                    Err(err) => {
                        return Err(BindError::unsupported(format!(
                            "a PROPERTY quantified over a domain dive cannot enumerate at start-up ({err})"
                        )));
                    }
                }
            }
            Expr::Junction {
                kind: JunctionKind::And,
                items,
                ..
            } => {
                for &i in items {
                    let n = self.node_string(i);
                    self.props(n, i, scope.clone(), wrap)?;
                }
                return Ok(());
            }
            Expr::Always(a) => {
                let a = *a;
                let mut pred = Pred::new(a, scope.clone());
                if let Some(w) = wrap {
                    pred.location = w;
                }
                if matches!(ir.expr(a), Expr::StepBox { .. }) {
                    self.implied_actions.push(Named { name, pred });
                } else if self.level(a, &scope) < 2 {
                    let n = match self.lookup_zero(a, &scope) {
                        Some((op_name, _)) => op_name,
                        None => name,
                    };
                    self.invariants.push(Named { name: n, pred });
                } else {
                    let mut pred = Pred::new(e, scope);
                    if let Some(w) = wrap {
                        pred.location = w;
                    }
                    self.implied_temporals.push(Named { name, pred });
                }
                return Ok(());
            }
            Expr::Selected(a) => return self.props(name, *a, scope, wrap),
            _ => {}
        }
        let level = self.level(e, &scope);
        let mut pred = Pred::new(e, scope);
        if let Some(w) = wrap {
            pred.location = w;
        }
        match level {
            0 | 1 => self.implied_inits.push(Named { name, pred }),
            3 => self.implied_temporals.push(Named { name, pred }),
            _ => {
                let loc = self.node_string(e);
                let ec = match ir.expr(e) {
                    Expr::StepBox { .. } => Ec::TlcConfigPropertyActionLevelSquareASubV,
                    Expr::StepAngle { .. } => Ec::TlcConfigPropertyActionLevelAngleASubV,
                    _ => Ec::TlcConfigPropertyActionLevel,
                };
                return Err(BindError::tlc(ec, &[&name, &loc]));
            }
        }
        Ok(())
    }
}

impl Binder<'_> {
    /// Whether SANY's `levelParams` of an expression are not empty: measured,
    /// it holds the `CONSTANT` and `VARIABLE` declarations the expression
    /// reads outside a prime (`x' = x` and `C' = C` have one, `x' = 1` and
    /// `[]TRUE` none), through the definitions it uses.
    fn has_level_params(&self, e: ExprId) -> bool {
        let ir = self.ir();
        let mut seen = std::collections::HashSet::new();
        let mut stack = vec![e];
        while let Some(x) = stack.pop() {
            match ir.expr(x) {
                Expr::Var { primed: false, .. }
                | Expr::Const(_)
                | Expr::SubstRef { primed: false, .. } => return true,
                Expr::Var { primed: true, .. }
                | Expr::SubstRef { primed: true, .. }
                | Expr::Prime(_) => {}
                Expr::Def(d) => {
                    if seen.insert(*d) {
                        stack.push(ir.def(*d).body);
                    }
                }
                other => {
                    if let Expr::Apply {
                        callee: OpRef::Def(d),
                        ..
                    } = other
                        && seen.insert(*d)
                    {
                        stack.push(ir.def(*d).body);
                    }
                    if let Expr::Apply {
                        callee: OpRef::Const(_) | OpRef::Subst(_),
                        ..
                    } = other
                    {
                        return true;
                    }
                    stack.extend(other.children());
                }
            }
        }
        false
    }

    /// `scope` with the variables of quantifier `q` bound as in `bound`.
    fn rebase_bindings(&self, scope: &Scope, bound: &Scope, q: ExprId) -> Scope {
        let ir = self.ir();
        let Expr::Quant { bounds, .. } = ir.expr(q) else {
            return bound.clone();
        };
        let mut s = scope.clone();
        for b in bounds {
            for &slot in &b.vars {
                let r = dive_ir::LocalRef { up: 0, slot };
                if let SlotView::Value(v) = self.ev().scope_slot(bound, r) {
                    s = self.ev().scope_bind(&s, slot, v);
                }
            }
        }
        s
    }
}

/// Whether SANY represents the expression as an `OpApplNode`.
fn is_op_appl(e: &Expr) -> bool {
    !matches!(
        e,
        Expr::Num(_)
            | Expr::Decimal { .. }
            | Expr::Str(_)
            | Expr::Let { .. }
            | Expr::Subst { .. }
            | Expr::Label { .. }
            | Expr::ExceptAt
            | Expr::Unevaluable(_)
    )
}
