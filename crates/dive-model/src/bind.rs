//! Binding the configuration's constants and overrides
//! (`SpecProcessor.processSpec`) and evaluating constant definitions at
//! start-up (`processConstantDefns`), in TLC's order.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use sha2::{Digest, Sha256};

use dive_config::{ConfigValue, ConstantEntry, ModelConfig, Spanned};
use dive_eval::{ConstBinding, ErrorClass, Evaluator, Options};
use dive_ir::{DeclId, DefId, Ir, ModuleId, StdOp, Symbol};
use dive_value::{Ctx, Interner, Value};

use crate::error::{BindError, Ec, ErrorKind, Message};
use crate::level::LevelCx;
use crate::model::{Init, Named, Pred};
use crate::tables::{Look, Tables};
use crate::walk::{file_modules, inner_modules};

pub(crate) type R<T> = Result<T, crate::control::StepError>;

/// How the binding stage runs.
#[derive(Clone, Debug)]
pub struct BindOptions {
    /// Whole-run policy for genuine temporal obligations and configured symmetry.
    pub symmetry_liveness: crate::SymmetryLiveness,
    /// The evaluator's options (the profile).
    pub eval: Options,
    /// TLC's `-deadlock` command-line flag: do not check for deadlock,
    /// whatever `CHECK_DEADLOCK` says.
    pub no_deadlock: bool,
    /// The run hosting the evaluator ([`dive_eval::Host`]), installed before
    /// TLC's start-up evaluation, so that constant definitions and `ASSUME`s
    /// draw from the run's random stream and see its registers. `None`: no
    /// host; operators that need one are refused by name.
    pub host: Option<std::sync::Arc<dyn dive_eval::Host>>,
}

impl BindOptions {
    /// `--compat=tlc`: TLC as users run it.
    #[must_use]
    pub fn tlc() -> Self {
        Self {
            eval: Options::tlc(),
            symmetry_liveness: crate::SymmetryLiveness::Tlc,
            no_deadlock: false,
            host: None,
        }
    }

    /// dive's default profile.
    #[must_use]
    pub fn dive() -> Self {
        Self {
            eval: Options::dive(),
            symmetry_liveness: crate::SymmetryLiveness::Unreduced,
            no_deadlock: false,
            host: None,
        }
    }
}

impl Default for BindOptions {
    fn default() -> Self {
        Self::dive()
    }
}

/// A constant's entry in TLC's constant table (`makeConstantTable`).
enum CfgConst<'a> {
    Plain(&'a Spanned<ConfigValue>),
    /// A constant operator given values for some arguments (`OpRcdValue`).
    Op,
}

/// The binder's state.
pub(crate) struct Binder<'a> {
    pub(crate) control: Option<Arc<dyn dive_eval::HostControl>>,
    pub(crate) t: Tables,
    pub(crate) cfg: &'a ModelConfig,
    pub(crate) opts: &'a BindOptions,
    pub(crate) names: Arc<Interner>,
    /// TLC's `defns`: the names the configuration is looked up in.
    pub(crate) defns: HashMap<String, Look>,
    pub(crate) output: Vec<Message>,
    pub(crate) init: Vec<Init>,
    pub(crate) next: Option<Pred>,
    /// The definition `NEXT` names, which TLC names the next-state relation
    /// after.
    pub(crate) next_def: Option<DefId>,
    pub(crate) invariants: Vec<Named>,
    pub(crate) implied_inits: Vec<Named>,
    pub(crate) implied_actions: Vec<Named>,
    pub(crate) temporals: Vec<Named>,
    pub(crate) implied_temporals: Vec<Named>,
    pub(crate) implied_temporal_action_names: Vec<String>,
    pub(crate) configured_properties: Vec<Named>,
    pub(crate) constraints: Vec<Named>,
    pub(crate) action_constraints: Vec<Named>,
    pub(crate) unsupported: Vec<String>,
    /// Definitions whose start-up evaluation dive refused (an unsupported
    /// feature or a resource limit): TLC may have given them a value, so
    /// whatever depends on that value is refused too.
    pub(crate) refused: HashMap<DefId, String>,
    /// Impure definitions whose start-up evaluation dive refused. Each is
    /// refused only if the checking path reaches it or its evaluation may
    /// have had a lasting effect in TLC (`reach::unsupported`).
    pub(crate) startup_refusals: Vec<StartupRefusal>,
    /// The constant definitions the start-up evaluation leaves to the core
    /// engine (`crate::defer`); `None` unless the core route asked.
    pub(crate) defer: Option<crate::defer::Deferral>,
    /// The left definitions a configured role uses, which the core
    /// evaluates.
    pub(crate) deferred: std::collections::BTreeSet<DefId>,
}

/// A refused start-up evaluation of an impure definition.
pub(crate) struct StartupRefusal {
    /// The definition.
    pub def: DefId,
    /// The refusal, naming the definition.
    pub what: String,
    /// The external-input refusal that evaluation recorded, if any; restored
    /// when the refusal is kept.
    pub input_refusal: Option<String>,
}

impl<'a> Binder<'a> {
    pub(crate) fn new(
        ir: Arc<Ir>,
        cfg: &'a ModelConfig,
        opts: &'a BindOptions,
        names: Arc<Interner>,
        control: Option<Arc<dyn dive_eval::HostControl>>,
        fallible_errors: bool,
    ) -> R<Self> {
        let mut tables = Tables::new(ir)?;
        tables.fallible_errors = fallible_errors;
        Ok(Self {
            control,
            t: tables,
            cfg,
            opts,
            names,
            defns: HashMap::new(),
            output: Vec::new(),
            init: Vec::new(),
            next: None,
            next_def: None,
            invariants: Vec::new(),
            implied_inits: Vec::new(),
            implied_actions: Vec::new(),
            temporals: Vec::new(),
            implied_temporals: Vec::new(),
            implied_temporal_action_names: Vec::new(),
            configured_properties: Vec::new(),
            constraints: Vec::new(),
            action_constraints: Vec::new(),
            unsupported: Vec::new(),
            refused: HashMap::new(),
            startup_refusals: Vec::new(),
            defer: None,
            deferred: std::collections::BTreeSet::new(),
        })
    }

    pub(crate) fn eval_error(
        &self,
        error: dive_eval::EvalError,
        to_binding: impl FnOnce(dive_eval::EvalError) -> BindError,
    ) -> crate::control::StepError {
        self.t.eval_error(error, to_binding)
    }

    pub(crate) fn propagate_interruption(&self, error: &dive_eval::EvalError) -> R<()> {
        if (self.control.is_some() || self.t.fallible_errors)
            && error.class() == ErrorClass::Interrupted
        {
            return Err(crate::control::StepError::Interrupted(error.clone()));
        }
        if self.t.fallible_errors
            && matches!(error.class(), ErrorClass::ResourceLimit | ErrorClass::Bug)
        {
            return Err(crate::control::StepError::EvaluationFailure(error.clone()));
        }
        Ok(())
    }

    pub(crate) fn ir(&self) -> &Ir {
        &self.t.ir
    }

    pub(crate) fn ctx(&self) -> Ctx {
        self.opts.eval.ctx
    }

    pub(crate) fn ev(&self) -> &Evaluator {
        self.t.ev()
    }

    /// Moves `Print` output collected by the evaluator into the output.
    pub(crate) fn drain_prints(&mut self) {
        for line in self.t.ev().take_output() {
            self.output.push(Message::Print(line));
        }
    }

    /// `Value.toString()`.
    pub(crate) fn show(&self, v: &Value) -> String {
        dive_value::print::to_tlc_string_lenient(v, self.ctx())
    }

    /// A configuration value as a value (`ModelConfig.parseValue`, then
    /// `initialize`, which normalizes sets).
    fn cfg_value(&self, v: &ConfigValue) -> R<Value> {
        Ok(match v {
            ConfigValue::Int(i) => Value::Int(i64::from(*i)),
            ConfigValue::String(s) => Value::string(&self.names, s),
            ConfigValue::Bool(b) => Value::Bool(*b),
            ConfigValue::ModelValue(m) => Value::model(&self.names, m),
            ConfigValue::Set(items) => {
                let mut out = Vec::with_capacity(items.len());
                for i in items {
                    out.push(self.cfg_value(&i.value)?);
                }
                Value::set(out, self.ctx()).map_err(|e| BindError {
                    kind: ErrorKind::Eval { code: 1000 },
                    message: e.message().to_owned(),
                })?
            }
        })
    }

    // -------------------------------------------------------------------
    // processSpec: constants and overrides
    // -------------------------------------------------------------------

    /// `SpecProcessor.processSpec` from `initializeConstants` on.
    #[allow(clippy::too_many_lines, reason = "TLC's sequence, step by step")]
    pub(crate) fn process_spec(&mut self) -> R<()> {
        let ir = self.t.ir.clone();
        let root = ir.root();
        // Built-in names (`processSpec`: TRUE, FALSE, BOOLEAN, the Strings
        // class's methods).
        self.defns
            .insert("TRUE".into(), Look::Value(Value::Bool(true)));
        self.defns
            .insert("FALSE".into(), Look::Value(Value::Bool(false)));
        self.defns.insert(
            "BOOLEAN".into(),
            Look::Value(Value::boolean_set(self.ctx())),
        );
        self.defns
            .insert("STRING".into(), Look::Value(Value::string_set(self.ctx())));

        // `initializeConstants` / `makeConstantTable`.
        let mut table: HashMap<&str, CfgConst<'_>> = HashMap::new();
        let mut arities: HashMap<&str, usize> = HashMap::new();
        for a in self.cfg.assignments() {
            let name = a.name.text.as_str();
            if a.args.is_empty() {
                table.insert(name, CfgConst::Plain(a.value));
            } else {
                if let Some(&n) = arities.get(name)
                    && n != a.args.len()
                {
                    return Err(BindError::tlc(Ec::TlcConfigOpArityInconsistent, &[name]).into());
                }
                arities.insert(name, a.args.len());
                table.insert(name, CfgConst::Op);
            }
        }
        let overrides = self.cfg.substitutions();

        // Root constants, in `getConstantDecls` order.
        let root_consts: Vec<DeclId> = ir.module(root).tlc_constants.clone();
        for &decl in &root_consts {
            let d = ir.decl(decl);
            let Some(c) = d.constant else { continue };
            match table.get(d.name.as_str()) {
                None => {
                    if !overrides.contains_key(d.name.as_str()) {
                        return Err(BindError::tlc(
                            Ec::TlcConfigValueNotAssignedToConstantParam,
                            &[&d.name],
                        )
                        .into());
                    }
                }
                Some(CfgConst::Plain(v)) => {
                    let v = self.cfg_value(&v.value)?;
                    self.t.consts[c.index()] = Some(Look::Value(v.clone()));
                    self.defns.insert(d.name.clone(), Look::Value(v));
                }
                Some(CfgConst::Op) => {
                    return Err(BindError::unsupported(format!(
                        "values for the constant operator {} given in the configuration (TLC's OpRcdValue)",
                        d.name
                    )).into());
                }
            }
        }

        // Root definitions, with configuration values (`Op = v`).
        let root_defs = self.t.op_defs(root);
        for (name, d) in &root_defs {
            match table.get(name.as_str()) {
                None => {
                    self.defns.insert(name.clone(), Look::Def(*d));
                }
                Some(CfgConst::Plain(v)) => {
                    if !ir.def(*d).params.is_empty() {
                        return Err(BindError::unsupported(format!(
                            "a value given in the configuration for the operator {name}, which takes arguments"
                        )).into());
                    }
                    let v = self.cfg_value(&v.value)?;
                    self.t.tool_obj.insert(*d, Look::Value(v.clone()));
                    self.defns.insert(name.clone(), Look::Value(v));
                }
                Some(CfgConst::Op) => {
                    return Err(BindError::unsupported(format!(
                        "values for the operator {name} given in the configuration (TLC's OpRcdValue)"
                    )).into());
                }
            }
        }

        // Module-scoped values (`C = [M] v`), on definitions' bodies.
        let mod_assign = self.cfg.module_assignments();
        for m in file_modules(&ir) {
            let mname = ir.module(m).name.clone();
            let Some(entries) = mod_assign.get(mname.as_str()) else {
                continue;
            };
            let mut mtable: HashMap<&str, &Spanned<ConfigValue>> = HashMap::new();
            let mut marity: HashMap<&str, usize> = HashMap::new();
            for a in entries {
                if !a.args.is_empty() {
                    if let Some(&n) = marity.get(a.name.text.as_str())
                        && n != a.args.len()
                    {
                        return Err(BindError::tlc(
                            Ec::TlcConfigOpArityInconsistent,
                            &[&a.name.text],
                        )
                        .into());
                    }
                    marity.insert(a.name.text.as_str(), a.args.len());
                    return Err(BindError::unsupported(format!(
                        "module-scoped values for the operator {} given in the configuration",
                        a.name.text
                    ))
                    .into());
                }
                mtable.insert(a.name.text.as_str(), a.value);
            }
            for (name, d) in self.t.op_defs(m) {
                if let Some(v) = mtable.get(name.as_str()) {
                    let v = self.cfg_value(&v.value)?;
                    self.set_body_obj(d, &Look::Value(v));
                }
            }
        }

        // Java overrides (`processModuleOverrides`) enter `defns` by name.
        for (i, def) in ir.defs().iter().enumerate() {
            if let Some(op) = def.std
                && def.source.is_none()
            {
                self.defns.insert(
                    def.name.clone(),
                    Look::Java(op, DefId(u32::try_from(i).unwrap_or(u32::MAX))),
                );
            }
        }
        self.install_user_java_overrides();

        // Overrides of root constants.
        let mut overridden: HashSet<String> = HashSet::new();
        for &decl in &root_consts {
            let d = ir.decl(decl);
            let Some(c) = d.constant else { continue };
            let Some(rhs) = overrides.get(d.name.as_str()) else {
                continue;
            };
            let rhs = rhs.text.as_str();
            if overrides.contains_key(rhs) {
                return Err(BindError::tlc(Ec::TlcConfigRhsIdAppearedAfterLhsId, &[rhs]).into());
            }
            let Some(val) = self.defns.get(rhs).cloned() else {
                return Err(BindError::tlc(Ec::TlcConfigWrongSubstitution, &[&d.name, rhs]).into());
            };
            self.t.consts[c.index()] = Some(val.clone());
            self.defns.insert(d.name.clone(), val);
            overridden.insert(d.name.clone());
        }
        // Overrides of root definitions.
        for (name, d) in &root_defs {
            let Some(rhs) = overrides.get(name.as_str()) else {
                continue;
            };
            let rhs = rhs.text.as_str();
            if overrides.contains_key(rhs) {
                return Err(BindError::tlc(Ec::TlcConfigRhsIdAppearedAfterLhsId, &[rhs]).into());
            }
            let Some(val) = self.defns.get(rhs).cloned() else {
                return Err(BindError::tlc(Ec::TlcConfigWrongSubstitution, &[name, rhs]).into());
            };
            if let Look::Def(r) = &val
                && ir.def(*d).params.len() != ir.def(*r).params.len()
            {
                return Err(BindError::tlc(
                    Ec::TlcConfigWrongSubstitutionNumberOfArgs,
                    &[name, rhs],
                )
                .into());
            }
            self.t.tool_obj.insert(*d, val.clone());
            self.defns.insert(name.clone(), val);
            overridden.insert(name.clone());
        }
        // Overrides naming nothing, in the `Hashtable`'s order.
        let keys = override_keys(self.cfg, None);
        for k in hashtable_order(&keys) {
            if !overridden.contains(k.as_str()) {
                return Err(BindError::tlc(Ec::TlcConfigIdDoesNotAppearInSpec, &[&k]).into());
            }
        }

        // Module-scoped overrides (`C <- [M] D`), on definitions' bodies.
        let mod_over = self.cfg.module_substitutions();
        let module_names: Vec<String> = file_modules(&ir)
            .iter()
            .map(|&m| ir.module(m).name.clone())
            .collect();
        for m in file_modules(&ir) {
            let mname = ir.module(m).name.clone();
            let Some(mdefs) = mod_over.get(mname.as_str()) else {
                continue;
            };
            let mut done: HashSet<String> = HashSet::new();
            for (name, d) in self.t.op_defs(m) {
                let Some(rhs) = mdefs.get(name.as_str()) else {
                    continue;
                };
                let rhs = rhs.text.as_str();
                if mdefs.contains_key(rhs) {
                    return Err(BindError::tlc(Ec::TlcConfigRhsIdAppearedAfterLhsId, &[rhs]).into());
                }
                let Some(val) = self.defns.get(rhs).cloned() else {
                    return Err(
                        BindError::tlc(Ec::TlcConfigWrongSubstitution, &[&name, rhs]).into(),
                    );
                };
                if let Look::Def(r) = &val
                    && ir.def(d).params.len() != ir.def(*r).params.len()
                {
                    return Err(BindError::tlc(
                        Ec::TlcConfigWrongSubstitutionNumberOfArgs,
                        &[&name, rhs],
                    )
                    .into());
                }
                self.set_body_obj(d, &val);
                done.insert(name);
            }
            let keys = override_keys(self.cfg, Some(&mname));
            for k in hashtable_order(&keys) {
                if !done.contains(k.as_str()) {
                    return Err(BindError::tlc(Ec::TlcConfigIdDoesNotAppearInSpec, &[&k]).into());
                }
            }
        }
        let mut mkeys: Vec<String> = Vec::new();
        for e in &self.cfg.constants {
            if let ConstantEntry::Substitution {
                module: Some(m), ..
            } = e
                && !mkeys.contains(&m.text)
            {
                mkeys.push(m.text.clone());
            }
        }
        for k in hashtable_order(&mkeys) {
            if !module_names.contains(&k) {
                return Err(BindError::tlc(Ec::TlcNoModules, &[&k]).into());
            }
        }
        Ok(())
    }

    /// Sets the tool object of `d`'s body. TLC's lookup of an instantiated
    /// copy (`I!Op`, whose body is `d`'s under the instance's substitutions)
    /// looks through the substitutions and finds it too.
    fn set_body_obj(&mut self, d: DefId, l: &Look) {
        let ir = self.t.ir.clone();
        for (i, def) in ir.defs().iter().enumerate() {
            let mut cur = def.source;
            let mut hops = 0;
            let mut hit = i == d.index();
            while let Some(s) = cur {
                if s == d {
                    hit = true;
                    break;
                }
                hops += 1;
                if hops > 64 {
                    break;
                }
                cur = ir.def(s).source;
            }
            if hit {
                self.t
                    .body_obj
                    .insert(DefId(u32::try_from(i).unwrap_or(u32::MAX)), l.clone());
            }
        }
    }

    /// TLC loads a Java class named after a loaded module from the module's
    /// directory and uses its methods instead of the module's definitions
    /// (`TLAClass.loadClass`, the first phase of `processModuleOverrides`).
    /// Only the pinned Hanoi Bits class has a native port; others are refused.
    fn install_user_java_overrides(&mut self) {
        let ir = self.t.ir.clone();
        for m in file_modules(&ir) {
            let module = ir.module(m);
            if module.standard {
                continue;
            }
            let Some(f) = module.file else { continue };
            let Some(info) = ir.files().get(f.index()) else {
                continue;
            };
            let Some(dir) = Path::new(&info.display).parent() else {
                continue;
            };
            let class = dir.join(format!("{}.class", module.name));
            if class.is_file() {
                if module.name == "Bits" && known_bits_class(&class) {
                    let defs: Vec<_> = ir
                        .defs()
                        .iter()
                        .enumerate()
                        .filter_map(|(i, def)| {
                            (def.module == m && def.name == "And" && def.source.is_none())
                                .then_some(DefId(u32::try_from(i).unwrap_or(u32::MAX)))
                        })
                        .collect();
                    for d in defs {
                        let native = Arc::make_mut(&mut self.t.ir);
                        if native.set_native_override(d, StdOp::BitsAnd) {
                            // TLC installs the same body override on instantiated copies.
                            for i in 0..native.defs().len() {
                                let id = DefId(u32::try_from(i).unwrap_or(u32::MAX));
                                let mut source = id;
                                for _ in 0..=native.defs().len() {
                                    if source == d {
                                        native.set_native_override(id, StdOp::BitsAnd);
                                        break;
                                    }
                                    let Some(next) = native.def(source).source else {
                                        break;
                                    };
                                    source = next;
                                }
                            }
                            self.defns
                                .insert("And".into(), Look::Java(StdOp::BitsAnd, d));
                        } else {
                            self.unsupported.push("Bits!And (local class signature does not match four value parameters)".into());
                        }
                    }
                    continue;
                }
                self.unsupported.push(format!(
                    "a Java class overriding module {} ({}.class)",
                    module.name, module.name
                ));
            }
        }
    }

    /// Builds the evaluator once the constants and overrides are bound.
    pub(crate) fn build_evaluator(&mut self) -> R<()> {
        let ir = self.t.ir.clone();
        let mut model = dive_eval::Model::new(ir.clone());
        model.names = self.names.clone();
        for (i, slot) in self.t.consts.iter().enumerate() {
            model.constants[i] = match slot {
                Some(Look::Value(v)) => Some(ConstBinding::Value(v.clone())),
                Some(Look::Def(d) | Look::Java(_, d)) => {
                    let constant = ir.decl(ir.consts()[i]);
                    if ir.def(*d).params.len() != constant.arity as usize {
                        return Err(BindError::tlc(
                            Ec::TlcConfigWrongSubstitutionNumberOfArgs,
                            &[&constant.name, &ir.def(*d).name],
                        )
                        .into());
                    }
                    Some(ConstBinding::Def(*d))
                }
                Some(Look::Literal(_)) | None => None,
            };
        }
        for map in [&self.t.tool_obj, &self.t.body_obj] {
            for (d, l) in map {
                match l {
                    Look::Value(v) => {
                        model.def_values.entry(*d).or_insert_with(|| v.clone());
                    }
                    Look::Def(r) | Look::Java(_, r) => {
                        model.redirects.entry(*d).or_insert(*r);
                    }
                    Look::Literal(_) => {}
                }
            }
        }
        // Construction returns ContextError, not an intrinsic EvalError.
        let ev = Evaluator::new(model, self.opts.eval)
            .map_err(|e| BindError::unsupported(e.to_string()))?;
        if let Some(h) = &self.opts.host {
            ev.set_host(h.clone());
        }
        self.t.set_evaluator(ev, self.control.is_some());
        Ok(())
    }

    // -------------------------------------------------------------------
    // processConstantDefns
    // -------------------------------------------------------------------

    /// `SpecProcessor.processConstantDefns`: for every module of the module
    /// table (and its inner modules), the constants the configuration binds
    /// to definitions, then every constant-level zero-arity definition, are
    /// evaluated, and the values kept. Failures are swallowed.
    pub(crate) fn process_constant_defns(&mut self) -> R<()> {
        if self.defer.is_some() {
            self.defer = Some(crate::defer::candidates(self));
        }
        let ir = self.t.ir.clone();
        for m in file_modules(&ir) {
            if self.processes_constant_defns(m) {
                self.constant_defns_of(m)?;
            }
        }
        self.drain_prints();
        Ok(())
    }

    /// `ModuleNode.processConstantDefns`: not for a module that is
    /// instantiated and has parameters.
    pub(crate) fn processes_constant_defns(&self, m: ModuleId) -> bool {
        let ir = &*self.t.ir;
        let module = ir.module(m);
        if !module.instantiated {
            return true;
        }
        let has_params = module.context.iter().any(|(_, s)| {
            matches!(s, Symbol::Decl(d) if {
                let decl = ir.decl(*d);
                decl.variable || decl.constant.is_some()
            })
        });
        !has_params
    }

    fn constant_defns_of(&mut self, m: ModuleId) -> R<()> {
        let ir = self.t.ir.clone();
        // Constants bound to definitions (`C <- D`).
        for &decl in &ir.module(m).tlc_constants.clone() {
            let d = ir.decl(decl);
            let Some(c) = d.constant else { continue };
            let Some(Look::Def(def)) = self.t.const_look(c) else {
                continue;
            };
            if ir.def(def).params.len() != d.arity as usize {
                return Err(BindError::tlc(
                    Ec::TlcConfigWrongSubstitutionNumberOfArgs,
                    &[&d.name, &ir.def(def).name],
                )
                .into());
            }
            if !ir.def(def).params.is_empty() {
                continue;
            }
            let (result, input_refusal) = self.startup_precompute(def);
            match result {
                Ok(Some(v)) => {
                    // `opDef.setToolObject(toolId, defVal)`.
                    self.t.tool_obj.insert(def, Look::Value(v));
                }
                Ok(None) => {}
                Err(e) => {
                    self.propagate_interruption(&e)?;
                    let scope = self.ev().scope(def).map_err(|e| {
                        self.eval_error(e, |e| BindError::unsupported(e.to_string()))
                    })?;
                    let level = LevelCx::new(&self.t).try_level(ir.def(def).body, &scope)?;
                    if level > 0 {
                        self.drain_prints();
                        // `(e instanceof EvalException) ? "" : " - specifically: " + e.getMessage()`.
                        let addendum = if e.class() != ErrorClass::Eval {
                            return Err(BindError::unsupported(format!(
                                "the message TLC prints when the definition {} that the configuration substitutes for {} fails at start-up ({e})",
                                ir.def(def).name,
                                d.name
                            )).into());
                        } else if e.is_eval_exception() {
                            String::new()
                        } else {
                            format!(" - specifically: {}", e.tool_text())
                        };
                        return Err(BindError::tlc(
                            Ec::TlcConfigSubstitutionNonConstant,
                            &[&d.name, &ir.def(def).name, &addendum],
                        )
                        .into());
                    }
                    self.note_startup_failure(def, &e, input_refusal)?;
                }
            }
        }
        // Constant-level definitions.
        for (name, d) in self.t.op_defs(m) {
            let def = ir.def(d);
            if !def.params.is_empty() {
                continue;
            }
            let origin = def.module;
            if !self.processes_constant_defns(origin) {
                continue;
            }
            let real = match self.t.tool_obj.get(&d) {
                Some(Look::Def(r)) => *r,
                Some(_) => continue,
                None => match self.t.lookup_def(d) {
                    Look::Def(r) => r,
                    Look::Value(_) | Look::Java(..) | Look::Literal(_) => continue,
                },
            };
            let scope = self
                .ev()
                .scope(real)
                .map_err(|e| self.eval_error(e, |e| BindError::unsupported(e.to_string())))?;
            let level = LevelCx::new(&self.t).try_level(ir.def(real).body, &scope)?;
            if level != 0 {
                continue;
            }
            if self.left_to_core(real) {
                continue;
            }
            let _ = name;
            self.keep_startup_value(real)?;
        }
        for inner in inner_modules(&ir, m) {
            self.constant_defns_of(inner)?;
        }
        Ok(())
    }

    /// Evaluates `real` at start-up and keeps its value, as
    /// `processConstantDefns` does; a failure is swallowed as TLC swallows it
    /// ([`Binder::note_startup_failure`]).
    fn keep_startup_value(&mut self, real: DefId) -> R<()> {
        let (result, input_refusal) = self.startup_precompute(real);
        match result {
            Ok(Some(v)) => {
                self.t.tool_obj.insert(real, Look::Value(v.clone()));
                let real_name = self.t.ir.def(real).name.clone();
                if matches!(self.defns.get(&real_name), Some(Look::Def(x)) if *x == real) {
                    self.defns.insert(real_name, Look::Value(v));
                }
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(e) => self.note_startup_failure(real, &e, input_refusal),
        }
    }

    /// Evaluates, as `processConstantDefns` would have, the definitions left
    /// to the core that the dive-eval evaluator may evaluate after binding
    /// on the temporal route (`crate::defer::evaluator_temporal`), and
    /// returns them: they are no longer left to the core.
    pub(crate) fn keep_temporal_startup_values(
        &mut self,
        model: &crate::Model,
    ) -> R<std::collections::BTreeSet<DefId>> {
        let Some(defer) = &self.defer else {
            return Ok(std::collections::BTreeSet::new());
        };
        if model.implied_temporals.is_empty() || defer.skip.is_empty() {
            return Ok(std::collections::BTreeSet::new());
        }
        let reached = crate::defer::evaluator_temporal(self, model);
        let settled: std::collections::BTreeSet<DefId> = defer
            .skip
            .iter()
            .copied()
            .filter(|d| reached.contains(d))
            .collect();
        for &d in &settled {
            self.keep_startup_value(d)?;
        }
        if let Some(defer) = &mut self.defer {
            for d in &settled {
                defer.skip.remove(d);
                defer.evaluate.remove(d);
            }
        }
        for d in &settled {
            self.deferred.remove(d);
        }
        Ok(settled)
    }

    /// Whether the start-up evaluation of `def` is left to the core
    /// (`crate::defer`), which evaluates it when it compiles its plan if a
    /// role uses it.
    fn left_to_core(&mut self, def: DefId) -> bool {
        let Some(defer) = &self.defer else {
            return false;
        };
        if !defer.skip.contains(&def) {
            return false;
        }
        if defer.evaluate.contains(&def) {
            self.deferred.insert(def);
        }
        true
    }

    /// Precomputes `def` at start-up, returning the external-input refusal
    /// this evaluation recorded separately. A successful evaluation's refusal
    /// is restored at once; a failed one's goes to
    /// [`Binder::note_startup_failure`].
    fn startup_precompute(
        &self,
        def: DefId,
    ) -> (Result<Option<Value>, dive_eval::EvalError>, Option<String>) {
        let ev = self.ev();
        let prior = ev.take_external_input_refusal();
        let result = ev.precompute(def);
        let mine = ev.take_external_input_refusal();
        ev.restore_external_input_refusal(prior);
        if result.is_ok() {
            ev.restore_external_input_refusal(mine);
            return (result, None);
        }
        (result, mine)
    }

    /// A start-up evaluation failed. TLC swallows the error; dive does too,
    /// unless dive refused something TLC may have evaluated, in which case
    /// the outcome may differ if the definition is impure (it prints or
    /// draws, or the checking path reaches it) and the model is refused.
    /// That decision waits for the checking path (`reach::unsupported`).
    fn note_startup_failure(
        &mut self,
        def: DefId,
        e: &dive_eval::EvalError,
        input_refusal: Option<String>,
    ) -> R<()> {
        self.propagate_interruption(e)?;
        if e.class() == ErrorClass::Eval {
            self.ev().restore_external_input_refusal(input_refusal);
            return Ok(());
        }
        let ir = &*self.t.ir;
        let what = format!(
            "the start-up evaluation of {}: {}",
            ir.def(def).name,
            e.message()
        );
        if ir.def(def).effects.intersects(dive_ir::Effects::IMPURE) {
            self.startup_refusals.push(StartupRefusal {
                def,
                what: what.clone(),
                input_refusal,
            });
        } else {
            self.ev().restore_external_input_refusal(input_refusal);
        }
        self.refused.entry(def).or_insert(what);
        Ok(())
    }

    /// Refuses a configuration name that names a definition whose start-up
    /// evaluation dive refused: TLC may have replaced it by a value, which
    /// changes how the name is treated.
    pub(crate) fn check_refused(&self, name: &str) -> R<()> {
        if let Some(Look::Def(d)) = self.defns.get(name)
            && let Some(why) = self.refused.get(d)
        {
            return Err(BindError::unsupported(why.clone()).into());
        }
        Ok(())
    }

    /// A configuration name looked up as TLC does (`defns.get(name)`).
    pub(crate) fn defn(&self, name: &str) -> Option<Look> {
        self.defns.get(name).cloned()
    }
}

/// The keys of the configuration's `<-` table for the root module (`None`)
/// or a module, in first-insertion order.
fn override_keys(cfg: &ModelConfig, module: Option<&str>) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    for e in &cfg.constants {
        if let ConstantEntry::Substitution {
            name, module: m, ..
        } = e
            && m.as_ref().map(|n| n.text.as_str()) == module
            && !keys.contains(&name.text)
        {
            keys.push(name.text.clone());
        }
    }
    keys
}

/// The keys of a `java.util.Hashtable<String, _>` built by putting `keys` in
/// order, in its enumeration order (`Hashtable.keys()`).
fn hashtable_order(keys: &[String]) -> Vec<String> {
    let hashes: Vec<i32> = keys
        .iter()
        .map(|k| dive_ir::hashtable::java_string_hash(k))
        .collect();
    dive_ir::hashtable::enumeration_order(&hashes)
        .into_iter()
        .filter_map(|i| keys.get(i).cloned())
        .collect()
}

/// SHA-256 of the supported local `Bits.class` from the pinned Examples Hanoi model.
///
/// Class presence and this exact digest establish the local input identity;
/// installation separately validates `And`'s four value parameters.
pub const HANOI_BITS_CLASS_SHA256: [u8; 32] = [
    0x22, 0x5d, 0x2d, 0x3d, 0x16, 0x3b, 0x5a, 0x2c, 0x29, 0x23, 0x54, 0x4d, 0xe6, 0x19, 0xfb, 0x6f,
    0x23, 0xcb, 0x86, 0x80, 0x4e, 0xd0, 0x6b, 0xd2, 0x84, 0x2c, 0xa2, 0x7a, 0x24, 0x2b, 0x84, 0x6b,
];

/// Hash local classes with bounded memory; unreadable bytes remain unsupported.
fn known_bits_class(path: &Path) -> bool {
    class_digest(path).is_ok_and(|digest| digest == HANOI_BITS_CLASS_SHA256)
}

fn class_digest(path: &Path) -> std::io::Result<[u8; 32]> {
    let mut file = std::fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            return Ok(hash.finalize().into());
        }
        hash.update(&buffer[..n]);
    }
}

/// SHA-256 identities of adjacent local Java classes, sorted by module name.
///
/// Only file-level nonstandard modules participate. Class bytes are streamed
/// through a fixed-size buffer; no Java code or TLA+ expression is evaluated.
/// An empty result preserves the identity of sources with no local classes.
///
/// # Errors
///
/// Returns a named refusal if an existing class cannot be read completely.
/// This does not establish whether a class has a supported native port.
pub fn local_java_class_digests(ir: &Ir) -> Result<Vec<(String, [u8; 32])>, String> {
    let mut digests = Vec::new();
    for m in file_modules(ir) {
        let module = ir.module(m);
        if module.standard {
            continue;
        }
        let Some(file) = module.file.and_then(|f| ir.files().get(f.index())) else {
            continue;
        };
        let Some(dir) = Path::new(&file.display).parent() else {
            continue;
        };
        let path = dir.join(format!("{}.class", module.name));
        if path.is_file() {
            let digest = class_digest(&path).map_err(|error| {
                format!(
                    "a Java class overriding module {} ({}.class): {error}",
                    module.name, module.name
                )
            })?;
            digests.push((module.name.clone(), digest));
        }
    }
    digests.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    Ok(digests)
}
