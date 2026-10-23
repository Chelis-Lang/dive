//! Context interval columns retain eager domains, Cartesian order and stops.
#![allow(clippy::expect_used, reason = "finite source and control assertions")]

use dive_eval::{Ec, ErrorClass, Evaluator, HostControl, Interrupted, Model, Options, St};
use dive_ir::Expr;
use dive_value::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const SOURCE: &str = r"---- MODULE ContextIntervals ----
EXTENDS Integers, TLC
First == \E i \in 1..4096: IF i=1 THEN TRUE ELSE 1 \div 0=0
FirstFalse == \A i \in 1..4096: IF i=1 THEN FALSE ELSE 1 \div 0=0
Empty == \A i \in 2..1: 1 \div 0=0
TlcCapacityDomain == (-2147483647)..2147483647
DefaultMidDomain == 0..2147483647
DefaultMaxDomain == 0..18446744073709551614
DefaultMidFirst == \E i \in DefaultMidDomain: Print(99, TRUE)
DefaultMidFalse == \A i \in DefaultMidDomain: Print(99, FALSE)
DefaultMaxFirst == \E i \in DefaultMaxDomain: Print(99, TRUE)
DefaultMaxFalse == \A i \in DefaultMaxDomain: Print(99, FALSE)
DefaultMidAfterEmpty == \A i \in Print(11, 2..1), j \in DefaultMidDomain, k \in Print(22, {0}): TRUE
BigCapacityDomain == (-18446744073709551616)..18446744073709551616
TlcCapacityFirst == \E i \in (-2147483647)..2147483647: IF i=-2147483647 THEN TRUE ELSE 1 \div 0=0
TlcCapacityFalse == \A i \in (-2147483647)..2147483647: IF i=-2147483647 THEN FALSE ELSE 1 \div 0=0
BigCapacityFirst == \E i \in (-18446744073709551616)..18446744073709551616: IF i=-18446744073709551616 THEN TRUE ELSE 1 \div 0=0
BigCapacityFalse == \A i \in (-18446744073709551616)..18446744073709551616: IF i=-18446744073709551616 THEN FALSE ELSE 1 \div 0=0
AllDomains == \A i \in Print(11, 1..4096), j \in Print(22, {0}): Print(99, FALSE)
AfterEmpty == \A i \in Print(11, 2..1), j \in Print(22, {0}), k \in 1 \div 0: Print(99, FALSE)
AfterEarlyFalse == \A i \in 1..4096, j \in 1 \div 0: FALSE
Grouped == \A i,j \in 1..2: Print(10*j+i, TRUE)
Mixed == \A i \in 1..2, j \in {10,20}: Print(100*j+i, TRUE)
Bindings == \E i,j \in 1..2: Print(10*j+i, TRUE)
TupleMismatch == \A <<a,b>> \in 1..2: TRUE
TupleThenError == \A <<a,b>> \in 1..2, j \in 1 \div 0: TRUE
TupleThenEmpty == \A <<a,b>> \in 1..2, j \in {}: 1 \div 0=0
TupleStored == \A <<a,b>> \in {<<1,2>>, <<3,4>>}, j \in 1..2: Print(100*j+10*a+b, TRUE)
Finite == \A i \in 1..4096: i>0
Holder(q) == q /\ q
Cached == Holder(Finite)
FirstError == \A i \in 1..4096: 1 \div 0=0
Big == \A i \in 9223372036854775806..9223372036854775808: Print(i, TRUE)
NegativeBig == \A i \in (-9223372036854775809)..(-9223372036854775807): Print(i, TRUE)
SmallTlcLast == \A i \in 2147483646..2147483647: Print(i, TRUE)
BigCarry == \A i \in 9223372036854775808..9223372036854775809, j \in 1..2: Print(i, Print(j, TRUE))
MachineLast == \A i \in 9223372036854775806..9223372036854775807: Print(i, TRUE)
MachineFirst == \A i \in (-9223372036854775808)..(-9223372036854775807): Print(i, TRUE)
BigEmpty == \A i \in 9223372036854775808..9223372036854775806: 1 \div 0=0
====
";

fn evaluator(options: Options) -> Evaluator {
    let analysis = dive_sema::analyze_source(
        "ContextIntervals.tla",
        SOURCE,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    Evaluator::new(
        Model::new(Arc::new(dive_ir::lower(&analysis).expect("IR"))),
        options,
    )
    .expect("evaluator")
}

fn run(ev: &Evaluator, name: &str) -> dive_eval::Result<Value> {
    let def = ev.ir().root_def(name).expect("definition");
    ev.eval_in(def, ev.ir().def(def).body, &St::constant())
}

#[test]
fn large_first_witness_false_and_empty_do_not_materialize_an_interval() {
    for options in [Options::dive(), Options::tlc()] {
        let ev = evaluator(options);
        for (name, value) in [("First", true), ("FirstFalse", false), ("Empty", true)] {
            assert_eq!(run(&ev, name).expect(name), Value::Bool(value));
        }
        assert_eq!(run(&ev, "SmallTlcLast").expect("TLC endpoint"), Value::TRUE);
        assert_eq!(ev.take_output(), ["2147483646  TRUE", "2147483647  TRUE"]);
    }
    // Keep ordinary ordering/error fixtures small if streaming regresses.
    // Wide domains retain the old size-admission error BEFORE the body in both
    // profiles. Comparing size() directly never allocates the interval, even
    // if a later regression incorrectly permits its cardinality.
    for (options, domain, first, first_false) in [
        (
            Options::tlc(),
            "TlcCapacityDomain",
            "TlcCapacityFirst",
            "TlcCapacityFalse",
        ),
        (
            Options::dive(),
            "BigCapacityDomain",
            "BigCapacityFirst",
            "BigCapacityFalse",
        ),
    ] {
        let ctx = options.ctx;
        let ev = evaluator(options);
        let set = match run(&ev, domain).expect("primitive interval") {
            Value::Set(set) => Some(set),
            _ => None,
        }
        .expect("interval set");
        let expected = dive_eval::EvalError::from(set.size(ctx).expect_err("old size admission"));
        assert_eq!(expected.class(), ErrorClass::Eval);
        assert_eq!(expected.ec(), Ec::General);
        for name in [first, first_false] {
            let actual = run(&ev, name).expect_err("size error before deciding body");
            assert_eq!(actual.class(), expected.class());
            assert_eq!(actual.ec(), expected.ec());
            assert_eq!(actual.message(), expected.message());
        }
    }
}

#[test]
fn default_materialization_ceiling_still_precedes_body_and_later_domains() {
    let options = Options::dive();
    let ctx = options.ctx;
    let ev = evaluator(options);
    for (domain, first, first_false, count, printed) in [
        (
            "DefaultMidDomain",
            "DefaultMidFirst",
            "DefaultMidFalse",
            2_147_483_648_u64,
            "0..2147483647",
        ),
        (
            "DefaultMaxDomain",
            "DefaultMaxFirst",
            "DefaultMaxFalse",
            u64::MAX,
            "0..18446744073709551614",
        ),
    ] {
        let set = match run(&ev, domain).expect("interval metadata") {
            Value::Set(set) => Some(set),
            _ => None,
        }
        .expect("interval set");
        assert_eq!(set.size(ctx).expect("default u64 size admission"), count);
        assert!(count > dive_value::MAX_ENUMERATION);
        // Do not invoke elements() to derive the expectation: a future cap
        // regression must never materialize these enormous domains in a test.
        let message = format!(
            "The set\n{printed}\nhas {count} elements, more than dive enumerates (2147483647)."
        );
        let expected = dive_eval::EvalError::from(dive_value::ValueError::new(
            dive_value::ErrorKind::TooLarge,
            message,
        ));
        for name in [first, first_false] {
            let actual = run(&ev, name).expect_err("bulk ceiling before body");
            assert_eq!(actual.class(), ErrorClass::Eval);
            assert_eq!(actual.ec(), Ec::General);
            assert_eq!(actual.message(), expected.message());
            assert_eq!(ev.take_output(), [] as [String; 0]);
        }
    }
    let actual = run(&ev, "DefaultMidAfterEmpty").expect_err("ceiling before later domain");
    assert_eq!(actual.class(), ErrorClass::Eval);
    assert_eq!(actual.ec(), Ec::General);
    assert_eq!(
        actual.message(),
        "The set\n0..2147483647\nhas 2147483648 elements, more than dive enumerates (2147483647)."
    );
    assert_eq!(ev.take_output(), ["11  {}"]);
}

#[test]
fn all_domain_evaluation_precedes_an_empty_product_or_a_deciding_body() {
    for options in [Options::dive(), Options::tlc()] {
        let ev = evaluator(options);
        assert_eq!(run(&ev, "AllDomains").expect("first false"), Value::FALSE);
        // TLC.Print emits both operands separated by two spaces, returning the
        // second. Compare complete events, not only the first operand's marker.
        assert_eq!(ev.take_output(), ["11  1..4096", "22  {0}", "99  FALSE"]);
        for (name, output) in [
            ("AfterEmpty", vec!["11  {}", "22  {0}"]),
            ("AfterEarlyFalse", vec![]),
        ] {
            let error = run(&ev, name).expect_err("domain error before body");
            assert_eq!(error.ec().code(), 2179);
            assert_eq!(ev.take_output(), output);
        }
    }
}

#[test]
fn grouped_mixed_and_stored_tuple_columns_are_first_variable_fastest() {
    for options in [Options::dive(), Options::tlc()] {
        let ev = evaluator(options);
        for (name, output) in [
            (
                "Grouped",
                vec!["11  TRUE", "12  TRUE", "21  TRUE", "22  TRUE"],
            ),
            (
                "Mixed",
                vec!["1001  TRUE", "1002  TRUE", "2001  TRUE", "2002  TRUE"],
            ),
            (
                "TupleStored",
                vec!["112  TRUE", "134  TRUE", "212  TRUE", "234  TRUE"],
            ),
        ] {
            assert_eq!(run(&ev, name).expect("complete quantifier"), Value::TRUE);
            assert_eq!(ev.take_output(), output);
        }
    }
}

#[test]
fn eager_scope_bindings_still_collect_every_context_without_running_the_body() {
    for options in [Options::dive(), Options::tlc()] {
        let ev = evaluator(options);
        let def = ev.ir().root_def("Bindings").expect("Bindings");
        let quant = ev.ir().def(def).body;
        let body = match ev.ir().expr(quant) {
            Expr::Quant { body, .. } => Some(*body),
            _ => None,
        }
        .expect("fixture quantifier");
        let scopes = ev
            .scope_bindings(&ev.scope(def).expect("scope"), quant, &St::constant())
            .expect("full context batch");
        assert_eq!(scopes.len(), 4);
        assert_eq!(ev.take_output(), [] as [String; 0]);
        for scope in scopes {
            assert_eq!(
                ev.eval_scoped(&scope, body, &St::constant()).expect("body"),
                Value::TRUE
            );
        }
        assert_eq!(
            ev.take_output(),
            ["11  TRUE", "12  TRUE", "21  TRUE", "22  TRUE"]
        );
    }
}

#[test]
fn tuple_mismatch_is_deferred_until_after_all_domains_and_empty_detection() {
    for options in [Options::dive(), Options::tlc()] {
        let ev = evaluator(options);
        assert_eq!(
            run(&ev, "TupleMismatch")
                .expect_err("integer is not a tuple")
                .ec(),
            Ec::ArgumentMismatch
        );
        assert_eq!(
            run(&ev, "TupleThenError")
                .expect_err("later domain wins")
                .ec()
                .code(),
            2179
        );
        assert_eq!(
            run(&ev, "TupleThenEmpty").expect("empty product"),
            Value::TRUE
        );
    }
}

#[derive(Debug)]
struct Stop(AtomicUsize);
impl HostControl for Stop {
    fn check(&self) -> Result<(), Interrupted> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Err(Interrupted::new(73))
    }
}

#[test]
fn typed_interruption_does_not_publish_a_partial_lazy_result_and_retry_completes() {
    for options in [Options::dive(), Options::tlc()] {
        let ev = evaluator(options);
        let holder = ev.ir().root_def("Holder").expect("Holder");
        let caller = ev.ir().root_def("Cached").expect("Cached");
        let args = match ev.ir().expr(ev.ir().def(caller).body) {
            Expr::Apply { args, .. } => Some(args.as_ref()),
            _ => None,
        }
        .expect("actual source application");
        let captured = ev
            .scope_apply(&ev.scope(caller).expect("actual caller"), holder, args)
            .expect("one retained lazy argument");
        let body = ev.ir().def(holder).body;
        let control = Arc::new(Stop(AtomicUsize::new(0)));
        let scope = ev.host_control(Some(control.clone())).expect("control");
        let error = ev
            .eval_scoped(&captured, body, &St::constant())
            .expect_err("cooperative stop");
        assert_eq!(error.class(), ErrorClass::Interrupted);
        assert_eq!(error.interruption().expect("typed identity").token(), 73);
        assert!(control.0.load(Ordering::Relaxed) > 0);
        drop(scope);
        assert_eq!(
            ev.eval_scoped(&captured, body, &St::constant())
                .expect("same thunk retry"),
            Value::TRUE
        );
        assert_eq!(
            ev.eval_scoped(&captured, body, &St::constant())
                .expect("completed cache"),
            Value::TRUE
        );
    }
}

#[test]
fn first_body_error_keeps_priority_before_the_next_host_checkpoint() {
    std::thread::spawn(|| {
        for options in [Options::dive(), Options::tlc()] {
            let ev = evaluator(options);
            let control = Arc::new(Stop(AtomicUsize::new(0)));
            let _scope = ev.host_control(Some(control.clone())).expect("control");
            let error = run(&ev, "FirstError").expect_err("real first error");
            assert_eq!(error.ec().code(), 2179);
            assert_eq!(error.class(), ErrorClass::Eval);
            assert_eq!(control.0.load(Ordering::Relaxed), 0);
        }
    })
    .join()
    .expect("fresh checkpoint thread");
}

#[test]
fn big_and_machine_endpoint_carries_do_not_overflow_or_skip_values() {
    let ev = evaluator(Options::dive());
    for (name, output) in [
        (
            "Big",
            vec![
                "9223372036854775806  TRUE",
                "9223372036854775807  TRUE",
                "9223372036854775808  TRUE",
            ],
        ),
        (
            "NegativeBig",
            vec![
                "-9223372036854775809  TRUE",
                "-9223372036854775808  TRUE",
                "-9223372036854775807  TRUE",
            ],
        ),
        (
            "BigCarry",
            vec![
                "1  TRUE",
                "9223372036854775808  TRUE",
                "1  TRUE",
                "9223372036854775809  TRUE",
                "2  TRUE",
                "9223372036854775808  TRUE",
                "2  TRUE",
                "9223372036854775809  TRUE",
            ],
        ),
        (
            "MachineLast",
            vec!["9223372036854775806  TRUE", "9223372036854775807  TRUE"],
        ),
        (
            "MachineFirst",
            vec!["-9223372036854775808  TRUE", "-9223372036854775807  TRUE"],
        ),
    ] {
        assert_eq!(run(&ev, name).expect("endpoints"), Value::TRUE);
        assert_eq!(ev.take_output(), output);
    }
    assert_eq!(
        run(&ev, "BigEmpty").expect("empty big interval"),
        Value::TRUE
    );
}
