//! Bounded final temporal checking on Core: verdicts, pinned counts and
//! independently validated lossless reports. These cases ran on the retired
//! reference final route until #791; exact and spilled seen storage and
//! `--chaos` with temporal properties are named Core refusals, asserted in
//! `temporal_budget.rs` and the other temporal tests.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

#[path = "support/core_final.rs"]
mod core_final;
#[path = "support/refused.rs"]
mod refused;

use core_final::Outcome;
use dive_liveness::analyze::Limits;
use dive_runtime::{CheckOptions, Verdict};
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
fn text(source: &str, cfg: &str) -> (String, String) {
    (source.to_owned(), cfg.to_owned())
}
/// Asserts that Core refuses `source` and `cfg` by naming `feature`.
fn refuses(source: &str, cfg: &str, opts: &CheckOptions, feature: &str) {
    let e = core_final::check(source, cfg, opts).expect_err(feature);
    refused::assert_refused(&e.json(), e.exit_code(), feature);
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
        let (src_text, cfg_text) = text(&source, &cfg);
        let (ir, cfg) = input(&source, &cfg);
        {
            let store = StoreMode::Fingerprint;
            let mut reference = None;
            for workers in [1, 4, 8] {
                let opts = CheckOptions {
                    store,
                    workers,
                    ..CheckOptions::tlc(0, 19)
                };
                let report = core_final::check(&src_text, &cfg_text, &opts).expect(name);
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
                        let document = v.document().to_json().to_string();
                        if let Some(previous) = &reference {
                            assert_eq!(&document, previous, "{name} {workers}");
                        } else {
                            reference = Some(document);
                        }
                        let model =
                            dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc())
                                .expect("fresh validation model");
                        let doc = dive_trace::TraceDoc::parse(&v.document().to_json().to_string())
                            .expect("lossless serialization");
                        assert_eq!(
                            dive_trace::validate_doc(&model, &doc, &cfg.model_values)
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
                assert_eq!((c.workers.requested, c.workers.used), (workers, workers));
                assert_eq!(c.options.seed, 19);
                assert_eq!(c.options.store, store);
            }
        }
    }
}

#[test]
fn default_profile_attributes_all_false_properties_and_repeat_runs_rebind() {
    let src = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = ~x\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nP == <>[]x\nQ == <>[](~x)\nR == []<>x\n====\n";
    let (ir, cfg) = text(src, "SPECIFICATION Spec\nPROPERTY P Q R\n");
    for _ in 0..2 {
        let r = core_final::check(&ir, &cfg, &CheckOptions::dive(17)).expect("run");
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
    let (ir, cfg) = text(src, "INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY P\n");
    let opts = CheckOptions::tlc(0, 0);
    let r = core_final::check(&ir, &cfg, &opts).expect("safety stop");
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
    let (ir, cfg) = text(
        &src.replace("VARIABLE x", "VARIABLE x\nASSUME FALSE"),
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    );
    let r = core_final::check(&ir, &cfg, &opts).expect("assumption stop");
    assert!(
        matches!(r.outcome(), Outcome::Safety(r) if matches!(r.verdict, Verdict::AssumptionFalse { .. }))
    );
}

#[test]
fn state_argument_holds_and_resource_limits_never_become_holds() {
    let src = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\nF(a) == []<>a\nP == F(x)\n====\n";
    let (ir, cfg) = text(src, "INIT Init\nNEXT Next\nPROPERTY P\n");
    let r = core_final::check(&ir, &cfg, &CheckOptions::dive(0)).expect("pure state argument");
    assert!(matches!(r.outcome(), Outcome::Holds(_)));
    // A state cap with temporal properties is a named Core refusal, never Holds.
    let files = core_final::Files::new(&ir, &cfg);
    let request = dive_api::CheckRequest {
        max_states: Some(1),
        ..files.request(&CheckOptions::dive(0))
    };
    let report = dive_api::check_file(&request).expect("report");
    refused::assert_refused(&report.to_json(), report.exit_code(), refused::TEMPORAL_CAP);
}

/// Shape of `specifications/YoYo`: the finite-binding preflight visits every
/// branch, but TLC evaluates a binder's domain only where evaluation reaches
/// it. An erroring domain behind a false guard is not an atom failure; the
/// same domain still fails where evaluation reaches it.
#[test]
fn unreached_binder_domain_errors_are_not_atom_failures() {
    let src = "---- MODULE M ----\nEXTENDS Integers\nVARIABLE x\nMin(S) == CHOOSE e \\in S : \\A y \\in S : e <= y\nBad == \\A v \\in {} \\ {Min({v \\in {x} : v > 9})} : FALSE\nInit == x = 0\nNext == /\\ x < 2\n        /\\ IF x > 5 THEN Bad ELSE TRUE\n        /\\ x' = x + 1\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nLive == <>(x = 2)\nReached == []<>(IF x > 1 THEN Bad ELSE TRUE)\n====\n";
    // Shape of `MultiCarElevator`: a two-argument function applied outside
    // its domain. TLC fails there; dive refuses because it does not
    // reproduce TLC's message, which prints the domain.
    let outside = src.replace(
        "Bad == \\A v \\in {} \\ {Min({v \\in {x} : v > 9})} : FALSE",
        "F[a \\in 0..9, b \\in {0, 1}] == {a, x}\nBad == \\A v \\in F[x, 9] : FALSE",
    );
    let cfg = "SPECIFICATION Spec\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n";
    for source in [src, outside.as_str()] {
        for opts in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            let (ir, cfg) = text(source, cfg);
            let r =
                core_final::check(&ir, &cfg, &opts).expect("an unreached domain is not evaluated");
            let Outcome::Holds(c) = r.outcome() else {
                panic!("Live holds: {:?}", r.outcome())
            };
            assert_eq!((c.stats.generated, c.stats.distinct), (3, 3));
        }
    }
    // Where evaluation reaches the domain, the error is a temporal failure,
    // whose report Core does not build: a named refusal, never a verdict.
    let reached = "SPECIFICATION Spec\nPROPERTY Reached\nCHECK_DEADLOCK FALSE\n";
    for source in [src, outside.as_str()] {
        for opts in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            refuses(source, reached, &opts, refused::TEMPORAL_FAILURE_REPORT);
        }
    }
}

#[test]
fn never_enabled_fairness_is_a_validated_violation() {
    let src = "---- MODULE M ----\nVARIABLE x\nInit == x = [n \\in {0} |-> 0]\nNext == x' \\in [{0} -> {0}] /\\ x' # x'\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nP == <>(x # x)\n====\n";
    let (ir, cfg) = text(
        src,
        "SPECIFICATION Spec\nPROPERTY P\nCHECK_DEADLOCK FALSE\n",
    );
    // `x' # x'` is never true, so Next is never enabled and WF holds on the
    // stuttering word, which violates P.
    let r = core_final::check(&ir, &cfg, &CheckOptions::dive(0)).expect("decided");
    assert!(
        matches!(r.outcome(), Outcome::Violation(v) if v.validation().validation == Validation::Valid),
        "{r:?}"
    );
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
        let (ir, cfg) = text(&src, &cfg);
        for opts in [CheckOptions::tlc(0, 17), CheckOptions::dive(17)] {
            {
                let store = StoreMode::Fingerprint;
                let opts = CheckOptions {
                    store,
                    ..opts.clone()
                };
                let report = core_final::check(&ir, &cfg, &opts).expect(name);
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
                    let pred = dive_model::Pred::new(
                        model.ir.def(d).body,
                        model.evaluator.scope(d).expect("valid scope"),
                    );
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
    // A domain too large to enumerate, checker introspection or a hidden
    // effect in a constant is a temporal validation failure: Core refuses it
    // by name. Under --compat=tlc the module's literal 1000000000000 is
    // already TLC's integer overflow, a model error.
    let src = "---- MODULE M ----\nEXTENDS Naturals, TLC\nCONSTANT S\nVARIABLE x\nInit == x = 0\nNext == x' = x\nNumbers == 0..1000000000000\nHidden == TLCGet(\"level\")\nP == \\A i \\in S: []<>(x=i)\n====\n";
    for constant in ["Numbers", "Hidden"] {
        let cfg = format!("INIT Init\nNEXT Next\nCONSTANT S <- {constant}\nPROPERTY P\n");
        refuses(
            src,
            &cfg,
            &CheckOptions::dive(0),
            refused::TEMPORAL_VALIDATION_FAILURE_REPORT,
        );
        let r = core_final::check(src, &cfg, &CheckOptions::tlc(0, 0)).expect("model error");
        let Outcome::Safety(r) = r.outcome() else {
            panic!("{constant}: {:?}", r.outcome())
        };
        assert!(
            matches!(r.verdict, Verdict::ModelError { .. }),
            "{:?}",
            r.verdict
        );
    }
    let src = "---- MODULE M ----\nEXTENDS TLC\nCONSTANT C\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\nHidden == PrintT(\"hidden effect\")\nP == []<>(x = C)\n====\n";
    refuses(
        src,
        "INIT Init\nNEXT Next\nCONSTANT C <- Hidden\nPROPERTY P\n",
        &CheckOptions::dive(0),
        refused::TEMPORAL_VALIDATION_FAILURE_REPORT,
    );
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
                    let pred = dive_model::Pred::new(
                        model.ir.def(d).body,
                        model.evaluator.scope(d).expect("valid scope"),
                    );
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
        let (ir, cfg) = text(&source, &config);
        for mut opts in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            {
                let store = StoreMode::Fingerprint;
                opts.store = store;
                let report = core_final::check(&ir, &cfg, &opts).expect(name);
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
        refuses(
            source,
            &format!("INIT Init\nNEXT Next\nPROPERTY {name}\n"),
            &CheckOptions::dive(0),
            refused::TEMPORAL_VALIDATION_FAILURE_REPORT,
        );
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
        for opts in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            refuses(
                &source,
                &config,
                &opts,
                refused::TEMPORAL_VALIDATION_FAILURE_REPORT,
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
                    let pred = dive_model::Pred::new(
                        model.ir.def(d).body,
                        model.evaluator.scope(d).expect("valid scope"),
                    );
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
        let (src_text, cfg_text) =
            text(source, &format!("INIT Init\nNEXT Next\nPROPERTY {name}\n"));
        let (ir, cfg) = input(source, &format!("INIT Init\nNEXT Next\nPROPERTY {name}\n"));
        let model =
            dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc()).expect("model");
        let d = model.ir.root_def(name).expect("definition");
        let pred = dive_model::Pred::new(
            model.ir.def(d).body,
            model.evaluator.scope(d).expect("valid scope"),
        );
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
        refuses(
            &src_text,
            &cfg_text,
            &CheckOptions::dive(0),
            refused::TEMPORAL_VALIDATION_FAILURE_REPORT,
        );
    }
    let (src_text, cfg_text) = text(source, "INIT Init\nNEXT Next\nPROPERTY Q\n");
    let report = core_final::check(&src_text, &cfg_text, &CheckOptions::dive(0))
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
        let (ir, cfg) = text(&src, &config);
        for mut opts in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            {
                let store = StoreMode::Fingerprint;
                opts.store = store;
                let report = core_final::check(&ir, &cfg, &opts).expect(name);
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
    let pred = dive_model::Pred::new(
        model.ir.def(d).body,
        model.evaluator.scope(d).expect("valid scope"),
    );
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
                    let pred = dive_model::Pred::new(
                        model.ir.def(d).body,
                        model.evaluator.scope(d).expect("valid scope"),
                    );
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
        let (ir, cfg) = text(&src, &config);
        for mut opts in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            {
                let store = StoreMode::Fingerprint;
                opts.store = store;
                let report = core_final::check(&ir, &cfg, &opts).expect(name);
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
        let (ir, cfg) = text(&src, &config);
        for mut opts in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            {
                let store = StoreMode::Fingerprint;
                opts.store = store;
                let report = core_final::check(&ir, &cfg, &opts).expect(name);
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
        let (ir, cfg) = text(&src, &config);
        for base in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            {
                let store = StoreMode::Fingerprint;
                let opts = CheckOptions {
                    store,
                    ..base.clone()
                };
                let report = core_final::check(&ir, &cfg, &opts).expect(name);
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
                    ("StateDomain", true),
                    ("All", false),
                    ("EmptyAny", false),
                    ("EmptyAll", true),
                    ("Pair", false),
                ] {
                    let d = ir.root_def(name).expect(name);
                    let p = dive_model::Pred::new(
                        ir.def(d).body,
                        m.evaluator.scope(d).expect("valid scope"),
                    );
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
        let (ir, cfg) = text(&src, &config);
        for base in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            {
                let store = StoreMode::Fingerprint;
                let opts = CheckOptions {
                    store,
                    ..base.clone()
                };
                let report = core_final::check(&ir, &cfg, &opts).expect(name);
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
                    let p = dive_model::Pred::new(
                        ir.def(d).body,
                        model.evaluator.scope(d).expect("valid scope"),
                    );
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
        let p = dive_model::Pred::new(
            ir.def(d).body,
            model.evaluator.scope(d).expect("valid scope"),
        );
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
        let (ir, cfg) = text(&src, &config);
        for base in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            {
                let store = StoreMode::Fingerprint;
                let opts = CheckOptions {
                    store,
                    ..base.clone()
                };
                let report = core_final::check(&ir, &cfg, &opts).expect(name);
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
                    let pred = dive_model::Pred::new(
                        ir.def(d).body,
                        model.evaluator.scope(d).expect("valid scope"),
                    );
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
        let p = dive_model::Pred::new(
            ir.def(d).body,
            model.evaluator.scope(d).expect("valid scope"),
        );
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

const SUBSTITUTION_LIMITS_SOURCE: &str = r"---- MODULE InstanceLimits ----
---- MODULE Inner ----
CONSTANT F(_)
VARIABLE v
OpClaim == []<>F(v)
Disabled == []~ENABLED (v'=~v)
Never == []~ENABLED (v'=~v /\ FALSE)
Uncovered == []~ENABLED (v'#v)
ValueClaim == []<>v
====
VARIABLE x
Identity(p) == p
I == INSTANCE Inner WITH F <- Identity, v <- x
Init == x=TRUE
Next == x'=~x
OP == I!OpClaim
EN == I!Disabled
DN == I!Never
UC == I!Uncovered
P == I!ValueClaim
====
";

#[test]
fn substituted_disabledness_keeps_finite_proofs_and_other_scope_limits() {
    let (ir, cfg) = input(SUBSTITUTION_LIMITS_SOURCE, "INIT Init\nNEXT Next\n");
    for options in [
        dive_model::BindOptions::tlc(),
        dive_model::BindOptions::dive(),
    ] {
        let model = dive_model::bind(ir.clone(), &cfg, &options).expect("bind");
        let role = |name| {
            let def = ir.root_def(name).expect(name);
            dive_model::Pred::new(
                ir.def(def).body,
                model.evaluator.scope(def).expect("valid scope"),
            )
        };
        let states = [Arc::from([Some(true.into())])];
        let op = role("OP");
        assert!(dive_liveness::plan::lower(&model, &op, Limits::default().formula).is_err());
        assert!(dive_trace::temporal::evaluate_lasso(&model, &op, &states, 0).is_err());
        for value in [true, false] {
            let states = [Arc::from([Some(value.into())])];
            for _ in 0..2 {
                for (name, truth) in [("EN", false), ("DN", true)] {
                    assert_eq!(
                        dive_trace::temporal::evaluate_lasso(&model, &role(name), &states, 0)
                            .expect("complete original INSTANCE finite cover"),
                        vec![truth],
                        "{name}/{value}"
                    );
                }
                assert!(
                    matches!(
                        dive_trace::temporal::evaluate_lasso(&model, &role("UC"), &states, 0),
                        Err(dive_trace::temporal::TemporalError::Unsupported(_))
                    ),
                    "missing finite cover never establishes disabledness"
                );
            }
        }
        let mut limit = Limits::default().formula;
        limit.expansion_steps = 3;
        assert_eq!(
            dive_liveness::plan::lower(&model, &role("P"), limit)
                .expect_err("budget")
                .code(),
            "DIVE-L0021"
        );
    }
}

#[test]
fn finite_data_constructors_match_pinned_temporal_truth_and_counts() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/temporal-constructors");
    for name in [
        "BcMapGood",
        "BcMapBad",
        "BcFilterGood",
        "BcFunctionGood",
        "BcNestedGood",
        "BcAction",
        "BcEmpty",
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
        let exit = std::fs::read_to_string(
            root.join("tlc-2026.10.06.014338")
                .join(format!("{name}.exit")),
        )
        .expect("exit");
        let (ir, cfg) = text(&source, &cfg);
        for base in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            {
                let store = StoreMode::Fingerprint;
                let opts = CheckOptions {
                    store,
                    ..base.clone()
                };
                let report = core_final::check(&ir, &cfg, &opts).expect(name);
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
fn constructor_words_retain_bound_variables_and_state_on_the_closing_edge() {
    let (ir, cfg) = input(
        r"---- MODULE ConstructorWords ----
VARIABLE x
P == []<>({IF b THEN x ELSE FALSE : b \in BOOLEAN}=BOOLEAN)
Q == <>[]({b \in BOOLEAN : b=x}={TRUE})
H(f) == []<>(f[TRUE])
R == H([b \in BOOLEAN |-> IF b THEN x ELSE ~x])
A == []<><<({x'#x : b \in {TRUE}}={TRUE})>>_x
Init == x=TRUE
Next == x'=~x
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
                for (name, truth) in [("P", any), ("Q", all), ("R", any), ("A", any && !all)] {
                    let d = ir.root_def(name).expect(name);
                    let pred = dive_model::Pred::new(
                        ir.def(d).body,
                        model.evaluator.scope(d).expect("valid scope"),
                    );
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
                        direct
                    );
                }
            }
        }
    }
}

#[test]
fn effectful_and_infinite_constructor_scopes_are_refused() {
    for property in [
        r"[]({Print(x,TRUE) : b \in BOOLEAN}={TRUE})",
        r#"[]({TLCGet("level") : b \in BOOLEAN}={TRUE})"#,
        r"[]({b : b \in Nat}={})",
    ] {
        let source = format!(
            "---- MODULE ConstructorRefused ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == x=TRUE\nNext == x'=~x\nP == {property}\n====\n"
        );
        let (ir, cfg) = input(&source, "INIT Init\nNEXT Next\n");
        let model =
            dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc()).expect("bind");
        let d = ir.root_def("P").expect("P");
        let pred = dive_model::Pred::new(
            ir.def(d).body,
            model.evaluator.scope(d).expect("valid scope"),
        );
        assert!(
            dive_liveness::plan::lower(&model, &pred, Limits::default().formula).is_err(),
            "{property}"
        );
        let states = vec![Arc::from([Some(true.into())])];
        assert!(
            dive_trace::temporal::evaluate_lasso(&model, &pred, &states, 0).is_err(),
            "{property}"
        );
        assert!(
            model.take_output().is_empty(),
            "effectful preflight: {property}"
        );
    }
}

#[test]
fn large_pure_constructor_scopes_match_primary_truth_and_fresh_replay() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, truth) in [
        ("LargeConstructorHolds", true),
        ("LargeConstructorFails", false),
    ] {
        let relative = format!("constant-predicate-binders/specifications/{name}/{name}");
        let file = root.join(format!("conformance/corpus/{relative}.tla"));
        let source = std::fs::read_to_string(&file).expect("source");
        let config = std::fs::read_to_string(file.with_extension("cfg")).expect("config");
        let golden: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(format!(
                "conformance/golden/tlc-2026.10.06.014338/{relative}.cfg.json"
            )))
            .expect("generated primary golden"),
        )
        .expect("golden JSON");
        let (ir, cfg) = input(&source, &config);
        for tlc in [true, false] {
            let binding = if tlc {
                dive_model::BindOptions::tlc()
            } else {
                dive_model::BindOptions::dive()
            };
            let model = dive_model::bind(ir.clone(), &cfg, &binding).expect("bind");
            let def = ir.root_def("P").expect("P");
            let pred =
                dive_model::Pred::new(ir.def(def).body, model.evaluator.scope(def).expect("scope"));
            let plan = dive_liveness::plan::lower(&model, &pred, Limits::default().formula)
                .expect("pure constructor admission");
            let state: dive_trace::State = Arc::from([Some(true.into())]);
            let st = dive_eval::St::action(state.clone(), state.clone());
            let row = plan
                .atoms
                .iter()
                .map(|a| a.evaluate(&model, &st).expect("atom"))
                .collect();
            assert_eq!(
                plan.formula
                    .evaluate_word(&[row], &[vec![]], 0, 100)
                    .expect("word"),
                vec![truth]
            );
            let fresh =
                dive_model::bind(ir.clone(), &cfg, &binding).expect("fresh validator model");
            let independent = dive_model::Pred::new(
                ir.def(def).body,
                fresh.evaluator.scope(def).expect("fresh scope"),
            );
            assert_eq!(
                dive_trace::temporal::evaluate_lasso(&fresh, &independent, &[state], 0)
                    .expect("replay"),
                vec![truth]
            );
            assert_large_constructor_report(&file, tlc, truth, &golden, &fresh, &cfg.model_values);
        }
    }
}

fn assert_large_constructor_report(
    file: &std::path::Path,
    tlc: bool,
    truth: bool,
    golden: &serde_json::Value,
    fresh: &dive_model::Model,
    model_values: &[String],
) {
    for store in [dive_api::Store::Exact, dive_api::Store::Fingerprint] {
        let report = dive_api::check_file(&dive_api::CheckRequest {
            file: file.to_path_buf(),
            compat_tlc: tlc,
            store,
            seed: Some(0),
            fp: Some(0),
            workers: Some(1),
            ..dive_api::CheckRequest::default()
        })
        .expect("check");
        let value = report.to_json_with_exit_convention(dive_api::ExitConvention::Tlc);
        assert_eq!(value["exit_code"], golden["result"]["exit_code"], "{value}");
        assert_eq!(value["tlc"]["code"], golden["result"]["verdict_code"]);
        assert_eq!(value["tlc"]["name"], golden["result"]["error_class"]);
        assert_eq!(value["property"], golden["result"]["violated_property"]);
        if truth {
            assert_eq!(value["verdict"], "success");
            for (native, primary) in [
                ("generated", "states_generated"),
                ("distinct", "distinct_states"),
                ("depth", "depth"),
            ] {
                assert_eq!(
                    value["statistics"][native],
                    golden["result"]["counts"][primary]
                );
            }
        } else {
            assert_eq!(value["verdict"], "invariant-violation");
            for (word, oracle) in [
                (&value["counterexample"]["trace"], false),
                (&golden["result"]["trace"]["json"], true),
            ] {
                let mut document = dive_trace::TraceDoc::from_json(word).expect("word");
                let ending = dive_trace::Ending::Invariant("P".into());
                if oracle {
                    // TLC's dumped states omit the separately verified deciding claim.
                    document = document.with_ending(ending);
                } else {
                    assert_eq!(document.ending, Some(ending));
                }
                assert_eq!(
                    dive_trace::validate_doc(fresh, &document, model_values)
                        .0
                        .validation,
                    Validation::Valid
                );
                let mut forged = document;
                forged.states[0].insert("x".into(), false.into());
                assert!(matches!(
                    dive_trace::validate_doc(fresh, &forged, model_values)
                        .0
                        .validation,
                    Validation::Invalid(_)
                ));
            }
        }
    }
}

#[test]
fn state_dependent_constructor_domains_use_actual_states_and_match_tlc() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/temporal-constructors");
    let source =
        std::fs::read_to_string(root.join("specs/BcStateDomain/BcStateDomain.tla")).expect("spec");
    let cfg =
        std::fs::read_to_string(root.join("specs/BcStateDomain/BcStateDomain.cfg")).expect("cfg");
    let exit = std::fs::read_to_string(root.join("tlc-2026.10.06.014338/BcStateDomain.exit"))
        .expect("oracle");
    assert_eq!(exit.trim(), "0", "TLC supports this domain");
    let (ir, cfg) = text(&source, &cfg);
    let stdout = std::fs::read_to_string(root.join("tlc-2026.10.06.014338/BcStateDomain.stdout"))
        .expect("oracle");
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        {
            let store = StoreMode::Fingerprint;
            let options = CheckOptions {
                store,
                ..options.clone()
            };
            let report = core_final::check(&ir, &cfg, &options).expect("finite state domains");
            let Outcome::Holds(c) = report.outcome() else {
                panic!("{report:?}");
            };
            assert_eq!(
                (c.stats.generated, c.stats.distinct, c.stats.depth),
                counts(&stdout)
            );
        }
    }
}

#[test]
fn state_domain_words_change_bindings_and_preserve_primed_closing_edges() {
    let (ir, cfg) = input(
        r"---- MODULE DomainWords ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Any == []<>(\E b \in IF x THEN {TRUE} ELSE {} : b)
All == <>[](\A b \in IF x THEN {} ELSE {FALSE} : b)
Map == []<>({b : b \in IF x THEN {TRUE} ELSE {}}={TRUE})
Filter == <>[]({b \in IF x THEN BOOLEAN ELSE {} : b}={TRUE})
H(f) == []<>(DOMAIN f={TRUE})
Fn == H([b \in IF x THEN {TRUE} ELSE {} |-> b])
Nested == []<>(\A b \in {x} : (\E c \in IF b THEN {TRUE} ELSE {} : c))
Prime == []<><<((\E b \in IF x THEN {TRUE} ELSE {} : b)')>>_x
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
                for (name, expected) in [
                    ("Any", any),
                    ("All", all),
                    ("Map", any),
                    ("Filter", all),
                    ("Fn", any),
                    ("Nested", any),
                    ("Prime", any && !all),
                ] {
                    let d = ir.root_def(name).expect(name);
                    let p = dive_model::Pred::new(
                        ir.def(d).body,
                        model.evaluator.scope(d).expect("valid scope"),
                    );
                    let direct = dive_trace::temporal::evaluate_lasso(&model, &p, &states, back)
                        .expect(name);
                    assert_eq!(direct, vec![expected; n], "{name} {values:?} {back}");
                    let plan = dive_liveness::plan::lower(&model, &p, Limits::default().formula)
                        .expect(name);
                    let rows: Vec<Vec<bool>> = (0..n)
                        .map(|i| {
                            let j = if i + 1 == n { back } else { i + 1 };
                            let st = dive_eval::St::action(states[i].clone(), states[j].clone());
                            plan.atoms
                                .iter()
                                .map(|a| a.evaluate(&model, &st).expect(name))
                                .collect()
                        })
                        .collect();
                    assert_eq!(
                        plan.formula
                            .evaluate_word(&rows, &rows, back, 1_000_000)
                            .expect(name),
                        direct
                    );
                }
            }
        }
    }
}

#[test]
fn actual_state_domain_bounds_and_analysis_markers_cannot_claim_truth() {
    let (ir, cfg) = input(
        r"---- MODULE DomainBounds ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x=TRUE
Next == x'=~x
Large == []({b : b \in IF x THEN 1..5000 ELSE {}}={})
Opaque == [](\E b \in IF x THEN BOOLEAN ELSE {} : Print(b,TRUE))
Unbounded == [](\E b \in IF x THEN Nat ELSE {} : b=b)
DynamicTemporal == \A b \in IF x THEN BOOLEAN ELSE {} : []<>(x=b)
====
",
        "INIT Init\nNEXT Next\n",
    );
    let model = dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc()).expect("bind");
    for name in ["Opaque", "DynamicTemporal"] {
        let d = ir.root_def(name).expect(name);
        let p = dive_model::Pred::new(
            ir.def(d).body,
            model.evaluator.scope(d).expect("valid scope"),
        );
        assert!(
            dive_liveness::plan::lower(&model, &p, Limits::default().formula).is_err(),
            "{name}"
        );
        assert!(
            dive_trace::temporal::evaluate_lasso(&model, &p, &[Arc::from([Some(true.into())])], 0)
                .is_err(),
            "{name}"
        );
        assert!(model.take_output().is_empty(), "{name}");
    }
    for name in ["Large", "Unbounded"] {
        let d = ir.root_def(name).expect(name);
        let p = dive_model::Pred::new(
            ir.def(d).body,
            model.evaluator.scope(d).expect("valid scope"),
        );
        let plan = dive_liveness::plan::lower(&model, &p, Limits::default().formula)
            .expect("pure dynamic domain planned without reading it");
        let state: dive_trace::State = Arc::from([Some(true.into())]);
        // The checker's atoms and replay both evaluate the 5000-element
        // domain, as TLC does; an infinite domain fails in both.
        assert_eq!(
            plan.atoms.iter().any(|a| a
                .evaluate(&model, &dive_eval::St::state(state.clone()))
                .is_err()),
            name == "Unbounded",
            "{name}"
        );
        let word =
            dive_trace::temporal::evaluate_lasso(&model, &p, &[Arc::from([Some(true.into())])], 0);
        assert_eq!(word.ok(), (name == "Large").then(|| vec![false]), "{name}");
        assert_eq!(
            dive_trace::temporal::evaluate_lasso(&model, &p, &[Arc::from([Some(false.into())])], 0)
                .expect("empty domain"),
            vec![name == "Large"], // Empty set equals {}; an existential over it is FALSE.
            "{name}"
        );
        // Empty current domain is valid. A nonempty later infinite domain refuses.
        assert!(
            name == "Large"
                || dive_trace::temporal::evaluate_lasso(
                    &model,
                    &p,
                    &[
                        Arc::from([Some(false.into())]),
                        Arc::from([Some(true.into())])
                    ],
                    0
                )
                .is_err(),
            "{name}"
        );
    }
    let d = ir.root_def("Large").expect("Large");
    let dive_ir::Expr::Always(body) = ir.expr(ir.def(d).body) else {
        panic!("always");
    };
    let dive_ir::Expr::Builtin { args, .. } = ir.expr(*body) else {
        panic!("equality");
    };
    let map = &args[0];
    let dive_ir::Expr::SetMap { expr: local, .. } = ir.expr(*map) else {
        panic!("map");
    };
    let original = model.evaluator.scope(d).expect("valid scope");
    let analysis = model
        .evaluator
        .scope_binder_analysis(&original, *map)
        .expect("analysis scope");
    let st = dive_eval::St::state(Arc::from([Some(true.into())]));
    let err = model
        .evaluator
        .eval_scoped(&analysis, *local, &st)
        .expect_err("unknown must never become a sample value");
    assert_eq!(err.class(), dive_eval::ErrorClass::Unsupported);
    assert!(
        model.evaluator.eval_scoped(&original, *local, &st).is_err(),
        "analysis left the original scope unbound"
    );
}

#[test]
fn state_domain_models_match_pinned_final_truth_counts_and_independent_validation() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/temporal-state-domains");
    for name in [
        "DfMap",
        "DfFilter",
        "DfFunction",
        "DfNested",
        "DfLazy",
        "DfPrime",
        "DfBad",
        "DfEnabled",
    ] {
        let src =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("spec");
        let config =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("cfg");
        let stdout =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.stdout")))
                .expect("oracle");
        let exit = std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.exit")))
            .expect("exit");
        let (src_text, cfg_text) = text(&src, &config);
        let (ir, cfg) = input(&src, &config);
        for base in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            {
                let store = StoreMode::Fingerprint;
                let options = CheckOptions {
                    store,
                    ..base.clone()
                };
                let report = core_final::check(&src_text, &cfg_text, &options).expect(name);
                let c = match report.outcome() {
                    Outcome::Holds(c) => {
                        assert_eq!(exit.trim(), "0", "{name}");
                        c.as_ref()
                    }
                    Outcome::Violation(v) => {
                        assert_eq!(exit.trim(), "13", "{name}");
                        assert_eq!(v.validation().validation, Validation::Valid, "{name}");
                        let fresh =
                            dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc())
                                .expect("fresh model");
                        assert_eq!(
                            dive_trace::validate_doc(&fresh, v.document(), &cfg.model_values)
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
            }
        }
    }
}

#[test]
fn successor_domains_are_read_without_placeholder_states() {
    let (ir, cfg) = input(
        r"---- MODULE ShiftedDomains ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Shifted == (\E b \in IF x THEN {TRUE} ELSE {} : b)'
Still == UNCHANGED x
====
",
        "INIT Init\nNEXT Next\n",
    );
    let model = dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc()).expect("bind");
    let d = ir.root_def("Shifted").expect("Shifted");
    let expr = ir.def(d).body;
    let scope = model.evaluator.scope(d).expect("valid scope");
    let before: dive_trace::State = Arc::from([Some(false.into())]);
    let after: dive_trace::State = Arc::from([Some(true.into())]);
    let edge = dive_eval::St::action(before.clone(), after.clone());
    assert_eq!(
        model
            .evaluator
            .eval_scoped(&scope, expr, &edge)
            .expect("predicate"),
        true.into()
    );
    let reverse = dive_eval::St::action(after, before.clone());
    assert_eq!(
        model
            .evaluator
            .eval_scoped(&scope, expr, &reverse)
            .expect("predicate"),
        false.into()
    );
    let missing = dive_eval::St::state(before);
    assert!(
        model.evaluator.eval_scoped(&scope, expr, &missing).is_err(),
        "missing state must not reuse the empty current domain"
    );
}

#[test]
fn finite_choose_predicates_keep_values_contexts_and_closing_words() {
    let (ir, cfg) = input(
        r"---- MODULE ChoiceWords ----
EXTENDS Naturals
VARIABLE x
Init == x=FALSE
Next == x'=~x
Min(S) == CHOOSE b \in S : \A c \in S : b<=c
Max(S) == CHOOSE b \in S : \A c \in S : b>=c
H(S) == []<>((CHOOSE b \in S : TRUE)=TRUE)
A == []<>(Min(IF x THEN {1,2} ELSE {3,4})=1)
B == []<>(Max(IF x THEN {1,2} ELSE {3,4})=4)
C == H(IF x THEN {TRUE} ELSE {FALSE})
D == []<>(\E i \in {x} : (CHOOSE b \in IF i THEN {TRUE} ELSE {FALSE} : b=i)=i)
E == []<>((CHOOSE t \in IF x THEN {<<TRUE,FALSE>>} ELSE {<<FALSE,TRUE>>} : t[1]#t[2]) = <<TRUE,FALSE>>)
F == []<>((CHOOSE i \in {1,2} : i>1)=2)
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
                    ("A", any),
                    ("B", !all),
                    ("C", any),
                    ("D", true),
                    ("E", any),
                    ("F", true),
                ] {
                    let d = ir.root_def(name).expect(name);
                    let p = dive_model::Pred::new(
                        ir.def(d).body,
                        model.evaluator.scope(d).expect("valid scope"),
                    );
                    let direct = dive_trace::temporal::evaluate_lasso(&model, &p, &states, back)
                        .expect(name);
                    assert_eq!(direct, vec![truth; n], "{name} {values:?} {back}");
                    let plan = dive_liveness::plan::lower(&model, &p, Limits::default().formula)
                        .expect(name);
                    let rows: Vec<Vec<bool>> = (0..n)
                        .map(|i| {
                            let j = if i + 1 == n { back } else { i + 1 };
                            let st = dive_eval::St::action(states[i].clone(), states[j].clone());
                            plan.atoms
                                .iter()
                                .map(|a| a.evaluate(&model, &st).expect(name))
                                .collect()
                        })
                        .collect();
                    assert_eq!(
                        plan.formula
                            .evaluate_word(&rows, &rows, back, 1_000_000)
                            .expect(name),
                        direct
                    );
                }
            }
        }
    }
}

#[test]
fn finite_choose_purity_limits_and_unsatisfied_predicates_never_supply_a_value() {
    let (ir, cfg) = input(
        r#"---- MODULE ChoiceRefused ----
EXTENDS Naturals,TLC
VARIABLE x
Init == x=TRUE
Next == x'=~x
Effect == []<>((CHOOSE b \in IF x THEN BOOLEAN ELSE {} : Print(b,TRUE))=TRUE)
Checker == []<>((CHOOSE b \in BOOLEAN : TLCGet("level")=1)=TRUE)
Infinite == []<>((CHOOSE i \in Nat : i=1)=1)
Huge == []<>((CHOOSE i \in IF x THEN 1..5000 ELSE {1} : TRUE)=1)
None == []<>((CHOOSE b \in IF x THEN {FALSE} ELSE {TRUE} : b)=TRUE)
Empty == []<>((CHOOSE b \in IF x THEN {} ELSE {TRUE} : TRUE)=TRUE)
====
"#,
        "INIT Init\nNEXT Next\n",
    );
    let model = dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc()).expect("bind");
    for name in ["Effect", "Checker", "Infinite"] {
        let d = ir.root_def(name).expect(name);
        let p = dive_model::Pred::new(
            ir.def(d).body,
            model.evaluator.scope(d).expect("valid scope"),
        );
        assert!(
            dive_liveness::plan::lower(&model, &p, Limits::default().formula).is_err(),
            "{name}"
        );
        assert!(
            dive_trace::temporal::evaluate_lasso(&model, &p, &[Arc::from([Some(true.into())])], 0)
                .is_err(),
            "{name}"
        );
        assert!(model.take_output().is_empty(), "{name}");
    }
    for name in ["Huge", "None", "Empty"] {
        let d = ir.root_def(name).expect(name);
        let p = dive_model::Pred::new(
            ir.def(d).body,
            model.evaluator.scope(d).expect("valid scope"),
        );
        let plan = dive_liveness::plan::lower(&model, &p, Limits::default().formula)
            .expect("dynamic pure choice");
        let st = dive_eval::St::state(Arc::from([Some(true.into())]));
        // The 5000-element choice is TLC's, for the checker's atoms and replay.
        assert_eq!(
            plan.atoms.iter().any(|a| a.evaluate(&model, &st).is_err()),
            name != "Huge",
            "{name}"
        );
        let word =
            dive_trace::temporal::evaluate_lasso(&model, &p, &[Arc::from([Some(true.into())])], 0);
        if name == "Huge" {
            assert_eq!(word, Ok(vec![true]), "{name}");
        } else {
            assert!(word.is_err(), "{name}");
        }
        assert_eq!(
            dive_trace::temporal::evaluate_lasso(&model, &p, &[Arc::from([Some(false.into())])], 0)
                .expect("a valid concrete choice"),
            vec![true],
            "{name}"
        );
    }
}

#[test]
fn finite_choose_models_match_pinned_truth_counts_in_both_profiles_and_stores() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/temporal-choose");
    for name in ["CfMin", "CfMax", "CfTuple", "CfNested", "CfLazy"] {
        let source =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("source");
        let config =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("config");
        let stdout =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.stdout")))
                .expect("oracle");
        let exit = std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.exit")))
            .expect("oracle exit");
        let (ir, cfg) = text(&source, &config);
        for base in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            {
                let store = StoreMode::Fingerprint;
                let options = CheckOptions {
                    store,
                    ..base.clone()
                };
                let report = core_final::check(&ir, &cfg, &options).expect(name);
                let c = match report.outcome() {
                    Outcome::Holds(c) => {
                        assert_eq!(exit.trim(), "0", "{name}");
                        c.as_ref()
                    }
                    Outcome::Violation(v) => {
                        assert_eq!(exit.trim(), "13", "{name}");
                        assert_eq!(v.validation().validation, Validation::Valid, "{name}");
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
fn captured_domains_affect_predicate_level_even_when_the_body_ignores_the_binder() {
    let (ir, cfg) = input(
        r"---- MODULE CapturedDomains ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
E(S) == []<>(\E b \in S : TRUE)
A(S) == <>[](\A b \in S : FALSE)
M(S) == []<>({TRUE : b \in S}={TRUE})
F(S) == []<>(DOMAIN [b \in S |-> TRUE]={TRUE})
PE == E(IF x THEN {TRUE} ELSE {})
PA == A(IF x THEN {TRUE} ELSE {})
PM == M(IF x THEN {TRUE} ELSE {})
PF == F(IF x THEN {TRUE} ELSE {})
====
",
        "INIT Init\nNEXT Next\n",
    );
    let model = dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc()).expect("bind");
    for name in ["PE", "PA", "PM", "PF"] {
        let d = ir.root_def(name).expect(name);
        let p = dive_model::Pred::new(
            ir.def(d).body,
            model.evaluator.scope(d).expect("valid scope"),
        );
        let plan = dive_liveness::plan::lower(&model, &p, Limits::default().formula)
            .expect("domain must remain state level");
        assert!(!plan.atoms.is_empty(), "{name}: no eager constant folding");
        for value in [false, true] {
            let state: dive_trace::State = Arc::from([Some(value.into())]);
            let truth = if name == "PA" { !value } else { value };
            let direct =
                dive_trace::temporal::evaluate_lasso(&model, &p, std::slice::from_ref(&state), 0)
                    .expect(name);
            assert_eq!(direct, vec![truth], "{name} {value}");
            let st = dive_eval::St::action(state.clone(), state);
            let row: Vec<bool> = plan
                .atoms
                .iter()
                .map(|a| a.evaluate(&model, &st).expect(name))
                .collect();
            assert_eq!(
                plan.formula
                    .evaluate_word(
                        std::slice::from_ref(&row),
                        std::slice::from_ref(&row),
                        0,
                        1_000_000
                    )
                    .expect(name),
                direct
            );
        }
    }
}

#[test]
fn pure_action_arguments_preserve_nested_caller_edges_on_every_closing_word() {
    let (ir, cfg) = input(
        r"---- MODULE ActionWords ----
EXTENDS Naturals,Sequences
VARIABLE x
Init == x=0
Next == x'=1-x
Less(a,b) == a<b
Relay(a,b) == Less(a,b)
Saved(a,b) == LET Before == a IN Before<b
Prefix(a,b) == Len(a)<Len(b) /\ SubSeq(b,1,Len(a))=a
P == []<><<Less(x,x')>>_x
Q == []<><<Relay(x,x')>>_x
R == []<><<Saved(x,x')>>_x
S == []<><<Prefix(IF x=0 THEN <<>> ELSE <<1>>,IF x'=0 THEN <<>> ELSE <<1>>)>>_x
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
                .map(|v| Arc::from([Some(i64::from(*v).into())]))
                .collect();
            for back in 0..n {
                let truth = values[back..].iter().any(|v| *v) && values[back..].iter().any(|v| !*v);
                for name in ["P", "Q", "R", "S"] {
                    let d = ir.root_def(name).expect(name);
                    let p = dive_model::Pred::new(
                        ir.def(d).body,
                        model.evaluator.scope(d).expect("valid scope"),
                    );
                    let direct = dive_trace::temporal::evaluate_lasso(&model, &p, &states, back)
                        .expect(name);
                    assert_eq!(direct, vec![truth; n], "{name} {values:?} {back}");
                    let plan = dive_liveness::plan::lower(&model, &p, Limits::default().formula)
                        .expect(name);
                    let rows: Vec<Vec<bool>> = (0..n)
                        .map(|i| {
                            let j = if i + 1 == n { back } else { i + 1 };
                            let st = dive_eval::St::action(states[i].clone(), states[j].clone());
                            plan.atoms
                                .iter()
                                .map(|a| a.evaluate(&model, &st).expect(name))
                                .collect()
                        })
                        .collect();
                    assert_eq!(
                        plan.formula
                            .evaluate_word(&rows, &rows, back, 1_000_000)
                            .expect(name),
                        direct
                    );
                }
            }
        }
    }
}

#[test]
fn action_argument_effects_and_action_in_temporal_helper_remain_refusals() {
    let (ir, cfg) = input(
        r#"---- MODULE ActionRefused ----
EXTENDS Naturals,TLC
VARIABLE x
Init == x=0
Next == x'=1-x
Less(a,b) == a<b
Identity(a) == a
Temporal(a) == []<><<a>>_x
P == []<><<Less(x,Print(x',x'))>>_x
Q == []<><<Less(x,TLCGet("level"))>>_x
R == Identity([]<>(x=0))
S == Temporal(x'=1)
====
"#,
        "INIT Init\nNEXT Next\n",
    );
    let model = dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc()).expect("bind");
    for name in ["P", "Q", "S"] {
        let d = ir.root_def(name).expect(name);
        let p = dive_model::Pred::new(
            ir.def(d).body,
            model.evaluator.scope(d).expect("valid scope"),
        );
        assert!(
            dive_liveness::plan::lower(&model, &p, Limits::default().formula).is_err(),
            "{name}"
        );
        assert!(
            dive_trace::temporal::evaluate_lasso(&model, &p, &[Arc::from([Some(0.into())])], 0)
                .is_err(),
            "{name}"
        );
        assert!(
            model.take_output().is_empty(),
            "{name}: no evaluation during preflight"
        );
    }
}

#[test]
fn pinned_action_argument_models_match_final_truth_counts_and_validate() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/temporal-action-args");
    for name in ["AaMath", "AaNested", "AaCaptured", "AaSequence"] {
        let source =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("spec");
        let config =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("config");
        let stdout =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.stdout")))
                .expect("oracle");
        let exit = std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.exit")))
            .expect("exit");
        assert_eq!(exit.trim(), "13", "{name}");
        let (ir, cfg) = text(&source, &config);
        for base in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            {
                let store = StoreMode::Fingerprint;
                let options = CheckOptions {
                    store,
                    ..base.clone()
                };
                let report = core_final::check(&ir, &cfg, &options).expect(name);
                let Outcome::Violation(v) = report.outcome() else {
                    panic!("{name}: {report:?}");
                };
                assert_eq!(v.validation().validation, Validation::Valid, "{name}");
                let c = &v.candidate().exploration;
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
fn pinned_multi_process_fairness_fits_bounded_products_and_rejects_unfair_mutants() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fairness-factoring");
    for name in ["FwFive", "FwMissing", "FwMixed"] {
        let source =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.tla"))).expect("spec");
        let config =
            std::fs::read_to_string(root.join(format!("specs/{name}/{name}.cfg"))).expect("config");
        let stdout =
            std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.stdout")))
                .expect("oracle");
        let exit = std::fs::read_to_string(root.join(format!("tlc-2026.10.06.014338/{name}.exit")))
            .expect("exit");
        assert_eq!(exit.trim(), if name == "FwMissing" { "13" } else { "0" });
        let (ir, cfg) = input(&source, &config);
        let mut limits = Limits::default();
        if name != "FwMixed" {
            limits.formula.tableau_nodes = 64;
            let model =
                dive_model::bind(ir.clone(), &cfg, &dive_model::BindOptions::tlc()).expect("model");
            let p = &model.property_claims()[0];
            let plan = dive_liveness::plan::lower_violation(&model, &p.pred, limits.formula)
                .expect("plan");
            assert!(
                matches!(
                    dive_liveness::formula::compile(&plan.formula, limits.formula),
                    Err(dive_liveness::formula::FormulaError::Limit(_))
                ),
                "unfactored {name}"
            );
        }
        for base in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            {
                let store = StoreMode::Fingerprint;
                for workers in [1, 4, 8] {
                    let opts = CheckOptions {
                        store,
                        workers,
                        ..base.clone()
                    };
                    let report = core_final::check(&source, &config, &opts).expect(name);
                    let c = match report.outcome() {
                        Outcome::Holds(c) => {
                            assert_ne!(name, "FwMissing");
                            c.as_ref()
                        }
                        Outcome::Violation(v) => {
                            assert_eq!(name, "FwMissing");
                            assert_eq!(v.validation().validation, Validation::Valid);
                            fairness_validation_controls(&ir, &cfg, &opts, v.document());
                            &v.candidate().exploration
                        }
                        Outcome::Safety(r) => panic!("{name}: {:?}", r.verdict),
                    };
                    assert_eq!(
                        (c.stats.generated, c.stats.distinct, c.stats.depth),
                        counts(&stdout),
                        "{name}"
                    );
                    assert_eq!((c.workers.requested, c.workers.used), (workers, workers));
                }
            }
        }
    }
}

fn fairness_validation_controls(
    ir: &Arc<dive_ir::Ir>,
    cfg: &dive_config::ModelConfig,
    opts: &CheckOptions,
    source: &dive_trace::TraceDoc,
) {
    let model = dive_model::bind(
        ir.clone(),
        cfg,
        &dive_model::BindOptions {
            eval: opts.eval,
            ..dive_model::BindOptions::tlc()
        },
    )
    .expect("fresh model");
    let mut doc =
        dive_trace::TraceDoc::parse(&source.to_json().to_string()).expect("lossless witness");
    assert_eq!(
        dive_trace::validate_doc(&model, &doc, &cfg.model_values)
            .0
            .validation,
        Validation::Valid
    );
    doc.ending = Some(dive_trace::Ending::Lasso {
        back_to: doc.states.len() - 1,
    });
    let validation = dive_trace::validate_doc(&model, &doc, &cfg.model_values)
        .0
        .validation;
    assert!(
        matches!(validation,Validation::Invalid(ref failure) if failure.check == dive_trace::Check::Fairness),
        "a forever-stuttering ending must fail temporal fairness: {validation:?}"
    );
}
