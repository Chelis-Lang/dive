//! Binding the configuration's constants and overrides
//! (`SpecProcessor.processSpec`) and evaluating constant definitions at
//! start-up (`processConstantDefns`), in TLC's order.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use dive_config::{ConfigValue, ConstantEntry, ModelConfig, Spanned};
use dive_eval::{ConstBinding, ErrorClass, Evaluator, Options};
use dive_ir::{DeclId, DefId, Ir, ModuleId, Symbol};
use dive_value::{Ctx, Interner, Value};

use crate::error::{BindError, Ec, ErrorKind, Message};
use crate::level::LevelCx;
use crate::model::{Init, Named, Pred};
use crate::tables::{Look, Tables};
use crate::walk::{file_modules, inner_modules};

pub(crate) type R<T> = Result<T, BindError>;

/// How the binding stage runs.
#[derive(Clone, Debug)]
pub struct BindOptions {
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
            no_deadlock: false,
            host: None,
        }
    }

    /// dive's default profile.
    #[must_use]
    pub fn dive() -> Self {
        Self {
            eval: Options::dive(),
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
    /// Diagnostic binding only: retain the configured pre-constant periodic body.
    pub(crate) periodic_snapshot: crate::periodic::Mode,
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
}

impl<'a> Binder<'a> {
    pub(crate) fn new(
        ir: Arc<Ir>,
        cfg: &'a ModelConfig,
        opts: &'a BindOptions,
        names: Arc<Interner>,
    ) -> R<Self> {
        Ok(Self {
            periodic_snapshot: crate::periodic::Mode::Ordinary,
            t: Tables::new(ir)?,
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
        })
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
                    return Err(BindError::tlc(Ec::TlcConfigOpArityInconsistent, &[name]));
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
                        ));
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
                    )));
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
                        )));
                    }
                    let v = self.cfg_value(&v.value)?;
                    self.t.tool_obj.insert(*d, Look::Value(v.clone()));
                    self.defns.insert(name.clone(), Look::Value(v));
                }
                Some(CfgConst::Op) => {
                    return Err(BindError::unsupported(format!(
                        "values for the operator {name} given in the configuration (TLC's OpRcdValue)"
                    )));
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
                        ));
                    }
                    marity.insert(a.name.text.as_str(), a.args.len());
                    return Err(BindError::unsupported(format!(
                        "module-scoped values for the operator {} given in the configuration",
                        a.name.text
                    )));
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
        self.refuse_user_java_overrides();

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
                return Err(BindError::tlc(Ec::TlcConfigRhsIdAppearedAfterLhsId, &[rhs]));
            }
            let Some(val) = self.defns.get(rhs).cloned() else {
                return Err(BindError::tlc(
                    Ec::TlcConfigWrongSubstitution,
                    &[&d.name, rhs],
                ));
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
                return Err(BindError::tlc(Ec::TlcConfigRhsIdAppearedAfterLhsId, &[rhs]));
            }
            let Some(val) = self.defns.get(rhs).cloned() else {
                return Err(BindError::tlc(Ec::TlcConfigWrongSubstitution, &[name, rhs]));
            };
            if let Look::Def(r) = &val
                && ir.def(*d).params.len() != ir.def(*r).params.len()
            {
                return Err(BindError::tlc(
                    Ec::TlcConfigWrongSubstitutionNumberOfArgs,
                    &[name, rhs],
                ));
            }
            self.t.tool_obj.insert(*d, val.clone());
            self.defns.insert(name.clone(), val);
            overridden.insert(name.clone());
        }
        // Overrides naming nothing, in the `Hashtable`'s order.
        let keys = override_keys(self.cfg, None);
        for k in hashtable_order(&keys) {
            if !overridden.contains(k.as_str()) {
                return Err(BindError::tlc(Ec::TlcConfigIdDoesNotAppearInSpec, &[&k]));
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
                    return Err(BindError::tlc(Ec::TlcConfigRhsIdAppearedAfterLhsId, &[rhs]));
                }
                let Some(val) = self.defns.get(rhs).cloned() else {
                    return Err(BindError::tlc(
                        Ec::TlcConfigWrongSubstitution,
                        &[&name, rhs],
                    ));
                };
                if let Look::Def(r) = &val
                    && ir.def(d).params.len() != ir.def(*r).params.len()
                {
                    return Err(BindError::tlc(
                        Ec::TlcConfigWrongSubstitutionNumberOfArgs,
                        &[&name, rhs],
                    ));
                }
                self.set_body_obj(d, &val);
                done.insert(name);
            }
            let keys = override_keys(self.cfg, Some(&mname));
            for k in hashtable_order(&keys) {
                if !done.contains(k.as_str()) {
                    return Err(BindError::tlc(Ec::TlcConfigIdDoesNotAppearInSpec, &[&k]));
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
                return Err(BindError::tlc(Ec::TlcNoModules, &[&k]));
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
    /// dive cannot run them, so such a model is refused.
    fn refuse_user_java_overrides(&mut self) {
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
                        ));
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
        let ev = Evaluator::new(model, self.opts.eval)
            .map_err(|e| BindError::unsupported(e.to_string()))?;
        if let Some(h) = &self.opts.host {
            ev.set_host(h.clone());
        }
        self.t.set_evaluator(ev);
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
    fn processes_constant_defns(&self, m: ModuleId) -> bool {
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
                ));
            }
            if !ir.def(def).params.is_empty() {
                continue;
            }
            match self.ev().precompute(def) {
                Ok(Some(v)) => {
                    // `opDef.setToolObject(toolId, defVal)`.
                    self.t.tool_obj.insert(def, Look::Value(v));
                }
                Ok(None) => {}
                Err(e) => {
                    let scope = self
                        .ev()
                        .scope(def)
                        .map_err(|e| BindError::unsupported(e.to_string()))?;
                    let level = LevelCx::new(&self.t).try_level(ir.def(def).body, &scope)?;
                    if level > 0 {
                        self.drain_prints();
                        // `(e instanceof EvalException) ? "" : " - specifically: " + e.getMessage()`.
                        let addendum = if e.class() != ErrorClass::Eval {
                            return Err(BindError::unsupported(format!(
                                "the message TLC prints when the definition {} that the configuration substitutes for {} fails at start-up ({e})",
                                ir.def(def).name,
                                d.name
                            )));
                        } else if e.is_eval_exception() {
                            String::new()
                        } else {
                            format!(" - specifically: {}", e.tool_text())
                        };
                        return Err(BindError::tlc(
                            Ec::TlcConfigSubstitutionNonConstant,
                            &[&d.name, &ir.def(def).name, &addendum],
                        ));
                    }
                    self.note_startup_failure(def, &e);
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
                .map_err(|e| BindError::unsupported(e.to_string()))?;
            let level = LevelCx::new(&self.t).try_level(ir.def(real).body, &scope)?;
            if level != 0 {
                continue;
            }
            match self.ev().precompute(real) {
                Ok(Some(v)) => {
                    self.t.tool_obj.insert(real, Look::Value(v.clone()));
                    let real_name = ir.def(real).name.clone();
                    if matches!(self.defns.get(&real_name), Some(Look::Def(x)) if *x == real) {
                        self.defns.insert(real_name, Look::Value(v));
                    }
                    let _ = name;
                }
                Ok(None) => {}
                Err(e) => self.note_startup_failure(real, &e),
            }
        }
        for inner in inner_modules(&ir, m) {
            self.constant_defns_of(inner)?;
        }
        Ok(())
    }

    /// A start-up evaluation failed. TLC swallows the error; dive does too,
    /// unless dive refused something TLC may have evaluated, in which case
    /// the outcome may differ if the definition is impure (it prints or
    /// draws) and the model is refused.
    fn note_startup_failure(&mut self, def: DefId, e: &dive_eval::EvalError) {
        if e.class() == ErrorClass::Eval {
            return;
        }
        let ir = &*self.t.ir;
        let what = format!(
            "the start-up evaluation of {}: {}",
            ir.def(def).name,
            e.message()
        );
        if ir.def(def).effects.intersects(dive_ir::Effects::IMPURE) {
            self.unsupported.push(what.clone());
        }
        self.refused.entry(def).or_insert(what);
    }

    /// Refuses a configuration name that names a definition whose start-up
    /// evaluation dive refused: TLC may have replaced it by a value, which
    /// changes how the name is treated.
    pub(crate) fn check_refused(&self, name: &str) -> R<()> {
        if let Some(Look::Def(d)) = self.defns.get(name)
            && let Some(why) = self.refused.get(d)
        {
            return Err(BindError::unsupported(why.clone()));
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
