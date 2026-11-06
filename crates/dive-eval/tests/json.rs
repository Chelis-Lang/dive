//! Pure JSON conversions against pinned TLC `94d0c50` probe outputs.
#![allow(clippy::expect_used, reason = "test fixture construction")]

use std::sync::Arc;

use dive_eval::{ErrorClass, Evaluator, Options, St};
use dive_value::Value;

#[test]
fn consumed_pinned_oracle_fixture_matches_both_profiles() {
    let source = include_str!("fixtures/json/JsonSupported.tla");
    let config = dive_config::parse(include_str!("fixtures/json/JsonSupported.cfg"))
        .into_config()
        .expect("config");
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/json/oracle.json")).expect("oracle");
    assert_eq!(oracle["exit"], 0);
    for options in [Options::tlc(), Options::dive()] {
        let analysis =
            dive_sema::analyze_source("JsonSupported.tla", source, &dive_sema::Options::default());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let actual = ir.root_def("Actual").expect("Actual");
        let model = dive_model::bind(
            ir,
            &config,
            &dive_model::BindOptions {
                eval: options,
                ..dive_model::BindOptions::default()
            },
        )
        .expect("production binding");
        let evaluator = &model.evaluator;
        let result = evaluator
            .eval_def(actual, &St::constant())
            .expect("JSON results");
        let elements = result
            .as_fcn()
            .and_then(dive_value::Fcn::as_tuple)
            .expect("tuple");
        let expected = oracle["expected"].as_array().expect("expected");
        assert_eq!(elements.len(), expected.len());
        for (actual, expected) in elements.iter().zip(expected) {
            let Value::Str(text) = actual else {
                unreachable!("JSON string")
            };
            assert_eq!(text.as_str(), expected.as_str().expect("string"));
        }
    }
}

fn evaluate(expression: &str, options: Options) -> dive_eval::Result<Value> {
    let source = format!(
        "---- MODULE JsonControl ----\nEXTENDS Naturals, Integers, Sequences, TLC, Json\nResult == {expression}\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("JsonControl.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let result = ir.root_def("Result").expect("Result");
    let model = dive_model::bind(
        ir,
        &dive_config::ModelConfig::default(),
        &dive_model::BindOptions {
            eval: options,
            ..dive_model::BindOptions::default()
        },
    )
    .expect("production binding");
    let evaluator = model.evaluator;
    evaluator.eval_def(result, &St::constant())
}

#[test]
fn scalar_sequence_and_normalized_set_outputs_match_tlc() {
    for options in [Options::tlc(), Options::dive()] {
        for (expression, expected) in [
            ("ToJson(1)", "1"),
            ("ToJson(TRUE)", "true"),
            ("ToJson(FALSE)", "false"),
            ("ToJson(<<>>)", "[]"),
            ("ToJson({3,1,2})", "[1,2,3]"),
            ("ToJson(1..3)", "[1,2,3]"),
            ("ToJson([i \\in 1..2 |-> i])", "[1,2]"),
            ("ToJson([a |-> 1])", r#"{"a":1}"#),
            (r#"ToJson(<<"x", TRUE, <<1,2>>>>)"#, r#"["x",true,[1,2]]"#),
            (
                r#"ToJson("<>&='\"\\\t\n\r\f  ")"#,
                r#""<>&='\"\\\t\n\r\f\u2028\u2029""#,
            ),
        ] {
            let value = evaluate(expression, options).expect(expression);
            let Value::Str(text) = value else {
                unreachable!("JSON conversion returns a string")
            };
            assert_eq!(text.as_str(), expected, "{expression}");
        }
    }
}

#[test]
fn object_order_is_canonical_by_default_and_explicitly_refused_in_tlc_mode() {
    let expression = "ToJson([b |-> 1, a |-> 2])";
    let Value::Str(text) = evaluate(expression, Options::dive()).expect("default object") else {
        unreachable!("string")
    };
    assert_eq!(text.as_str(), r#"{"a":2,"b":1}"#);
    for expression in [expression, "ToJson(<<[b |-> 1, a |-> 2]>>)"] {
        let error = evaluate(expression, Options::tlc()).expect_err("object history is absent");
        assert_eq!(error.class(), ErrorClass::Unsupported);
        assert!(error.message().contains("history-dependent object order"));
    }
}

#[test]
fn ambiguous_and_unimplemented_conversions_are_named_refusals() {
    for (options, default_profile) in [(Options::tlc(), false), (Options::dive(), true)] {
        for expression in [
            "ToJson(Nat)",
            "ToJson(STRING)",
            "ToJson({Nat})",
            "ToJson([a |-> Nat])",
            r#"JsonSerialize("must-not-create.json", <<1,2>>)"#,
            r#"JsonDeserialize("must-not-read.json")"#,
            r#"ndJsonSerialize("must-not-create.ndjson", <<1,2>>)"#,
            r#"ndJsonDeserialize("must-not-read.ndjson")"#,
            "ToJsonArray(<<1,2>>)",
            "ToJsonObject(<<1,2>>)",
        ] {
            let error = evaluate(expression, options).expect_err("unsupported conversion");
            assert_eq!(
                error.class(),
                ErrorClass::Unsupported,
                "{expression}: {error}"
            );
            if default_profile && expression == "ToJson({Nat})" {
                assert_eq!(
                    error.message(),
                    "mathematical identity requires materialized values; symbolic or cross-order children are unsupported"
                );
            } else {
                assert!(error.message().contains("Json!"));
            }
        }
    }
}

fn injected(
    options: Options,
    make: impl FnOnce(&dive_value::Interner) -> Value,
) -> dive_eval::Result<Value> {
    let source =
        "---- MODULE JsonInjected ----\nEXTENDS Json\nCONSTANT C\nResult == ToJson(C)\n====\n";
    let analysis =
        dive_sema::analyze_source("JsonInjected.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let result = ir.root_def("Result").expect("Result");
    let mut model = dive_eval::Model::new(ir);
    model.constants[0] = Some(dive_eval::ConstBinding::Value(make(&model.names)));
    let evaluator = Evaluator::new(model, options).expect("evaluator");
    evaluator.eval_def(result, &St::constant())
}

#[test]
fn control_characters_and_injected_set_order_follow_gson_and_run_order() {
    for options in [Options::tlc(), Options::dive()] {
        let value = injected(options, |names| {
            Value::string(names, "\u{0000}\u{0001}\u{0008}\u{000c}/é😀")
        })
        .expect("string");
        let Value::Str(text) = value else {
            unreachable!("string")
        };
        assert_eq!(text.as_str(), "\"\\u0000\\u0001\\b\\f/é😀\"");
    }
    let value = injected(Options::dive(), |names| {
        let b = Value::string(names, "b");
        let a = Value::string(names, "a");
        Value::set(vec![b, a], dive_value::Ctx::tlc()).expect("set")
    })
    .expect("canonical set");
    let Value::Str(text) = value else {
        unreachable!("string")
    };
    assert_eq!(text.as_str(), r#"["a","b"]"#);
}

#[test]
fn recursive_conversion_honors_the_evaluation_depth_limit() {
    let mut options = Options::dive();
    options.max_depth = 8;
    let error = injected(options, |_| {
        (0..12).fold(Value::TRUE, |value, _| Value::tuple(vec![value]))
    })
    .expect_err("bounded conversion");
    assert_eq!(error.class(), ErrorClass::ResourceLimit);
}

#[test]
fn compound_domain_history_is_refused_without_changing_default_semantics() {
    let source = include_str!("fixtures/json/JsonDomainHistory.tla");
    let config = dive_config::parse(include_str!("fixtures/json/JsonDomainHistory.cfg"))
        .into_config()
        .expect("config");
    for options in [Options::tlc(), Options::dive()] {
        let analysis = dive_sema::analyze_source(
            "JsonDomainHistory.tla",
            source,
            &dive_sema::Options::default(),
        );
        assert!(analysis.accepted());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let defs: Vec<_> = ["Record", "Tuple", "Set", "Function", "Predicate"]
            .iter()
            .map(|name| ir.root_def(name).expect("definition"))
            .collect();
        let model = dive_model::bind(
            ir,
            &config,
            &dive_model::BindOptions {
                eval: options,
                ..dive_model::BindOptions::default()
            },
        )
        .expect("production binding");
        for def in &defs {
            let result = model.evaluator.eval_def(*def, &St::constant());
            if options.ctx.order == dive_value::ValueOrder::Tlc {
                let error = result.expect_err("lost record construction history");
                assert_eq!(error.class(), ErrorClass::Unsupported);
                assert!(
                    error
                        .message()
                        .contains("history-sensitive compound object domain")
                );
            } else {
                assert!(result.is_ok(), "{result:?}");
            }
        }
        if options.ctx.order == dive_value::ValueOrder::Canonical {
            let record = model
                .evaluator
                .eval_def(defs[0], &St::constant())
                .expect("default");
            let Value::Str(text) = record else {
                unreachable!("JSON string")
            };
            assert_eq!(text.as_str(), r#"{"[a |-> 2, b |-> 1]":7}"#);
            assert_eq!(
                model
                    .evaluator
                    .eval_def(defs[4], &St::constant())
                    .expect("predicate"),
                Value::FALSE
            );
        }
    }
}

#[test]
fn stable_compound_domains_remain_supported() {
    for options in [Options::tlc(), Options::dive()] {
        for (expression, expected) in [
            ("ToJson([i \\in {0} |-> 7])", r#"{"0":7}"#),
            ("ToJson([i \\in {FALSE} |-> 7])", r#"{"FALSE":7}"#),
            ("ToJson([i \\in {<<1>>} |-> 7])", r#"{"<<1>>":7}"#),
            ("ToJson([i \\in {[a |-> 1]} |-> 7])", r#"{"[a |-> 1]":7}"#),
            ("ToJson([i \\in {{1,2}} |-> 7])", r#"{"{1, 2}":7}"#),
            (
                "ToJson([i \\in {(2 :> 20) @@ (1 :> 10)} |-> 7])",
                r#"{"<<10, 20>>":7}"#,
            ),
            (
                "ToJson([i \\in {(2 :> 20) @@ (0 :> 10)} |-> 7])",
                r#"{"(0 :> 10 @@ 2 :> 20)":7}"#,
            ),
        ] {
            let Value::Str(text) = evaluate(expression, options).expect("stable domain") else {
                unreachable!("JSON string")
            };
            assert_eq!(text.as_str(), expected, "{expression}");
        }
    }
}

#[test]
fn domain_and_value_children_share_the_configured_depth_boundary() {
    for mut options in [Options::tlc(), Options::dive()] {
        options.max_depth = 8;
        for kind in ["tuple", "set", "function"] {
            for domain in [false, true] {
                for depth in [5, 6, 12] {
                    let result = injected(options, |_| {
                        let deep = (0..depth).fold(Value::TRUE, |item, _| match kind {
                            "tuple" => Value::tuple(vec![item]),
                            "set" => Value::set(vec![item], options.ctx).expect("set"),
                            _ => Value::fcn(vec![(Value::Int(0), item)], options.ctx)
                                .expect("function"),
                        });
                        let pair = if domain {
                            (deep, Value::Int(7))
                        } else {
                            (Value::Int(0), deep)
                        };
                        Value::fcn(vec![pair], options.ctx).expect("outer object")
                    });
                    if depth == 5 {
                        assert!(result.is_ok(), "{kind} domain={domain}: {result:?}");
                    } else {
                        assert_eq!(
                            result.expect_err("configured boundary").class(),
                            ErrorClass::ResourceLimit,
                            "{kind} domain={domain}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn combinatorial_domain_printing_is_an_explicit_refusal() {
    for options in [Options::tlc(), Options::dive()] {
        let error = injected(options, |_| {
            let domain = Value::subset(Value::nat(options.ctx), options.ctx);
            Value::fcn(vec![(domain, Value::Int(7))], options.ctx).expect("object")
        })
        .expect_err("unbounded domain expansion");
        assert_eq!(error.class(), ErrorClass::Unsupported);
        assert!(
            error
                .message()
                .contains("domain-set printing is not bounded")
        );
    }
}

#[derive(Debug)]
struct CountingDomain(std::sync::atomic::AtomicUsize);

impl dive_value::DeferredSet for CountingDomain {
    fn kind(&self) -> dive_value::DeferredKind {
        dive_value::DeferredKind::Map
    }

    fn elements(&self, _: dive_value::Ctx) -> dive_value::Result<Vec<Value>> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(vec![Value::TRUE])
    }

    fn member(&self, _: &Value, _: dive_value::Ctx) -> Option<dive_value::Result<bool>> {
        None
    }
}

#[test]
fn public_conversion_refuses_deferred_domains_before_printer_callbacks() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    for options in [Options::tlc(), Options::dive()] {
        let provider = Arc::new(CountingDomain(AtomicUsize::new(0)));
        let construction_calls = AtomicUsize::new(0);
        let error = injected(options, |_| {
            let domain = Value::tuple(vec![Value::deferred(provider.clone(), options.ctx)]);
            let object = Value::fcn(vec![(domain, Value::Int(7))], options.ctx).expect("object");
            // Construction may hash/materialize the key before conversion.
            construction_calls.store(provider.0.load(Ordering::Relaxed), Ordering::Relaxed);
            object
        })
        .expect_err("bounded printer");
        assert_eq!(error.class(), ErrorClass::Unsupported);
        assert!(
            error
                .message()
                .contains("domain-set printing is not bounded")
        );
        assert_eq!(
            provider.0.load(Ordering::Relaxed),
            construction_calls.load(Ordering::Relaxed)
        );
    }
}
