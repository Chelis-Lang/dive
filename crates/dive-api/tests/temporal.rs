//! Final temporal API boundaries, independently validated lossless reports.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_api::temporal::{self, Error, Outcome};
use dive_check::{CheckOptions, Verdict};
use dive_liveness::analyze::Limits;
use dive_store::StoreMode;
use dive_trace::Validation;
use std::sync::Arc;

fn input(source: &str, cfg: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let name = source.split_whitespace().nth(2).expect("module");
    let a = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    assert!(a.accepted(), "{a:?}");
    (
        Arc::new(dive_ir::lower(&a).expect("IR")),
        dive_config::parse(cfg).into_config().expect("cfg"),
    )
}
fn counts(stdout: &str) -> (u64, u64, u64) {
    let line = stdout
        .lines()
        .find(|s| s.contains("states generated,") && s.contains("distinct states found"))
        .expect("counts");
    let ns: Vec<u64> = line
        .split_whitespace()
        .filter_map(|s| s.replace(',', "").parse().ok())
        .collect();
    let depth = stdout
        .lines()
        .find(|s| s.contains("depth of the complete state graph search"))
        .expect("depth")
        .split_whitespace()
        .last()
        .expect("number")
        .trim_end_matches('.')
        .parse()
        .expect("number");
    (ns[0], ns[1], depth)
}

#[test]
fn pinned_final_runs_preserve_counts_and_return_only_validated_periodic_claims() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/tlc-probes");
    for name in [
        "LvNoFair",
        "LvAlwaysEventually",
        "LvEventuallyAlways",
        "LvLeadsTo",
        "LvActionProp",
        "LvLassoMid",
        "LvEAPrefix",
        "LvStutterActionConstraint",
        "LvWFIntermittent",
        "LvStutterEnd",
        "LvWeakFair",
        "LvSFIntermittent",
        "LvConstraint",
    ] {
        let source =
            std::fs::read_to_string(root.join("specs").join(name).join(format!("{name}.tla")))
                .expect("spec");
        let cfg =
            std::fs::read_to_string(root.join("specs").join(name).join(format!("{name}.cfg")))
                .expect("cfg");
        let stdout = std::fs::read_to_string(
            root.join("tlc-2026.10.06.014338")
                .join(format!("{name}.stdout")),
        )
        .expect("oracle");
        let (ir, cfg) = input(&source, &cfg);
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            for workers in [1, 8] {
                let opts = CheckOptions {
                    store,
                    workers,
                    ..CheckOptions::tlc(0, 19)
                };
                let report =
                    temporal::check_final(&ir, &cfg, &opts, Limits::default()).expect(name);
                let c = match report.outcome() {
                    Outcome::Holds(c) => {
                        assert!(
                            name == "LvWeakFair"
                                || name == "LvSFIntermittent"
                                || name == "LvConstraint",
                            "{name}"
                        );
                        c.as_ref()
                    }
                    Outcome::Violation(v) => {
                        assert_ne!(name, "LvWeakFair");
                        assert_ne!(name, "LvSFIntermittent");
                        assert_ne!(name, "LvConstraint");
                        assert_eq!(v.validation().validation, Validation::Valid);
                        assert_eq!(v.document().properties, v.candidate().properties);
                        let model =
                            dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc())
                                .expect("fresh validation model");
                        let doc = dive_trace::TraceDoc::parse(&v.document().to_json().to_string())
                            .expect("lossless serialization");
                        assert_eq!(
                            dive_trace::validate_doc(&model, &doc, &cfg.model_values, None)
                                .0
                                .validation,
                            Validation::Valid,
                            "{name}"
                        );
                        &v.candidate().exploration
                    }
                    Outcome::Safety(r) => panic!("{name}: {:?}", r.verdict),
                };
                assert_eq!(
                    (c.stats.generated, c.stats.distinct, c.stats.depth),
                    counts(&stdout),
                    "{name}"
                );
                assert_eq!((c.workers.requested, c.workers.used), (workers, 1));
                assert_eq!(c.options.seed, 19);
                assert_eq!(c.options.store, store);
            }
        }
    }
}

#[test]
fn default_profile_attributes_all_false_properties_and_repeat_runs_rebind() {
    let src = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = ~x\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nP == <>[]x\nQ == <>[](~x)\nR == []<>x\n====\n";
    let (ir, cfg) = input(src, "SPECIFICATION Spec\nPROPERTY P Q R\n");
    for _ in 0..2 {
        let r = temporal::check_final(&ir, &cfg, &CheckOptions::dive(17), Limits::default())
            .expect("run");
        let Outcome::Violation(v) = r.outcome() else {
            panic!("two properties false")
        };
        assert_eq!(v.document().properties, ["P", "Q"]);
        assert_eq!(v.validation().validation, Validation::Valid);
        assert_eq!(v.candidate().exploration.options.seed, 17);
    }
}

#[test]
fn safety_stops_retain_trace_statistics_and_assumption_failures() {
    let src = "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nInv == ~x\nP == []<>x\n====\n";
    let (ir, cfg) = input(src, "INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY P\n");
    let opts = CheckOptions::tlc(0, 0);
    let r = temporal::check_final(&ir, &cfg, &opts, Limits::default()).expect("safety stop");
    let Outcome::Safety(r) = r.outcome() else {
        panic!("invariant takes precedence")
    };
    assert!(matches!(r.verdict, Verdict::InvariantViolated { .. }));
    assert_eq!(
        (
            r.stats.expect("counters").generated,
            r.stats.expect("counters").distinct
        ),
        (2, 2)
    );
    assert_eq!(r.counterexample.as_ref().expect("trace").states.len(), 2);
    let (ir, cfg) = input(
        &src.replace("VARIABLE x", "VARIABLE x\nASSUME FALSE"),
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    );
    let r = temporal::check_final(&ir, &cfg, &opts, Limits::default()).expect("assumption stop");
    assert!(
        matches!(r.outcome(), Outcome::Safety(r) if matches!(r.verdict, Verdict::AssumptionFalse { .. }))
    );
}

#[test]
fn state_argument_holds_and_resource_limits_never_become_holds() {
    let src = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\nF(a) == []<>a\nP == F(x)\n====\n";
    let (ir, cfg) = input(src, "INIT Init\nNEXT Next\nPROPERTY P\n");
    let r = temporal::check_final(&ir, &cfg, &CheckOptions::dive(0), Limits::default())
        .expect("pure state argument");
    assert!(matches!(r.outcome(), Outcome::Holds(_)));
    let r = temporal::check_final(
        &ir,
        &cfg,
        &CheckOptions::dive(0),
        Limits {
            capture: dive_check::capture::Limits {
                states: 0,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .expect("capture limit result");
    assert!(
        matches!(r.outcome(), Outcome::Safety(r) if matches!(r.verdict, Verdict::ResourceLimit { .. }))
    );
    let e = temporal::check_final(
        &ir,
        &cfg,
        &CheckOptions::dive(0),
        Limits {
            properties: 0,
            ..Default::default()
        },
    )
    .expect_err("planning bound");
    assert_eq!(e.code(), "DIVE-L0034");
}

#[test]
fn undecidable_independent_enabledness_is_a_refusal_not_a_violation() {
    let src = "---- MODULE M ----\nVARIABLE x\nInit == x = [n \\in {0} |-> 0]\nNext == x' \\in [{0} -> {0}] /\\ x' # x'\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nP == <>(x # x)\n====\n";
    let (ir, cfg) = input(
        src,
        "SPECIFICATION Spec\nPROPERTY P\nCHECK_DEADLOCK FALSE\n",
    );
    let e = temporal::check_final(&ir, &cfg, &CheckOptions::dive(0), Limits::default())
        .expect_err("independent procedure cannot cover a lazy function set");
    assert!(
        matches!(&e, Error::Validation { report, .. } if matches!(report.validation, Validation::Unsupported(_))),
        "{e:?}"
    );
    assert_eq!(e.code(), "DIVE-A0032");
}

#[test]
fn pinned_constant_domain_quantifiers_match_verdicts_and_counts() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/temporal-quantifiers");
    for name in ["QtVisit", "QtFairVisit", "QtStable", "QtEmpty", "QtTuple"] {
        let src =
            std::fs::read_to_string(root.join("specs").join(name).join(format!("{name}.tla")))
                .expect("spec");
        let cfg =
            std::fs::read_to_string(root.join("specs").join(name).join(format!("{name}.cfg")))
                .expect("cfg");
        let stdout = std::fs::read_to_string(
            root.join("tlc-2026.10.06.014338")
                .join(format!("{name}.stdout")),
        )
        .expect("oracle");
        let (ir, cfg) = input(&src, &cfg);
        for opts in [CheckOptions::tlc(0, 17), CheckOptions::dive(17)] {
            for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                let opts = CheckOptions {
                    store,
                    ..opts.clone()
                };
                let report =
                    temporal::check_final(&ir, &cfg, &opts, Limits::default()).expect(name);
                let c = match report.outcome() {
                    Outcome::Holds(c) => {
                        assert!(name == "QtFairVisit" || name == "QtTuple", "{name}");
                        c.as_ref()
                    }
                    Outcome::Violation(v) => {
                        assert!(
                            name == "QtVisit" || name == "QtStable" || name == "QtEmpty",
                            "{name}"
                        );
                        assert_eq!(v.validation().validation, Validation::Valid);
                        &v.candidate().exploration
                    }
                    Outcome::Safety(r) => panic!("{name}: {:?}", r.verdict),
                };
                assert_eq!(
                    (c.stats.generated, c.stats.distinct, c.stats.depth),
                    counts(&stdout),
                    "{name}"
                );
            }
        }
    }
}

#[test]
fn quantified_word_truth_matches_cycle_algebra_and_empty_domain_identities() {
    let src = "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == \\A i \\in {TRUE,FALSE}: []<>(x=i)\nQ == \\E i \\in {TRUE,FALSE}: <>[](x=i)\nE == \\E i \\in {}: []<>x\nA == \\A i \\in {}: []<>x\nT == \\A <<i,j>> \\in {<<TRUE,FALSE>>,<<FALSE,TRUE>>}: []<>(x=i \\/ x=j)\n====\n";
    let (ir, cfg) = input(src, "INIT Init\nNEXT Next\n");
    let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
    for n in 1..=4 {
        for bits in 0..(1_usize << n) {
            let values: Vec<bool> = (0..n).map(|i| (bits & (1 << i)) != 0).collect();
            let states: Vec<dive_trace::State> = values
                .iter()
                .map(|b| Arc::from([Some(dive_value::Value::Bool(*b))]))
                .collect();
            for back in 0..n {
                let mixed = values[back..].iter().any(|b| *b) && values[back..].iter().any(|b| !*b);
                for (name, expected) in [
                    ("P", mixed),
                    ("Q", !mixed),
                    ("E", false),
                    ("A", true),
                    ("T", true),
                ] {
                    let d = model.ir.root_def(name).expect("predicate");
                    let pred =
                        dive_model::Pred::new(model.ir.def(d).body, model.evaluator.scope(d));
                    assert_eq!(
                        dive_trace::temporal::evaluate_lasso(&model, &pred, &states, back)
                            .expect(name),
                        vec![expected; n],
                        "{name} {values:?} back {back}"
                    );
                    let plan = dive_liveness::plan::lower(&model, &pred, Limits::default().formula)
                        .expect(name);
                    let rows: Vec<Vec<bool>> = (0..n)
                        .map(|i| {
                            let next = if i + 1 == n { back } else { i + 1 };
                            let st = dive_eval::St::action(states[i].clone(), states[next].clone());
                            plan.atoms
                                .iter()
                                .map(|a| a.evaluate(&model, &st).expect("atom"))
                                .collect()
                        })
                        .collect();
                    assert_eq!(
                        plan.formula
                            .evaluate_word(&rows, &rows, back, 1_000_000)
                            .expect("formula"),
                        vec![expected; n]
                    );
                }
            }
        }
    }
}

#[test]
fn quantifier_limits_and_hidden_constant_effects_are_not_success() {
    let src = "---- MODULE M ----\nEXTENDS Naturals, TLC\nCONSTANT S\nVARIABLE x\nInit == x = 0\nNext == x' = x\nNumbers == 0..1000000000000\nHidden == TLCGet(\"level\")\nP == \\A i \\in S: []<>(x=i)\n====\n";
    let (ir, cfg) = input(
        src,
        "INIT Init\nNEXT Next\nCONSTANT S <- Numbers\nPROPERTY P\n",
    );
    let e = temporal::check_final(&ir, &cfg, &CheckOptions::dive(0), Limits::default())
        .expect_err("domain bound before enumeration");
    assert_eq!(e.code(), "DIVE-L0021", "{e}");
    let (ir, cfg) = input(
        src,
        "INIT Init\nNEXT Next\nCONSTANT S <- Hidden\nPROPERTY P\n",
    );
    let e = temporal::check_final(&ir, &cfg, &CheckOptions::dive(0), Limits::default())
        .expect_err("hidden checker introspection");
    assert!(
        e.to_string().contains("TLCGet") || matches!(e, Error::Binding(_)),
        "{e}"
    );
    let src = "---- MODULE M ----\nEXTENDS TLC\nCONSTANT C\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\nHidden == PrintT(\"hidden effect\")\nP == []<>(x = C)\n====\n";
    let (ir, cfg) = input(
        src,
        "INIT Init\nNEXT Next\nCONSTANT C <- Hidden\nPROPERTY P\n",
    );
    assert!(matches!(
        temporal::check_final(&ir, &cfg, &CheckOptions::dive(0), Limits::default()),
        Err(Error::Analysis(_))
    ));
}

#[test]
fn conditional_word_truth_uses_current_state_or_temporal_guard() {
    let source = "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\n\
    P == IF x THEN []x ELSE <>x\nQ == IF ~x THEN []<>x ELSE <>[]~x\n\
    R == IF ~~x THEN []x ELSE <>x\n\
    T == IF []<>x THEN <>[]x ELSE <>[]~x\n\
    O == IF x THEN IF ~x THEN FALSE ELSE []x ELSE <>x\n====\n";
    let (ir, cfg) = input(source, "INIT Init\nNEXT Next\n");
    let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
    for n in 1..=4 {
        for bits in 0..(1_usize << n) {
            let values: Vec<bool> = (0..n).map(|i| bits & (1 << i) != 0).collect();
            let states: Vec<dive_trace::State> = values
                .iter()
                .map(|b| Arc::from([Some(dive_value::Value::Bool(*b))]))
                .collect();
            for back in 0..n {
                let gf = values[back..].iter().any(|b| *b);
                let fg = values[back..].iter().all(|b| *b);
                for name in ["P", "Q", "R", "T", "O"] {
                    let expected: Vec<bool> = (0..n)
                        .map(|i| {
                            let mut reachable = Vec::new();
                            let mut at = i;
                            while !reachable.contains(&at) {
                                reachable.push(at);
                                at = if at + 1 == n { back } else { at + 1 };
                            }
                            let always = reachable.iter().all(|j| values[*j]);
                            let eventual = reachable.iter().any(|j| values[*j]);
                            match name {
                                "P" | "R" | "O" => {
                                    if values[i] {
                                        always
                                    } else {
                                        eventual
                                    }
                                }
                                "Q" => {
                                    if values[i] {
                                        !gf
                                    } else {
                                        gf
                                    }
                                }
                                "T" => {
                                    if gf {
                                        fg
                                    } else {
                                        !gf
                                    }
                                }
                                _ => panic!("expected formula"),
                            }
                        })
                        .collect();
                    let d = model.ir.root_def(name).expect("definition");
                    let pred =
                        dive_model::Pred::new(model.ir.def(d).body, model.evaluator.scope(d));
                    assert_eq!(
                        dive_trace::temporal::evaluate_lasso(&model, &pred, &states, back)
                            .expect(name),
                        expected,
                        "{name} {values:?} back {back}"
                    );
                    let plan = dive_liveness::plan::lower(&model, &pred, Limits::default().formula)
                        .expect(name);
                    let rows: Vec<Vec<bool>> = (0..n)
                        .map(|i| {
                            let next = if i + 1 == n { back } else { i + 1 };
                            let st = dive_eval::St::action(states[i].clone(), states[next].clone());
                            plan.atoms
                                .iter()
                                .map(|a| a.evaluate(&model, &st).expect("atom"))
                                .collect()
                        })
                        .collect();
                    assert_eq!(
                        plan.formula
                            .evaluate_word(&rows, &rows, back, 1_000_000)
                            .expect("formula"),
                        expected,
                        "plan {name} {values:?} back {back}"
                    );
                }
            }
        }
    }
}

#[test]
fn pinned_temporal_conditionals_match_oracle_counts_in_both_profiles_and_stores() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/temporal-conditionals");
    for name in ["CdState", "CdFair", "CdTemporalGuard"] {
        let source =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("source");
        let config =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("config");
        let stdout =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.stdout")))
                .expect("stdout");
        let exit: i32 =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.exit")))
                .expect("exit")
                .trim()
                .parse()
                .expect("code");
        let (ir, cfg) = input(&source, &config);
        for mut opts in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                opts.store = store;
                let report =
                    temporal::check_final(&ir, &cfg, &opts, Limits::default()).expect(name);
                let complete = match report.outcome() {
                    Outcome::Holds(c) => {
                        assert_eq!(exit, 0, "{name}");
                        c.as_ref()
                    }
                    Outcome::Violation(v) => {
                        assert_eq!(exit, 13, "{name}");
                        assert_eq!(v.validation().validation, Validation::Valid);
                        assert_eq!(v.document().properties, ["P"]);
                        &v.candidate().exploration
                    }
                    Outcome::Safety(r) => panic!("{name}: {:?}", r.verdict),
                };
                assert_eq!(
                    (
                        complete.stats.generated,
                        complete.stats.distinct,
                        complete.stats.depth
                    ),
                    counts(&stdout),
                    "{name}"
                );
            }
        }
    }
}

#[test]
fn incomplete_cases_and_effectful_conditional_branches_are_refused() {
    let source = "---- MODULE M ----\nEXTENDS TLC\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == CASE x -> []<>x\nQ == IF x THEN []<>x ELSE []PrintT(\"hidden\")\n====\n";
    for name in ["P", "Q"] {
        let (ir, cfg) = input(source, &format!("INIT Init\nNEXT Next\nPROPERTY {name}\n"));
        let error = temporal::check_final(&ir, &cfg, &CheckOptions::dive(0), Limits::default())
            .expect_err("undefined/effectful conditional cannot become a verdict");
        assert!(matches!(error, Error::Analysis(_)), "{error}");
    }
}

#[test]
fn oracle_rejected_temporal_conditionals_remain_named_refusals() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/temporal-conditionals");
    for (name, deciding_code) in [("CdAction", 2214), ("CdCaseOverlap", 2213)] {
        let source =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("source");
        let config =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("config");
        let stdout =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.stdout")))
                .expect("stdout");
        assert!(
            stdout.contains(&format!("STARTMSG {deciding_code}:1")),
            "{name}: {stdout}"
        );
        let (ir, cfg) = input(&source, &config);
        for opts in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            let error = temporal::check_final(&ir, &cfg, &opts, Limits::default())
                .expect_err("no verdict for oracle-rejected formula");
            assert!(matches!(error, Error::Analysis(_)), "{name}: {error}");
            assert!(
                error.to_string().contains(if name == "CdAction" {
                    "action-level conditional"
                } else {
                    "Case"
                }),
                "{error}"
            );
        }
    }
}

#[test]
fn constant_temporal_calls_keep_scopes_and_match_direct_word_semantics() {
    let source = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = 1-x\nHit(i) == x=i\nVisit(i) == []<>Hit(i)\nStable(i) == <>[]Hit(i)\nInc(i) == i+1\nWrap(i) == Visit(Inc(i))\nPair(a,b) == []<>(Hit(a) /\\ ~Hit(b))\nP == Pair(0,1) /\\ Pair(1,0)\nQ == \\E i \\in {0,1}: Stable(i)\nR == Wrap(0)\n====\n";
    let (ir, cfg) = input(source, "INIT Init\nNEXT Next\n");
    let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
    for n in 1..=4 {
        for bits in 0..(1_usize << n) {
            let values: Vec<bool> = (0..n).map(|i| bits & (1 << i) != 0).collect();
            let states: Vec<dive_trace::State> = values
                .iter()
                .map(|b| Arc::from([Some(dive_value::Value::from(i64::from(*b)))]))
                .collect();
            for back in 0..n {
                let mixed = values[back..].iter().any(|b| *b) && values[back..].iter().any(|b| !*b);
                let one = values[back..].iter().any(|b| *b);
                for (name, expected) in [("P", mixed), ("Q", !mixed), ("R", one)] {
                    let d = model.ir.root_def(name).expect("definition");
                    let pred =
                        dive_model::Pred::new(model.ir.def(d).body, model.evaluator.scope(d));
                    let direct = dive_trace::temporal::evaluate_lasso(&model, &pred, &states, back)
                        .expect(name);
                    assert_eq!(direct, vec![expected; n], "{name} {values:?} back {back}");
                    let plan = dive_liveness::plan::lower(&model, &pred, Limits::default().formula)
                        .expect(name);
                    let rows: Vec<Vec<bool>> = (0..n)
                        .map(|i| {
                            let next = if i + 1 == n { back } else { i + 1 };
                            let st = dive_eval::St::action(states[i].clone(), states[next].clone());
                            plan.atoms
                                .iter()
                                .map(|a| a.evaluate(&model, &st).expect("atom"))
                                .collect()
                        })
                        .collect();
                    assert_eq!(
                        plan.formula
                            .evaluate_word(&rows, &rows, back, 1_000_000)
                            .expect("formula"),
                        direct,
                        "{name}"
                    );
                }
            }
        }
    }
}

#[test]
fn effectful_arguments_refuse_while_pure_state_arguments_keep_their_truth() {
    let source = "---- MODULE M ----\nEXTENDS TLC, Naturals\nVARIABLE x\nInit == x=0\nNext == x'=1-x\nVisit(i) == []<>(x=i)\nHidden(i) == TLCGet(\"level\")+i\nWrap(i) == Visit(i)\nP == Wrap(Hidden(0)) /\\ Wrap(0)\nQ == Visit(x)\nR == Visit(PrintT(\"hidden\"))\n====\n";
    for name in ["P", "R"] {
        let (ir, cfg) = input(source, &format!("INIT Init\nNEXT Next\nPROPERTY {name}\n"));
        let model =
            dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc()).expect("model");
        let d = model.ir.root_def(name).expect("definition");
        let pred = dive_model::Pred::new(model.ir.def(d).body, model.evaluator.scope(d));
        assert!(
            dive_liveness::plan::lower(&model, &pred, Limits::default().formula).is_err(),
            "{name}"
        );
        let states: Vec<dive_trace::State> =
            vec![Arc::from([Some(dive_value::Value::from(0_i64))])];
        assert!(
            matches!(
                dive_trace::temporal::evaluate_lasso(&model, &pred, &states, 0),
                Err(dive_trace::temporal::TemporalError::Unsupported(_))
            ),
            "{name}"
        );
        assert!(
            temporal::check_final(&ir, &cfg, &CheckOptions::dive(0), Limits::default()).is_err(),
            "{name}"
        );
    }
    let (ir, cfg) = input(source, "INIT Init\nNEXT Next\nPROPERTY Q\n");
    let report = temporal::check_final(&ir, &cfg, &CheckOptions::dive(0), Limits::default())
        .expect("x=x at every position");
    assert!(matches!(report.outcome(), Outcome::Holds(_)));
}

#[test]
fn pinned_constant_temporal_calls_match_truth_counts_and_validate_independently() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/temporal-calls");
    for name in ["PcVisit", "PcFair", "PcStable", "PcScopes", "PcTuple"] {
        let src =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("source");
        let config =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("config");
        let stdout =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.stdout")))
                .expect("oracle");
        let code: i32 =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.exit")))
                .expect("exit")
                .trim()
                .parse()
                .expect("number");
        let (ir, cfg) = input(&src, &config);
        for mut opts in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                opts.store = store;
                let report =
                    temporal::check_final(&ir, &cfg, &opts, Limits::default()).expect(name);
                let evidence = match report.outcome() {
                    Outcome::Holds(c) => {
                        assert_eq!(code, 0, "{name}");
                        c.as_ref()
                    }
                    Outcome::Violation(v) => {
                        assert_eq!(code, 13, "{name}");
                        assert_eq!(v.validation().validation, Validation::Valid);
                        assert_eq!(v.document().properties, ["P"]);
                        &v.candidate().exploration
                    }
                    Outcome::Safety(r) => panic!("{name}: {:?}", r.verdict),
                };
                assert_eq!(
                    (
                        evidence.stats.generated,
                        evidence.stats.distinct,
                        evidence.stats.depth
                    ),
                    counts(&stdout),
                    "{name}"
                );
            }
        }
    }
}

#[test]
fn temporal_call_purity_walk_consumes_the_planning_budget() {
    let src = "---- MODULE M ----\nVARIABLE x\nInit == x=TRUE\nNext == x'=x\nG(i) == x=i\nF(i) == []<>G(i)\nP == F(TRUE)\n====\n";
    let (ir, cfg) = input(src, "INIT Init\nNEXT Next\nPROPERTY P\n");
    let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
    let d = model.ir.root_def("P").expect("predicate");
    let pred = dive_model::Pred::new(model.ir.def(d).body, model.evaluator.scope(d));
    let mut limits = Limits::default().formula;
    limits.expansion_steps = 3;
    let err = dive_liveness::plan::lower(&model, &pred, limits)
        .expect_err("small budget cannot report success");
    assert_eq!(err.code(), "DIVE-L0021");
}

#[test]
fn state_arguments_are_position_dependent_and_priming_uses_the_closing_edge() {
    let src = "---- MODULE M ----\nVARIABLE x\nInit == x=FALSE\nNext == x'=~x\nLive(p) == []<>p\nStable(p) == <>[]p\nBoth(p) == Live(p) /\\ Live(~p)\nSelect(p) == IF p THEN []p ELSE <>p\nChange(p) == []<><<p' # p>>_p\nP == Live(x)\nQ == Stable(x)\nR == Both(x)\nC == Change(x)\nI == Select(x)\n====\n";
    let (ir, cfg) = input(src, "INIT Init\nNEXT Next\n");
    let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
    for n in 1..=4 {
        for bits in 0..(1_usize << n) {
            let values: Vec<bool> = (0..n).map(|i| bits & (1 << i) != 0).collect();
            let states: Vec<dive_trace::State> = values
                .iter()
                .map(|b| Arc::from([Some(dive_value::Value::Bool(*b))]))
                .collect();
            for back in 0..n {
                let any = values[back..].iter().any(|b| *b);
                let all = values[back..].iter().all(|b| *b);
                for name in ["P", "Q", "R", "C", "I"] {
                    let expected: Vec<bool> = (0..n)
                        .map(|i| match name {
                            "P" => any,
                            "Q" => all,
                            "R" | "C" => any && !all,
                            "I" => {
                                let mut reach = Vec::new();
                                let mut at = i;
                                while !reach.contains(&at) {
                                    reach.push(at);
                                    at = if at + 1 == n { back } else { at + 1 };
                                }
                                if values[i] {
                                    reach.iter().all(|j| values[*j])
                                } else {
                                    reach.iter().any(|j| values[*j])
                                }
                            }
                            _ => panic!("formula"),
                        })
                        .collect();
                    let d = model.ir.root_def(name).expect("definition");
                    let pred =
                        dive_model::Pred::new(model.ir.def(d).body, model.evaluator.scope(d));
                    let direct = dive_trace::temporal::evaluate_lasso(&model, &pred, &states, back)
                        .expect(name);
                    assert_eq!(direct, expected, "{name} {values:?} back {back}");
                    let plan = dive_liveness::plan::lower(&model, &pred, Limits::default().formula)
                        .expect(name);
                    let rows: Vec<Vec<bool>> = (0..n)
                        .map(|i| {
                            let next = if i + 1 == n { back } else { i + 1 };
                            let st = dive_eval::St::action(states[i].clone(), states[next].clone());
                            plan.atoms
                                .iter()
                                .map(|a| a.evaluate(&model, &st).expect("atom"))
                                .collect()
                        })
                        .collect();
                    assert_eq!(
                        plan.formula
                            .evaluate_word(&rows, &rows, back, 1_000_000)
                            .expect("formula"),
                        direct,
                        "{name}"
                    );
                }
            }
        }
    }
}

#[test]
fn pinned_state_argument_operators_match_truth_counts_and_validate_independently() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/state-temporal-calls");
    for name in [
        "SpHold",
        "SpStable",
        "SpGuard",
        "SpNested",
        "SpTuple",
        "SpNonstutter",
        "SpForwardFair",
    ] {
        let src =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("source");
        let config =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("config");
        let stdout =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.stdout")))
                .expect("oracle");
        let code: i32 =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.exit")))
                .expect("exit")
                .trim()
                .parse()
                .expect("number");
        let (ir, cfg) = input(&src, &config);
        for mut opts in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                opts.store = store;
                let report =
                    temporal::check_final(&ir, &cfg, &opts, Limits::default()).expect(name);
                let evidence = match report.outcome() {
                    Outcome::Holds(c) => {
                        assert_eq!(code, 0, "{name}");
                        c.as_ref()
                    }
                    Outcome::Violation(v) => {
                        assert_eq!(code, 13, "{name}");
                        assert_eq!(v.validation().validation, Validation::Valid);
                        assert_eq!(v.document().properties, ["P"]);
                        &v.candidate().exploration
                    }
                    Outcome::Safety(r) => panic!("{name}: {:?}", r.verdict),
                };
                assert_eq!(
                    (
                        evidence.stats.generated,
                        evidence.stats.distinct,
                        evidence.stats.depth
                    ),
                    counts(&stdout),
                    "{name}"
                );
            }
        }
    }
    // SANY requires eventual actions to use <<A>>_v. Keep that negative
    // syntax/level control separate from supported, scoped non-stuttering.
    let src = std::fs::read_to_string(root.join("specs/SpPrime/SpPrime.tla")).expect("source");
    let a = dive_sema::analyze_source("SpPrime.tla", &src, &dive_sema::Options::default());
    assert!(
        !a.accepted(),
        "bare eventual action is rejected by SANY too"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("tlc-2026.10.06.014338/SpPrime.sany.exit"))
            .expect("exit")
            .trim(),
        "4"
    );
}

#[test]
fn scoped_fairness_and_enabledness_match_pinned_truth_and_independent_validation() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scoped-fairness");
    for name in ["SfDisabled", "SfEnabled", "SfReach"] {
        let src =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("source");
        let config =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("config");
        let stdout =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.stdout")))
                .expect("oracle");
        let code: i32 =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.exit")))
                .expect("exit")
                .trim()
                .parse()
                .expect("number");
        let (ir, cfg) = input(&src, &config);
        for mut opts in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                opts.store = store;
                let report =
                    temporal::check_final(&ir, &cfg, &opts, Limits::default()).expect(name);
                let evidence = match report.outcome() {
                    Outcome::Holds(c) => {
                        assert_eq!(code, 0, "{name}");
                        c.as_ref()
                    }
                    Outcome::Violation(v) => {
                        assert_eq!(code, 13, "{name}");
                        assert_eq!(v.validation().validation, Validation::Valid);
                        assert_eq!(v.document().properties, ["P"]);
                        // Only state 1 repeats: Go(1) never occurs in this cycle.
                        // WF must be proved from disabledness, not recurrence.
                        assert!(
                            v.candidate().states[v.candidate().back_to..]
                                .iter()
                                .all(|s| s[0] == Some(dive_value::Value::from(1_i64)))
                        );
                        &v.candidate().exploration
                    }
                    Outcome::Safety(r) => panic!("{name}: {:?}", r.verdict),
                };
                assert_eq!(
                    (
                        evidence.stats.generated,
                        evidence.stats.distinct,
                        evidence.stats.depth
                    ),
                    counts(&stdout),
                    "{name}"
                );
            }
        }
    }
}

#[test]
fn quantified_actions_and_predicates_match_pinned_final_truth_and_validate() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/action-quantifiers");
    for name in [
        "QfReach",
        "QfDisabled",
        "QfStrong",
        "QfAllState",
        "QfTuple",
        "QfNested",
    ] {
        let src =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("source");
        let config =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("config");
        let stdout =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.stdout")))
                .expect("oracle");
        let exit = std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.exit")))
            .expect("exit");
        let (ir, cfg) = input(&src, &config);
        for base in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                let opts = CheckOptions {
                    store,
                    ..base.clone()
                };
                let report =
                    temporal::check_final(&ir, &cfg, &opts, Limits::default()).expect(name);
                let c = match report.outcome() {
                    Outcome::Holds(c) => {
                        assert_eq!(exit.trim(), "0", "{name}");
                        c.as_ref()
                    }
                    Outcome::Violation(v) => {
                        assert_eq!(exit.trim(), "13", "{name}");
                        assert_eq!(v.validation().validation, Validation::Valid, "{name}");
                        assert_eq!(v.document().properties, ["P"]);
                        &v.candidate().exploration
                    }
                    Outcome::Safety(r) => panic!("{name}: {:?}", r.verdict),
                };
                assert_eq!(
                    (c.stats.generated, c.stats.distinct, c.stats.depth),
                    counts(&stdout),
                    "{name}"
                );
            }
        }
    }
}

#[test]
fn finite_predicate_quantifiers_keep_every_context_and_empty_identity() {
    let (ir, cfg) = input(
        "---- MODULE QWords ----\nVARIABLE x\nInit == x=TRUE\nNext == x'=~x\n\
        Visit(i) == x=i\n\
        Any == []<>(\\E i \\in {TRUE,FALSE} : Visit(i))\n\
        All == []<>(\\A i \\in {TRUE,FALSE} : Visit(i))\n\
        Pair == []<>(\\E <<i,j>> \\in {<<TRUE,FALSE>>,<<FALSE,TRUE>>} : x=i /\\ x=j)\n\
        EmptyAny == []<>(\\E i \\in {} : Visit(i))\n\
        EmptyAll == []<>(\\A i \\in {} : Visit(i))\n\
        StateDomain == []<>(\\E i \\in {x} : Visit(i))\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    let m = dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc()).expect("bind");
    for len in 1..=4 {
        for mask in 0_usize..(1 << len) {
            let word: Vec<bool> = (0..len).map(|i| mask & (1 << i) != 0).collect();
            let states: Vec<dive_trace::State> = word
                .iter()
                .map(|b| Arc::from([Some((*b).into())]))
                .collect();
            for back in 0..len {
                for (name, expected) in [
                    ("Any", true),
                    ("All", false),
                    ("EmptyAny", false),
                    ("EmptyAll", true),
                    ("Pair", false),
                ] {
                    let d = ir.root_def(name).expect(name);
                    let p = dive_model::Pred::new(ir.def(d).body, m.evaluator.scope(d));
                    let plan = dive_liveness::plan::lower(
                        &m,
                        &p,
                        dive_liveness::formula::Limits::default(),
                    )
                    .expect(name);
                    let labels: Vec<Vec<bool>> = (0..len)
                        .map(|i| {
                            let next = if i + 1 == len { back } else { i + 1 };
                            let st = dive_eval::St::action(states[i].clone(), states[next].clone());
                            plan.atoms
                                .iter()
                                .map(|atom| atom.evaluate(&m, &st).expect("atom"))
                                .collect()
                        })
                        .collect();
                    let value = plan
                        .formula
                        .evaluate_word(&labels, &labels, back, 1_000_000)
                        .expect("formula");
                    assert_eq!(value[0], expected, "{name} {word:?} {back}");
                    let direct = dive_trace::temporal::evaluate_lasso(&m, &p, &states, back)
                        .expect("independent word");
                    assert_eq!(direct[0], expected, "{name} {word:?} {back}");
                }
            }
        }
    }
    let d = ir.root_def("StateDomain").expect("StateDomain");
    let p = dive_model::Pred::new(ir.def(d).body, m.evaluator.scope(d));
    assert!(dive_liveness::plan::lower(&m, &p, dive_liveness::formula::Limits::default()).is_err());
    assert!(
        dive_trace::temporal::evaluate_lasso(&m, &p, &[Arc::from([Some(true.into())])], 0).is_err()
    );
}
#[test]
fn let_scopes_match_pinned_final_truth_and_validate_independently() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/temporal-let");
    for name in [
        "LtState",
        "LtNested",
        "LtGuard",
        "LtAction",
        "LtCaptured",
        "LtDisabled",
    ] {
        let src =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("source");
        let config =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("config");
        let stdout =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.stdout")))
                .expect("oracle");
        let exit = std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.exit")))
            .expect("exit");
        let (ir, cfg) = input(&src, &config);
        for base in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                let opts = CheckOptions {
                    store,
                    ..base.clone()
                };
                let report =
                    temporal::check_final(&ir, &cfg, &opts, Limits::default()).expect(name);
                let c = match report.outcome() {
                    Outcome::Holds(c) => {
                        assert_eq!(exit.trim(), "0", "{name}");
                        c.as_ref()
                    }
                    Outcome::Violation(v) => {
                        assert_eq!(exit.trim(), "13", "{name}");
                        assert_eq!(v.validation().validation, Validation::Valid, "{name}");
                        assert_eq!(v.document().properties, ["P"]);
                        &v.candidate().exploration
                    }
                    Outcome::Safety(r) => panic!("{name}: {:?}", r.verdict),
                };
                assert_eq!(
                    (c.stats.generated, c.stats.distinct, c.stats.depth),
                    counts(&stdout),
                    "{name}"
                );
            }
        }
    }
}

#[test]
fn let_word_contexts_match_cycle_algebra_without_freezing_state() {
    let (ir, cfg) = input(
        "---- MODULE LetWords ----\nVARIABLE x\nInit == x=TRUE\nNext == x'=~x\n\
        P == LET Seen==x IN []<>Seen\n\
        Q == LET Seen==x IN <>[]Seen\n\
        R == LET Visit(q)==[]<>q IN Visit(x) /\\ Visit(~x)\n\
        C == LET Seen==x IN []<><<Seen' #Seen>>_Seen\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    let model = dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc()).expect("bind");
    for n in 1..=4 {
        for mask in 0_usize..(1 << n) {
            let values: Vec<bool> = (0..n).map(|i| mask & (1 << i) != 0).collect();
            let states: Vec<dive_trace::State> = values
                .iter()
                .map(|v| Arc::from([Some((*v).into())]))
                .collect();
            for back in 0..n {
                let any = values[back..].iter().any(|v| *v);
                let all = values[back..].iter().all(|v| *v);
                for (name, truth) in [
                    ("P", any),
                    ("Q", all),
                    ("R", any && !all),
                    ("C", any && !all),
                ] {
                    let d = ir.root_def(name).expect(name);
                    let p = dive_model::Pred::new(ir.def(d).body, model.evaluator.scope(d));
                    let direct = dive_trace::temporal::evaluate_lasso(&model, &p, &states, back)
                        .expect(name);
                    assert_eq!(direct, vec![truth; n], "{name} {values:?} {back}");
                    let plan = dive_liveness::plan::lower(&model, &p, Limits::default().formula)
                        .expect(name);
                    let rows: Vec<Vec<bool>> = (0..n)
                        .map(|i| {
                            let next = if i + 1 == n { back } else { i + 1 };
                            let st = dive_eval::St::action(states[i].clone(), states[next].clone());
                            plan.atoms
                                .iter()
                                .map(|a| a.evaluate(&model, &st).expect("atom"))
                                .collect()
                        })
                        .collect();
                    assert_eq!(
                        plan.formula
                            .evaluate_word(&rows, &rows, back, 1_000_000)
                            .expect("formula"),
                        direct,
                        "{name}"
                    );
                }
            }
        }
    }
}

#[test]
fn used_let_effects_and_checker_reads_refuse_without_evaluation() {
    let (ir, cfg) = input(
        "---- MODULE LetEffects ----\nEXTENDS TLC\nVARIABLE x\nInit == x=TRUE\nNext == x'=x\n\
        E == LET Echo==Print(x,x) IN []Echo\n\
        R == LET Register==TLCGet(\"level\") IN [](Register=0)\n====\n",
        "INIT Init\nNEXT Next\n",
    );
    let model = dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc()).expect("bind");
    let _ = model.take_output();
    for name in ["E", "R"] {
        let d = ir.root_def(name).expect(name);
        let p = dive_model::Pred::new(ir.def(d).body, model.evaluator.scope(d));
        assert!(dive_liveness::plan::lower(&model, &p, Limits::default().formula).is_err());
        assert!(
            dive_trace::temporal::evaluate_lasso(&model, &p, &[Arc::from([Some(true.into())])], 0)
                .is_err()
        );
        assert!(
            model.take_output().is_empty(),
            "{name} executed a used effect during preflight"
        );
    }
}

#[test]
fn instance_substitutions_match_pinned_final_truth_and_validate_independently() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/temporal-substitutions");
    for name in [
        "IsReach",
        "IsStable",
        "IsAction",
        "IsQuant",
        "IsNested",
        "IsComplement",
    ] {
        let src =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("source");
        let config =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("config");
        let stdout =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.stdout")))
                .expect("oracle");
        let exit = std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.exit")))
            .expect("exit");
        let (ir, cfg) = input(&src, &config);
        for base in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                let opts = CheckOptions {
                    store,
                    ..base.clone()
                };
                let report =
                    temporal::check_final(&ir, &cfg, &opts, Limits::default()).expect(name);
                let c = match report.outcome() {
                    Outcome::Holds(c) => {
                        assert_eq!(exit.trim(), "0", "{name}");
                        c.as_ref()
                    }
                    Outcome::Violation(v) => {
                        assert!(matches!(exit.trim(), "12" | "13"), "{name}: {exit}");
                        assert_eq!(v.validation().validation, Validation::Valid, "{name}");
                        assert_eq!(v.document().properties, ["P"]);
                        &v.candidate().exploration
                    }
                    Outcome::Safety(r) => panic!("{name}: {:?}", r.verdict),
                };
                assert_eq!(
                    (c.stats.generated, c.stats.distinct, c.stats.depth),
                    counts(&stdout),
                    "{name}"
                );
            }
        }
    }
}

#[test]
fn instance_state_expression_cannot_replace_a_constant_declaration() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/temporal-substitutions");
    let src = std::fs::read_to_string(root.join("specs/IsStateTarget/IsStateTarget.tla"))
        .expect("source");
    let exit = std::fs::read_to_string(root.join("tlc-2026.10.06.014338/IsStateTarget.sany.exit"))
        .expect("oracle");
    assert_eq!(exit.trim(), "4");
    let analysis =
        dive_sema::analyze_source("IsStateTarget.tla", &src, &dive_sema::Options::default());
    assert!(
        !analysis.accepted(),
        "state substitution for a constant was accepted"
    );
}

#[test]
fn substituted_words_keep_nested_state_and_priming_lazy() {
    let (ir, cfg) = input(
        r"---- MODULE InstanceWords ----
---- MODULE Middle ----
---- MODULE Leaf ----
CONSTANT C
VARIABLE v
P == []<>(v=C)
Q == <>[](v=C)
R == []<>(v=C) /\ []<>(v#C)
A == []<><<v'#v>>_v
====
CONSTANT MC
VARIABLE w
J == INSTANCE Leaf WITH C <- MC, v <- ~w
P == J!P
Q == J!Q
R == J!R
A == J!A
====
VARIABLE x
I == INSTANCE Middle WITH MC <- TRUE, w <- x
Init == x=TRUE
Next == x'=~x
P == I!P
Q == I!Q
R == I!R
A == I!A
====
",
        "INIT Init\nNEXT Next\n",
    );
    let model = dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc()).expect("bind");
    for n in 1..=4 {
        for mask in 0_usize..(1 << n) {
            let values: Vec<bool> = (0..n).map(|i| mask & (1 << i) != 0).collect();
            let states: Vec<dive_trace::State> = values
                .iter()
                .map(|v| Arc::from([Some((*v).into())]))
                .collect();
            for back in 0..n {
                let any = values[back..].iter().any(|v| *v);
                let all = values[back..].iter().all(|v| *v);
                for (name, truth) in [
                    ("P", !all),
                    ("Q", !any),
                    ("R", any && !all),
                    ("A", any && !all),
                ] {
                    let d = ir.root_def(name).expect(name);
                    let pred = dive_model::Pred::new(ir.def(d).body, model.evaluator.scope(d));
                    let direct = dive_trace::temporal::evaluate_lasso(&model, &pred, &states, back)
                        .expect(name);
                    assert_eq!(direct, vec![truth; n], "{name} {values:?} {back}");
                    let plan = dive_liveness::plan::lower(&model, &pred, Limits::default().formula)
                        .expect(name);
                    let rows: Vec<Vec<bool>> = (0..n)
                        .map(|i| {
                            let next = if i + 1 == n { back } else { i + 1 };
                            let st = dive_eval::St::action(states[i].clone(), states[next].clone());
                            plan.atoms
                                .iter()
                                .map(|a| a.evaluate(&model, &st).expect("atom"))
                                .collect()
                        })
                        .collect();
                    assert_eq!(
                        plan.formula
                            .evaluate_word(&rows, &rows, back, 1_000_000)
                            .expect("formula"),
                        direct,
                        "{name}"
                    );
                }
            }
        }
    }
}

#[test]
fn effectful_substitutions_refuse_before_evaluation() {
    let (ir, cfg) = input(
        r#"---- MODULE InstanceEffects ----
EXTENDS TLC
---- MODULE Inner ----
VARIABLE v
Claim == []<>(v=TRUE)
====
VARIABLE x
E == INSTANCE Inner WITH v <- Print(x,x)
R == INSTANCE Inner WITH v <- (TLCGet("level")=0)
Init == x=TRUE
Next == x'=x
EP == E!Claim
RP == R!Claim
====
"#,
        "INIT Init\nNEXT Next\n",
    );
    let model = dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc()).expect("bind");
    let _ = model.take_output();
    for name in ["EP", "RP"] {
        let d = ir.root_def(name).expect(name);
        let p = dive_model::Pred::new(ir.def(d).body, model.evaluator.scope(d));
        assert!(dive_liveness::plan::lower(&model, &p, Limits::default().formula).is_err());
        assert!(
            dive_trace::temporal::evaluate_lasso(&model, &p, &[Arc::from([Some(true.into())])], 0)
                .is_err()
        );
        assert!(
            model.take_output().is_empty(),
            "effect executed during preflight"
        );
    }
}

#[test]
fn unsupported_substitution_bindings_and_disabledness_never_claim_truth() {
    let (ir, cfg) = input(
        r"---- MODULE InstanceLimits ----
---- MODULE Inner ----
CONSTANT F(_)
VARIABLE v
OpClaim == []<>F(v)
Disabled == []~ENABLED (v'=~v)
ValueClaim == []<>v
====
VARIABLE x
Identity(p) == p
I == INSTANCE Inner WITH F <- Identity, v <- x
Init == x=TRUE
Next == x'=~x
OP == I!OpClaim
EN == I!Disabled
P == I!ValueClaim
====
",
        "INIT Init\nNEXT Next\n",
    );
    let model = dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc()).expect("bind");
    let states = [Arc::from([Some(true.into())])];
    let op = ir.root_def("OP").expect("OP");
    let op = dive_model::Pred::new(ir.def(op).body, model.evaluator.scope(op));
    assert!(dive_liveness::plan::lower(&model, &op, Limits::default().formula).is_err());
    assert!(dive_trace::temporal::evaluate_lasso(&model, &op, &states, 0).is_err());
    let en = ir.root_def("EN").expect("EN");
    let en = dive_model::Pred::new(ir.def(en).body, model.evaluator.scope(en));
    assert!(
        dive_trace::temporal::evaluate_lasso(&model, &en, &states, 0).is_err(),
        "substituted disabledness needs a separate finite cover proof"
    );
    let p = ir.root_def("P").expect("P");
    let p = dive_model::Pred::new(ir.def(p).body, model.evaluator.scope(p));
    let mut limit = Limits::default().formula;
    limit.expansion_steps = 3;
    assert_eq!(
        dive_liveness::plan::lower(&model, &p, limit)
            .expect_err("budget")
            .code(),
        "DIVE-L0021"
    );
}
