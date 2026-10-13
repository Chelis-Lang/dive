//! A direct native choice relation, separate from ordinary random evaluation.

use dive_ir::{Arg, Expr, OpRef, StdOp};
use dive_value::{SetKind, Value};

use crate::env::control;
use crate::{
    ContextError, ContextErrorKind, Ec, EvalError, Evaluator, Result, Scope, St, StateRef,
};

const STORED_ENTRY_LIMIT: usize = 65_536;

impl Evaluator {
    /// Tests the support of one direct native `RandomElement(S)` application.
    ///
    /// This evaluates the actual argument in `scope` and `st`, then checks
    /// `candidate` against an interval or a stored finite scalar set. It never
    /// invokes a random host or returns a candidate to ordinary evaluation.
    /// `None` means the expression is outside this relation operation (including
    /// aliases, configured native redirects and surrounding expressions).
    /// Callers must separately enforce single-occurrence, startup and complete
    /// predicate contracts. This operation is not a purity certificate.
    ///
    /// The state must be constant context, or Init context with the correct number
    /// of entirely unassigned slots and no successor. Candidates are scalar:
    /// strings/model values must have tokens resolving to the same exact text
    /// in this evaluator. Compound candidates refuse before domain evaluation.
    ///
    /// `limit` bounds stored-set entries before normalization, capped at 65,536;
    /// intervals use exact bounds without enumeration. Stored entries obey the
    /// same flat scalar/token check. Compound/deferred sets remain unsupported.
    /// Normal argument evaluation can execute effects under the installed host;
    /// this operation itself supplies no host authority or purity certificate.
    ///
    /// ```
    /// use std::sync::Arc;
    /// use dive_eval::{Evaluator, Model, Options, St};
    /// let source = "---- MODULE M ----\nEXTENDS TLC\nChoice == RandomElement({7})\n====\n";
    /// let analysis = dive_sema::analyze_source("M.tla", source, &Default::default());
    /// let ir = Arc::new(dive_ir::lower(&analysis).unwrap());
    /// let choice = ir.root_def("Choice").unwrap();
    /// let expr = ir.def(choice).body;
    /// let ev = Evaluator::new(Model::new(ir), Options::tlc()).unwrap();
    /// let scope = ev.scope(choice).unwrap();
    /// assert_eq!(ev.random_element_member(&scope, expr, &St::constant(), &7.into(), 10).unwrap(), Some(true));
    /// assert_eq!(ev.random_element_member(&scope, expr, &St::constant(), &8.into(), 10).unwrap(), Some(false));
    /// ```
    ///
    /// # Errors
    /// Foreign context/value, argument evaluation errors, native empty/non-set
    /// domain errors, unsupported domains, comparison errors or entry limits.
    pub fn random_element_member(
        &self,
        scope: &Scope,
        expr: dive_ir::ExprId,
        st: &St,
        candidate: &Value,
        limit: usize,
    ) -> Result<Option<bool>> {
        self.validate_scope(scope)?;
        self.admit_scoped_expr(scope, expr)?;
        self.random_relation_state(st)?;
        let Expr::Apply { callee, args } = self.core.ir.expr(expr) else {
            return Ok(None);
        };
        let direct = match callee {
            OpRef::Std(op) => *op == StdOp::TlcRandomElement,
            OpRef::Def(def) => {
                self.core.redirect(*def) == *def
                    && self.core.ir.def(*def).std == Some(StdOp::TlcRandomElement)
                    && !self.is_value_override(*def)
            }
            _ => false,
        };
        if !direct
            || self
                .core
                .std_redirects
                .contains_key(&StdOp::TlcRandomElement)
        {
            return Ok(None);
        }
        // Non-direct expressions delegate without granting relation authority.
        // In particular, a pure compound Init must keep its deterministic path.
        self.random_relation_scalar(candidate, "RandomElement relation compound candidate")?;
        let [Arg::Expr { expr: domain, .. }] = args.as_ref() else {
            return Err(EvalError::unsupported(
                "direct RandomElement relation arity",
            ));
        };
        // Ordinary argument evaluation retains its errors and runtime effects.
        let domain = self.eval_scoped(scope, *domain, st)?;
        let wrong = || {
            EvalError::eval_exception(
                Ec::ApplyingToWrongValue,
                format!(
                    "Applying RandomElement to the following value,\nwhich is not a finite set:\n{}",
                    self.core.ppr(&domain)
                ),
            )
        };
        let Value::Set(set) = &domain else {
            // Formatting a function can force a deferred value nested within it.
            // Keep the native wrong-domain class without visiting its contents.
            if matches!(&domain, Value::Fcn(_)) {
                return Err(EvalError::eval_exception(
                    Ec::ApplyingToWrongValue,
                    "Applying RandomElement to a function value, which is not a finite set.",
                ));
            }
            return Err(wrong());
        };
        match set.kind() {
            SetKind::Nat | SetKind::Int | SetKind::Strings => return Err(wrong()),
            SetKind::Interval => {
                let size = set
                    .size(self.core.ctx)
                    .map_err(|e| self.core.wrap_override(StdOp::TlcRandomElement, e.into()))?;
                if size == 0 {
                    return Err(self.core.wrap_override(StdOp::TlcRandomElement, EvalError::general(format!(
                        "Attempted to retrieve out-of-bounds element from the interval value {}.", self.core.ppr(&domain)
                    ))));
                }
            }
            SetKind::Enum | SetKind::Raw => {
                // Observe only the actual explicit storage, including Raw sets
                // that already have a normalized cache. Never enumerate Deferred.
                let Some(stored) = set.stored_elements() else {
                    return Err(EvalError::unsupported(
                        "RandomElement relation has no stored scalar entries",
                    ));
                };
                if stored.len() > limit.min(STORED_ENTRY_LIMIT) {
                    return Err(EvalError::resource_limit(
                        "RandomElement relation stored-set entries",
                    ));
                }
                for entry in stored {
                    self.random_relation_scalar(
                        entry,
                        "RandomElement relation compound stored element",
                    )?;
                }
                // Every immediate entry was checked before normalization or member.
                // Keep the normal comparator, normalization order and error class.
                let values = set
                    .elements(self.core.ctx)
                    .map_err(|e| self.core.wrap_override(StdOp::TlcRandomElement, e.into()))?;
                if values.is_empty() {
                    return Err(self.core.wrap_override(
                        StdOp::TlcRandomElement,
                        EvalError::general("Index 0 out of bounds for length 0"),
                    ));
                }
            }
            _ => {
                return Err(EvalError::unsupported(
                    "RandomElement relation requires an interval or stored finite scalar set",
                ));
            }
        }
        self.core.member(candidate, &domain).map(Some)
    }

    fn random_relation_state(&self, st: &St) -> Result<()> {
        let allowed = match (&st.s0, &st.s1, st.control) {
            (StateRef::Empty, StateRef::Empty, control::CONST) => true,
            (StateRef::Vals(slots), StateRef::Empty, control::INIT) => {
                slots.len() == self.core.ir.vars().len() && slots.iter().all(Option::is_none)
            }
            _ => false,
        };
        if allowed {
            Ok(())
        } else {
            Err(ContextError::new(
                ContextErrorKind::Structure,
                "RandomElement relation requires Const or empty Init with correct slots and no successor",
            )
            .into())
        }
    }

    fn random_relation_scalar(&self, value: &Value, compound: &'static str) -> Result<()> {
        match value {
            Value::Bool(_) | Value::Int(_) | Value::BigInt(_) => Ok(()),
            Value::Str(atom) | Value::Model(atom) => {
                if self
                    .core
                    .names
                    .resolve(atom.tok())
                    .is_some_and(|owned| owned.as_str() == atom.as_str())
                {
                    Ok(())
                } else {
                    Err(ContextError::new(
                        ContextErrorKind::ForeignContext,
                        "RandomElement relation scalar token does not resolve to its exact text",
                    )
                    .into())
                }
            }
            Value::Set(_) | Value::Fcn(_) => Err(EvalError::unsupported(compound)),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, reason = "test fixture assertions")]

    use std::fmt;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use dive_ir::ExprId;
    use dive_value::{Ctx, DeferredKind, DeferredSet, Int, IntMode, Interner, Value};

    use crate::{
        ConstBinding, Ec, ErrorClass, Evaluator, Host, HostError, Model, Options, PrintSink, Scope,
        St, StateRef,
    };

    const SOURCE: &str = "---- MODULE Relation ----\nEXTENDS Naturals, TLC\nCONSTANT Domain\nVARIABLE x\nChoice == RandomElement(Domain)\nEffectChoice == RandomElement(IF PrintT(TRUE) THEN Domain ELSE Domain)\nParam(v) == RandomElement(v)\nUsesVar == RandomElement({x})\nPure == 7\n====\n";

    #[derive(Default)]
    struct Sink(AtomicUsize);
    impl PrintSink for Sink {
        fn print(&self, _: String) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[derive(Default)]
    struct Callbacks(AtomicUsize);
    impl fmt::Debug for Callbacks {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.fetch_add(1, Ordering::SeqCst);
            f.write_str("callback-counted deferred source")
        }
    }
    impl DeferredSet for Callbacks {
        fn kind(&self) -> DeferredKind {
            self.0.fetch_add(1, Ordering::SeqCst);
            DeferredKind::Filter
        }
        fn elements(&self, _: Ctx) -> dive_value::Result<Vec<Value>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(vec![Value::Int(7)])
        }
        fn member(&self, _: &Value, _: Ctx) -> Option<dive_value::Result<bool>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Some(Ok(true))
        }
        fn key(&self) -> Vec<Value> {
            self.0.fetch_add(1, Ordering::SeqCst);
            vec![Value::Int(7)]
        }
    }

    #[derive(Debug, Default)]
    struct Draws(AtomicUsize);
    impl Host for Draws {
        fn random(
            &self,
            _: dive_ir::StdOp,
            _: &[Value],
            _: Ctx,
        ) -> Option<Result<Value, HostError>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Some(Ok(Value::Int(7)))
        }
    }

    struct Fixture {
        ev: Evaluator,
        names: Arc<Interner>,
        sink: Arc<Sink>,
        ctx: Ctx,
    }
    impl Fixture {
        fn new(tlc: bool, domain: impl FnOnce(&Interner, Ctx) -> Value) -> Self {
            let analysis =
                dive_sema::analyze_source("Relation.tla", SOURCE, &dive_sema::Options::default());
            assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
            let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
            let mut model = Model::new(ir);
            let options = if tlc { Options::tlc() } else { Options::dive() };
            let ctx = options.ctx;
            model.constants[0] = Some(ConstBinding::Value(domain(&model.names, ctx)));
            let names = model.names.clone();
            let sink = Arc::new(Sink::default());
            let ev = Evaluator::with_sink(model, options, sink.clone()).expect("evaluator");
            Self {
                ev,
                names,
                sink,
                ctx,
            }
        }
        fn predicate(&self, name: &str) -> (Scope, ExprId) {
            let def = self.ev.ir().root_def(name).expect("definition");
            (
                self.ev.scope(def).expect("scope"),
                self.ev.ir().def(def).body,
            )
        }
        fn member(
            &self,
            name: &str,
            st: &St,
            candidate: &Value,
            limit: usize,
        ) -> crate::Result<Option<bool>> {
            let (scope, expr) = self.predicate(name);
            self.ev
                .random_element_member(&scope, expr, st, candidate, limit)
        }
    }
    fn empty_init() -> St {
        St::initial(Arc::from([None]))
    }
    fn error_code(result: crate::Result<Option<bool>>, code: &str) {
        let error = result.expect_err("relation must reject input");
        assert!(error.message().contains(code), "{error}");
    }
    fn foreign(names: &Interner, mode: usize) -> Value {
        let target = names.intern("owned scalar");
        let source = Interner::new();
        let token = match mode {
            0 => usize::try_from(target.tok()).expect("token fits usize") + 1,
            1 | 3 => usize::try_from(target.tok()).expect("token fits usize"),
            _ => names.len() + 1,
        };
        for index in 0..token {
            source.intern(&format!("foreign filler {index}"));
        }
        Value::string(
            &source,
            if mode == 1 {
                "different scalar"
            } else {
                "owned scalar"
            },
        )
    }

    #[test]
    fn foreign_scalar_candidate_checks_precede_effects_in_both_profiles() {
        for tlc in [false, true] {
            let fixture = Fixture::new(tlc, |names, ctx| {
                Value::set_raw(vec![Value::string(names, "owned scalar")], ctx)
            });
            for mode in 0..3 {
                for model_value in [false, true] {
                    let candidate = foreign(&fixture.names, mode);
                    let candidate = if model_value {
                        let Value::Str(atom) = candidate else {
                            panic!("scalar")
                        };
                        Value::Model(atom)
                    } else {
                        candidate
                    };
                    let before = fixture.names.len();
                    error_code(
                        fixture.member("EffectChoice", &empty_init(), &candidate, 10),
                        "C004",
                    );
                    assert_eq!(fixture.names.len(), before);
                    assert_eq!(fixture.sink.0.load(Ordering::SeqCst), 0);
                }
            }
            // Different Arc ownership is harmless when both token and text agree.
            let candidate = foreign(&fixture.names, 3);
            assert_eq!(
                fixture
                    .member("Choice", &St::constant(), &candidate, 10)
                    .expect("matching token/text"),
                Some(true)
            );
            assert_eq!(
                fixture
                    .member("EffectChoice", &empty_init(), &candidate, 10)
                    .expect("normal domain evaluation"),
                Some(true)
            );
            assert_eq!(fixture.sink.0.load(Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn compound_candidates_refuse_before_domain_or_deferred_callbacks() {
        for tlc in [false, true] {
            let fixture = Fixture::new(tlc, |_, ctx| Value::set_raw(vec![Value::Int(7)], ctx));
            for cached in [false, true] {
                let calls = Arc::new(Callbacks::default());
                let deferred = Value::deferred(calls.clone(), fixture.ctx);
                if cached {
                    let Value::Set(set) = &deferred else {
                        panic!("set")
                    };
                    set.elements(fixture.ctx).expect("prime cache explicitly");
                }
                let candidates = [
                    deferred.clone(),
                    Value::tuple(vec![deferred.clone()]),
                    Value::set_raw(vec![deferred.clone()], fixture.ctx),
                ];
                // Tuple construction can inspect deferred keys. Only the
                // relation operation must add no callbacks to this baseline.
                let before = calls.0.load(Ordering::SeqCst);
                for candidate in candidates {
                    let count = fixture.names.len();
                    let error = fixture
                        .member("EffectChoice", &empty_init(), &candidate, 10)
                        .expect_err("compound candidate");
                    assert_eq!(error.class(), ErrorClass::Unsupported);
                    assert!(error.message().contains("compound candidate"));
                    assert_eq!(fixture.sink.0.load(Ordering::SeqCst), 0);
                    assert_eq!(calls.0.load(Ordering::SeqCst), before);
                    assert_eq!(fixture.names.len(), count);
                }
            }
        }
    }

    #[test]
    fn state_contract_refuses_assigned_foreign_deferred_and_invalid_contexts_before_effects() {
        for tlc in [false, true] {
            let fixture = Fixture::new(tlc, |_, ctx| Value::set_raw(vec![Value::Int(7)], ctx));
            let calls = Arc::new(Callbacks::default());
            let mut constant_with_assignment = St::constant();
            constant_with_assignment.s0 = StateRef::Vals(Arc::from([Some(Value::Int(7))]));
            let mut init_with_successor = empty_init();
            init_with_successor.s1 = StateRef::Vals(Arc::from([None]));
            let mut null_initial = empty_init();
            null_initial.s0 = StateRef::Null;
            let mut null_successor = empty_init();
            null_successor.s1 = StateRef::Null;
            for state in [
                St::empty(),
                St::default(),
                St::state(Arc::from([None])),
                St::state(Arc::from([Some(Value::Int(7))])),
                St::initial(Arc::from([Some(foreign(&fixture.names, 0))])),
                St::initial(Arc::from([Some(Value::deferred(
                    calls.clone(),
                    fixture.ctx,
                ))])),
                St::initial(Arc::from([])),
                St::initial(Arc::from([None, None])),
                St::action(Arc::from([None]), Arc::from([None])),
                empty_init().keep_lazy(),
                constant_with_assignment,
                init_with_successor,
                null_initial,
                null_successor,
            ] {
                let count = fixture.names.len();
                error_code(
                    fixture.member("EffectChoice", &state, &Value::Int(7), 10),
                    "C005",
                );
                assert_eq!(fixture.sink.0.load(Ordering::SeqCst), 0);
                assert_eq!(calls.0.load(Ordering::SeqCst), 0);
                assert_eq!(fixture.names.len(), count);
            }
            for state in [St::constant(), empty_init()] {
                assert_eq!(
                    fixture
                        .member("Choice", &state, &Value::Int(7), 10)
                        .expect("supported context"),
                    Some(true)
                );
            }
            let error = fixture
                .member("UsesVar", &empty_init(), &Value::Int(7), 10)
                .expect_err("missing variable stays unassigned");
            assert_eq!(error.class(), ErrorClass::Eval);
            assert_eq!(fixture.sink.0.load(Ordering::SeqCst), 0);
            assert_eq!(calls.0.load(Ordering::SeqCst), 0);
        }
    }

    #[test]
    fn context_and_expression_guards_win_before_value_guards_and_domain_effects() {
        for tlc in [false, true] {
            let fixture = Fixture::new(tlc, |_, ctx| Value::set_raw(vec![Value::Int(7)], ctx));
            let other = Fixture::new(tlc, |_, ctx| Value::set_raw(vec![Value::Int(7)], ctx));
            let (scope, expression) = fixture.predicate("EffectChoice");
            let (foreign_scope, _) = other.predicate("EffectChoice");
            let compound = Value::tuple(vec![Value::Int(7)]);
            error_code(
                fixture.ev.random_element_member(
                    &foreign_scope,
                    expression,
                    &St::empty(),
                    &compound,
                    10,
                ),
                "C004",
            );
            error_code(
                fixture.ev.random_element_member(
                    &scope,
                    ExprId(u32::MAX),
                    &St::empty(),
                    &compound,
                    10,
                ),
                "C001",
            );
            let (_, parameter_body) = fixture.predicate("Param");
            error_code(
                fixture.ev.random_element_member(
                    &scope,
                    parameter_body,
                    &St::constant(),
                    &compound,
                    10,
                ),
                "C005",
            );
            assert_eq!(fixture.sink.0.load(Ordering::SeqCst), 0);
            assert_eq!(other.sink.0.load(Ordering::SeqCst), 0);
        }
    }

    #[test]
    fn stored_foreign_entries_reject_raw_cached_raw_and_enum_before_member() {
        for tlc in [false, true] {
            for mode in 0..3 {
                for representation in 0..3 {
                    for model_value in [false, true] {
                        let fixture = Fixture::new(tlc, |names, ctx| {
                            let candidate = foreign(names, mode);
                            let entry = if model_value {
                                let Value::Str(atom) = candidate else {
                                    panic!("scalar")
                                };
                                Value::Model(atom)
                            } else {
                                candidate
                            };
                            if representation == 2 {
                                Value::set(vec![entry], ctx).expect("single stored entry")
                            } else {
                                let value = Value::set_raw(vec![entry], ctx);
                                if representation == 1 {
                                    let Value::Set(set) = &value else {
                                        panic!("set")
                                    };
                                    set.elements(ctx).expect("prime normalization explicitly");
                                }
                                value
                            }
                        });
                        let count = fixture.names.len();
                        error_code(
                            fixture.member("Choice", &St::constant(), &Value::Int(7), 10),
                            "C004",
                        );
                        assert_eq!(fixture.names.len(), count);
                        assert_eq!(fixture.sink.0.load(Ordering::SeqCst), 0);
                    }
                }
            }
        }
    }

    #[test]
    fn deferred_domains_and_compound_stored_entries_refuse_without_callbacks_even_if_cached() {
        for tlc in [false, true] {
            for representation in 0..4 {
                for cached in [false, true] {
                    let calls = Arc::new(Callbacks::default());
                    let fixture = Fixture::new(tlc, |_, ctx| {
                        let deferred = Value::deferred(calls.clone(), ctx);
                        if cached {
                            let Value::Set(set) = &deferred else {
                                panic!("set")
                            };
                            set.elements(ctx).expect("prime deferred cache explicitly");
                        }
                        match representation {
                            0 => deferred,
                            1 => Value::set_raw(vec![deferred], ctx),
                            2 => Value::set_raw(vec![Value::tuple(vec![deferred])], ctx),
                            _ => Value::set(vec![Value::tuple(vec![deferred])], ctx)
                                .expect("single compound enum entry"),
                        }
                    });
                    let before = calls.0.load(Ordering::SeqCst);
                    let error = fixture
                        .member("Choice", &St::constant(), &Value::Int(7), 10)
                        .expect_err("unsupported domain");
                    assert_eq!(error.class(), ErrorClass::Unsupported);
                    assert_eq!(calls.0.load(Ordering::SeqCst), before);
                    assert_eq!(fixture.sink.0.load(Ordering::SeqCst), 0);
                }
            }
        }
    }

    #[test]
    fn wrong_function_domain_preserves_native_class_without_formatting_nested_deferred_values() {
        for tlc in [false, true] {
            let calls = Arc::new(Callbacks::default());
            let fixture = Fixture::new(tlc, |_, ctx| {
                Value::tuple(vec![Value::deferred(calls.clone(), ctx)])
            });
            let before = calls.0.load(Ordering::SeqCst);
            let error = fixture
                .member("Choice", &St::constant(), &Value::Int(7), 10)
                .expect_err("not a set");
            assert_eq!(error.class(), ErrorClass::Eval);
            assert_eq!(error.ec(), Ec::ApplyingToWrongValue);
            assert_eq!(calls.0.load(Ordering::SeqCst), before);
        }
    }

    #[test]
    fn domain_budget_counts_storage_before_dedup_and_has_a_hard_ceiling() {
        for tlc in [false, true] {
            for cached in [false, true] {
                let fixture = Fixture::new(tlc, |_, ctx| {
                    let value = Value::set_raw(vec![Value::Int(7); 3], ctx);
                    if cached {
                        let Value::Set(set) = &value else {
                            panic!("set")
                        };
                        set.elements(ctx).expect("prime deduplicated cache");
                    }
                    value
                });
                let error = fixture
                    .member("Choice", &St::constant(), &Value::Int(7), 2)
                    .expect_err("raw storage is three entries");
                assert_eq!(error.class(), ErrorClass::ResourceLimit);
            }
            let fixture = Fixture::new(tlc, |_, ctx| {
                Value::set_raw(vec![Value::Int(7); 65_537], ctx)
            });
            assert_eq!(
                fixture
                    .member("Choice", &St::constant(), &Value::Int(7), usize::MAX)
                    .expect_err("hard entry bound")
                    .class(),
                ErrorClass::ResourceLimit
            );
            let bounded = Fixture::new(tlc, |_, ctx| {
                Value::set_raw(vec![Value::Int(7); 65_536], ctx)
            });
            assert_eq!(
                bounded
                    .member("Choice", &St::constant(), &Value::Int(7), usize::MAX)
                    .expect("exact bound"),
                Some(true)
            );
        }
    }

    #[test]
    fn owned_model_true_is_not_bool_true_and_member_keeps_profile_errors() {
        for tlc in [false, true] {
            let fixture = Fixture::new(tlc, |_, ctx| Value::interval(1.into(), 10.into(), ctx));
            let owned_model = Value::model(&fixture.names, "TRUE");
            assert_eq!(
                fixture
                    .member("Choice", &empty_init(), &owned_model, 10)
                    .expect("owned untyped Model TRUE"),
                Some(false)
            );
            let error = fixture
                .member("Choice", &empty_init(), &Value::TRUE, 10)
                .expect_err("Bool TRUE is a typed membership error");
            assert_eq!(error.class(), ErrorClass::Eval);
            assert_eq!(error.ec(), Ec::General);
            let typed_model = Value::model(&fixture.names, "A_value");
            assert_eq!(
                fixture
                    .member("Choice", &empty_init(), &typed_model, 10)
                    .expect_err("typed model interval error")
                    .ec(),
                Ec::General
            );
            let raw = Fixture::new(tlc, |names, ctx| {
                Value::set_raw(
                    vec![Value::Int(7), Value::string(names, "owned scalar")],
                    ctx,
                )
            });
            let error = raw
                .member("Choice", &St::constant(), &Value::Int(7), 10)
                .expect_err("native normalization comparability error");
            assert_eq!(error.class(), ErrorClass::Eval);
            assert_eq!(error.ec(), Ec::JavaMethodOverride);
        }
    }

    #[test]
    fn scalar_support_never_calls_random_host_and_big_integer_is_not_a_compound_value() {
        for tlc in [false, true] {
            let fixture = Fixture::new(tlc, |_, ctx| Value::set_raw(vec![Value::Int(7)], ctx));
            let calls = Arc::new(Draws::default());
            fixture.ev.set_host(calls.clone());
            assert_eq!(
                fixture
                    .member("Choice", &empty_init(), &Value::Int(7), 10)
                    .expect("support membership"),
                Some(true)
            );
            assert_eq!(
                fixture
                    .member("Choice", &empty_init(), &Value::Int(8), 10)
                    .expect("support exclusion"),
                Some(false)
            );
            assert_eq!(calls.0.load(Ordering::SeqCst), 0);
        }
        let large = Value::int(
            Int::from_literal("9223372036854775808", IntMode::Big).expect("big integer"),
        );
        assert!(matches!(&large, Value::BigInt(_)));
        let fixture = Fixture::new(false, |_, ctx| Value::set_raw(vec![large.clone()], ctx));
        assert_eq!(
            fixture
                .member("Choice", &St::constant(), &large, 10)
                .expect("big scalar"),
            Some(true)
        );
    }

    #[test]
    fn non_direct_expression_delegates_without_inspecting_a_compound_candidate() {
        let fixture = Fixture::new(true, |_, ctx| Value::set_raw(vec![Value::Int(7)], ctx));
        let calls = Arc::new(Callbacks::default());
        let candidate = Value::deferred(calls.clone(), fixture.ctx);
        assert_eq!(
            fixture
                .member("Pure", &empty_init(), &candidate, 10)
                .expect("not a direct native call"),
            None
        );
        assert_eq!(calls.0.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.sink.0.load(Ordering::SeqCst), 0);
    }
}
