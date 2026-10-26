//! The plan compiler (`04-plan-compiler.md`): `dive_ir` plus the binder
//! handoff to a [`Plan`].
//!
//! Passes, in one walk:
//!
//! 1. **Static resolution.** Configuration redirects (`Op <- D`), startup
//!    values of constant definitions and constants become pool constants.
//! 2. **Operator inlining.** Every non-recursive user operator is inlined.
//!    Each argument is substituted directly when it is a literal, a constant,
//!    a bound variable or a state read (pure and unable to fail); otherwise it
//!    becomes a thunk slot, forced at most once per activation; an argument
//!    that reads primed variables is re-evaluated by name, as TLC's cache
//!    check would see a different successor buffer. Zero-arity `LET`
//!    definitions are thunks, re-enumerated by name in action position
//!    (TLC binds no `LET` in the generator). Recursive operators are
//!    compiled once into their own frame.
//! 3. **Slot resolution.** Locals become offsets in one flat frame per role.
//! 4. **Specialization.** Arithmetic, comparisons, `x' = e`, `x' \in S`,
//!    `UNCHANGED` of variable tuples, record fields by constant name, and
//!    membership in symbolic sets get their own node kinds.
//! 5. **Constant folding.** A closed subexpression (no state read, no free
//!    slot) is evaluated once at compile time; if it fails it is left alone,
//!    so the error surfaces where TLC raises it.
//! 6. **Read sets and memo sites.** Every node records the variables and
//!    free slots it reads; invariant and constraint conjuncts and action
//!    guards become memo sites keyed on them.
//!
//! Anything else is refused, naming the construct.

use std::collections::HashMap;
use std::sync::Arc;

use dive_ir::{
    Arg, Bound, BuiltinOp, DefId, DefScope, Expr, ExprId, Ir, JunctionKind, Level, LocalRef, OpRef,
    QuantKind, Slot, StdOp,
};
use dive_value::Value;

use crate::Error;
use crate::eval::Machine;
use crate::handle::Handle;
use crate::memo::{MAX_KEY, Memo};
use crate::node::Kind;
use crate::opts::Opts;
use crate::plan::{
    Action, Arith, Bind, Check, Cmp, Code, CompileStats, Conjunct, Fun, G, GNode, MemoSite, Node,
    P, Plan, Quant, SetOp, StdFn, Update,
};
use crate::store::Store;

#[path = "compile_gen.rs"]
mod generator;

/// The largest plan the compiler builds (inlining blow-up guard).
const MAX_NODES: usize = 4_000_000;

/// The most loop iterations one constant fold may spend (R9). A larger
/// closed expression, such as `CoffeeCan`'s initial filter over 1,002,001
/// records, is left to run time, where the generator streams it instead of
/// building it in the compile arena.
pub const FOLD_FUEL: u64 = 1 << 12;

/// The most loop iterations all constant folds of one compile may spend
/// together: the compile-time budget (R9). Once it is spent, nothing more
/// is folded.
pub const FOLD_BUDGET: u64 = 1 << 22;

/// A role as the reference binder split it: an expression evaluated in a
/// definition's frame, with some of that frame's slots bound to values.
#[derive(Clone, Debug)]
pub struct Role {
    /// The name TLC reports.
    pub name: String,
    /// The expression.
    pub expr: ExprId,
    /// The definition whose frame it is evaluated in.
    pub def: DefId,
    /// Slots of that frame bound by splitting (`\E` variables and inlined
    /// parameters), with their values.
    pub slots: Vec<(Slot, Value)>,
}

/// A configured constant.
#[derive(Clone, Debug)]
pub enum ConstBinding {
    /// `C = v`.
    Value(Value),
    /// `C <- D`.
    Def(DefId),
}

/// What the reference binder hands to the core (`04-plan-compiler.md` §2).
#[derive(Clone, Debug)]
pub struct Handoff<'a> {
    /// The lowered specification.
    pub ir: &'a Ir,
    /// The value semantics the startup values were built under.
    pub ctx: dive_value::Ctx,
    /// Constants, by `ConstId`.
    pub constants: Vec<Option<ConstBinding>>,
    /// `Op <- D` redirects, resolved to their final target.
    pub redirects: HashMap<DefId, DefId>,
    /// Redirects of Java-overridden operators (`Nat <- NatOverride`).
    pub std_redirects: HashMap<StdOp, DefId>,
    /// Startup values of constant definitions.
    pub precomputed: HashMap<DefId, Value>,
    /// Initial predicates.
    pub init: Vec<Role>,
    /// Next-state actions, split, in TLC's order.
    pub next: Vec<Role>,
    /// `INVARIANT`s.
    pub invariants: Vec<Role>,
    /// `CONSTRAINT`s.
    pub constraints: Vec<Role>,
    /// `ACTION_CONSTRAINT`s, evaluated on a step: unprimed variables read
    /// the parent, primed ones the successor.
    pub action_constraints: Vec<Role>,
    /// Whether deadlock is checked.
    pub check_deadlock: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Plain,
    Primed,
}

#[derive(Clone, Copy, Debug)]
enum ArgHow {
    Alias(P),
    Thunk(u32, P),
    ByName,
}

#[derive(Clone, Debug)]
enum Sb {
    Unset,
    Flat(u32),
    Arg {
        expr: ExprId,
        env: usize,
        how: ArgHow,
    },
    LetVal {
        def: DefId,
        env: usize,
        thunk: Option<(u32, P)>,
        owner: usize,
    },
}

/// Saved slot bindings: environment, slot, old binding.
type Saved = Vec<(usize, usize, Sb)>;

#[derive(Clone, Debug)]
struct EnvFrame {
    def: DefId,
    slots: Vec<Sb>,
    parent: Option<usize>,
}

#[derive(Clone, Debug, Default)]
struct Info {
    vars: u64,
    primed: u64,
    free: Vec<u32>,
    calls: bool,
    /// Contains a fold that ran out of fuel: enclosing folds would too.
    heavy: bool,
}

fn union(a: &mut Vec<u32>, b: &[u32]) {
    if b.is_empty() {
        return;
    }
    a.extend_from_slice(b);
    a.sort_unstable();
    a.dedup();
}

fn minus(a: &mut Vec<u32>, b: &[u32]) {
    a.retain(|x| !b.contains(x));
}

/// Compiles the handoff into a plan.
///
/// # Errors
/// [`crate::ErrorKind::Unsupported`] naming the first construct the M0 core
/// does not implement.
pub fn compile(h: &Handoff<'_>, store: Arc<Store>, opts: Opts) -> Result<Plan, Error> {
    let nvars = h.ir.vars().len();
    if nvars > 64 {
        return Err(Error::unsupported("more than 64 state variables (M0 core)"));
    }
    let mut c = Compiler {
        h,
        ir: h.ir,
        code: Code::default(),
        info: Vec::new(),
        envs: Vec::new(),
        m: Machine::new(store, nvars, opts, Memo::default()),
        consts: HashMap::new(),
        pre: HashMap::new(),
        next_slot: 0,
        reset_stack: Vec::new(),
        at_stack: Vec::new(),
        init_mode: false,
        funs: HashMap::new(),
        def_cache: HashMap::new(),
        opts,
        stats: CompileStats::default(),
        site_names: Vec::new(),
        role_name: String::new(),
    };
    let mut init = Vec::new();
    for r in &h.init {
        init.push(c.action(r, true)?);
    }
    let mut next = Vec::new();
    for r in &h.next {
        next.push(c.action(r, false)?);
    }
    let mut invariants = Vec::new();
    for r in &h.invariants {
        invariants.push(c.check(r)?);
    }
    let mut constraints = Vec::new();
    for r in &h.constraints {
        constraints.push(c.check(r)?);
    }
    let mut action_constraints = Vec::new();
    for r in &h.action_constraints {
        let mut k = c.check(r)?;
        // The parent-delta skip compares a state with its parent; a step
        // predicate has no such parent.
        k.conjuncts.iter_mut().for_each(|j| j.skippable = false);
        action_constraints.push(k);
    }
    c.stats.nodes = c.code.nodes.len();
    c.stats.gnodes = c.code.gnodes.len();
    let vars =
        h.ir.vars()
            .iter()
            .map(|&d| h.ir.decl(d).name.clone())
            .collect();
    if c.m.heap.poisoned() {
        return Err(Error::internal("a stale handle was read while compiling"));
    }
    Ok(Plan {
        code: c.code,
        vars,
        init,
        next,
        invariants,
        constraints,
        action_constraints,
        check_deadlock: h.check_deadlock,
        stats: c.stats,
    })
}

struct Compiler<'a> {
    h: &'a Handoff<'a>,
    ir: &'a Ir,
    code: Code,
    info: Vec<Info>,
    envs: Vec<EnvFrame>,
    m: Machine,
    consts: HashMap<usize, Handle>,
    pre: HashMap<DefId, Handle>,
    next_slot: u32,
    reset_stack: Vec<Vec<u32>>,
    at_stack: Vec<u32>,
    init_mode: bool,
    funs: HashMap<(DefId, bool), u32>,
    /// Zero-arity top-level definitions of state level, cached once per
    /// frame activation: their thunk slot and body.
    def_cache: HashMap<DefId, (u32, P)>,
    opts: Opts,
    stats: CompileStats,
    site_names: Vec<String>,
    role_name: String,
}

impl Compiler<'_> {
    fn alloc(&mut self) -> u32 {
        let s = self.next_slot;
        self.next_slot += 1;
        s
    }

    fn redirect(&self, d: DefId) -> DefId {
        let mut d = d;
        for _ in 0..64 {
            match self.h.redirects.get(&d) {
                Some(&t) if t != d => d = t,
                _ => break,
            }
        }
        d
    }

    fn convert(&mut self, v: &Value) -> Result<Handle, Error> {
        crate::convert::from_value(&mut self.m.heap, v, self.h.ctx)
    }

    fn precomputed(&mut self, d: DefId) -> Result<Option<Handle>, Error> {
        if let Some(&h) = self.pre.get(&d) {
            return Ok(Some(h));
        }
        let Some(v) = self.h.precomputed.get(&d) else {
            return Ok(None);
        };
        let h = self.convert(v)?;
        self.pre.insert(d, h);
        Ok(Some(h))
    }

    fn new_env(&mut self, def: DefId, parent: Option<usize>) -> usize {
        let n = self.ir.def(def).frame.len();
        self.envs.push(EnvFrame {
            def,
            slots: vec![Sb::Unset; n],
            parent,
        });
        self.envs.len() - 1
    }

    /// The environment of the enclosing activation of `def`, from `env`
    /// outwards.
    fn find_env(&self, env: usize, def: DefId) -> Option<usize> {
        let mut e = Some(env);
        while let Some(i) = e {
            if self.envs[i].def == def {
                return Some(i);
            }
            e = self.envs[i].parent;
        }
        None
    }

    fn lexical_parent(&self, d: DefId, env: usize) -> Result<Option<usize>, Error> {
        match self.ir.def(d).scope {
            DefScope::TopLevel => Ok(None),
            DefScope::Local { parent } | DefScope::Inline { parent } => self
                .find_env(env, parent)
                .map(Some)
                .ok_or_else(|| Error::unsupported("a LET operator used outside its LET (M0 core)")),
            DefScope::LetValue { .. } => Err(Error::internal("a LET value used as an operator")),
        }
    }

    // ------------------------------------------------------------------
    // Nodes, read sets and folding
    // ------------------------------------------------------------------

    #[allow(
        clippy::too_many_lines,
        clippy::many_single_char_names,
        reason = "one arm per node kind"
    )]
    fn info_of(&self, n: &Node) -> Info {
        let mut i = Info::default();
        let mut add = |p: P, i: &mut Info| {
            if let Some(c) = self.info.get(p as usize) {
                i.vars |= c.vars;
                i.primed |= c.primed;
                i.calls |= c.calls;
                i.heavy |= c.heavy;
                union(&mut i.free, &c.free);
            }
        };
        let binders =
            |binds: &[Bind], body: &[P], i: &mut Info, add: &mut dyn FnMut(P, &mut Info)| {
                let mut inner = Info::default();
                for &b in body {
                    add(b, &mut inner);
                }
                let bound: Vec<u32> = binds.iter().flat_map(|b| b.slots.iter().copied()).collect();
                minus(&mut inner.free, &bound);
                i.vars |= inner.vars;
                i.primed |= inner.primed;
                i.calls |= inner.calls;
                i.heavy |= inner.heavy;
                union(&mut i.free, &inner.free);
                for b in binds {
                    add(b.domain, i);
                }
            };
        match n {
            Node::Const(_) => {}
            Node::Slot(s) => i.free.push(*s),
            Node::Lazy { body, .. } | Node::Scope { body, .. } | Node::Memo { body, .. } => {
                add(*body, &mut i);
            }
            Node::Var(v) => i.vars |= 1u64 << v,
            Node::Primed(v) => i.primed |= 1u64 << v,
            Node::Not(a)
            | Node::Neg(a)
            | Node::Powerset(a)
            | Node::BigUnion(a)
            | Node::Domain(a)
            | Node::Select(a, _) => {
                add(*a, &mut i);
            }
            Node::And(xs)
            | Node::Or(xs)
            | Node::SetEnum(xs)
            | Node::Product(xs)
            | Node::Tuple(xs)
            | Node::Std(_, xs) => {
                for &x in xs {
                    add(x, &mut i);
                }
            }
            Node::Implies(a, b)
            | Node::Equiv(a, b)
            | Node::Eq(a, b)
            | Node::Neq(a, b)
            | Node::In(a, b)
            | Node::NotIn(a, b)
            | Node::Subseteq(a, b)
            | Node::Arith(_, a, b)
            | Node::Cmp(_, a, b)
            | Node::Range(a, b)
            | Node::SetBin(_, a, b)
            | Node::FcnSet(a, b)
            | Node::Apply(a, b) => {
                add(*a, &mut i);
                add(*b, &mut i);
            }
            Node::If(a, b, c) => {
                add(*a, &mut i);
                add(*b, &mut i);
                add(*c, &mut i);
            }
            Node::Case(arms, other) => {
                for &(g, v) in arms {
                    add(g, &mut i);
                    add(v, &mut i);
                }
                if let Some(o) = other {
                    add(*o, &mut i);
                }
            }
            Node::RecSet(fs) | Node::Record(fs) => {
                for &(_, x) in fs {
                    add(x, &mut i);
                }
            }
            Node::Filter { bind, pred } => {
                binders(std::slice::from_ref(bind), &[*pred], &mut i, &mut add);
            }
            Node::Map { binds, body }
            | Node::FcnCons { binds, body }
            | Node::Quant { binds, body, .. } => {
                binders(binds, &[*body], &mut i, &mut add);
            }
            Node::Except { base, updates } => {
                add(*base, &mut i);
                for u in updates {
                    for &p in &u.path {
                        add(p, &mut i);
                    }
                    let mut inner = Info::default();
                    add(u.value, &mut inner);
                    minus(&mut inner.free, &[u.at]);
                    i.vars |= inner.vars;
                    i.primed |= inner.primed;
                    i.calls |= inner.calls;
                    i.heavy |= inner.heavy;
                    union(&mut i.free, &inner.free);
                }
            }
            Node::Call { args, .. } => {
                for &a in args {
                    add(a, &mut i);
                }
                i.calls = true;
                i.vars = u64::MAX;
            }
        }
        i
    }

    fn push(&mut self, n: Node) -> Result<P, Error> {
        if self.code.nodes.len() >= MAX_NODES {
            return Err(Error::unsupported(
                "a plan larger than the M0 core's inlining limit",
            ));
        }
        let info = self.info_of(&n);
        let foldable = !matches!(
            n,
            Node::Const(_) | Node::Slot(_) | Node::Var(_) | Node::Primed(_) | Node::Memo { .. }
        );
        #[allow(clippy::cast_possible_truncation, reason = "bounded by MAX_NODES")]
        let id = self.code.nodes.len() as P;
        self.code.nodes.push(n);
        let closed = info.vars == 0
            && info.primed == 0
            && info.free.is_empty()
            && !info.calls
            && !info.heavy;
        self.info.push(info);
        if self.opts.fold && foldable && closed {
            self.fold(id);
        }
        Ok(id)
    }

    /// Folds a closed node to a constant, within [`FOLD_FUEL`] and what is
    /// left of [`FOLD_BUDGET`]. A fold that fails or runs out of fuel leaves
    /// the node alone: its error, if any, surfaces where TLC raises it.
    fn fold(&mut self, id: P) {
        let left = FOLD_BUDGET.saturating_sub(self.stats.fold_fuel);
        if left == 0 {
            self.stats.fold_skipped += 1;
            return;
        }
        let fuel = FOLD_FUEL.min(left);
        let started = std::time::Instant::now();
        self.m.fuel = fuel;
        let old = self.m.enter(self.next_slot + 1);
        let r = self.m.eval(&self.code, id);
        self.m.leave(old);
        let spent = fuel - self.m.fuel;
        let exhausted = self.m.fuel == 0;
        self.m.fuel = u64::MAX;
        let r = r.and_then(|h| self.m.heap.intern_const(h));
        self.m.heap.reset();
        self.stats.fold_fuel += spent;
        self.stats.fold_nanos += u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        match r {
            Ok(h) if !exhausted && !h.is_empty() => {
                self.code.nodes[id as usize] = Node::Const(h);
                self.info[id as usize] = Info::default();
                self.stats.folded += 1;
            }
            _ if exhausted => {
                self.info[id as usize].heavy = true;
                self.stats.fold_abandoned += 1;
            }
            _ => {}
        }
    }

    fn konst(&mut self, h: Handle) -> Result<P, Error> {
        self.push(Node::Const(h))
    }

    fn gpush(&mut self, g: GNode) -> G {
        #[allow(clippy::cast_possible_truncation, reason = "bounded by MAX_NODES")]
        let id = self.code.gnodes.len() as G;
        self.code.gnodes.push(g);
        id
    }

    fn string(&mut self, s: &str) -> Result<Handle, Error> {
        self.m.heap.string(s)
    }

    fn is_trivial(&self, p: P) -> bool {
        matches!(
            self.code.nodes.get(p as usize),
            Some(Node::Const(_) | Node::Slot(_) | Node::Var(_) | Node::Primed(_))
        )
    }

    /// A memo site for a body, if it is eligible.
    fn site(&mut self, name: String, p: P) -> Result<Option<P>, Error> {
        if !self.opts.memo || self.init_mode {
            return Ok(None);
        }
        let Some(info) = self.info.get(p as usize) else {
            return Ok(None);
        };
        if info.primed != 0 || matches!(self.code.nodes.get(p as usize), Some(Node::Const(_))) {
            return Ok(None);
        }
        #[allow(clippy::cast_possible_truncation, reason = "at most 64 variables")]
        let vars: Vec<u16> = (0..64u16)
            .filter(|&v| info.vars & (1u64 << v) != 0)
            .take(MAX_KEY + 1)
            .collect();
        let slots = info.free.clone();
        if vars.len() + slots.len() > MAX_KEY || (vars.is_empty() && slots.is_empty()) {
            return Ok(None);
        }
        #[allow(clippy::cast_possible_truncation, reason = "bounded")]
        let site = self.code.sites.len() as u32;
        self.code.sites.push(MemoSite {
            name: name.clone(),
            vars: vars.into(),
            slots: slots.into(),
        });
        self.site_names.push(name);
        Ok(Some(self.push(Node::Memo { site, body: p })?))
    }

    // ------------------------------------------------------------------
    // Roles
    // ------------------------------------------------------------------

    fn role_env(&mut self, r: &Role) -> Result<(usize, Vec<(u32, Handle)>), Error> {
        self.next_slot = 0;
        self.reset_stack.clear();
        self.def_cache.clear();
        self.at_stack.clear();
        self.role_name.clone_from(&r.name);
        let def = self.ir.def(r.def);
        if def.depth != 0 {
            return Err(Error::unsupported(
                "a role inside a LET or LAMBDA (M0 core)",
            ));
        }
        let env = self.new_env(r.def, None);
        let mut preset = Vec::new();
        for (slot, v) in &r.slots {
            let flat = self.alloc();
            let h = self.convert(v)?;
            if let Some(s) = self.envs[env].slots.get_mut(slot.index()) {
                *s = Sb::Flat(flat);
            }
            preset.push((flat, h));
        }
        Ok((env, preset))
    }

    fn action(&mut self, r: &Role, init: bool) -> Result<Action, Error> {
        self.init_mode = init;
        let (env, preset) = self.role_env(r)?;
        let g = self.g(r.expr, env)?;
        Ok(Action {
            name: r.name.clone(),
            g,
            frame: self.next_slot,
            preset,
        })
    }

    fn check(&mut self, r: &Role) -> Result<Check, Error> {
        self.init_mode = false;
        let (env, preset) = self.role_env(r)?;
        if !preset.is_empty() {
            return Err(Error::unsupported(
                "an invariant with bound parameters (M0 core)",
            ));
        }
        let mut parts = Vec::new();
        self.conjuncts(r.expr, env, &mut parts)?;
        let mut conjuncts = Vec::new();
        for (k, (e, env)) in parts.into_iter().enumerate() {
            let p = self.p(e, env, Mode::Plain)?;
            let info = self.info[p as usize].clone();
            let skippable = info.free.is_empty() && info.primed == 0;
            let reads = info.vars;
            let (p, site) = match self.site(format!("{}#{k}", r.name), p)? {
                Some(m) => {
                    let site = match self.code.nodes.get(m as usize) {
                        Some(Node::Memo { site, .. }) => Some(*site),
                        _ => None,
                    };
                    (m, site)
                }
                None => (p, None),
            };
            conjuncts.push(Conjunct {
                p,
                reads,
                skippable,
                site,
            });
        }
        Ok(Check {
            name: r.name.clone(),
            conjuncts,
            frame: self.next_slot,
        })
    }

    /// Flattens a conjunction, through zero-arity definitions, into its
    /// conjuncts in order.
    fn conjuncts(
        &mut self,
        e: ExprId,
        env: usize,
        out: &mut Vec<(ExprId, usize)>,
    ) -> Result<(), Error> {
        match &self.ir.node(e).expr {
            Expr::Junction {
                kind: JunctionKind::And,
                items,
                ..
            } => {
                for &i in items {
                    self.conjuncts(i, env, out)?;
                }
                Ok(())
            }
            Expr::Label { body, .. } | Expr::Selected(body) => self.conjuncts(*body, env, out),
            Expr::Def(d) => {
                let d = self.redirect(*d);
                let def = self.ir.def(d);
                if self.h.precomputed.contains_key(&d)
                    || def.std.is_some()
                    || def.recursive
                    || !def.params.is_empty()
                    || matches!(def.scope, DefScope::LetValue { .. })
                {
                    out.push((e, env));
                    return Ok(());
                }
                let parent = self.lexical_parent(d, env)?;
                let inner = self.new_env(d, parent);
                self.conjuncts(def.body, inner, out)
            }
            _ => {
                out.push((e, env));
                Ok(())
            }
        }
    }

    // ------------------------------------------------------------------
    // Value compilation
    // ------------------------------------------------------------------

    #[allow(clippy::too_many_lines, reason = "one arm per expression kind")]
    fn p(&mut self, e: ExprId, env: usize, mode: Mode) -> Result<P, Error> {
        let expr = &self.ir.node(e).expr;
        match expr {
            Expr::Bool(b) => self.konst(Handle::bool(*b)),
            Expr::Num(s) => {
                let v = s.parse::<i64>().ok().and_then(Handle::int).ok_or_else(|| {
                    Error::unsupported(format!(
                        "the numeral {s} (beyond the M0 core's 61-bit integers)"
                    ))
                })?;
                self.konst(v)
            }
            Expr::Decimal { .. } => Err(Error::unsupported("decimal numbers")),
            Expr::Str(s) => {
                let h = self.string(s)?;
                self.konst(h)
            }
            Expr::Var { var, primed } => {
                #[allow(clippy::cast_possible_truncation, reason = "at most 64 variables")]
                let i = var.index() as u16;
                if self.init_mode {
                    if *primed || mode == Mode::Primed {
                        return Err(Error::unsupported(
                            "a primed variable in an initial predicate",
                        ));
                    }
                    return self.push(Node::Primed(i));
                }
                match (*primed, mode) {
                    (false, Mode::Plain) => self.push(Node::Var(i)),
                    (true, Mode::Plain) | (false, Mode::Primed) => self.push(Node::Primed(i)),
                    (true, Mode::Primed) => Err(Error::unsupported("a doubly primed variable")),
                }
            }
            Expr::Const(c) => match self.h.constants.get(c.index()).cloned().flatten() {
                Some(ConstBinding::Value(v)) => {
                    let h = if let Some(&h) = self.consts.get(&c.index()) {
                        h
                    } else {
                        let h = self.convert(&v)?;
                        self.consts.insert(c.index(), h);
                        h
                    };
                    self.konst(h)
                }
                Some(ConstBinding::Def(d)) => self.def_use(d, env, mode),
                None => Err(Error::unsupported("an unbound constant")),
            },
            Expr::Local(r) => self.local(*r, env, mode),
            Expr::Def(d) => self.def_use(*d, env, mode),
            Expr::Apply { callee, args } => self.apply(callee, args, env, mode),
            Expr::Builtin { op, args } => self.builtin(*op, args, env, mode),
            Expr::Junction { kind, items, .. } => {
                let mut ps = Vec::with_capacity(items.len());
                for &i in items {
                    ps.push(self.p(i, env, mode)?);
                }
                match kind {
                    JunctionKind::And => self.push(Node::And(ps.into())),
                    JunctionKind::Or => self.push(Node::Or(ps.into())),
                }
            }
            Expr::Not(a) => {
                let a = self.p(*a, env, mode)?;
                self.push(Node::Not(a))
            }
            Expr::Implies(a, b) => {
                let a = self.p(*a, env, mode)?;
                let b = self.p(*b, env, mode)?;
                self.push(Node::Implies(a, b))
            }
            Expr::Equiv(a, b) => {
                let a = self.p(*a, env, mode)?;
                let b = self.p(*b, env, mode)?;
                self.push(Node::Equiv(a, b))
            }
            Expr::If { cond, then, other } => {
                let c = self.p(*cond, env, mode)?;
                let t = self.p(*then, env, mode)?;
                let o = self.p(*other, env, mode)?;
                self.push(Node::If(c, t, o))
            }
            Expr::Case { arms, other } => {
                let mut out = Vec::with_capacity(arms.len());
                for a in arms {
                    let g = self.p(a.guard, env, mode)?;
                    let v = self.p(a.value, env, mode)?;
                    out.push((g, v));
                }
                let o = match other {
                    Some(o) => Some(self.p(*o, env, mode)?),
                    None => None,
                };
                self.push(Node::Case(out.into(), o))
            }
            Expr::Quant { kind, bounds, body } => {
                let (binds, saved) = self.binds(bounds, env, mode)?;
                let b = self.p(*body, env, mode);
                self.unbind(saved);
                let b = b?;
                let kind = match kind {
                    QuantKind::Exists => Quant::Exists,
                    QuantKind::Forall => Quant::Forall,
                    QuantKind::Choose => Quant::Choose,
                };
                self.push(Node::Quant {
                    kind,
                    binds: binds.into(),
                    body: b,
                })
            }
            Expr::SetFilter { bound, pred } => {
                let (mut binds, saved) = self.binds(std::slice::from_ref(bound), env, mode)?;
                let b = self.p(*pred, env, mode);
                self.unbind(saved);
                let b = b?;
                let bind = binds
                    .pop()
                    .ok_or_else(|| Error::internal("filter without a bound"))?;
                self.push(Node::Filter { bind, pred: b })
            }
            Expr::SetMap { expr, bounds } => {
                let (binds, saved) = self.binds(bounds, env, mode)?;
                let b = self.p(*expr, env, mode);
                self.unbind(saved);
                let b = b?;
                self.push(Node::Map {
                    binds: binds.into(),
                    body: b,
                })
            }
            Expr::FcnCons { bounds, body } => {
                let (binds, saved) = self.binds(bounds, env, mode)?;
                let b = self.p(*body, env, mode);
                self.unbind(saved);
                let b = b?;
                self.push(Node::FcnCons {
                    binds: binds.into(),
                    body: b,
                })
            }
            Expr::RecFcn { .. } => Err(Error::unsupported(
                "recursive function definitions (M0 core)",
            )),
            Expr::SetEnum(items) => {
                let ps = self.ps(items, env, mode)?;
                self.push(Node::SetEnum(ps.into()))
            }
            Expr::Tuple(items) => {
                let ps = self.ps(items, env, mode)?;
                self.push(Node::Tuple(ps.into()))
            }
            Expr::Cartesian(items) => {
                let ps = self.ps(items, env, mode)?;
                self.push(Node::Product(ps.into()))
            }
            Expr::Record(fields) | Expr::RecordSet(fields) => {
                let is_set = matches!(expr, Expr::RecordSet(_));
                let mut out = Vec::with_capacity(fields.len());
                for f in fields {
                    let name = self.string(&f.name)?;
                    let v = self.p(f.value, env, mode)?;
                    out.push((name, v));
                }
                out.sort_by(|a, b| self.m.heap.cmp(a.0, b.0));
                if out.windows(2).any(|w| w[0].0 == w[1].0) {
                    return Err(Error::unsupported("a record with a repeated field"));
                }
                if is_set {
                    self.push(Node::RecSet(out.into()))
                } else {
                    self.push(Node::Record(out.into()))
                }
            }
            Expr::FcnSet { domain, range } => {
                let d = self.p(*domain, env, mode)?;
                let r = self.p(*range, env, mode)?;
                self.push(Node::FcnSet(d, r))
            }
            Expr::FcnApply { fcn, arg } => {
                let f = self.p(*fcn, env, mode)?;
                let a = self.p(*arg, env, mode)?;
                self.push(Node::Apply(f, a))
            }
            Expr::RecordSelect { record, field } => {
                let r = self.p(*record, env, mode)?;
                let f = self.string(field)?;
                self.push(Node::Select(r, f))
            }
            Expr::Except { base, updates } => {
                let b = self.p(*base, env, mode)?;
                let mut out = Vec::with_capacity(updates.len());
                for u in updates {
                    let path = self.ps(&u.path, env, mode)?;
                    let at = self.alloc();
                    self.at_stack.push(at);
                    let v = self.p(u.value, env, mode);
                    self.at_stack.pop();
                    out.push(Update {
                        path: path.into(),
                        at,
                        value: v?,
                    });
                }
                self.push(Node::Except {
                    base: b,
                    updates: out.into(),
                })
            }
            Expr::ExceptAt => match self.at_stack.last() {
                Some(&at) => self.push(Node::Slot(at)),
                None => Err(Error::unsupported("@ outside EXCEPT")),
            },
            Expr::Let { defs, body } => {
                let body = *body;
                let defs = defs.clone();
                let (b, reset) = self.let_scope(&defs, env, |c| c.p(body, env, mode))?;
                if reset.is_empty() {
                    Ok(b)
                } else {
                    self.push(Node::Scope {
                        reset: reset.into(),
                        body: b,
                    })
                }
            }
            Expr::Selected(a) | Expr::Label { body: a, .. } => self.p(*a, env, mode),
            Expr::Prime(a) => {
                if mode == Mode::Primed {
                    return Err(Error::unsupported("a doubly primed expression"));
                }
                if self.init_mode {
                    return Err(Error::unsupported(
                        "a primed expression in an initial predicate",
                    ));
                }
                self.p(*a, env, Mode::Primed)
            }
            Expr::Unchanged(a) => self.unchanged_value(*a, env, mode),
            Expr::Subst { .. } | Expr::SubstRef { .. } => {
                Err(Error::unsupported("INSTANCE substitution (M0 core)"))
            }
            // `[A]_v` is `A \/ UNCHANGED v`; `<<A>>_v` is `A /\ v' # v`
            // (TLC's `OPCODE_sa`, `OPCODE_aa` in `eval`).
            Expr::StepBox { action, sub } | Expr::StepAngle { action, sub } => {
                if mode == Mode::Primed || self.init_mode {
                    return Err(Error::unsupported(
                        "[A]_v under a prime or in an initial predicate (core)",
                    ));
                }
                let boxed = matches!(expr, Expr::StepBox { .. });
                let (action, sub) = (*action, *sub);
                let a = self.p(action, env, mode)?;
                let x = self.p(sub, env, Mode::Plain)?;
                let y = self.p(sub, env, Mode::Primed)?;
                let same = self.push(Node::Eq(x, y))?;
                if boxed {
                    self.push(Node::Or([a, same].into()))
                } else {
                    let changed = self.push(Node::Not(same))?;
                    self.push(Node::And([a, changed].into()))
                }
            }
            Expr::Enabled(_) => Err(Error::unsupported("ENABLED (M0 core)")),
            Expr::Cdot(..) => Err(Error::unsupported("action composition (M0 core)")),
            Expr::Always(_)
            | Expr::Eventually(_)
            | Expr::LeadsTo(..)
            | Expr::WhilePlus(..)
            | Expr::Wf { .. }
            | Expr::Sf { .. }
            | Expr::TemporalQuant { .. } => Err(Error::unsupported(
                "a temporal formula in a state or action role",
            )),
            Expr::Unbound(n) => Err(Error::unsupported(format!("the unbound name {n}"))),
            Expr::Unevaluable(_) => Err(Error::unsupported("an unevaluable expression")),
        }
    }

    fn ps(&mut self, items: &[ExprId], env: usize, mode: Mode) -> Result<Vec<P>, Error> {
        let mut out = Vec::with_capacity(items.len());
        for &i in items {
            out.push(self.p(i, env, mode)?);
        }
        Ok(out)
    }

    fn unchanged_value(&mut self, a: ExprId, env: usize, mode: Mode) -> Result<P, Error> {
        if mode == Mode::Primed || self.init_mode {
            return Err(Error::unsupported(
                "UNCHANGED under a prime or in an initial predicate",
            ));
        }
        let x = self.p(a, env, Mode::Plain)?;
        let y = self.p(a, env, Mode::Primed)?;
        self.push(Node::Eq(x, y))
    }

    /// Compiles bound groups: the domains first, then each variable gets a
    /// fresh slot. Returns the saved bindings to restore.
    fn binds(
        &mut self,
        bounds: &[Bound],
        env: usize,
        mode: Mode,
    ) -> Result<(Vec<Bind>, Saved), Error> {
        let mut domains = Vec::with_capacity(bounds.len());
        for b in bounds {
            let Some(d) = b.domain else {
                return Err(Error::unsupported("an unbounded quantifier or CHOOSE"));
            };
            domains.push(self.p(d, env, mode)?);
        }
        let mut saved = Vec::new();
        let mut binds = Vec::with_capacity(bounds.len());
        for (b, domain) in bounds.iter().zip(domains) {
            let mut slots = Vec::with_capacity(b.vars.len());
            for v in &b.vars {
                let flat = self.alloc();
                let old = self.envs[env]
                    .slots
                    .get(v.index())
                    .cloned()
                    .unwrap_or(Sb::Unset);
                saved.push((env, v.index(), old));
                if let Some(s) = self.envs[env].slots.get_mut(v.index()) {
                    *s = Sb::Flat(flat);
                }
                slots.push(flat);
            }
            binds.push(Bind {
                slots: slots.into(),
                tuple: b.tuple,
                domain,
            });
        }
        Ok((binds, saved))
    }

    fn unbind(&mut self, saved: Saved) {
        for (env, slot, old) in saved.into_iter().rev() {
            if let Some(s) = self.envs[env].slots.get_mut(slot) {
                *s = old;
            }
        }
    }

    /// Binds a `LET`'s zero-arity definitions as lazy values in `env`, runs
    /// `f`, and returns its result with the thunk slots to reset on entry.
    fn let_scope<T>(
        &mut self,
        defs: &[DefId],
        env: usize,
        f: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<(T, Vec<u32>), Error> {
        let owner = self.reset_stack.len();
        self.reset_stack.push(Vec::new());
        let mut saved = Vec::new();
        for &d in defs {
            if let DefScope::LetValue { parent, slot, .. } = self.ir.def(d).scope {
                let target = self.find_env(env, parent).unwrap_or(env);
                let old = self.envs[target]
                    .slots
                    .get(slot.index())
                    .cloned()
                    .unwrap_or(Sb::Unset);
                saved.push((target, slot.index(), old));
                if let Some(s) = self.envs[target].slots.get_mut(slot.index()) {
                    *s = Sb::LetVal {
                        def: d,
                        env: target,
                        thunk: None,
                        owner,
                    };
                }
            }
        }
        let r = f(self);
        self.unbind(saved);
        let reset = self.reset_stack.pop().unwrap_or_default();
        Ok((r?, reset))
    }

    fn frame_env(&self, env: usize, up: u32) -> Result<usize, Error> {
        let mut e = env;
        for _ in 0..up {
            e = self.envs[e]
                .parent
                .ok_or_else(|| Error::internal("a local reference beyond the outermost frame"))?;
        }
        Ok(e)
    }

    fn local(&mut self, r: LocalRef, env: usize, mode: Mode) -> Result<P, Error> {
        let fe = self.frame_env(env, r.up)?;
        let sb = self.envs[fe]
            .slots
            .get(r.slot.index())
            .cloned()
            .unwrap_or(Sb::Unset);
        match sb {
            Sb::Flat(f) => self.push(Node::Slot(f)),
            Sb::Arg {
                expr,
                env: aenv,
                how,
            } => match (how, mode) {
                (ArgHow::Alias(p), Mode::Plain) => Ok(p),
                (ArgHow::Alias(p), Mode::Primed) => match self.code.nodes.get(p as usize) {
                    Some(Node::Var(i)) => {
                        let i = *i;
                        self.push(Node::Primed(i))
                    }
                    Some(Node::Const(_) | Node::Slot(_)) => Ok(p),
                    _ => {
                        self.stats.by_name += 1;
                        self.p(expr, aenv, Mode::Primed)
                    }
                },
                (ArgHow::Thunk(slot, body), Mode::Plain) => self.push(Node::Lazy { slot, body }),
                (ArgHow::Thunk(..), Mode::Primed) | (ArgHow::ByName, _) => {
                    self.stats.by_name += 1;
                    self.p(expr, aenv, mode)
                }
            },
            Sb::LetVal {
                def,
                env: lenv,
                thunk,
                owner,
            } => {
                let d = self.ir.def(def);
                if mode == Mode::Primed || d.level >= Level::Action {
                    return self.p(d.body, lenv, mode);
                }
                let (slot, body) = if let Some(t) = thunk {
                    t
                } else {
                    let body = self.p(d.body, lenv, Mode::Plain)?;
                    let slot = self.alloc();
                    if let Some(Sb::LetVal { thunk, .. }) =
                        self.envs[fe].slots.get_mut(r.slot.index())
                    {
                        *thunk = Some((slot, body));
                    }
                    if let Some(list) = self.reset_stack.get_mut(owner) {
                        list.push(slot);
                    }
                    (slot, body)
                };
                if self.is_trivial(body) {
                    return Ok(body);
                }
                self.push(Node::Lazy { slot, body })
            }
            Sb::Unset => Err(Error::internal(format!(
                "unbound local {} in {}",
                self.ir
                    .def(self.envs[fe].def)
                    .frame
                    .get(r.slot.index())
                    .map_or("?", |s| s.name.as_str()),
                self.ir.def(self.envs[fe].def).name
            ))),
        }
    }

    /// A use of a zero-arity definition.
    fn def_use(&mut self, d: DefId, env: usize, mode: Mode) -> Result<P, Error> {
        let d = self.redirect(d);
        if let Some(h) = self.precomputed(d)? {
            return self.konst(h);
        }
        let def = self.ir.def(d);
        if let Some(op) = def.std {
            return self.std_apply(op, &[], env, mode);
        }
        if !def.params.is_empty() {
            return Err(Error::unsupported(format!(
                "the operator {} used as a value",
                def.name
            )));
        }
        if def.recursive {
            return self.call(d, &[], env, mode);
        }
        let cacheable = mode == Mode::Plain && !self.init_mode && def.scope == DefScope::TopLevel;
        if cacheable && let Some(&(slot, body)) = self.def_cache.get(&d) {
            return self.push(Node::Lazy { slot, body });
        }
        let parent = self.lexical_parent(d, env)?;
        let inner = self.new_env(d, parent);
        let body = self.p(def.body, inner, mode)?;
        // A state-level definition is a pure function of the current state:
        // evaluate it at most once per frame activation (the state does not
        // change within one). TLC re-evaluates it; the result is the same.
        let info = &self.info[body as usize];
        if cacheable && info.free.is_empty() && info.primed == 0 && !self.is_trivial(body) {
            let slot = self.alloc();
            self.def_cache.insert(d, (slot, body));
            self.stats.cached_defs += 1;
            return self.push(Node::Lazy { slot, body });
        }
        Ok(body)
    }

    fn apply(&mut self, callee: &OpRef, args: &[Arg], env: usize, mode: Mode) -> Result<P, Error> {
        match callee {
            OpRef::Def(d) => {
                let d = self.redirect(*d);
                let def = self.ir.def(d);
                if let Some(op) = def.std {
                    return self.std_apply(op, args, env, mode);
                }
                if args.is_empty() {
                    return self.def_use(d, env, mode);
                }
                if def.recursive {
                    return self.call(d, args, env, mode);
                }
                let body = def.body;
                let (inner, reset) = self.inline(d, args, env, mode)?;
                let b = self.p(body, inner, mode)?;
                if reset.is_empty() {
                    Ok(b)
                } else {
                    self.push(Node::Scope {
                        reset: reset.into(),
                        body: b,
                    })
                }
            }
            OpRef::Std(op) => {
                if let Some(&t) = self.h.std_redirects.get(op) {
                    return self.apply(&OpRef::Def(t), args, env, mode);
                }
                self.std_apply(*op, args, env, mode)
            }
            OpRef::Param(_) => Err(Error::unsupported("operator parameters (M0 core)")),
            OpRef::Builtin(op) => Err(Error::unsupported(format!(
                "the built-in {} as an operator argument (M0 core)",
                op.sany_name()
            ))),
            OpRef::Const(_) => Err(Error::unsupported("constant operators (M0 core)")),
            OpRef::Subst(_) => Err(Error::unsupported("INSTANCE substitution (M0 core)")),
            OpRef::Unbound(n) => Err(Error::unsupported(format!("the unbound operator {n}"))),
        }
    }

    /// Binds an inlined operator's parameters: returns its environment and
    /// the thunk slots to reset on entry.
    fn inline(
        &mut self,
        d: DefId,
        args: &[Arg],
        env: usize,
        mode: Mode,
    ) -> Result<(usize, Vec<u32>), Error> {
        let def = self.ir.def(d);
        if def.params.len() != args.len() {
            return Err(Error::internal(format!(
                "{} applied to {} arguments",
                def.name,
                args.len()
            )));
        }
        let parent = self.lexical_parent(d, env)?;
        let inner = self.new_env(d, parent);
        let mut reset = Vec::new();
        self.stats.inlined += 1;
        for (i, a) in args.iter().enumerate() {
            let Arg::Expr { expr, .. } = a else {
                return Err(Error::unsupported("operator arguments (M0 core)"));
            };
            let cp = self.p(*expr, env, mode)?;
            let how = if self.opts.eager && self.is_trivial(cp) {
                self.stats.aliased += 1;
                ArgHow::Alias(cp)
            } else if self.info[cp as usize].primed == 0 {
                self.stats.thunks += 1;
                let slot = self.alloc();
                reset.push(slot);
                ArgHow::Thunk(slot, cp)
            } else {
                ArgHow::ByName
            };
            if let Some(s) = self.envs[inner].slots.get_mut(i) {
                *s = Sb::Arg {
                    expr: *expr,
                    env,
                    how,
                };
            }
        }
        Ok((inner, reset))
    }

    fn call(&mut self, d: DefId, args: &[Arg], env: usize, mode: Mode) -> Result<P, Error> {
        if mode == Mode::Primed {
            return Err(Error::unsupported(
                "a recursive operator under a prime (M0 core)",
            ));
        }
        let fun = self.fun(d)?;
        let mut ps = Vec::with_capacity(args.len());
        for a in args {
            let Arg::Expr { expr, .. } = a else {
                return Err(Error::unsupported("operator arguments (M0 core)"));
            };
            ps.push(self.p(*expr, env, mode)?);
        }
        self.push(Node::Call {
            fun,
            args: ps.into(),
        })
    }

    fn fun(&mut self, d: DefId) -> Result<u32, Error> {
        let key = (d, self.init_mode);
        if let Some(&f) = self.funs.get(&key) {
            return Ok(f);
        }
        let def = self.ir.def(d);
        if def.scope != DefScope::TopLevel {
            return Err(Error::unsupported("a recursive LET operator (M0 core)"));
        }
        #[allow(clippy::cast_possible_truncation, reason = "bounded")]
        let id = self.code.funs.len() as u32;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "parameter counts are small"
        )]
        let params = def.params.len() as u32;
        self.code.funs.push(Fun {
            name: def.name.clone(),
            params,
            frame: params,
            body: 0,
        });
        self.funs.insert(key, id);
        let saved_slot = self.next_slot;
        let saved_resets = std::mem::take(&mut self.reset_stack);
        let saved_cache = std::mem::take(&mut self.def_cache);
        let saved_at = std::mem::take(&mut self.at_stack);
        self.next_slot = params;
        let env = self.new_env(d, None);
        for i in 0..def.params.len() {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "parameter counts are small"
            )]
            let s = Sb::Flat(i as u32);
            if let Some(x) = self.envs[env].slots.get_mut(i) {
                *x = s;
            }
        }
        let body = self.p(def.body, env, Mode::Plain);
        let frame = self.next_slot;
        self.next_slot = saved_slot;
        self.reset_stack = saved_resets;
        self.def_cache = saved_cache;
        self.at_stack = saved_at;
        let body = body?;
        if let Some(f) = self.code.funs.get_mut(id as usize) {
            f.body = body;
            f.frame = frame;
        }
        Ok(id)
    }

    fn std_apply(&mut self, op: StdOp, args: &[Arg], env: usize, mode: Mode) -> Result<P, Error> {
        use StdOp as S;
        if let Some(&t) = self.h.std_redirects.get(&op) {
            return if args.is_empty() {
                self.def_use(t, env, mode)
            } else {
                self.apply(&OpRef::Def(t), args, env, mode)
            };
        }
        let mut ps = Vec::with_capacity(args.len());
        for a in args {
            let Arg::Expr { expr, .. } = a else {
                return Err(Error::unsupported(format!(
                    "{} with an operator argument",
                    op.qualified_name()
                )));
            };
            ps.push(self.p(*expr, env, mode)?);
        }
        let arg = |i: usize| {
            ps.get(i)
                .copied()
                .ok_or_else(|| Error::internal("missing argument"))
        };
        let arith = |o: Arith| -> Result<Node, Error> { Ok(Node::Arith(o, arg(0)?, arg(1)?)) };
        let cmp = |o: Cmp| -> Result<Node, Error> { Ok(Node::Cmp(o, arg(0)?, arg(1)?)) };
        let n = match op {
            S::NaturalsPlus | S::IntegersPlus => arith(Arith::Add)?,
            S::NaturalsMinus | S::IntegersMinus => arith(Arith::Sub)?,
            S::NaturalsTimes | S::IntegersTimes => arith(Arith::Mul)?,
            S::NaturalsDiv | S::IntegersDiv => arith(Arith::Div)?,
            S::NaturalsMod | S::IntegersMod => arith(Arith::Mod)?,
            S::NaturalsExpt | S::IntegersExpt => arith(Arith::Pow)?,
            S::NaturalsLt | S::IntegersLt => cmp(Cmp::Lt)?,
            S::NaturalsLeq | S::IntegersLeq => cmp(Cmp::Le)?,
            S::NaturalsGt | S::IntegersGt => cmp(Cmp::Gt)?,
            S::NaturalsGeq | S::IntegersGeq => cmp(Cmp::Ge)?,
            S::NaturalsDotDot | S::IntegersDotDot => Node::Range(arg(0)?, arg(1)?),
            S::IntegersNeg => Node::Neg(arg(0)?),
            S::NaturalsNat | S::IntegersNat => {
                let h = self.m.heap.symbolic(Kind::Nat, 0, &[]);
                let h = self.m.heap.intern_const(h)?;
                Node::Const(h)
            }
            S::IntegersInt => {
                let h = self.m.heap.symbolic(Kind::Int, 0, &[]);
                let h = self.m.heap.intern_const(h)?;
                Node::Const(h)
            }
            S::SequencesLen => Node::Std(StdFn::Len, ps.into()),
            S::SequencesHead => Node::Std(StdFn::Head, ps.into()),
            S::SequencesTail => Node::Std(StdFn::Tail, ps.into()),
            S::SequencesAppend => Node::Std(StdFn::Append, ps.into()),
            S::SequencesConcat => Node::Std(StdFn::Concat, ps.into()),
            S::SequencesSubSeq => Node::Std(StdFn::SubSeq, ps.into()),
            S::SequencesSeq => Node::Std(StdFn::Seq, ps.into()),
            S::FiniteSetsCardinality => Node::Std(StdFn::Cardinality, ps.into()),
            S::FiniteSetsIsFiniteSet => Node::Std(StdFn::IsFiniteSet, ps.into()),
            other => {
                return Err(Error::unsupported(format!(
                    "the standard operator {} (M0 core)",
                    other.qualified_name()
                )));
            }
        };
        self.push(n)
    }

    fn builtin(
        &mut self,
        op: BuiltinOp,
        args: &[ExprId],
        env: usize,
        mode: Mode,
    ) -> Result<P, Error> {
        use BuiltinOp as B;
        match op {
            B::Prime => {
                let a = *args
                    .first()
                    .ok_or_else(|| Error::internal("prime without operand"))?;
                if mode == Mode::Primed || self.init_mode {
                    return Err(Error::unsupported(
                        "a doubly primed expression or a prime in Init",
                    ));
                }
                return self.p(a, env, Mode::Primed);
            }
            B::Unchanged => {
                let a = *args
                    .first()
                    .ok_or_else(|| Error::internal("UNCHANGED without operand"))?;
                return self.unchanged_value(a, env, mode);
            }
            B::Boolean => {
                let mut v = vec![Handle::FALSE, Handle::TRUE];
                let h = self.m.heap.set_from(&mut v);
                let h = self.m.heap.intern_const(h)?;
                return self.konst(h);
            }
            B::String => return Err(Error::unsupported("STRING (M0 core)")),
            B::Enabled => return Err(Error::unsupported("ENABLED (M0 core)")),
            B::Cdot => return Err(Error::unsupported("action composition (M0 core)")),
            B::Always | B::Eventually | B::LeadsTo | B::WhilePlus => {
                return Err(Error::unsupported(
                    "a temporal operator in a state or action role",
                ));
            }
            _ => {}
        }
        let ps = self.ps(args, env, mode)?;
        let a = |i: usize| {
            ps.get(i)
                .copied()
                .ok_or_else(|| Error::internal("missing operand"))
        };
        let n = match op {
            B::Eq => Node::Eq(a(0)?, a(1)?),
            B::Neq => Node::Neq(a(0)?, a(1)?),
            B::In => Node::In(a(0)?, a(1)?),
            B::NotIn => Node::NotIn(a(0)?, a(1)?),
            B::Subseteq => Node::Subseteq(a(0)?, a(1)?),
            B::Cup => Node::SetBin(SetOp::Cup, a(0)?, a(1)?),
            B::Cap => Node::SetBin(SetOp::Cap, a(0)?, a(1)?),
            B::SetMinus => Node::SetBin(SetOp::Minus, a(0)?, a(1)?),
            B::PowerSet => Node::Powerset(a(0)?),
            B::BigUnion => Node::BigUnion(a(0)?),
            B::Domain => Node::Domain(a(0)?),
            B::Times => Node::Product(ps.clone().into()),
            B::Not => Node::Not(a(0)?),
            B::And => Node::And(ps.clone().into()),
            B::Or => Node::Or(ps.clone().into()),
            B::Implies => Node::Implies(a(0)?, a(1)?),
            B::Equiv => Node::Equiv(a(0)?, a(1)?),
            _ => {
                return Err(Error::unsupported(format!(
                    "the built-in {} (M0 core)",
                    op.sany_name()
                )));
            }
        };
        self.push(n)
    }

    // ------------------------------------------------------------------
    // Generator compilation
    // ------------------------------------------------------------------

    fn guard(&mut self, p: P) -> Result<G, Error> {
        let name = format!("{} guard", self.role_name);
        let p = self.site(name, p)?.unwrap_or(p);
        Ok(self.gpush(GNode::Guard(p)))
    }

    fn guard_expr(&mut self, e: ExprId, env: usize) -> Result<G, Error> {
        let p = self.p(e, env, Mode::Plain)?;
        self.guard(p)
    }

    #[allow(
        clippy::too_many_lines,
        clippy::many_single_char_names,
        reason = "one arm per generator rule"
    )]
    fn g(&mut self, e: ExprId, env: usize) -> Result<G, Error> {
        let expr = &self.ir.node(e).expr;
        match expr {
            Expr::Junction { kind, items, .. } => match kind {
                JunctionKind::And => {
                    let mut out = Vec::with_capacity(items.len());
                    for &i in items {
                        let g = self.g(i, env)?;
                        let p = self.p(i, env, Mode::Plain)?;
                        out.push((g, p));
                    }
                    Ok(self.gpush(GNode::Conj(out.into())))
                }
                JunctionKind::Or => {
                    let mut out = Vec::with_capacity(items.len());
                    for &i in items {
                        out.push(self.g(i, env)?);
                    }
                    Ok(self.gpush(GNode::Disj(out.into())))
                }
            },
            Expr::Quant {
                kind: kind @ (QuantKind::Exists | QuantKind::Forall),
                bounds,
                body,
            } if bounds.iter().all(|b| b.domain.is_some()) => {
                let exists = *kind == QuantKind::Exists;
                let body = *body;
                let (binds, saved) = self.binds(bounds, env, Mode::Plain)?;
                let r = (|| {
                    let bg = self.g(body, env)?;
                    let bp = self.p(body, env, Mode::Plain)?;
                    Ok((bg, bp))
                })();
                self.unbind(saved);
                let (bg, bp) = r?;
                let binds: Box<[Bind]> = binds.into();
                Ok(self.gpush(if exists {
                    GNode::Exists {
                        binds,
                        body: bg,
                        body_p: bp,
                    }
                } else {
                    GNode::Forall {
                        binds,
                        body: bg,
                        body_p: bp,
                    }
                }))
            }
            Expr::If { cond, then, other } => {
                let c = self.p(*cond, env, Mode::Plain)?;
                let t = self.g(*then, env)?;
                let o = self.g(*other, env)?;
                Ok(self.gpush(GNode::If {
                    cond: c,
                    then: t,
                    other: o,
                }))
            }
            Expr::Case { arms, other } => {
                let mut out = Vec::with_capacity(arms.len());
                for a in arms {
                    let p = self.p(a.guard, env, Mode::Plain)?;
                    let g = self.g(a.value, env)?;
                    out.push((p, g));
                }
                let o = match other {
                    Some(o) => Some(self.g(*o, env)?),
                    None => None,
                };
                Ok(self.gpush(GNode::Case {
                    arms: out.into(),
                    other: o,
                }))
            }
            Expr::Implies(a, b) => {
                let l = self.p(*a, env, Mode::Plain)?;
                let r = self.g(*b, env)?;
                Ok(self.gpush(GNode::Implies { lhs: l, rhs: r }))
            }
            Expr::Builtin {
                op: BuiltinOp::Eq,
                args,
            } if args.len() == 2 => {
                if let Some(var) = self.target(args[0], env, false)? {
                    let rhs = self.p(args[1], env, Mode::Plain)?;
                    return Ok(self.gpush(GNode::Assign { var, rhs }));
                }
                self.guard_expr(e, env)
            }
            Expr::Builtin {
                op: BuiltinOp::In,
                args,
            } if args.len() == 2 => {
                if let Some(var) = self.target(args[0], env, false)? {
                    let set = self.p(args[1], env, Mode::Plain)?;
                    // TLC enumerates a lazy union without normalizing it,
                    // so its elements can repeat (§6).
                    if let Some(raw) = self.raw_plan(args[1], env)? {
                        return Ok(self.gpush(GNode::AssignInRaw { var, set, raw }));
                    }
                    return Ok(self.gpush(GNode::AssignIn { var, set }));
                }
                self.guard_expr(e, env)
            }
            Expr::Builtin {
                op: BuiltinOp::Subseteq,
                args,
            } if args.len() == 2 => {
                // `x' \subseteq S` enumerates `SUBSET S`, normalized
                // (`Tool.java:1379-1421`).
                if let Some(var) = self.target(args[0], env, false)? {
                    let base = self.p(args[1], env, Mode::Plain)?;
                    let set = self.push(Node::Powerset(base))?;
                    return Ok(self.gpush(GNode::AssignIn { var, set }));
                }
                self.guard_expr(e, env)
            }
            Expr::Builtin {
                op: BuiltinOp::Unchanged,
                args,
            } if args.len() == 1 => {
                if self.init_mode {
                    return self.guard_expr(e, env);
                }
                let mut items = Vec::new();
                self.unchanged_items(args[0], env, &mut items)?;
                Ok(self.gpush(GNode::Unchanged(items.into())))
            }
            Expr::Unchanged(a) => {
                if self.init_mode {
                    return self.guard_expr(e, env);
                }
                let mut items = Vec::new();
                self.unchanged_items(*a, env, &mut items)?;
                Ok(self.gpush(GNode::Unchanged(items.into())))
            }
            Expr::StepBox { action, sub } => {
                if self.init_mode {
                    return self.guard_expr(e, env);
                }
                let a = self.g(*action, env)?;
                let mut items = Vec::new();
                self.unchanged_items(*sub, env, &mut items)?;
                Ok(self.gpush(GNode::StepBox {
                    action: a,
                    vars: items.into(),
                }))
            }
            Expr::StepAngle { action, sub } => {
                if self.init_mode {
                    return self.guard_expr(e, env);
                }
                let (action, sub) = (*action, *sub);
                let a = self.g(action, env)?;
                let same = self.changed_test(sub, env)?;
                Ok(self.gpush(GNode::StepAngle { action: a, same }))
            }
            Expr::FcnApply { fcn, arg } => {
                let (fcn, arg) = (*fcn, *arg);
                let Some((lambda, lenv)) = self.fcn_lambda(fcn, env, 0)? else {
                    return self.guard_expr(e, env);
                };
                let Expr::FcnCons { bounds, body } = self.ir.node(lambda).expr.clone() else {
                    return self.guard_expr(e, env);
                };
                if bounds.iter().any(|b| b.domain.is_none()) {
                    return self.guard_expr(e, env);
                }
                let (binds, saved) = self.binds(&bounds, lenv, Mode::Plain)?;
                let r = (|| {
                    let a = self.p(arg, env, Mode::Plain)?;
                    let b = self.g(body, lenv)?;
                    Ok((a, b))
                })();
                self.unbind(saved);
                let (a, b) = r?;
                Ok(self.gpush(GNode::FcnAction {
                    binds: binds.into(),
                    arg: a,
                    body: b,
                }))
            }
            Expr::Let { defs, body } => {
                let body = *body;
                let defs = defs.clone();
                let (g, reset) = self.let_scope(&defs, env, |c| c.g(body, env))?;
                if reset.is_empty() {
                    Ok(g)
                } else {
                    Ok(self.gpush(GNode::Scope {
                        reset: reset.into(),
                        body: g,
                    }))
                }
            }
            Expr::Selected(a) | Expr::Label { body: a, .. } => self.g(*a, env),
            Expr::Def(d) => {
                let d = self.redirect(*d);
                let def = self.ir.def(d);
                if self.h.precomputed.contains_key(&d)
                    || def.std.is_some()
                    || def.recursive
                    || !def.params.is_empty()
                    || matches!(def.scope, DefScope::LetValue { .. })
                {
                    return self.guard_expr(e, env);
                }
                let parent = self.lexical_parent(d, env)?;
                let inner = self.new_env(d, parent);
                self.g(def.body, inner)
            }
            Expr::Apply {
                callee: OpRef::Def(d),
                args,
            } => {
                let d = self.redirect(*d);
                let def = self.ir.def(d);
                if def.std.is_some() || def.recursive || args.is_empty() {
                    if args.is_empty()
                        && def.std.is_none()
                        && !def.recursive
                        && !self.h.precomputed.contains_key(&d)
                    {
                        let parent = self.lexical_parent(d, env)?;
                        let inner = self.new_env(d, parent);
                        return self.g(def.body, inner);
                    }
                    return self.guard_expr(e, env);
                }
                let body = def.body;
                let (inner, reset) = self.inline(d, args, env, Mode::Plain)?;
                let g = self.g(body, inner)?;
                if reset.is_empty() {
                    Ok(g)
                } else {
                    Ok(self.gpush(GNode::Scope {
                        reset: reset.into(),
                        body: g,
                    }))
                }
            }
            Expr::Local(r) => {
                let fe = self.frame_env(env, r.up)?;
                match self.envs[fe].slots.get(r.slot.index()).cloned() {
                    Some(Sb::Arg {
                        expr, env: aenv, ..
                    }) => {
                        self.stats.by_name += 1;
                        self.g(expr, aenv)
                    }
                    Some(Sb::LetVal { def, env: lenv, .. }) => {
                        let body = self.ir.def(def).body;
                        self.g(body, lenv)
                    }
                    _ => self.guard_expr(e, env),
                }
            }
            _ => self.guard_expr(e, env),
        }
    }

    /// The variable an assignment's left side names (TLC's `getPrimedVar`,
    /// or `getVar` in an initial predicate).
    fn target(&mut self, e: ExprId, env: usize, under_prime: bool) -> Result<Option<u16>, Error> {
        #[allow(clippy::cast_possible_truncation, reason = "at most 64 variables")]
        match &self.ir.node(e).expr {
            Expr::Var { var, primed } => {
                let ok = if self.init_mode {
                    !*primed && !under_prime
                } else {
                    *primed != under_prime
                };
                Ok(ok.then_some(var.index() as u16))
            }
            Expr::Prime(a) if !self.init_mode && !under_prime => self.target(*a, env, true),
            Expr::Builtin {
                op: BuiltinOp::Prime,
                args,
            } if !self.init_mode && !under_prime && args.len() == 1 => {
                self.target(args[0], env, true)
            }
            Expr::Label { body: a, .. } | Expr::Selected(a) => self.target(*a, env, under_prime),
            Expr::Local(r) => {
                let fe = self.frame_env(env, r.up)?;
                match self.envs[fe].slots.get(r.slot.index()).cloned() {
                    Some(Sb::Arg {
                        expr, env: aenv, ..
                    }) => self.target(expr, aenv, under_prime),
                    Some(Sb::LetVal { def, env: lenv, .. }) => {
                        let body = self.ir.def(def).body;
                        self.target(body, lenv, under_prime)
                    }
                    _ => Ok(None),
                }
            }
            Expr::Def(d) => {
                let d = self.redirect(*d);
                let def = self.ir.def(d);
                if self.h.precomputed.contains_key(&d)
                    || def.std.is_some()
                    || !def.params.is_empty()
                    || def.recursive
                {
                    return Ok(None);
                }
                let parent = self.lexical_parent(d, env)?;
                let inner = self.new_env(d, parent);
                self.target(def.body, inner, under_prime)
            }
            _ => Ok(None),
        }
    }
}
