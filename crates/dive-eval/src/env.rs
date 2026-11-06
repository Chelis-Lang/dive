//! Environments: frames of slots, thunks, operator values, the dynamic
//! substitution context, and the states an expression is evaluated in.
//!
//! TLC binds names in an immutable linked `Context` (`tlc2.util.Context`) and
//! looks them up by semantic node. dive resolves names to frame slots when
//! lowering (ADR 0013 §2), so an [`Env`] is a frame of slots plus TLC's
//! dynamically scoped part: instance substitutions and `@`.
//!
//! **Persistence.** A thunk, a lazy set or a lazy function captures the
//! environment it was created in, as TLC's `LazyValue` captures its
//! `Context`. Frames are therefore copy-on-write: binding a slot clones the
//! frame only if something captured it ([`Arc::make_mut`]), so a quantifier
//! that rebinds its variable never changes what an earlier capture sees.

use std::sync::{Arc, Mutex, PoisonError};

use dive_ir::{BuiltinOp, ConstId, DeclId, DefId, ExprId, StdOp};
use dive_value::Value;

use crate::lazy::LazyFcn;

/// TLC's `EvalControl` flags.
pub(crate) mod control {
    /// `KeepLazy`: function constructors stay lambdas.
    pub(crate) const KEEP_LAZY: u8 = 1;
    /// `Primed`.
    pub(crate) const PRIMED: u8 = 1 << 1;
    /// `Enabled`.
    pub(crate) const ENABLED: u8 = 1 << 2;
    /// `Init`.
    pub(crate) const INIT: u8 = 1 << 3;
    /// `Const`.
    pub(crate) const CONST: u8 = 1 << 4;
    /// Source attachment by the diagnostic tool. This changes error provenance,
    /// never a specification's value or the ordinary evaluation controls.
    pub(crate) const DIAGNOSTIC: u8 = 1 << 5;
    /// Startup pre-evaluation has no running checker register writer. This
    /// private stage is distinct from ordinary constant-level ASSUMEs.
    pub(crate) const PRECOMPUTE: u8 = 1 << 6;
    /// `EvalControl.Clear`.
    pub(crate) const CLEAR: u8 = 0;

    /// `EvalControl.semanticallyEquivalent`: equal up to `KeepLazy`, `Init`
    /// and `Const`.
    pub(crate) fn equivalent(a: u8, b: u8) -> bool {
        let mask = !(KEEP_LAZY | INIT | CONST | DIAGNOSTIC);
        a & mask == b & mask
    }
}

/// A state as the evaluator sees it: TLC's `TLCState.Null` (no state),
/// `TLCState.Empty` (a state with no variable assigned), or the values of the
/// root module's variables by [`dive_ir::VarId`], `None` where unassigned.
/// Existential queries can instead retain hidden assignments by canonical name.
#[derive(Clone, Debug, Default)]
pub enum StateRef {
    /// `TLCState.Null`.
    Null,
    /// `TLCState.Empty`.
    #[default]
    Empty,
    /// A (possibly partial) state.
    Vals(Arc<[Option<Value>]>),
    /// A query-local partial assignment indexed by canonical variable name.
    Named(Arc<crate::NamedState>),
}

impl StateRef {
    /// TLC's `TLCState.isSubset`: assigned root variables in the cached state
    /// have the same value here. Hidden query names do not invalidate a lazy
    /// value; this observable cache behavior matters across ENABLED candidates.
    /// Mixed indexed/named representations never establish cache inclusion.
    pub(crate) fn includes(&self, cached: &Self) -> bool {
        match (self, cached) {
            (_, Self::Null | Self::Empty) => true,
            (Self::Vals(s), Self::Vals(c)) => {
                Arc::ptr_eq(s, c)
                    || c.iter().enumerate().all(|(i, cv)| {
                        cv.as_ref()
                            .is_none_or(|cv| s.get(i).and_then(Option::as_ref) == Some(cv))
                    })
            }
            (Self::Named(s), Self::Named(c)) => {
                std::sync::Weak::ptr_eq(&s.context, &c.context)
                    && (Arc::ptr_eq(s, c)
                        || s.context.upgrade().is_some_and(|core| {
                            core.ir.vars().iter().all(|decl| {
                                let name = core.ir.decl(*decl).name.as_str();
                                c.get(name).is_none_or(|value| s.get(name) == Some(value))
                            })
                        }))
            }
            (Self::Null | Self::Empty, Self::Vals(c)) => c.iter().all(Option::is_none),
            _ => false,
        }
    }
}

/// The states and control flags an expression is evaluated under: TLC's
/// `(s0, s1, control)`.
#[derive(Clone, Debug, Default)]
pub struct St {
    /// The current state (`s0`).
    pub s0: StateRef,
    /// The successor (`s1`).
    pub s1: StateRef,
    pub(crate) control: u8,
}

impl St {
    /// Re-evaluates this context with TLC's diagnostic source-attachment hook.
    ///
    /// Only receivers actually constructed in the evaluation acquire a source;
    /// previously computed or stored values retain their original provenance.
    #[must_use]
    pub fn diagnostic(&self) -> Self {
        self.with_control(self.control | control::DIAGNOSTIC)
    }

    /// Constant-level evaluation, as TLC evaluates `ASSUME`s
    /// (`Tool.isValid(expr)`: `Empty`, `Empty`, `EvalControl.Const`).
    #[must_use]
    pub fn constant() -> Self {
        Self {
            s0: StateRef::Empty,
            s1: StateRef::Empty,
            control: control::CONST,
        }
    }

    /// TLC's `(TLCState.Empty, TLCState.Empty, EvalControl.Clear)`: what
    /// `Tool.getActions` evaluates the arguments of inlined operators and the
    /// domains of split `\E`s in.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            s0: StateRef::Empty,
            s1: StateRef::Empty,
            control: control::CLEAR,
        }
    }

    /// A state predicate in `state` (`Empty` successor, `Clear`).
    #[must_use]
    pub fn state(state: Arc<[Option<Value>]>) -> Self {
        Self {
            s0: StateRef::Vals(state),
            s1: StateRef::Empty,
            control: control::CLEAR,
        }
    }

    /// An initial predicate on a supplied state (`Empty` successor, `Init`).
    ///
    /// Unlike ordinary state checks, this evaluates `TLCGet("level")` as 0.
    /// Use [`Self::state`] for invariants and implied initial properties,
    /// which are checked after the initial state has been generated.
    ///
    /// ```
    /// use dive_eval::St;
    /// use std::sync::Arc;
    /// let initial = St::initial(Arc::from([Some(0.into())]));
    /// ```
    #[must_use]
    pub fn initial(state: Arc<[Option<Value>]>) -> Self {
        Self {
            control: control::INIT,
            ..Self::state(state)
        }
    }

    /// An action between `s0` and `s1` (`Clear`).
    #[must_use]
    pub fn action(s0: Arc<[Option<Value>]>, s1: Arc<[Option<Value>]>) -> Self {
        Self {
            s0: StateRef::Vals(s0),
            s1: StateRef::Vals(s1),
            control: control::CLEAR,
        }
    }

    /// Evaluates a query relation in the supplied partial named successor while
    /// retaining the current state.
    #[must_use]
    pub fn enabled_candidate(&self, successor: Arc<crate::NamedState>) -> Self {
        Self {
            s1: StateRef::Named(successor),
            control: control::ENABLED,
            ..self.clone()
        }
    }

    /// Reads the current side of an ENABLED query, preserving its cutoff control.
    #[must_use]
    pub fn enabled_current(&self) -> Self {
        Self {
            s1: StateRef::Empty,
            control: control::ENABLED,
            ..self.clone()
        }
    }

    /// Moves the whole successor into the current position for primed evaluation.
    /// Hidden named assignments are retained.
    #[must_use]
    pub fn primed_successor(&self) -> Self {
        Self {
            s0: self.s1.clone(),
            s1: StateRef::Null,
            control: Self::primed_if_enabled(self.control),
        }
    }

    pub(crate) fn with_control(&self, control: u8) -> Self {
        Self {
            control,
            ..self.clone()
        }
    }

    pub(crate) fn keep_lazy(&self) -> Self {
        self.with_control(self.control | control::KEEP_LAZY)
    }

    pub(crate) fn is(&self, flag: u8) -> bool {
        self.control & flag != 0
    }

    /// `EvalControl.setPrimedIfEnabled`.
    pub(crate) fn primed_if_enabled(control: u8) -> u8 {
        if control & control::ENABLED != 0 {
            control | control::PRIMED
        } else {
            control
        }
    }
}

/// The result of evaluating an expression: a value, or a function TLC keeps
/// as a lambda (`FcnLambdaValue`).
#[derive(Clone, Debug)]
pub enum Ev {
    /// A value.
    V(Value),
    /// A lazy function.
    F(Arc<LazyFcn>),
}

impl From<Value> for Ev {
    fn from(v: Value) -> Self {
        Self::V(v)
    }
}

/// An operator value: what an operator parameter, an operator argument or a
/// substituted constant operator is bound to.
#[derive(Clone, Debug)]
pub(crate) enum OpVal {
    /// A user definition. For a `LET` operator or `LAMBDA`, `parent` is the
    /// frame it closes over.
    Def {
        def: DefId,
        parent: Option<Arc<Frame>>,
        /// The dynamic context it was passed in (TLC's `OpLambdaValue` keeps
        /// its context).
        dynamic: Option<Dyn>,
    },
    /// A Java override.
    Std(StdOp),
    /// A built-in operator.
    Builtin(BuiltinOp),
    /// A constant operator bound by the configuration.
    Const(ConstId),
    /// A declaration with no binding (`TLC_CONFIG_UNDEFINED_OR_NO_OPERATOR`).
    Unbound(Arc<str>),
}

/// What a slot or a substitution holds.
#[derive(Clone, Debug, Default)]
pub(crate) enum Sv {
    /// Not bound yet.
    #[default]
    Unset,
    /// A value.
    Val(Value),
    /// A pure unknown binder value used only by static level/purity analysis.
    AnalysisBound,
    /// A pure unknown constant-level binder value (a binder over a constant
    /// domain) used only by static level/purity analysis.
    AnalysisConstant,
    /// A lazy function.
    Fcn(Arc<LazyFcn>),
    /// A `LazyValue`.
    Thunk(Arc<Thunk>),
    /// An operator, boxed so that a slot stays as small as a value.
    Op(Box<OpVal>),
}

impl Sv {
    /// A slot holding the operator `op`.
    pub(crate) fn op(op: OpVal) -> Self {
        Self::Op(Box::new(op))
    }
}

impl From<Ev> for Sv {
    fn from(ev: Ev) -> Self {
        match ev {
            Ev::V(v) => Self::Val(v),
            Ev::F(f) => Self::Fcn(f),
        }
    }
}

// A slot is no larger than a value plus its tag: frames copy and free slots
// on every call, so their size is on the hot path.
const _: () = assert!(std::mem::size_of::<Sv>() <= 24);

/// The number of slots a frame stores inline, in the frame's own allocation.
/// Most activations (operator parameters, a quantifier's variables, a few
/// `LET` definitions) fit; larger frames keep their slots on the heap.
const INLINE_SLOTS: usize = 4;

/// The slots of a frame: inline up to [`INLINE_SLOTS`], so that a call
/// allocates its frame once, and on the heap beyond.
#[derive(Clone, Debug)]
pub(crate) enum Slots {
    Inline { len: u8, items: [Sv; INLINE_SLOTS] },
    Heap(Vec<Sv>),
}

impl Slots {
    /// `n` unbound slots.
    fn unset(n: usize) -> Self {
        match u8::try_from(n) {
            Ok(len) if n <= INLINE_SLOTS => Self::Inline {
                len,
                items: Default::default(),
            },
            _ => Self::Heap(vec![Sv::Unset; n]),
        }
    }

    /// Grows to at least `n` slots, binding the new ones to [`Sv::Unset`].
    fn ensure_len(&mut self, n: usize) {
        if n <= self.len() {
            return;
        }
        match self {
            Self::Inline { len, .. } if n <= INLINE_SLOTS => {
                // `n <= INLINE_SLOTS`, which fits in a `u8`.
                *len = u8::try_from(n).unwrap_or(u8::MAX);
            }
            Self::Inline { len, items } => {
                let mut v: Vec<Sv> = items[..usize::from(*len)]
                    .iter_mut()
                    .map(std::mem::take)
                    .collect();
                v.resize(n, Sv::Unset);
                *self = Self::Heap(v);
            }
            Self::Heap(v) => v.resize(n, Sv::Unset),
        }
    }
}

impl std::ops::Deref for Slots {
    type Target = [Sv];
    fn deref(&self) -> &[Sv] {
        match self {
            Self::Inline { len, items } => &items[..usize::from(*len)],
            Self::Heap(v) => v,
        }
    }
}

impl std::ops::DerefMut for Slots {
    fn deref_mut(&mut self) -> &mut [Sv] {
        match self {
            Self::Inline { len, items } => &mut items[..usize::from(*len)],
            Self::Heap(v) => v,
        }
    }
}

impl<'a> IntoIterator for &'a Slots {
    type Item = &'a Sv;
    type IntoIter = std::slice::Iter<'a, Sv>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// A frame: the slots of one definition's activation.
#[derive(Clone, Debug)]
pub(crate) struct Frame {
    /// The definition whose frame this is.
    pub(crate) def: DefId,
    /// Its frame depth ([`dive_ir::Def::depth`]).
    pub(crate) depth: u32,
    pub(crate) slots: Slots,
    /// The frame a `LET` operator or `LAMBDA` closes over.
    pub(crate) parent: Option<Arc<Frame>>,
}

/// One instance substitution in the dynamic context.
#[derive(Debug)]
pub(crate) struct SubstLink {
    pub(crate) decl: DeclId,
    pub(crate) val: Sv,
    /// The number of links up to and including this one.
    pub(crate) depth: usize,
    pub(crate) next: Option<Arc<SubstLink>>,
}

/// TLC's dynamically scoped bindings: instance substitutions and `@`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Dyn {
    pub(crate) subst: Option<Arc<SubstLink>>,
    pub(crate) at: Option<Value>,
    /// `ENABLED` evaluates its body in `Context.branch(c)`: a lookup with
    /// TLC's `cutoff` flag (primed evaluation of a variable, and the
    /// enumerator's `getVar`) stops there, so substitutions made outside the
    /// `ENABLED`, those at depth `<=` this, are hidden from it.
    pub(crate) cut: Option<usize>,
}

impl Dyn {
    pub(crate) fn lookup(&self, decl: DeclId) -> Option<&Sv> {
        self.lookup_cut(decl, false)
    }

    /// `Context.lookup(var, cutoff)`.
    pub(crate) fn lookup_cut(&self, decl: DeclId, cutoff: bool) -> Option<&Sv> {
        let floor = if cutoff { self.cut.unwrap_or(0) } else { 0 };
        let mut cur = self.subst.as_deref();
        while let Some(link) = cur {
            if link.depth <= floor {
                return None;
            }
            if link.decl == decl {
                return Some(&link.val);
            }
            cur = link.next.as_deref();
        }
        None
    }

    /// Conses a substitution onto the context.
    pub(crate) fn push(&mut self, decl: DeclId, val: Sv) {
        let next = self.subst.take();
        let depth = next.as_ref().map_or(0, |l| l.depth) + 1;
        self.subst = Some(Arc::new(SubstLink {
            decl,
            val,
            depth,
            next,
        }));
    }

    /// `Context.branch`: marks the current depth.
    pub(crate) fn branch(&mut self) {
        self.cut = Some(self.subst.as_ref().map_or(0, |l| l.depth));
    }
}

/// An environment: the current frame and the dynamic context.
#[derive(Clone, Debug)]
pub(crate) struct Env {
    pub(crate) frame: Arc<Frame>,
    pub(crate) dynamic: Dyn,
}

impl Env {
    /// A fresh frame for definition `def` with `slots` slots.
    pub(crate) fn new(
        def: DefId,
        depth: u32,
        slots: usize,
        parent: Option<Arc<Frame>>,
        dynamic: Dyn,
    ) -> Self {
        Self {
            frame: Arc::new(Frame {
                def,
                depth,
                slots: Slots::unset(slots),
                parent,
            }),
            dynamic,
        }
    }

    /// Binds a slot of the current frame (copy-on-write).
    pub(crate) fn bind(&mut self, slot: usize, v: Sv) {
        let frame = Arc::make_mut(&mut self.frame);
        frame.slots.ensure_len(slot + 1);
        if let Some(target) = frame.slots.get_mut(slot) {
            *target = v;
        }
    }

    /// The frame `up` levels out.
    pub(crate) fn frame_up(&self, up: u32) -> Option<&Arc<Frame>> {
        let mut f = &self.frame;
        for _ in 0..up {
            f = f.parent.as_ref()?;
        }
        Some(f)
    }

    /// The frame at nesting depth `depth`, walking outwards.
    pub(crate) fn frame_at_depth(&self, depth: u32) -> Option<&Arc<Frame>> {
        let mut f = &self.frame;
        while f.depth > depth {
            f = f.parent.as_ref()?;
        }
        (f.depth == depth).then_some(f)
    }
}

/// A forced thunk's value and the circumstances it was computed under.
#[derive(Debug)]
pub(crate) struct Cached {
    pub(crate) val: Ev,
    pub(crate) s0: StateRef,
    pub(crate) s1: StateRef,
    pub(crate) control: u8,
    /// The [`crate::Host::expression_cache_domain`] it was computed under.
    pub(crate) domain: u64,
}

/// TLC's `LazyValue`: an expression with its environment, forced at most once
/// under TLC's caching rule (`LazyValue.getCachedValue`).
///
/// Ordinary evaluation (cache domain 0) uses `cache`. Evaluation under a host
/// with its own cache domain (independent replay) uses `foreign`, which never
/// supplies a value to, or receives one from, another domain.
#[derive(Debug)]
pub(crate) struct Thunk {
    pub(crate) expr: ExprId,
    pub(crate) env: Env,
    pub(crate) cachable: bool,
    pub(crate) cache: Mutex<Option<Cached>>,
    foreign: Mutex<Option<Cached>>,
}

impl Thunk {
    // Metadata inspection must check both original computation and warm values,
    // without consulting getCachedValue or keeping the cache lock while walking.
    pub(crate) fn stored_caches(&self) -> [Option<(Ev, StateRef, StateRef)>; 2] {
        let snapshot = |slot: &Mutex<Option<Cached>>| {
            slot.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref()
                .map(|cached| (cached.val.clone(), cached.s0.clone(), cached.s1.clone()))
        };
        [snapshot(&self.cache), snapshot(&self.foreign)]
    }

    pub(crate) fn new(expr: ExprId, env: Env, cachable: bool) -> Arc<Self> {
        Arc::new(Self {
            expr,
            env,
            cachable,
            cache: Mutex::new(None),
            foreign: Mutex::new(None),
        })
    }

    fn slot(&self, domain: u64) -> &Mutex<Option<Cached>> {
        if domain == 0 {
            &self.cache
        } else {
            &self.foreign
        }
    }

    /// `LazyValue.getCachedValue`, for evaluation in cache `domain`.
    pub(crate) fn cached(&self, st: &St, domain: u64) -> Option<Ev> {
        if !self.cachable {
            return None;
        }
        let cache = self
            .slot(domain)
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let c = cache.as_ref()?;
        (c.domain == domain
            && st.s0.includes(&c.s0)
            && st.s1.includes(&c.s1)
            && control::equivalent(st.control, c.control))
        .then(|| c.val.clone())
    }

    /// Records a value forced in cache `domain`.
    pub(crate) fn store(&self, st: &St, val: &Ev, domain: u64) {
        if !self.cachable {
            return;
        }
        let mut cache = self
            .slot(domain)
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *cache = Some(Cached {
            val: val.clone(),
            s0: st.s0.clone(),
            s1: st.s1.clone(),
            control: st.control,
            domain,
        });
    }
}

/// Whether `env` holds, directly or through the values it binds, a cached
/// thunk or a lazy function whose expression has side effects
/// (`Effects::IMPURE`). See `Evaluator::scope_shares_side_effects`.
pub(crate) fn shares_side_effects(ir: &dive_ir::Ir, env: &Env) -> bool {
    captures_with_effects(ir, env, dive_ir::Effects::IMPURE, false, None)
}

/// Captured operators are opaque here: callers can supply them through the
/// Rust scope API without a corresponding statically analysed IR call site.
/// All thunks/functions must also have closed effects before filter execution.
pub(crate) fn filter_has_unproved_captures(
    ir: &dive_ir::Ir,
    env: &Env,
    forbidden: dive_ir::Effects,
) -> bool {
    captures_with_effects(ir, env, forbidden, true, None)
}

/// Checks retained executable contexts, including actual captured callables,
/// without evaluating them or walking arbitrary stored values.
pub(crate) fn scope_has_effects(
    ir: &dive_ir::Ir,
    env: &Env,
    callable_effects: &dyn Fn(&OpVal) -> bool,
) -> bool {
    captures_with_effects(
        ir,
        env,
        dive_ir::Effects::IMPURE.union(dive_ir::Effects::LIVE_TLCGET),
        false,
        Some(callable_effects),
    )
}

fn captures_with_effects(
    ir: &dive_ir::Ir,
    env: &Env,
    forbidden: dive_ir::Effects,
    reject_operators: bool,
    callable_effects: Option<&dyn Fn(&OpVal) -> bool>,
) -> bool {
    let mut seen = std::collections::HashSet::new();
    let mut frames: Vec<&Arc<Frame>> = vec![&env.frame];
    let mut svs: Vec<&Sv> = Vec::new();
    let mut dyns: Vec<&Dyn> = vec![&env.dynamic];
    let impure = |e: ExprId| {
        ir.nodes()
            .get(e.index())
            .map_or(reject_operators, |n| n.effects.intersects(forbidden))
    };
    loop {
        if let Some(f) = frames.pop() {
            if seen.insert(Arc::as_ptr(f) as usize) {
                svs.extend(f.slots.iter());
                if let Some(p) = &f.parent {
                    frames.push(p);
                }
            }
        } else if let Some(d) = dyns.pop() {
            let mut cur = d.subst.as_deref();
            while let Some(link) = cur {
                if !seen.insert(std::ptr::from_ref(link) as usize) {
                    break;
                }
                svs.push(&link.val);
                cur = link.next.as_deref();
            }
        } else if let Some(sv) = svs.pop() {
            match sv {
                Sv::Thunk(t) => {
                    if seen.insert(Arc::as_ptr(t) as usize) {
                        if (reject_operators || callable_effects.is_some() || t.cachable)
                            && impure(t.expr)
                        {
                            return true;
                        }
                        frames.push(&t.env.frame);
                        dyns.push(&t.env.dynamic);
                    }
                }
                Sv::Fcn(f) => {
                    if seen.insert(Arc::as_ptr(f) as usize) {
                        if impure(f.body) {
                            return true;
                        }
                        frames.push(&f.env.frame);
                        dyns.push(&f.env.dynamic);
                    }
                }
                Sv::Op(op)
                    if reject_operators || callable_effects.is_some_and(|effects| effects(op)) =>
                {
                    return true;
                }
                Sv::Op(op) => {
                    if let OpVal::Def {
                        parent, dynamic, ..
                    } = &**op
                    {
                        if let Some(p) = parent {
                            frames.push(p);
                        }
                        if let Some(d) = dynamic {
                            dyns.push(d);
                        }
                    }
                }
                Sv::Unset | Sv::Val(_) | Sv::AnalysisBound | Sv::AnalysisConstant => {}
            }
        } else {
            return false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{INLINE_SLOTS, Slots, Sv};
    use dive_value::Value;

    #[test]
    fn slots_grow_from_inline_to_heap_keeping_bindings() {
        let mut slots = Slots::unset(1);
        assert!(matches!(slots, Slots::Inline { len: 1, .. }));
        slots[0] = Sv::Val(Value::Int(7));
        slots.ensure_len(INLINE_SLOTS);
        assert!(matches!(slots, Slots::Inline { .. }));
        assert_eq!(slots.len(), INLINE_SLOTS);
        slots.ensure_len(INLINE_SLOTS + 2);
        assert!(matches!(slots, Slots::Heap(_)));
        assert_eq!(slots.len(), INLINE_SLOTS + 2);
        assert!(matches!(&slots[0], Sv::Val(Value::Int(7))));
        assert!(slots[1..].iter().all(|s| matches!(s, Sv::Unset)));
        slots.ensure_len(1);
        assert_eq!(slots.len(), INLINE_SLOTS + 2);
        assert!(matches!(Slots::unset(INLINE_SLOTS + 1), Slots::Heap(_)));
    }
}
