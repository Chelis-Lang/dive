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
    Action, Arith, Bind, Check, Cmp, Code, CompileStats, Conjunct, Fun, G, GNode, LibArg, MemoSite,
    Node, P, Plan, Quant, SetOp, StdFn, Update,
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
    /// The `INSTANCE` substitutions active where splitting found the role
    /// (it found it inside an instantiated definition).
    pub substs: Vec<RoleSubst>,
}

/// An `INSTANCE` substitution active at a split role.
#[derive(Clone, Debug)]
pub enum RoleSubst {
    /// `decl <- expr`, with `expr` evaluated in the frame of `def` with
    /// `slots` bound, outside any substitution.
    Expr {
        /// The substituted declaration.
        decl: dive_ir::DeclId,
        /// The expression.
        expr: ExprId,
        /// The definition whose frame it is evaluated in.
        def: DefId,
        /// That frame's bound slots.
        slots: Vec<(Slot, Value)>,
    },
    /// `decl` bound to a value.
    Value {
        /// The substituted declaration.
        decl: dive_ir::DeclId,
        /// The value.
        value: Value,
    },
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
    /// The definitions among `precomputed` whose value is a configuration
    /// override (`Op <- value`): their body is not their value.
    pub overrides: std::collections::HashSet<DefId>,
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
    /// Implied initial predicates: the state-level parts of `PROPERTY`s,
    /// checked on the initial states.
    pub implied_inits: Vec<Role>,
    /// Implied actions: the `[A]_v` of `[][A]_v` in `PROPERTY`s, evaluated
    /// on every step as `ACTION_CONSTRAINT`s are.
    pub implied_actions: Vec<Role>,
    /// Whether deadlock is checked.
    pub check_deadlock: bool,
    /// `SYMMETRY`: the closure's nonidentity maps, each as its moved
    /// model values `(from, to)` by name; empty without one.
    pub symmetry: Vec<Vec<(String, String)>>,
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
    /// The function name inside a recursive function definition.
    RecSelf(u32),
    /// An operator parameter bound to an operator argument: the operator,
    /// resolved in the caller's environment `env`.
    OpArg {
        op: OpRef,
        env: usize,
    },
}

/// A function being compiled from a definition nested in others (a
/// recursive `LET` operator or function): its references to enclosing
/// locals become captures, passed by every call (lambda lifting).
#[derive(Debug)]
struct Lift {
    /// The first environment of the function: lower ones enclose it.
    floor: usize,
    /// The definition's lexical parent environment.
    parent: Option<usize>,
    /// The function.
    fun: u32,
    /// Captures: frames out from `parent`, slot there, capture slot.
    caps: Vec<(u32, usize, u32)>,
    /// Recursive calls inside the body, completed with the captures.
    self_calls: Vec<P>,
}

/// One active `INSTANCE` substitution `decl <- value` (`Expr::Subst`), in
/// the dynamic context TLC binds it in (`Expr::SubstRef`).
#[derive(Clone, Debug)]
struct SubstEntry {
    decl: dive_ir::DeclId,
    /// The value: an argument, or a constant (`RoleSubst::Value`).
    value: Result<Arg, P>,
    /// The environment outside the substitution.
    env: usize,
    /// How an expression value is passed (as an operator argument is).
    how: ArgHow,
    /// The dynamic context outside the substitution: the stack below its
    /// group.
    depth: usize,
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
    /// Reaches an operator with an effect (`Print`, `PrintT`): TLC performs
    /// it on every evaluation, so the node is never folded, memoized,
    /// cached or skipped (`04-plan-compiler.md` §3).
    effect: bool,
    /// Whether TLC's value of a set expression is `Reducible`
    /// (`04-plan-compiler.md` §4.1), so that TLC builds a filter, function
    /// or set operation over it eagerly.
    red: Red,
}

/// Whether TLC's value of a set expression is `Reducible`: a set
/// enumeration or an interval (`crates/dive-eval/src/core.rs`,
/// `is_reducible`). TLC builds `{x \in S : P}`, `[x \in S |-> e]` and
/// `S \cup T` eagerly over reducible operands and keeps them lazy
/// otherwise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Red {
    /// Always reducible.
    Yes,
    /// Never reducible.
    No,
    /// Not known statically.
    #[default]
    Maybe,
}

/// The most nodes of a constant [`Compiler::holds_lazy_set`] looks at.
const LAZY_SCAN: usize = 4096;

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
        effects: false,
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
        recfuns: HashMap::new(),
        def_slots: std::collections::HashSet::new(),
        lifts: Vec::new(),
        substs: Vec::new(),
        fun_caps: HashMap::new(),
        def_cache: HashMap::new(),
        opts,
        stats: CompileStats::default(),
        site_names: Vec::new(),
        role_name: String::new(),
    };
    // The initial predicates are conjoined, as TLC's (and the reference
    // engine's `init_states`): one action.
    let init = if h.init.is_empty() {
        Vec::new()
    } else {
        vec![c.init_action(&h.init)?]
    };
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
        action_constraints.push(c.step_check(r)?);
    }
    let mut implied_inits = Vec::new();
    for r in &h.implied_inits {
        implied_inits.push(c.check(r)?);
    }
    let mut implied_actions = Vec::new();
    for r in &h.implied_actions {
        implied_actions.push(c.step_check(r)?);
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
    let symmetry = if h.symmetry.is_empty() {
        None
    } else {
        let strings = c.m.heap.store().strings();
        let mut maps = Vec::with_capacity(h.symmetry.len());
        for m in &h.symmetry {
            let mut pairs = Vec::with_capacity(m.len());
            for (a, b) in m {
                pairs.push((strings.intern(a)?, strings.intern(b)?));
            }
            maps.push(pairs);
        }
        Some(crate::symmetry::Symmetry::new(maps))
    };
    Ok(Plan {
        code: c.code,
        vars,
        init,
        next,
        invariants,
        constraints,
        action_constraints,
        implied_inits,
        implied_actions,
        check_deadlock: h.check_deadlock,
        symmetry,
        effects: c.effects,
        stats: c.stats,
    })
}

struct Compiler<'a> {
    h: &'a Handoff<'a>,
    /// Whether any compiled node reaches an effect (`Info::effect`).
    effects: bool,
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
    /// Recursive function definitions `f[x \in S] == e` compiled as
    /// functions of their argument.
    recfuns: HashMap<(DefId, bool), u32>,
    /// The thunk slots of `def_cache`.
    def_slots: std::collections::HashSet<u32>,
    /// The nested functions being compiled, innermost last.
    lifts: Vec<Lift>,
    /// The active `INSTANCE` substitutions, innermost last.
    substs: Vec<SubstEntry>,
    /// Each lifted function's captures: frames out from the definition's
    /// lexical parent, and slot there.
    fun_caps: HashMap<u32, Vec<(u32, usize)>>,
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
        // TLC's startup value of a constant definition may be lazy (a
        // filter or function over `Nat`, a recursive function): the core
        // then compiles the definition's body, which has the same value.
        let h = match self.convert(v) {
            Ok(h) => h,
            Err(e)
                if e.kind() == crate::ErrorKind::Unsupported && !self.h.overrides.contains(&d) =>
            {
                return Ok(None);
            }
            Err(e) => return Err(e),
        };
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
                i.effect |= c.effect;
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
                i.effect |= inner.effect;
                union(&mut i.free, &inner.free);
                for b in binds {
                    add(b.domain, i);
                }
            };
        match n {
            Node::Const(_) => {}
            Node::Slot(s) => i.free.push(*s),
            Node::Lazy { body, .. }
            | Node::Scope { body, .. }
            | Node::Memo { body, .. }
            | Node::Spec(body) => {
                add(*body, &mut i);
            }
            Node::InLazy { elem, set, .. } => {
                add(*elem, &mut i);
                add(*set, &mut i);
            }
            Node::Beta { chain, arg, .. } => {
                add(*chain, &mut i);
                add(*arg, &mut i);
            }
            Node::DomCheck { slot, domain, body } => {
                add(*domain, &mut i);
                add(*body, &mut i);
                minus(&mut i.free, &[*slot]);
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
            Node::Lib(op, xs) => {
                for &x in xs {
                    add(x, &mut i);
                }
                i.effect |= crate::stdlib::effectful(*op);
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
                    i.effect |= inner.effect;
                    union(&mut i.free, &inner.free);
                }
            }
            Node::LibHo(_, args) => {
                for a in args {
                    match a {
                        LibArg::Value(p) => add(*p, &mut i),
                        LibArg::Op { params, body } => {
                            let mut inner = Info::default();
                            add(*body, &mut inner);
                            minus(&mut inner.free, params);
                            i.vars |= inner.vars;
                            i.primed |= inner.primed;
                            i.calls |= inner.calls;
                            i.heavy |= inner.heavy;
                            i.effect |= inner.effect;
                            union(&mut i.free, &inner.free);
                        }
                    }
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
        i.red = self.red_of(n);
        i
    }

    fn red(&self, p: P) -> Red {
        self.info.get(p as usize).map_or(Red::Maybe, |i| i.red)
    }

    /// TLC's reducibility of a node's value (`Info::red`).
    fn red_of(&self, n: &Node) -> Red {
        match n {
            Node::Const(h) => {
                if !h.is_compound() {
                    return Red::Yes;
                }
                match self.m.heap.view(*h).kind {
                    Kind::Product
                    | Kind::FcnSet
                    | Kind::RecSet
                    | Kind::Subset
                    | Kind::Nat
                    | Kind::Int
                    | Kind::SeqSet => Red::No,
                    _ => Red::Yes,
                }
            }
            Node::SetEnum(_) | Node::Range(..) | Node::Map { .. } => Red::Yes,
            Node::Product(_)
            | Node::FcnSet(..)
            | Node::RecSet(_)
            | Node::Powerset(_)
            | Node::Std(StdFn::Seq, _) => Red::No,
            Node::Filter { bind, .. } => match self.red(bind.domain) {
                Red::Yes => Red::Yes,
                Red::No => Red::No,
                Red::Maybe => Red::Maybe,
            },
            // `S \ T` is reducible as `S` is; `S \cup T` and `S \cap T` are
            // reducible when both are, and lazy when neither is.
            Node::SetBin(SetOp::Minus, a, _) => self.red(*a),
            Node::SetBin(_, a, b) => match (self.red(*a), self.red(*b)) {
                (Red::Yes, Red::Yes) => Red::Yes,
                (Red::No, Red::No) => Red::No,
                _ => Red::Maybe,
            },
            Node::Spec(b)
            | Node::Lazy { body: b, .. }
            | Node::Scope { body: b, .. }
            | Node::Memo { body: b, .. } => self.red(*b),
            _ => Red::Maybe,
        }
    }

    /// Pushes a filter, function constructor or set operation, wrapped in
    /// `Node::Spec` when TLC would build it lazily: the core builds it
    /// eagerly, so an evaluation error inside it may be one TLC never
    /// raises (`04-plan-compiler.md` §4.1).
    fn push_lazy(&mut self, n: Node) -> Result<P, Error> {
        let eager = match &n {
            Node::Filter { bind, .. } => self.red(bind.domain) == Red::Yes,
            Node::FcnCons { binds, .. } => binds.iter().all(|b| self.red(b.domain) == Red::Yes),
            Node::SetBin(SetOp::Minus, a, _) => self.red(*a) == Red::Yes,
            Node::SetBin(_, a, b) => self.red(*a) == Red::Yes && self.red(*b) == Red::Yes,
            _ => true,
        };
        let p = self.push(n)?;
        if eager {
            Ok(p)
        } else {
            self.stats.speculative += 1;
            self.push(Node::Spec(p))
        }
    }

    /// Whether a set node is one TLC certainly keeps lazy, a filter or a
    /// set operation over non-reducible sets: membership in it is decided
    /// structurally (`Node::InLazy`), as TLC decides it.
    fn lazy_set(&self, p: P) -> bool {
        let p = self.unspec(p);
        match self.code.nodes.get(p as usize) {
            Some(Node::Filter { .. } | Node::SetBin(..)) => self.red(p) == Red::No,
            Some(Node::Lazy { body, .. } | Node::Scope { body, .. }) => self.lazy_set(*body),
            _ => false,
        }
    }

    /// The function constructor a function expression leads to through
    /// thunks, scopes and speculation wrappers, if any.
    fn lambda(&self, mut p: P) -> Option<P> {
        loop {
            match self.code.nodes.get(p as usize)? {
                Node::Spec(b) | Node::Lazy { body: b, .. } | Node::Scope { body: b, .. } => p = *b,
                Node::FcnCons { .. } => return Some(p),
                _ => return None,
            }
        }
    }

    /// `f[a]`. TLC evaluates `f` with `KEEP_LAZY`, so a function
    /// constructor reached directly, or through a definition, stays lazy,
    /// and so does one over a non-reducible domain. One reached through a
    /// `LET` value or an operator argument (a `LazyValue`) over reducible
    /// domains is built eagerly (fixture `LzApplyDef`). A lazy one with one
    /// bound variable is applied without being built (`Node::Beta`); a
    /// lazy one of several is built speculatively.
    fn apply_value(&mut self, f: P, a: P) -> Result<P, Error> {
        let Some(fc) = self.lambda(f) else {
            return self.push(Node::Apply(f, a));
        };
        let Some(Node::FcnCons { binds, .. }) = self.code.nodes.get(fc as usize) else {
            return self.push(Node::Apply(f, a));
        };
        let single = binds.len() == 1 && binds[0].slots.len() == 1 && !binds[0].tuple;
        let non_reducible = binds.iter().any(|b| self.red(b.domain) == Red::No);
        let mut thunk = false;
        let mut p = f;
        while let Some(
            n @ (Node::Spec(b) | Node::Lazy { body: b, .. } | Node::Scope { body: b, .. }),
        ) = self.code.nodes.get(p as usize)
        {
            if let Node::Lazy { slot, .. } = n {
                thunk |= !self.def_slots.contains(slot);
            }
            p = *b;
        }
        if !non_reducible && thunk {
            // Built eagerly, as TLC builds it (speculatively already when
            // its domains may not be reducible: `push_lazy`).
            return self.push(Node::Apply(f, a));
        }
        if single {
            self.stats.beta += 1;
            return self.push(Node::Beta {
                chain: f,
                fc,
                arg: a,
            });
        }
        let f = if matches!(self.code.nodes.get(f as usize), Some(Node::Spec(_))) {
            f
        } else {
            self.stats.speculative += 1;
            self.push(Node::Spec(f))?
        };
        self.push(Node::Apply(f, a))
    }

    /// `DOMAIN f` of a function constructor TLC certainly keeps lazy (over
    /// a non-reducible domain): its domain, without building it
    /// (`FcnLambdaValue.getDomain`). `None` otherwise.
    fn lambda_domain(&mut self, f: P) -> Result<Option<P>, Error> {
        let Some(fc) = self.lambda(f) else {
            return Ok(None);
        };
        let Some(Node::FcnCons { binds, .. }) = self.code.nodes.get(fc as usize) else {
            return Ok(None);
        };
        if !binds.iter().any(|b| self.red(b.domain) == Red::No) {
            return Ok(None);
        }
        if binds.len() == 1 && (binds[0].tuple || binds[0].slots.len() == 1) {
            return Ok(Some(binds[0].domain));
        }
        let mut ds = Vec::new();
        for b in binds {
            if b.tuple {
                ds.push(b.domain);
            } else {
                ds.extend(std::iter::repeat_n(b.domain, b.slots.len()));
            }
        }
        Ok(Some(self.push(Node::Product(ds.into()))?))
    }

    /// The recursive function a function expression names, if it is the
    /// name of `f[x \in S] == e` (outside or inside its body), compiled as
    /// a function of its argument.
    fn rec_target(&mut self, fcn: ExprId, env: usize) -> Result<Option<(DefId, u32)>, Error> {
        let d = match &self.ir.node(fcn).expr {
            Expr::Local(r) => {
                let fe = self.frame_env(env, r.up)?;
                let Some(Sb::RecSelf(f)) = self.envs[fe].slots.get(r.slot.index()) else {
                    return Ok(None);
                };
                let f = *f;
                if self.lifts.last().is_some_and(|l| fe < l.floor) {
                    return Err(Error::unsupported(
                        "a recursive function used inside a nested recursive definition (M1 core)",
                    ));
                }
                let d = self.envs[fe].def;
                return Ok(Some((d, f)));
            }
            Expr::Def(d) => *d,
            Expr::Apply {
                callee: OpRef::Def(d),
                args,
            } if args.is_empty() => *d,
            _ => return Ok(None),
        };
        let d = self.redirect(d);
        if self.precomputed(d)?.is_some() {
            return Ok(None);
        }
        let def = self.ir.def(d);
        if !def.params.is_empty() || def.std.is_some() {
            return Ok(None);
        }
        let Expr::RecFcn { .. } = &self.ir.node(def.body).expr else {
            return Ok(None);
        };
        self.recfun(d, env).map(|f| Some((d, f)))
    }

    /// Compiles `f[x \in S] == e` as a function of its argument, in slot 0:
    /// `DomCheck { 0, S, e }`, with `f[...]` inside `e` a recursive call.
    fn recfun(&mut self, d: DefId, call_env: usize) -> Result<u32, Error> {
        let key = (d, self.init_mode);
        if let Some(&f) = self.recfuns.get(&key) {
            return Ok(f);
        }
        let def = self.ir.def(d);
        let Expr::RecFcn { name, bounds, body } = &self.ir.node(def.body).expr else {
            return Err(Error::internal("not a recursive function definition"));
        };
        let (name, body) = (*name, *body);
        let [b] = &bounds[..] else {
            return Err(Error::unsupported(
                "a recursive function of several bound groups (M1 core)",
            ));
        };
        let ([x], false, Some(domain)) = (&b.vars[..], b.tuple, b.domain) else {
            return Err(Error::unsupported(
                "a recursive function of several arguments or a tuple pattern (M1 core)",
            ));
        };
        let x = *x;
        #[allow(clippy::cast_possible_truncation, reason = "bounded")]
        let id = self.code.funs.len() as u32;
        self.code.funs.push(Fun {
            name: def.name.clone(),
            params: 1,
            frame: 1,
            body: 0,
            caps: Box::new([]),
        });
        self.recfuns.insert(key, id);
        let saved_slot = self.next_slot;
        let saved_resets = std::mem::take(&mut self.reset_stack);
        let saved_cache = std::mem::take(&mut self.def_cache);
        let saved_at = std::mem::take(&mut self.at_stack);
        self.next_slot = 1;
        let (env, lifted) = match self.begin_fun(d, id, call_env) {
            Ok(x) => x,
            Err(e) => {
                self.next_slot = saved_slot;
                self.reset_stack = saved_resets;
                self.def_cache = saved_cache;
                self.at_stack = saved_at;
                return Err(e);
            }
        };
        if let Some(s) = self.envs[env].slots.get_mut(name.index()) {
            *s = Sb::RecSelf(id);
        }
        if let Some(s) = self.envs[env].slots.get_mut(x.index()) {
            *s = Sb::Flat(0);
        }
        let r = self
            .p(domain, env, Mode::Plain)
            .and_then(|dom| Ok((dom, self.p(body, env, Mode::Plain)?)));
        let ended = self.end_fun(id, lifted);
        let frame = self.next_slot;
        self.next_slot = saved_slot;
        self.reset_stack = saved_resets;
        self.def_cache = saved_cache;
        self.at_stack = saved_at;
        let (dom, b) = r?;
        ended?;
        let checked = self.push(Node::DomCheck {
            slot: 0,
            domain: dom,
            body: b,
        })?;
        if let Some(f) = self.code.funs.get_mut(id as usize) {
            f.body = checked;
            f.frame = frame;
        }
        Ok(id)
    }

    /// The node under any `Node::Spec` wrappers.
    fn unspec(&self, mut p: P) -> P {
        while let Some(Node::Spec(b)) = self.code.nodes.get(p as usize) {
            p = *b;
        }
        p
    }

    /// Refuses an effect inside an expression whose evaluation order or
    /// count is TLC's representation choice rather than the expression's:
    /// a function constructor, filter or set map (TLC may build it eagerly,
    /// lazily or once per application), a speculative evaluation, and an
    /// operator argument of a library operator (applied in TLC's internal
    /// order).
    fn effect_allowed(&self, n: &Node) -> Result<(), Error> {
        let inside = |p: P| self.info.get(p as usize).is_some_and(|i| i.effect);
        let lazy = match n {
            Node::FcnCons { body, .. } | Node::Map { body, .. } => inside(*body),
            Node::Filter { pred, .. } => inside(*pred),
            Node::Spec(b) => inside(*b),
            Node::LibHo(_, args) => args
                .iter()
                .any(|a| matches!(a, LibArg::Op { body, .. } if inside(*body))),
            _ => false,
        };
        if lazy {
            return Err(Error::unsupported(
                "Print or PrintT inside a function constructor, set filter, set map or operator argument, which TLC may evaluate lazily or in its own order (M1 core)",
            ));
        }
        Ok(())
    }

    /// Whether a node may build a value TLC keeps as an interval or a lazy
    /// set, which printing must know (`Store::note_lazy_sets`).
    fn builds_lazy_set(&self, n: &Node) -> bool {
        match n {
            Node::Range(..)
            | Node::Domain(_)
            | Node::Powerset(_)
            | Node::FcnSet(..)
            | Node::RecSet(_)
            | Node::Product(_) => true,
            Node::Const(h) => self.holds_lazy_set(*h, &mut 0),
            _ => false,
        }
    }

    /// Whether a constant holds an interval or a lazy set, looking at most
    /// at [`LAZY_SCAN`] nodes (beyond, it is assumed to).
    fn holds_lazy_set(&self, h: Handle, seen: &mut usize) -> bool {
        if !h.is_compound() {
            return false;
        }
        *seen += 1;
        if *seen > LAZY_SCAN {
            return true;
        }
        let v = self.m.heap.view(h);
        if matches!(
            v.kind,
            Kind::Interval | Kind::Subset | Kind::FcnSet | Kind::RecSet | Kind::Product
        ) {
            return true;
        }
        let data = v.data.to_vec();
        data.into_iter().any(|d| self.holds_lazy_set(d, seen))
    }

    fn push(&mut self, n: Node) -> Result<P, Error> {
        if self.code.nodes.len() >= MAX_NODES {
            return Err(Error::unsupported(
                "a plan larger than the M0 core's inlining limit",
            ));
        }
        let info = self.info_of(&n);
        if !self.m.heap.store().lazy_sets() && self.builds_lazy_set(&n) {
            self.m.heap.store().note_lazy_sets();
        }
        if info.effect {
            self.effect_allowed(&n)?;
            self.effects = true;
        }
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
            && !info.heavy
            && !info.effect;
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
                // The folded node keeps TLC's reducibility of the expression.
                let red = self.info[id as usize].red;
                self.info[id as usize] = Info {
                    red,
                    ..Info::default()
                };
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
        if info.primed != 0
            || info.effect
            || matches!(self.code.nodes.get(p as usize), Some(Node::Const(_)))
        {
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

    /// Binds the `LET` values of a role's frame that splitting left unbound
    /// (the role sits inside their `LET`): lazy values, as TLC's
    /// `getActions` binds them, forced at most once per evaluation of the
    /// action (its frame starts empty).
    fn bind_role_lets(&mut self, env: usize, def: DefId) {
        let owner = self.reset_stack.len().saturating_sub(1);
        for (i, info) in self.ir.def(def).frame.iter().enumerate() {
            let dive_ir::SlotKind::LetValue(d) = info.kind else {
                continue;
            };
            if let Some(s @ Sb::Unset) = self.envs[env].slots.get_mut(i) {
                *s = Sb::LetVal {
                    def: d,
                    env,
                    thunk: None,
                    owner,
                };
            }
        }
    }

    fn role_env(&mut self, r: &Role) -> Result<(usize, Vec<(u32, Handle)>), Error> {
        self.next_slot = 0;
        self.role_env_after(r)
    }

    /// [`Self::role_env`] for a role compiled into the same frame as the
    /// previous one: its slots follow the previous role's.
    fn role_env_after(&mut self, r: &Role) -> Result<(usize, Vec<(u32, Handle)>), Error> {
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
        self.reset_stack.push(Vec::new());
        self.bind_role_lets(env, r.def);
        // Substitutions around the role: values are evaluated by name in
        // their own frame, outside any substitution (TLC's generator binds
        // them uncached: B5).
        self.substs.clear();
        for rs in &r.substs {
            let entry = match rs {
                RoleSubst::Value { decl, value } => {
                    let h = self.convert(value)?;
                    let p = self.konst(h)?;
                    SubstEntry {
                        decl: *decl,
                        value: Err(p),
                        env,
                        how: ArgHow::Alias(p),
                        depth: 0,
                    }
                }
                RoleSubst::Expr {
                    decl,
                    expr,
                    def,
                    slots,
                } => {
                    if self.ir.def(*def).depth != 0 {
                        return Err(Error::unsupported(
                            "an INSTANCE substitution inside a LET or LAMBDA (M1 core)",
                        ));
                    }
                    let senv = self.new_env(*def, None);
                    for (slot, v) in slots {
                        let flat = self.alloc();
                        let h = self.convert(v)?;
                        if let Some(s) = self.envs[senv].slots.get_mut(slot.index()) {
                            *s = Sb::Flat(flat);
                        }
                        preset.push((flat, h));
                    }
                    self.bind_role_lets(senv, *def);
                    SubstEntry {
                        decl: *decl,
                        value: Ok(Arg::Expr {
                            expr: *expr,
                            pass: dive_ir::Pass::Lazy,
                        }),
                        env: senv,
                        how: ArgHow::ByName,
                        depth: 0,
                    }
                }
            };
            self.substs.push(entry);
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

    /// The initial predicates, conjoined left to right into one action
    /// named after the first.
    fn init_action(&mut self, roles: &[Role]) -> Result<Action, Error> {
        let [first, rest @ ..] = roles else {
            return Err(Error::internal("no initial predicate"));
        };
        if rest.is_empty() {
            return self.action(first, true);
        }
        self.init_mode = true;
        let mut items = Vec::with_capacity(roles.len());
        let mut presets = Vec::new();
        for (i, r) in roles.iter().enumerate() {
            let (env, preset) = if i == 0 {
                self.role_env(r)?
            } else {
                self.role_env_after(r)?
            };
            presets.extend(preset);
            let g = self.g(r.expr, env)?;
            let p = self.p(r.expr, env, Mode::Plain)?;
            items.push((g, p));
        }
        let g = self.gpush(GNode::Conj(items.into()));
        Ok(Action {
            name: first.name.clone(),
            g,
            frame: self.next_slot,
            preset: presets,
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
            let skippable = info.free.is_empty() && info.primed == 0 && !info.effect;
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

    /// A step predicate: a check whose conjuncts are never skipped, since
    /// the parent-delta skip compares a state with its parent and a step
    /// predicate has no such parent.
    fn step_check(&mut self, r: &Role) -> Result<Check, Error> {
        let mut k = self.check(r)?;
        k.conjuncts.iter_mut().for_each(|j| j.skippable = false);
        Ok(k)
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
                if self.m.heap.int_mode() == crate::int::IntMode::Tlc && s.parse::<i32>().is_err() {
                    // TLC rejects the numeral before checking starts.
                    return Err(Error::unsupported(format!(
                        "the numeral {s}, outside TLC's 32-bit integers (--compat=tlc)"
                    )));
                }
                let v = if let Some(h) = s.parse::<i64>().ok().and_then(Handle::int) {
                    h
                } else {
                    // Outside the inline range: a big integer constant (D-004).
                    let b = s
                        .parse::<num_bigint::BigInt>()
                        .map_err(|_| Error::unsupported(format!("the numeral {s}")))?;
                    let h = self.m.heap.int_from_big(&b);
                    let h = self.m.heap.intern_const(h);
                    self.m.heap.reset();
                    h?
                };
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
            Expr::Const(c) => {
                let c = *c;
                match self.h.constants.get(c.index()).cloned().flatten() {
                    Some(ConstBinding::Def(d)) => self.def_use(d, env, mode),
                    _ => self.const_expr(c),
                }
            }
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
                self.push_lazy(Node::Filter { bind, pred: b })
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
                self.push_lazy(Node::FcnCons {
                    binds: binds.into(),
                    body: b,
                })
            }
            Expr::RecFcn { .. } => Err(Error::unsupported(
                "a recursive function used other than applied (M1 core)",
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
                let (fcn, arg) = (*fcn, *arg);
                if let Some((d, fun)) = self.rec_target(fcn, env)? {
                    if mode == Mode::Primed {
                        return Err(Error::unsupported(
                            "a recursive function under a prime (M1 core)",
                        ));
                    }
                    let a = self.p(arg, env, mode)?;
                    return self.fun_call(d, fun, vec![a], env);
                }
                let f = self.p(fcn, env, mode)?;
                let a = self.p(arg, env, mode)?;
                self.apply_value(f, a)
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
            Expr::Subst { bindings, body } => {
                let (bindings, body) = (bindings.clone(), *body);
                let (b, reset) =
                    self.subst_scope(&bindings, env, mode, |c| c.p(body, env, mode))?;
                if reset.is_empty() {
                    Ok(b)
                } else {
                    self.push(Node::Scope {
                        reset: reset.into(),
                        body: b,
                    })
                }
            }
            Expr::SubstRef { decl, primed } => {
                let (decl, primed) = (*decl, *primed);
                if primed && mode == Mode::Primed {
                    return Err(Error::unsupported("a doubly primed expression"));
                }
                let mode = if primed { Mode::Primed } else { mode };
                self.subst_ref(decl, mode)
            }
            // `[A]_v` is `A \/ UNCHANGED v`; `<<A>>_v` is `A /\ v' # v`
            // (TLC's `OPCODE_sa`, `OPCODE_aa` in `eval`).
            Expr::StepBox { action, sub } | Expr::StepAngle { action, sub } => {
                if mode == Mode::Primed || self.init_mode {
                    return Err(Error::unsupported(
                        "[A]_v under a prime or in an initial predicate (M1 core)",
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

    /// A constant bound to a value by the configuration.
    fn const_expr(&mut self, c: dive_ir::ConstId) -> Result<P, Error> {
        match self.h.constants.get(c.index()).cloned().flatten() {
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
            Some(ConstBinding::Def(_)) => Err(Error::unsupported(
                "a constant substituted by a definition, inside an instance (M1 core)",
            )),
            None => Err(Error::unsupported("an unbound constant")),
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
        if self.lifts.last().is_some_and(|l| fe < l.floor) {
            return self.capture(fe, r.slot.index(), mode);
        }
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
            Sb::RecSelf(_) => Err(Error::unsupported(
                "a recursive function used other than applied (M1 core)",
            )),
            Sb::OpArg { .. } => Err(Error::internal("an operator parameter used as a value")),
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

    /// A reference, inside a lifted function, to a local of an enclosing
    /// definition: a capture slot of the function.
    fn capture(&mut self, fe: usize, slot: usize, mode: Mode) -> Result<P, Error> {
        if mode == Mode::Primed {
            return Err(Error::unsupported(
                "a recursive LET definition reading an enclosing local under a prime (M1 core)",
            ));
        }
        if matches!(self.envs[fe].slots.get(slot), Some(Sb::RecSelf(_))) {
            return Err(Error::unsupported(
                "a recursive function used inside a nested recursive definition (M1 core)",
            ));
        }
        let Some(l) = self.lifts.last() else {
            return Err(Error::internal("a capture outside a lifted function"));
        };
        let mut k = 0u32;
        let mut e = l.parent;
        loop {
            match e {
                Some(x) if x == fe => break,
                Some(x) => {
                    e = self.envs[x].parent;
                    k += 1;
                }
                None => return Err(Error::internal("a capture outside the definition's scope")),
            }
        }
        if let Some(&(_, _, c)) = l.caps.iter().find(|c| c.0 == k && c.1 == slot) {
            return self.push(Node::Slot(c));
        }
        let c = self.alloc();
        if let Some(l) = self.lifts.last_mut() {
            l.caps.push((k, slot, c));
        }
        self.push(Node::Slot(c))
    }

    /// A call of a compiled function: the arguments, then the captures
    /// (compiled at the call site), or, inside the function itself, its own
    /// capture slots (filled in when its body is complete).
    fn fun_call(&mut self, d: DefId, fun: u32, mut ps: Vec<P>, env: usize) -> Result<P, Error> {
        if let Some(i) = self.lifts.iter().position(|l| l.fun == fun) {
            if i + 1 != self.lifts.len() {
                return Err(Error::unsupported(
                    "mutually recursive nested definitions (M1 core)",
                ));
            }
            let call = self.push(Node::Call {
                fun,
                args: ps.into(),
            })?;
            if let Some(l) = self.lifts.last_mut() {
                l.self_calls.push(call);
            }
            return Ok(call);
        }
        let caps = self.fun_caps.get(&fun).cloned().unwrap_or_default();
        if !caps.is_empty() {
            let Some(parent) = self.lexical_parent(d, env)? else {
                return Err(Error::internal("captures of a top-level definition"));
            };
            for (k, slot) in caps {
                #[allow(clippy::cast_possible_truncation, reason = "frame slots are u32")]
                let r = LocalRef {
                    up: k,
                    slot: dive_ir::Slot(slot as u32),
                };
                let v = self.local(r, parent, Mode::Plain)?;
                let v = if self.is_trivial(v) {
                    v
                } else {
                    self.stats.speculative += 1;
                    self.push(Node::Spec(v))?
                };
                ps.push(v);
            }
        }
        self.push(Node::Call {
            fun,
            args: ps.into(),
        })
    }

    /// Starts compiling a function for `d`: its environment, with the
    /// lexical parent for a nested definition (whose enclosing locals then
    /// become captures).
    fn begin_fun(&mut self, d: DefId, fun: u32, env: usize) -> Result<(usize, bool), Error> {
        let parent = self.lexical_parent(d, env)?;
        let inner = self.new_env(d, parent);
        let lifted = parent.is_some();
        if lifted {
            self.lifts.push(Lift {
                floor: inner,
                parent,
                fun,
                caps: Vec::new(),
                self_calls: Vec::new(),
            });
        }
        Ok((inner, lifted))
    }

    /// Ends `begin_fun`: records the captures and completes the recursive
    /// calls with them.
    fn end_fun(&mut self, fun: u32, lifted: bool) -> Result<(), Error> {
        if !lifted {
            return Ok(());
        }
        let Some(l) = self.lifts.pop() else {
            return Err(Error::internal("unbalanced lifted function"));
        };
        let slots: Vec<u32> = l.caps.iter().map(|c| c.2).collect();
        for &call in &l.self_calls {
            let mut extra = Vec::with_capacity(slots.len());
            for &c in &slots {
                extra.push(self.push(Node::Slot(c))?);
            }
            if let Some(Node::Call { args, .. }) = self.code.nodes.get_mut(call as usize) {
                let mut v = args.to_vec();
                v.extend(extra);
                *args = v.into();
            }
        }
        if let Some(f) = self.code.funs.get_mut(fun as usize) {
            f.caps = slots.clone().into();
        }
        self.fun_caps
            .insert(fun, l.caps.iter().map(|c| (c.0, c.1)).collect());
        Ok(())
    }

    /// Binds an instance's substitutions for `f`, as operator arguments are
    /// bound (`inline`): each expression value is compiled outside the
    /// substitution, then aliased, made a thunk, or passed by name. Returns
    /// `f`'s result with the thunk slots to reset on entry.
    fn subst_scope<T>(
        &mut self,
        bindings: &[dive_ir::SubstBinding],
        env: usize,
        mode: Mode,
        f: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<(T, Vec<u32>), Error> {
        let depth = self.substs.len();
        let mut reset = Vec::new();
        let mut entries = Vec::with_capacity(bindings.len());
        for b in bindings {
            let how = match &b.value {
                Arg::Expr { expr, .. } => {
                    let cp = self.p(*expr, env, mode)?;
                    if self.opts.eager && self.is_trivial(cp) {
                        self.stats.aliased += 1;
                        ArgHow::Alias(cp)
                    } else if self.info[cp as usize].primed == 0 {
                        self.stats.thunks += 1;
                        let slot = self.alloc();
                        reset.push(slot);
                        ArgHow::Thunk(slot, cp)
                    } else {
                        ArgHow::ByName
                    }
                }
                Arg::Op { .. } => ArgHow::ByName,
            };
            entries.push(SubstEntry {
                decl: b.decl,
                value: Ok(b.value.clone()),
                env,
                how,
                depth,
            });
        }
        self.stats.substituted += entries.len();
        self.substs.extend(entries);
        let r = f(self);
        self.substs.truncate(depth);
        Ok((r?, reset))
    }

    /// Runs `f` in the dynamic context of depth `depth` (outside the
    /// substitutions above it).
    fn outside<T>(&mut self, depth: usize, f: impl FnOnce(&mut Self) -> T) -> T {
        let inner = self.substs.split_off(depth.min(self.substs.len()));
        let r = f(self);
        self.substs.extend(inner);
        r
    }

    /// The operator an operator declaration stands for: the innermost
    /// active substitution, followed through substitutions by declarations
    /// of enclosing instances (`INSTANCE M WITH F <- F`, nested), each
    /// looked up in the context outside its own substitution.
    fn subst_op(&self, decl: dive_ir::DeclId) -> Result<OpRef, Error> {
        let mut decl = decl;
        let mut limit = self.substs.len();
        loop {
            let Some(i) = self.substs[..limit].iter().rposition(|s| s.decl == decl) else {
                return match self.ir.decl(decl).constant {
                    Some(c) => Ok(OpRef::Const(c)),
                    None => Err(Error::unsupported(
                        "an unsubstituted operator declaration (M1 core)",
                    )),
                };
            };
            let s = &self.substs[i];
            match &s.value {
                Ok(Arg::Op {
                    op: OpRef::Subst(d2),
                    ..
                }) => {
                    decl = *d2;
                    limit = s.depth.min(i);
                }
                Ok(Arg::Op {
                    op: op @ (OpRef::Def(_) | OpRef::Std(_) | OpRef::Const(_)),
                    ..
                }) if !matches!(op, OpRef::Def(d) if self.ir.def(self.redirect(*d)).scope != DefScope::TopLevel) =>
                {
                    return Ok(op.clone());
                }
                _ => {
                    return Err(Error::unsupported(
                        "an INSTANCE substitution of an operator by a non-top-level operator (M1 core)",
                    ));
                }
            }
        }
    }

    /// A reference to a substituted declaration: the innermost active
    /// substitution, or else the declaration itself (a root variable or a
    /// constant).
    fn subst_ref(&mut self, decl: dive_ir::DeclId, mode: Mode) -> Result<P, Error> {
        let Some(i) = self.substs.iter().rposition(|s| s.decl == decl) else {
            let d = self.ir.decl(decl);
            #[allow(clippy::cast_possible_truncation, reason = "at most 64 variables")]
            return match (d.var, d.constant) {
                (Some(v), _) if self.init_mode => {
                    if mode == Mode::Primed {
                        return Err(Error::unsupported(
                            "a primed variable in an initial predicate",
                        ));
                    }
                    self.push(Node::Primed(v.index() as u16))
                }
                (Some(v), _) => match mode {
                    Mode::Plain => self.push(Node::Var(v.index() as u16)),
                    Mode::Primed => self.push(Node::Primed(v.index() as u16)),
                },
                (None, Some(c)) => {
                    let e = self.const_expr(c)?;
                    Ok(e)
                }
                (None, None) => Err(Error::unsupported(format!(
                    "the unsubstituted variable {} of an instantiated module (M1 core)",
                    d.name
                ))),
            };
        };
        let SubstEntry {
            value,
            env,
            how,
            depth,
            ..
        } = self.substs[i].clone();
        let expr = match value {
            Ok(Arg::Expr { expr, .. }) => expr,
            Err(p) => return Ok(p),
            Ok(Arg::Op { .. }) => {
                return Err(Error::unsupported(
                    "an operator substitution used as a value (M1 core)",
                ));
            }
        };
        match (how, mode) {
            (ArgHow::Alias(p), Mode::Plain) => Ok(p),
            (ArgHow::Alias(p), Mode::Primed) => match self.code.nodes.get(p as usize) {
                Some(Node::Var(v)) => {
                    let v = *v;
                    self.push(Node::Primed(v))
                }
                Some(Node::Const(_) | Node::Slot(_)) => Ok(p),
                _ => {
                    self.stats.by_name += 1;
                    self.outside(depth, |c| c.p(expr, env, Mode::Primed))
                }
            },
            (ArgHow::Thunk(slot, body), Mode::Plain) => self.push(Node::Lazy { slot, body }),
            (ArgHow::Thunk(..), Mode::Primed) | (ArgHow::ByName, _) => {
                self.stats.by_name += 1;
                self.outside(depth, |c| c.p(expr, env, mode))
            }
        }
    }

    /// Whether the innermost substitution of a declaration is a value
    /// (`RoleSubst::Value`).
    fn subst_is_value(&self, decl: dive_ir::DeclId) -> bool {
        self.substs
            .iter()
            .rev()
            .find(|s| s.decl == decl)
            .is_some_and(|s| s.value.is_err())
    }

    /// The substitution for a declaration, if one is active: its expression,
    /// environment and dynamic depth.
    fn subst_expr(&self, decl: dive_ir::DeclId) -> Option<Result<(ExprId, usize, usize), Error>> {
        let i = self.substs.iter().rposition(|s| s.decl == decl)?;
        let s = &self.substs[i];
        Some(match &s.value {
            Ok(Arg::Expr { expr, .. }) => Ok((*expr, s.env, s.depth)),
            Ok(Arg::Op { .. }) => Err(Error::unsupported(
                "an operator substitution used as a value (M1 core)",
            )),
            Err(_) => Err(Error::unsupported(
                "a substitution by a value used as an action or a variable (M1 core)",
            )),
        })
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
        if cacheable
            && info.free.is_empty()
            && info.primed == 0
            && !info.effect
            && !self.is_trivial(body)
        {
            let slot = self.alloc();
            self.def_cache.insert(d, (slot, body));
            self.def_slots.insert(slot);
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
                self.inline_apply(d, args, env, env, mode)
            }
            OpRef::Std(op) => {
                if let Some(&t) = self.h.std_redirects.get(op) {
                    return self.apply(&OpRef::Def(t), args, env, mode);
                }
                self.std_apply(*op, args, env, mode)
            }
            OpRef::Param(r) => self.apply_param(*r, args, env, env, mode),
            OpRef::Builtin(op) => Err(Error::unsupported(format!(
                "the built-in {} as an operator argument (M0 core)",
                op.sany_name()
            ))),
            // `CONSTANT F(_)` with `F <- D` in the configuration.
            OpRef::Const(c) => match self.h.constants.get(c.index()).cloned().flatten() {
                Some(ConstBinding::Def(d)) => self.apply(&OpRef::Def(d), args, env, mode),
                _ => Err(Error::unsupported(
                    "a constant operator the configuration does not bind to a definition (M1 core)",
                )),
            },
            // The operator is resolved outside the substitution; the
            // arguments are compiled here, and TLC applies the operator in
            // the current (dynamic) context.
            OpRef::Subst(decl) => {
                let op = self.subst_op(*decl)?;
                self.apply(&op, args, env, mode)
            }
            OpRef::Unbound(n) => Err(Error::unsupported(format!("the unbound operator {n}"))),
        }
    }

    /// Inlines a non-recursive operator applied to `args`, which are
    /// compiled in `env`; the operator's lexical parent is found from
    /// `def_env` (the caller's, or where an operator argument was passed).
    fn inline_apply(
        &mut self,
        d: DefId,
        args: &[Arg],
        env: usize,
        def_env: usize,
        mode: Mode,
    ) -> Result<P, Error> {
        let body = self.ir.def(d).body;
        let (inner, reset) = self.inline(d, args, env, def_env, mode)?;
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

    /// Inlines a non-recursive operator applied as an action (see
    /// [`Self::inline_apply`]).
    fn g_inline(&mut self, d: DefId, args: &[Arg], env: usize, def_env: usize) -> Result<G, Error> {
        let body = self.ir.def(d).body;
        let (inner, reset) = self.inline(d, args, env, def_env, Mode::Plain)?;
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

    /// Applies, as an action, an operator resolved from a constant, a
    /// substitution or a parameter; `aenv` is where it was resolved.
    fn g_apply_op(
        &mut self,
        op: &OpRef,
        args: &[Arg],
        e: ExprId,
        env: usize,
        aenv: usize,
    ) -> Result<G, Error> {
        let d = match *op {
            OpRef::Def(d) => d,
            OpRef::Const(c) => match self.h.constants.get(c.index()).cloned().flatten() {
                Some(ConstBinding::Def(d)) => d,
                _ => return self.guard_expr(e, env),
            },
            _ => return self.guard_expr(e, env),
        };
        let d = self.redirect(d);
        let def = self.ir.def(d);
        if def.std.is_some() || def.recursive || args.is_empty() {
            return self.guard_expr(e, env);
        }
        let def_env = if def.scope == DefScope::TopLevel {
            env
        } else {
            aenv
        };
        self.g_inline(d, args, env, def_env)
    }

    /// An operator argument as a function of `arity` fresh frame slots, for
    /// the standard modules' higher-order operators (`SelectSeq`, the
    /// Folds, ...): the slots, and its body compiled with its parameters
    /// bound to them. The caller sets the slots and evaluates the body for
    /// each element. The body's free slots include the parameter slots,
    /// which the caller's node must treat as bound, as a binder's are.
    ///
    /// The operator is resolved as an application of it would be:
    /// - through operator parameters passed on;
    /// - through constant operators bound in the configuration
    ///   (`F <- D`);
    /// - through `INSTANCE` substitutions.
    ///
    /// A `LAMBDA` or `LET` operator's body is compiled in the environment
    /// where it was written, so it reads the caller's locals.
    ///
    /// # Errors
    /// Unsupported for a std or built-in operator passed as the argument
    /// (`FoldSet(+, 0, S)`), a recursive operator, or an expression
    /// argument; internal for an arity mismatch.
    pub(crate) fn op_fn(
        &mut self,
        a: &Arg,
        env: usize,
        arity: usize,
    ) -> Result<(Box<[u32]>, P), Error> {
        let Arg::Op { op, .. } = a else {
            return Err(Error::unsupported(
                "an expression where an operator argument is expected (M1 core)",
            ));
        };
        let (op, aenv) = match op {
            OpRef::Param(r) => self.resolve_param(*r, env)?,
            OpRef::Subst(decl) => (self.subst_op(*decl)?, env),
            op => (op.clone(), env),
        };
        let d = match op {
            OpRef::Def(d) => d,
            OpRef::Const(c) => match self.h.constants.get(c.index()).cloned().flatten() {
                Some(ConstBinding::Def(d)) => d,
                _ => {
                    return Err(Error::unsupported(
                        "a constant operator the configuration does not bind to a definition (M1 core)",
                    ));
                }
            },
            _ => {
                return Err(Error::unsupported(
                    "a std or built-in operator passed to a higher-order operator (M1 core)",
                ));
            }
        };
        let d = self.redirect(d);
        let def = self.ir.def(d);
        if def.std.is_some() || def.recursive {
            return Err(Error::unsupported(
                "a std or recursive operator passed to a higher-order operator (M1 core)",
            ));
        }
        if def.params.len() != arity {
            return Err(Error::internal(format!(
                "{} passed where an operator of {arity} arguments is expected",
                def.name
            )));
        }
        let body = def.body;
        let def_env = if def.scope == DefScope::TopLevel {
            env
        } else {
            aenv
        };
        let parent = self.lexical_parent(d, def_env)?;
        let inner = self.new_env(d, parent);
        let mut slots = Vec::with_capacity(arity);
        for i in 0..arity {
            let slot = self.alloc();
            if let Some(s) = self.envs[inner].slots.get_mut(i) {
                *s = Sb::Flat(slot);
            }
            slots.push(slot);
        }
        let b = self.p(body, inner, Mode::Plain)?;
        Ok((slots.into(), b))
    }

    /// The operator argument an operator parameter is bound to, following
    /// parameters passed on, and the environment it was passed in.
    fn resolve_param(&self, r: LocalRef, look_env: usize) -> Result<(OpRef, usize), Error> {
        let mut r = r;
        let mut look = look_env;
        loop {
            let fe = self.frame_env(look, r.up)?;
            if self.lifts.last().is_some_and(|l| fe < l.floor) {
                return Err(Error::unsupported(
                    "an enclosing operator parameter inside a recursive LET definition (M1 core)",
                ));
            }
            let Some(Sb::OpArg { op, env }) = self.envs[fe].slots.get(r.slot.index()).cloned()
            else {
                return Err(Error::unsupported(
                    "an operator parameter of a recursive operator (M1 core)",
                ));
            };
            match op {
                OpRef::Param(r2) => {
                    r = r2;
                    look = env;
                }
                op => return Ok((op, env)),
            }
        }
    }

    /// Applies an operator parameter: the operator argument bound to it.
    /// The arguments are compiled in `env`; the parameter is looked up from
    /// `look_env`. TLC applies the argument as a closure over the caller's
    /// context, which inlining it there reproduces.
    fn apply_param(
        &mut self,
        r: LocalRef,
        args: &[Arg],
        env: usize,
        look_env: usize,
        mode: Mode,
    ) -> Result<P, Error> {
        let (op, aenv) = self.resolve_param(r, look_env)?;
        match op {
            OpRef::Param(_) => Err(Error::internal("an unresolved operator parameter")),
            OpRef::Def(d) => {
                let d = self.redirect(d);
                let def = self.ir.def(d);
                if def.std.is_some() || def.scope == DefScope::TopLevel {
                    self.apply(&OpRef::Def(d), args, env, mode)
                } else if def.recursive {
                    Err(Error::unsupported(
                        "a recursive LET operator passed as an operator argument (M1 core)",
                    ))
                } else {
                    self.inline_apply(d, args, env, aenv, mode)
                }
            }
            OpRef::Subst(_) => Err(Error::unsupported(
                "an INSTANCE-substituted operator passed as an operator argument (M1 core)",
            )),
            op @ (OpRef::Std(_) | OpRef::Builtin(_) | OpRef::Const(_) | OpRef::Unbound(_)) => {
                self.apply(&op, args, env, mode)
            }
        }
    }

    /// Binds an inlined operator's parameters: returns its environment and
    /// the thunk slots to reset on entry.
    fn inline(
        &mut self,
        d: DefId,
        args: &[Arg],
        env: usize,
        def_env: usize,
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
        let parent = self.lexical_parent(d, def_env)?;
        let inner = self.new_env(d, parent);
        let mut reset = Vec::new();
        self.stats.inlined += 1;
        for (i, a) in args.iter().enumerate() {
            let expr = match a {
                Arg::Expr { expr, .. } => expr,
                Arg::Op { op, .. } => {
                    if let Some(s) = self.envs[inner].slots.get_mut(i) {
                        *s = Sb::OpArg {
                            op: op.clone(),
                            env,
                        };
                    }
                    continue;
                }
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
        let fun = self.fun(d, env)?;
        let mut ps = Vec::with_capacity(args.len());
        for a in args {
            let Arg::Expr { expr, .. } = a else {
                return Err(Error::unsupported("operator arguments (M0 core)"));
            };
            // TLC passes a recursive operator's arguments lazily; the core
            // evaluates them at the call, speculatively.
            let a = self.p(*expr, env, mode)?;
            let a = if self.is_trivial(a) {
                a
            } else {
                self.stats.speculative += 1;
                self.push(Node::Spec(a))?
            };
            ps.push(a);
        }
        self.fun_call(d, fun, ps, env)
    }

    fn fun(&mut self, d: DefId, call_env: usize) -> Result<u32, Error> {
        let key = (d, self.init_mode);
        if let Some(&f) = self.funs.get(&key) {
            return Ok(f);
        }
        let def = self.ir.def(d);
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
            caps: Box::new([]),
        });
        self.funs.insert(key, id);
        let saved_slot = self.next_slot;
        let saved_resets = std::mem::take(&mut self.reset_stack);
        let saved_cache = std::mem::take(&mut self.def_cache);
        let saved_at = std::mem::take(&mut self.at_stack);
        self.next_slot = params;
        let (env, lifted) = match self.begin_fun(d, id, call_env) {
            Ok(x) => x,
            Err(e) => {
                self.next_slot = saved_slot;
                self.reset_stack = saved_resets;
                self.def_cache = saved_cache;
                self.at_stack = saved_at;
                return Err(e);
            }
        };
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
        let ended = self.end_fun(id, lifted);
        let frame = self.next_slot;
        self.next_slot = saved_slot;
        self.reset_stack = saved_resets;
        self.def_cache = saved_cache;
        self.at_stack = saved_at;
        let body = body?;
        ended?;
        if let Some(f) = self.code.funs.get_mut(id as usize) {
            f.body = body;
            f.frame = frame;
        }
        Ok(id)
    }

    /// A [`crate::stdlib`] operator with operator arguments
    /// (`Node::LibHo`): each operator argument is compiled with
    /// [`Self::op_fn`] at the arity the library expects.
    fn lib_ho(&mut self, op: StdOp, args: &[Arg], env: usize, mode: Mode) -> Result<P, Error> {
        if mode == Mode::Primed {
            return Err(Error::unsupported(format!(
                "{} under a prime (M1 core)",
                op.qualified_name()
            )));
        }
        let mut out = Vec::with_capacity(args.len());
        for (i, a) in args.iter().enumerate() {
            match (crate::stdlib::op_arity(op, i), a) {
                (Some(arity), _) => {
                    let (params, body) = self.op_fn(a, env, arity)?;
                    if crate::stdlib::takes_op_value(op)
                        && matches!(self.code.nodes.get(body as usize), Some(Node::Const(_)))
                    {
                        return Err(Error::unsupported(format!(
                            "{} with an operator whose body is constant: TLC may fail to apply it",
                            op.qualified_name()
                        )));
                    }
                    out.push(LibArg::Op { params, body });
                }
                (None, Arg::Expr { expr, .. }) => {
                    out.push(LibArg::Value(self.p(*expr, env, mode)?));
                }
                (None, Arg::Op { .. }) => {
                    return Err(Error::unsupported(format!(
                        "{} with an operator where a value is expected",
                        op.qualified_name()
                    )));
                }
            }
        }
        self.push(Node::LibHo(op, out.into()))
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
        if crate::stdlib::higher_order(op) {
            return self.lib_ho(op, args, env, mode);
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
            S::IntegersDiv => arith(Arith::Div)?,
            S::NaturalsDiv => arith(Arith::DivNaturals)?,
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
            S::BagsEmptyBag => Node::Std(StdFn::EmptyBag, ps.into()),
            S::BagsSetToBag => Node::Std(StdFn::SetToBag, ps.into()),
            S::BagsBagToSet => Node::Std(StdFn::BagToSet, ps.into()),
            S::BagsBagIn => Node::Std(StdFn::BagIn, ps.into()),
            S::BagsCopiesIn => Node::Std(StdFn::CopiesIn, ps.into()),
            other if crate::stdlib::implements(other) => Node::Lib(other, ps.into()),
            other => {
                return Err(Error::unsupported(format!(
                    "the standard operator {} (M0 core)",
                    other.qualified_name()
                )));
            }
        };
        self.push(n)
    }

    /// `S` in `UNION {S}` when `S` is a set constructor (`[D -> R]`,
    /// `[a : R]`, `SUBSET R`, `A \X B`). `UNION {S}` is then `S`: TLC's
    /// `UnionValue` tests membership in each member, so `S` stays lazy
    /// instead of being enumerated as the element of a set in every state.
    /// Other operands keep TLC's check that the member is a set.
    fn union_of_one_constructor(&self, args: &[ExprId]) -> Option<ExprId> {
        let a = *args.first()?;
        let Expr::SetEnum(items) = &self.ir.node(a).expr else {
            return None;
        };
        let [item] = items.as_ref() else {
            return None;
        };
        matches!(
            &self.ir.node(*item).expr,
            Expr::FcnSet { .. }
                | Expr::RecordSet(_)
                | Expr::Builtin {
                    op: BuiltinOp::PowerSet | BuiltinOp::Times,
                    ..
                }
        )
        .then_some(*item)
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
            B::BigUnion => {
                if let Some(item) = self.union_of_one_constructor(args) {
                    return self.p(item, env, mode);
                }
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
            B::Cup | B::Cap | B::SetMinus => {
                let o = match op {
                    B::Cup => SetOp::Cup,
                    B::Cap => SetOp::Cap,
                    _ => SetOp::Minus,
                };
                return self.push_lazy(Node::SetBin(o, a(0)?, a(1)?));
            }
            B::In | B::NotIn => {
                let (x, set) = (a(0)?, a(1)?);
                if self.lazy_set(set) {
                    return self.push(Node::InLazy {
                        elem: x,
                        set,
                        neg: op == B::NotIn,
                    });
                }
                if op == B::In {
                    Node::In(x, set)
                } else {
                    Node::NotIn(x, set)
                }
            }
            B::Eq => Node::Eq(a(0)?, a(1)?),
            B::Neq => Node::Neq(a(0)?, a(1)?),
            B::Subseteq => Node::Subseteq(a(0)?, a(1)?),
            B::PowerSet => Node::Powerset(a(0)?),
            B::BigUnion => Node::BigUnion(a(0)?),
            B::Domain => {
                let f = a(0)?;
                if let Some(d) = self.lambda_domain(f)? {
                    return Ok(d);
                }
                Node::Domain(f)
            }
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
                    // TLC enumerates a lazy set in full, as the core does:
                    // no speculation.
                    let set = self.p(args[1], env, Mode::Plain)?;
                    let set = self.unspec(set);
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
            Expr::Subst { bindings, body } => {
                let (bindings, body) = (bindings.clone(), *body);
                let (g, reset) =
                    self.subst_scope(&bindings, env, Mode::Plain, |c| c.g(body, env))?;
                if reset.is_empty() {
                    Ok(g)
                } else {
                    Ok(self.gpush(GNode::Scope {
                        reset: reset.into(),
                        body: g,
                    }))
                }
            }
            // A substituted name used as an action is enumerated by name
            // (TLC binds substitutions uncached in the generator: B5).
            Expr::SubstRef {
                decl,
                primed: false,
            } if !self.subst_is_value(*decl) => match self.subst_expr(*decl) {
                Some(r) => {
                    let (expr, aenv, depth) = r?;
                    self.stats.by_name += 1;
                    self.outside(depth, |c| c.g(expr, aenv))
                }
                None => self.guard_expr(e, env),
            },
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
                self.g_inline(d, args, env, env)
            }
            // An operator that stands for a definition, applied as an
            // action: a constant operator bound to a definition (`F <- D`),
            // an operator declaration an instance substitutes, or an
            // operator parameter (the argument, inlined where it was
            // passed).
            Expr::Apply {
                callee: OpRef::Const(c),
                args,
            } => self.g_apply_op(&OpRef::Const(*c), args, e, env, env),
            Expr::Apply {
                callee: OpRef::Subst(decl),
                args,
            } => {
                let op = self.subst_op(*decl)?;
                self.g_apply_op(&op, args, e, env, env)
            }
            Expr::Apply {
                callee: OpRef::Param(r),
                args,
            } => {
                let (op, aenv) = self.resolve_param(*r, env)?;
                self.g_apply_op(&op, args, e, env, aenv)
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
            // `x'` with `x` substituted is the substitution's variable
            // (`getPrimedVar`); unsubstituted, the root variable.
            Expr::SubstRef { decl, primed } => {
                let (decl, primed) = (*decl, *primed);
                if primed && under_prime {
                    return Ok(None);
                }
                let under = under_prime || primed;
                if self.subst_is_value(decl) {
                    return Ok(None);
                }
                if let Some(r) = self.subst_expr(decl) {
                    let (expr, aenv, depth) = r?;
                    return self.outside(depth, |c| c.target(expr, aenv, under));
                }
                let Some(v) = self.ir.decl(decl).var else {
                    return Ok(None);
                };
                let ok = if self.init_mode { !under } else { under };
                Ok(ok.then_some(v.index() as u16))
            }
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
