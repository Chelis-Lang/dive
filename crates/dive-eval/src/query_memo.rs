//! Invocation-owned lazy evaluation for bounded pure ENABLED queries.
//! Normal cached results never supply query truth and are never populated here.

use std::sync::{Arc, Mutex, PoisonError, Weak};

use dive_value::{SetKind, Value};

use crate::core::Core;
use crate::env::{Env, Ev, SelectSeqBudget, St, StateRef, Sv, Thunk, control};
use crate::{EvalError, Result};

#[derive(Debug)]
struct Entry {
    thunk: Weak<Thunk>,
    value: Ev,
    current: Vec<Option<Value>>,
    successor: Vec<Option<Value>>,
    control: u8,
    resolver: Option<Weak<dyn crate::EnabledResolver>>,
}

/// Owned by one pure invocation. Materialized values cannot retain evaluator
/// providers; lazy functions retain work counters and only a weak memo owner.
#[derive(Debug, Default)]
pub(crate) struct QueryMemo(Mutex<Vec<Entry>>);

#[derive(Clone, Debug)]
pub(crate) enum MemoRef {
    Owner(Arc<QueryMemo>),
    Observer(Weak<QueryMemo>),
}
impl Default for MemoRef {
    fn default() -> Self {
        Self::Owner(Arc::new(QueryMemo::default()))
    }
}
impl MemoRef {
    pub(crate) fn observer(&self) -> Self {
        Self::Observer(match self {
            Self::Owner(memo) => Arc::downgrade(memo),
            Self::Observer(memo) => memo.clone(),
        })
    }
    pub(crate) fn same_invocation(&self, other: &Self) -> bool {
        let Self::Observer(a) = self.observer() else {
            return false;
        };
        let Self::Observer(b) = other.observer() else {
            return false;
        };
        Weak::ptr_eq(&a, &b) && a.strong_count() != 0
    }

    fn upgrade(&self) -> Result<Arc<QueryMemo>> {
        match self {
            Self::Owner(memo) => Ok(memo.clone()),
            Self::Observer(memo) => memo
                .upgrade()
                .ok_or_else(|| EvalError::unsupported("bounded query lazy cache invocation ended")),
        }
    }
}

fn charge(remaining: &mut usize, count: usize) -> Result<()> {
    *remaining = remaining
        .checked_sub(count)
        .ok_or_else(|| EvalError::resource_limit("bounded query lazy cache work"))?;
    Ok(())
}

/// Checks stored structure without enumerating symbolic sets or invoking code.
/// This prevents a memoized value from owning its invocation through a deferred
/// evaluator value. Symbolic built-in sets contain only ordinary owned data.
fn owned(core: &Core, value: &Value, remaining: &mut usize) -> Result<()> {
    charge(remaining, 1)?;
    let mut work = vec![(value.clone(), 0_usize)];
    while let Some((value, depth)) = work.pop() {
        if depth > 128 {
            return Err(EvalError::resource_limit("bounded query lazy cache depth"));
        }
        let mut reserve = |count| -> Result<()> {
            charge(remaining, count)?;
            work.try_reserve(count)
                .map_err(|_| EvalError::resource_limit("bounded query lazy cache allocation"))
        };
        match value {
            Value::Str(atom) | Value::Model(atom) => {
                if core
                    .names
                    .resolve(atom.tok())
                    .is_none_or(|a| a.as_str() != atom.as_str())
                {
                    return Err(EvalError::unsupported("bounded query lazy cache namespace"));
                }
            }
            Value::Fcn(function) => {
                reserve(function.len().saturating_mul(2))?;
                for (key, value) in function.iter() {
                    work.push((key, depth + 1));
                    work.push((value.clone(), depth + 1));
                }
            }
            Value::Set(set) => {
                if set.kind() == SetKind::Deferred {
                    return Err(EvalError::unsupported(
                        "bounded query lazy cache deferred value",
                    ));
                }
                if let Some(elements) = set.stored_elements() {
                    reserve(elements.len())?;
                    work.extend(elements.iter().cloned().map(|v| (v, depth + 1)));
                } else {
                    reserve(set.component_count().saturating_mul(2))?;
                    for (name, value) in set.components() {
                        if let Some(name) = name {
                            work.push((Value::Str(name), depth + 1));
                        }
                        work.push((value, depth + 1));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

// This walk checks retained storage only. An ordinary cache is never a source
// of the query result. Its stored owners must still be checked before a lazy
// function's captured frame can be retained by this memo.
type Seen = std::collections::BTreeSet<(u8, usize)>;
fn owned_ev(
    core: &Arc<Core>,
    value: &Ev,
    remaining: &mut usize,
    depth: usize,
    seen: &mut Seen,
) -> Result<()> {
    charge(remaining, 1)?;
    if depth > 128 {
        return Err(EvalError::resource_limit(
            "bounded query lazy function depth",
        ));
    }
    match value {
        Ev::V(value) => owned(core, value, remaining),
        Ev::F(function) => {
            if !seen.insert((0, Arc::as_ptr(function) as usize)) {
                return Ok(());
            }
            if !Arc::ptr_eq(core, &function.core)
                || function
                    .st
                    .enabled
                    .as_ref()
                    .is_some_and(|i| i.owner.is_some())
                || function
                    .st
                    .selectseq
                    .as_ref()
                    .is_some_and(|p| matches!(p.memo, MemoRef::Owner(_)))
            {
                return Err(EvalError::unsupported("bounded query lazy function owner"));
            }
            for state in [&function.st.s0, &function.st.s1] {
                for value in state.values() {
                    owned(core, value, remaining)?;
                }
            }
            for bound in &function.bounds {
                owned(core, &bound.domain, remaining)?;
            }
            if let Some(value) = function.materialized.get(0) {
                owned(core, &value, remaining)?;
            }
            for except in &function.excepts {
                for value in except.path.iter() {
                    owned(core, value, remaining)?;
                }
                owned_ev(core, &except.value, remaining, depth + 1, seen)?;
            }
            owned_env(core, &function.env, remaining, depth + 1, seen)
        }
    }
}
fn owned_sv(
    core: &Arc<Core>,
    value: &Sv,
    remaining: &mut usize,
    depth: usize,
    seen: &mut Seen,
) -> Result<()> {
    charge(remaining, 1)?;
    if depth > 128 {
        return Err(EvalError::resource_limit(
            "bounded query lazy function depth",
        ));
    }
    match value {
        Sv::Val(value) => owned(core, value, remaining),
        Sv::Fcn(function) => owned_ev(core, &Ev::F(function.clone()), remaining, depth + 1, seen),
        Sv::Thunk(thunk) => {
            if !seen.insert((1, Arc::as_ptr(thunk) as usize)) {
                return Ok(());
            }
            let cached = {
                let cache = thunk.cache.lock().unwrap_or_else(PoisonError::into_inner);
                cache
                    .as_ref()
                    .map(|c| (c.val.clone(), c.s0.clone(), c.s1.clone()))
            };
            if let Some((value, current, successor)) = cached {
                owned_ev(core, &value, remaining, depth + 1, seen)?;
                for state in [&current, &successor] {
                    for value in state.values() {
                        owned(core, value, remaining)?;
                    }
                }
            }
            owned_env(core, &thunk.env, remaining, depth + 1, seen)
        }
        Sv::Op(crate::env::OpVal::Def {
            parent, dynamic, ..
        }) => {
            if let Some(frame) = parent {
                owned_frame(core, frame, remaining, depth + 1, seen)?;
            }
            if let Some(dynamic) = dynamic {
                owned_dynamic(core, dynamic, remaining, depth + 1, seen)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
fn owned_frame(
    core: &Arc<Core>,
    frame: &Arc<crate::env::Frame>,
    remaining: &mut usize,
    depth: usize,
    seen: &mut Seen,
) -> Result<()> {
    charge(remaining, 1)?;
    if depth > 128 {
        return Err(EvalError::resource_limit(
            "bounded query lazy function depth",
        ));
    }
    if !seen.insert((2, Arc::as_ptr(frame) as usize)) {
        return Ok(());
    }
    for slot in &frame.slots {
        owned_sv(core, slot, remaining, depth + 1, seen)?;
    }
    if let Some(parent) = &frame.parent {
        owned_frame(core, parent, remaining, depth + 1, seen)?;
    }
    Ok(())
}
fn owned_dynamic(
    core: &Arc<Core>,
    dynamic: &crate::env::Dyn,
    remaining: &mut usize,
    depth: usize,
    seen: &mut Seen,
) -> Result<()> {
    if let Some(value) = &dynamic.at {
        owned(core, value, remaining)?;
    }
    let mut current = dynamic.subst.as_ref();
    while let Some(link) = current {
        charge(remaining, 1)?;
        if !seen.insert((3, Arc::as_ptr(link) as usize)) {
            break;
        }
        owned_sv(core, &link.val, remaining, depth + 1, seen)?;
        current = link.next.as_ref();
    }
    Ok(())
}
fn owned_env(
    core: &Arc<Core>,
    env: &Env,
    remaining: &mut usize,
    depth: usize,
    seen: &mut Seen,
) -> Result<()> {
    owned_frame(core, &env.frame, remaining, depth + 1, seen)?;
    owned_dynamic(core, &env.dynamic, remaining, depth + 1, seen)
}

fn roots(core: &Arc<Core>, state: &StateRef, remaining: &mut usize) -> Result<Vec<Option<Value>>> {
    if matches!(state, StateRef::Null | StateRef::Empty) {
        return Ok(Vec::new());
    }
    charge(remaining, core.ir.vars().len())?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(core.ir.vars().len())
        .map_err(|_| EvalError::resource_limit("bounded query lazy cache allocation"))?;
    for (index, decl) in core.ir.vars().iter().enumerate() {
        let value = core.state_variable(state, &core.ir.decl(*decl).name, Some(index))?;
        if let Some(value) = value {
            owned(core, value, remaining)?;
        }
        result.push(value.cloned());
    }
    Ok(result)
}

fn includes(current: &[Option<Value>], cached: &[Option<Value>]) -> bool {
    cached.iter().enumerate().all(|(index, value)| {
        value
            .as_ref()
            .is_none_or(|value| current.get(index).and_then(Option::as_ref) == Some(value))
    })
}

impl Core {
    pub(crate) fn force_query_memo(
        self: &Arc<Self>,
        thunk: &Arc<Thunk>,
        st: &St,
        policy: &SelectSeqBudget,
    ) -> Result<Ev> {
        if !Weak::ptr_eq(&policy.context, &Arc::downgrade(self)) {
            return Err(EvalError::unsupported("foreign bounded query lazy cache"));
        }
        let memo = policy.memo.upgrade()?;
        let (current, successor) = {
            let mut remaining = policy
                .remaining
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            (
                roots(self, &st.s0, &mut remaining)?,
                roots(self, &st.s1, &mut remaining)?,
            )
        };
        let key = Arc::downgrade(thunk);
        let resolver = st.enabled.as_ref().map(|i| i.resolver.clone());
        {
            let mut entries = memo.0.lock().unwrap_or_else(PoisonError::into_inner);
            let mut remaining = policy
                .remaining
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            charge(&mut remaining, entries.len())?;
            entries.retain(|entry| entry.thunk.strong_count() != 0);
            for entry in entries.iter() {
                charge(
                    &mut remaining,
                    1 + entry.current.len() + entry.successor.len(),
                )?;
                let same_resolver = match (&resolver, &entry.resolver) {
                    (None, None) => true,
                    (Some(a), Some(b)) => Weak::ptr_eq(a, b),
                    _ => false,
                };
                if Weak::ptr_eq(&key, &entry.thunk)
                    && same_resolver
                    && control::equivalent(st.control, entry.control)
                    && includes(&current, &entry.current)
                    && includes(&successor, &entry.successor)
                {
                    owned_ev(self, &entry.value, &mut remaining, 0, &mut Seen::new())?;
                    return Ok(entry.value.clone());
                }
            }
            if entries.len() >= policy.limit
                && !entries.iter().any(|entry| Weak::ptr_eq(&key, &entry.thunk))
            {
                return Err(EvalError::resource_limit(
                    "bounded query lazy cache entries",
                ));
            }
        }
        // Neither lock is held across evaluation, which can force another thunk.
        let value = self.eval(&mut thunk.env.clone(), st, thunk.expr)?;
        {
            let mut remaining = policy
                .remaining
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            owned_ev(
                self,
                &value,
                &mut remaining,
                0,
                &mut std::collections::BTreeSet::new(),
            )?;
        }
        let mut entries = memo.0.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = Entry {
            thunk: key,
            value: value.clone(),
            current,
            successor,
            control: st.control,
            resolver,
        };
        if let Some(old) = entries
            .iter_mut()
            .find(|old| Weak::ptr_eq(&old.thunk, &entry.thunk))
        {
            *old = entry;
        } else {
            if entries.len() >= policy.limit {
                return Err(EvalError::resource_limit(
                    "bounded query lazy cache entries",
                ));
            }
            entries
                .try_reserve(1)
                .map_err(|_| EvalError::resource_limit("bounded query lazy cache allocation"))?;
            entries.push(entry);
        }
        Ok(value)
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "private actual-forcing and ownership controls"
)]
mod tests {
    use super::*;
    use crate::env::Sv;
    use crate::{Evaluator, Model, Options};

    fn evaluator() -> Evaluator {
        let analysis = dive_sema::analyze_source(
            "Memo.tla",
            "---- MODULE Memo ----\nEXTENDS TLC\nVARIABLES x,y\nRead == Print(x',x')\n====\n",
            &dive_sema::Options::default(),
        );
        assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
        Evaluator::new(
            Model::new(Arc::new(dive_ir::lower(&analysis).expect("IR"))),
            Options::tlc(),
        )
        .expect("evaluator")
    }

    fn setup(ev: &Evaluator, limit: usize) -> (crate::Scope, Arc<Thunk>, St) {
        let def = ev.ir().root_def("Read").expect("Read");
        let scope = ev.scope(def).expect("scope");
        let thunk = Thunk::new(ev.ir().def(def).body, scope.env.clone(), true);
        let invocation = ev
            .begin_pure_predicate(
                &scope,
                &St::state(Arc::from([Some(0.into()), Some(0.into())])),
                limit,
            )
            .expect("invocation");
        let next = ev
            .named_state([(ev.ir().vars()[0], Value::Int(1))])
            .expect("assignment");
        (scope, thunk, invocation.enabled_candidate(next))
    }

    fn force(ev: &Evaluator, scope: &crate::Scope, thunk: &Arc<Thunk>, st: &St) -> Result<Value> {
        ev.core
            .force(
                &mut scope.env.clone(),
                &Sv::Thunk(thunk.clone()),
                st,
                thunk.expr,
            )
            .and_then(|value| ev.core.force_fcn(value))
    }

    #[test]
    fn own_invocation_ignores_ordinary_cache_and_observes_root_changes() {
        let ev = evaluator();
        let (scope, thunk, st) = setup(&ev, 4096);
        thunk.store(&st, &Ev::V(99.into()), 0);
        assert_eq!(
            force(&ev, &scope, &thunk, &st).expect("original source"),
            Value::Int(1)
        );
        assert_eq!(ev.take_output().len(), 1);
        assert_eq!(
            force(&ev, &scope, &thunk, &st).expect("local memo"),
            Value::Int(1)
        );
        assert!(ev.take_output().is_empty(), "no additional source effects");
        assert!(
            matches!(thunk.cached(&st, 0), Some(Ev::V(Value::Int(99)))),
            "ordinary cache unchanged"
        );
        let changed = st.enabled_candidate(
            ev.named_state([(ev.ir().vars()[0], Value::Int(2))])
                .expect("changed"),
        );
        assert_eq!(
            force(&ev, &scope, &thunk, &changed).expect("changed source"),
            Value::Int(2)
        );
        assert_eq!(ev.take_output().len(), 1);
        let (_, _, fresh) = setup(&ev, 4096);
        assert_eq!(
            force(&ev, &scope, &thunk, &fresh).expect("fresh invocation"),
            Value::Int(1)
        );
        assert_eq!(ev.take_output().len(), 1);
    }

    #[test]
    fn cache_entry_bound_precedes_new_body_and_owners_do_not_form_cycles() {
        let ev = evaluator();
        let (scope, thunk, st) = setup(&ev, 1);
        assert_eq!(
            force(&ev, &scope, &thunk, &st).expect("first entry"),
            Value::Int(1)
        );
        assert_eq!(ev.take_output().len(), 1);
        let other = Thunk::new(thunk.expr, thunk.env.clone(), true);
        assert_eq!(
            force(&ev, &scope, &other, &st)
                .expect_err("entry bound")
                .class(),
            crate::ErrorClass::ResourceLimit
        );
        assert!(
            ev.take_output().is_empty(),
            "new source not evaluated after exhausted entry quota"
        );
        let weak_thunk = Arc::downgrade(&thunk);
        let weak_budget = Arc::downgrade(st.selectseq.as_ref().expect("budget"));
        drop(thunk);
        assert!(
            weak_thunk.upgrade().is_none(),
            "memo keys do not retain source frames"
        );
        assert_eq!(
            force(&ev, &scope, &other, &st).expect("dead entry reclaimed"),
            Value::Int(1)
        );
        drop(st);
        assert!(
            weak_budget.upgrade().is_none(),
            "cached data cannot retain its invocation"
        );
    }

    #[derive(Debug)]
    struct UnreachedResolver;
    impl crate::EnabledResolver for UnreachedResolver {
        fn resolve(&self, _: &crate::Scope, _: dive_ir::ExprId, _: &St) -> Result<bool> {
            Err(EvalError::unsupported("test resolver must not be reached"))
        }
    }

    #[test]
    fn recursive_function_memo_reuses_identity_and_releases_its_owner() {
        let analysis = dive_sema::analyze_source(
            "Memo.tla",
            "---- MODULE Memo ----\nEXTENDS Naturals\nVARIABLE x\nRead == LET f[t \\in {0}] == IF t=0 THEN {0} ELSE f[t-1] IN f\n====\n",
            &dive_sema::Options::default(),
        );
        assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
        let ev = Evaluator::new(
            Model::new(Arc::new(dive_ir::lower(&analysis).expect("IR"))),
            Options::tlc(),
        )
        .expect("evaluator");
        let def = ev.ir().root_def("Read").expect("Read");
        let scope = ev.scope(def).expect("scope");
        let thunk = Thunk::new(ev.ir().def(def).body, scope.env.clone(), true);
        let invocation = ev
            .begin_pure_predicate(&scope, &St::state(Arc::from([Some(0.into())])), 4096)
            .expect("invocation");
        let mut state = invocation.enabled_candidate(
            ev.named_state([(ev.ir().vars()[0], 1.into())])
                .expect("candidate"),
        );
        let resolver: Arc<dyn crate::EnabledResolver> = Arc::new(UnreachedResolver);
        let weak_resolver = Arc::downgrade(&resolver);
        state = ev
            .with_enabled_resolver(&state, resolver.clone())
            .expect("resolver");
        drop(resolver);
        let first = ev
            .core
            .force(
                &mut scope.env.clone(),
                &Sv::Thunk(thunk.clone()),
                &state,
                thunk.expr,
            )
            .expect("lazy recursive function");
        let second = ev
            .core
            .force(
                &mut scope.env.clone(),
                &Sv::Thunk(thunk.clone()),
                &state,
                thunk.expr,
            )
            .expect("cached function");
        assert!(
            matches!(first, Ev::F(_)) && matches!(second, Ev::F(_)),
            "recursive functions stay lazy"
        );
        let (Ev::F(first), Ev::F(second)) = (first, second) else {
            return;
        };
        assert!(
            Arc::ptr_eq(&first, &second),
            "same actual function, no materialization"
        );
        assert!(
            thunk.cache.lock().expect("cache").is_none(),
            "ordinary cache untouched"
        );
        let memo = state
            .selectseq
            .as_ref()
            .expect("policy")
            .memo
            .upgrade()
            .expect("memo");
        let weak_memo = Arc::downgrade(&memo);
        let weak_work = Arc::downgrade(&state.selectseq.as_ref().expect("policy").remaining);
        drop(memo);
        drop(second);
        drop(state);
        drop(invocation);
        assert!(
            weak_memo.upgrade().is_none(),
            "retained function cannot own its memo"
        );
        assert!(
            weak_resolver.upgrade().is_none(),
            "cached function cannot retain the client callback"
        );
        assert!(
            weak_work.upgrade().is_some(),
            "live function retains original work counters"
        );
        drop(first);
        assert!(
            weak_work.upgrade().is_none(),
            "function release frees work counters"
        );
    }

    #[test]
    fn foreign_budget_and_exhausted_work_refuse_before_body() {
        let ev = evaluator();
        let (scope, thunk, mut st) = setup(&ev, 4096);
        let foreign = evaluator();
        let (_, _, other) = setup(&foreign, 4096);
        st.selectseq.clone_from(&other.selectseq);
        assert_eq!(
            force(&ev, &scope, &thunk, &st)
                .expect_err("foreign budget")
                .class(),
            crate::ErrorClass::Unsupported
        );
        assert!(ev.take_output().is_empty(), "no additional source effects");
        let (_, _, st) = setup(&ev, 4096);
        *st.selectseq
            .as_ref()
            .expect("budget")
            .remaining
            .lock()
            .expect("work") = 0;
        assert_eq!(
            force(&ev, &scope, &thunk, &st)
                .expect_err("work bound")
                .class(),
            crate::ErrorClass::ResourceLimit
        );
        assert!(ev.take_output().is_empty(), "no additional source effects");
    }
}
