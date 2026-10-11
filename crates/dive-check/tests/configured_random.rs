//! Configured `RandomElement` streams agree; other native RNG routes stay refused.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_check::{CheckOptions, Verdict};
use dive_ir::Ir;
use dive_store::StoreMode;
use std::sync::Arc;

const REFUSAL: &str = "configured native random replacements need bound-context stream classification and independent replay";
fn lower(source: &str) -> Arc<Ir> {
    let analysis = dive_sema::analyze_source(
        "ConfiguredRandom.tla",
        source,
        &dive_sema::Options::default(),
    );
    Arc::new(dive_ir::lower(&analysis).expect("valid source"))
}
fn source(invariant: &str, extra: &str) -> String {
    format!(
        "---- MODULE ConfiguredRandom ----\nEXTENDS Naturals, TLC\nCONSTANT C(_)\nVARIABLES n,x\n{extra}\nInit == n=0 /\\ x=0\nNext == n<3 /\\ n'=n+1 /\\ x'=C(1..100)\nInv == {invariant}\nV == n\n====\n"
    )
}
fn config(extra: &str) -> dive_config::ModelConfig {
    dive_config::parse(&format!("INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\nCONSTANT C <- RandomElement\n{extra}"))
        .into_config().expect("configuration")
}
fn assert_stream(ir: &Arc<Ir>, cfg: &dive_config::ModelConfig) {
    let text = &ir
        .files()
        .iter()
        .find(|f| !f.standard)
        .expect("source")
        .source;
    let direct = text
        .lines()
        .map(|line| {
            if line.starts_with("CONSTANT") || line.starts_with("C(S) ==") {
                line.to_owned()
            } else {
                line.replace("C(", "RandomElement(")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let direct = lower(&direct);
    for profile in [dive_eval::Profile::Dive, dive_eval::Profile::Tlc] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            for width in [1, 4, 8] {
                for chaos in [None, Some(19)] {
                    let mut opts = CheckOptions::tlc(0, 7);
                    opts.profile = profile;
                    opts.eval = dive_eval::Options::for_profile(profile);
                    opts.tlc_fingerprints = profile == dive_eval::Profile::Tlc;
                    opts.store = store;
                    opts.workers = width;
                    opts.chaos = chaos;
                    let actual = dive_check::check(ir, cfg, &opts);
                    let expected = dive_check::check(&direct, cfg, &opts);
                    if let Verdict::Unsupported { feature } = &expected.verdict {
                        assert_eq!(
                            feature,
                            "TLC initial-predecessor random counterexample replay is not proved"
                        );
                        assert!(expected.counterexample.is_none());
                    }
                    assert_eq!(actual.verdict, expected.verdict);
                    assert_eq!(actual.stats, expected.stats);
                    assert_eq!(actual.tlc_prints_stats, expected.tlc_prints_stats);
                    assert_eq!(actual.output, expected.output);
                    assert_eq!(actual.warnings, expected.warnings);
                    assert_eq!(actual.collision, expected.collision);
                    assert_eq!(actual.var_names, expected.var_names);
                    assert_eq!(actual.tlc_var_order, expected.tlc_var_order);
                    assert_eq!(actual.frontier, expected.frontier);
                    assert_eq!(actual.predecessor, expected.predecessor);
                    // Source action columns differ when C is spelled RandomElement.
                    let values = |r: &dive_check::CheckResult| {
                        r.counterexample.as_ref().map(|t| {
                            t.states
                                .iter()
                                .map(|s| s.values.clone())
                                .collect::<Vec<_>>()
                        })
                    };
                    assert_eq!(values(&actual), values(&expected));
                    assert_eq!(
                        (actual.workers.requested, actual.workers.used),
                        (width, 1),
                        "{actual:?}"
                    );
                    assert!(actual.workers.sequential);
                    assert_eq!(actual.workers, expected.workers);
                    if profile == dive_eval::Profile::Tlc {
                        let stats = actual.stats.expect("search");
                        if text.contains("Inv == x # 74") {
                            assert_eq!(actual.verdict, Verdict::Success);
                            assert_eq!((stats.generated, stats.distinct, stats.depth), (4, 4, 4));
                        } else if text.contains("Inv == x # 80") {
                            assert!(
                                matches!(&actual.verdict,Verdict::Unsupported {feature} if feature=="TLC initial-predecessor random counterexample replay is not proved")
                            );
                            assert!(actual.counterexample.is_none());
                            assert_eq!((stats.generated, stats.distinct, stats.depth), (2, 2, 2));
                        }
                    }
                }
            }
        }
    }
}
fn assert_refuses(ir: &Arc<Ir>, cfg: &dive_config::ModelConfig) {
    for profile in [dive_eval::Profile::Dive, dive_eval::Profile::Tlc] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            for width in [1, 4, 8] {
                for chaos in [None, Some(19)] {
                    let mut opts = CheckOptions::tlc(0, 7);
                    opts.profile = profile;
                    opts.eval = dive_eval::Options::for_profile(profile);
                    opts.tlc_fingerprints = profile == dive_eval::Profile::Tlc;
                    opts.store = store;
                    opts.workers = width;
                    opts.chaos = chaos;
                    let result = dive_check::check(ir, cfg, &opts);
                    assert!(
                        matches!(&result.verdict,Verdict::Unsupported {feature} if feature == REFUSAL),
                        "{result:?}"
                    );
                    assert_eq!((result.workers.requested, result.workers.used), (width, 0));
                    assert!(
                        result.stats.is_none()
                            && result.counterexample.is_none()
                            && result.collision.is_none()
                    );
                    assert!(result.output.is_empty() && result.warnings.is_empty());
                }
            }
        }
    }
}
#[test]
fn both_concrete_wrong_verdicts_match_the_direct_stream() {
    for invariant in ["x # 74", "x # 80"] {
        assert_stream(&lower(&source(invariant, "")), &config(""));
    }
}
#[test]
fn configured_native_authority_matches_startup_and_initial_effects() {
    for extra in [
        "ASSUME PrintT(\"must not print\")",
        "Startup == PrintT(\"must not print\")",
        // Presence of a visible random node must not hide the cfg authority:
        // the scheduling route still needs to be proved against that authority.
        "Unused == RandomElement(1..100)",
    ] {
        assert_stream(&lower(&source("x # 74", extra)), &config(""));
    }
    let init_effect =
        source("x # 74", "").replace("x=0", "x=IF PrintT(\"must not print\") THEN 0 ELSE 1");
    assert_stream(&lower(&init_effect), &config(""));
}
#[test]
fn configured_context_alias_and_typed_native_replacement_match_the_stream() {
    let instance = lower(&source("x # 80", "I == INSTANCE TLC"));
    let custom = lower(&source(
        "x # 80",
        "Replacement(S) == CHOOSE a \\in S : TRUE",
    ));
    let id = custom
        .defs()
        .iter()
        .position(|d| d.name == "Replacement")
        .expect("replacement");
    let mut value = serde_json::to_value(custom.as_ref()).expect("IR JSON");
    value["defs"][id]["std"] =
        serde_json::to_value(dive_ir::StdOp::TlcRandomElement).expect("native authority");
    let custom = Arc::new(serde_json::from_value(value).expect("IR"));
    dive_eval::validate_ir(&custom).expect("valid typed native replacement");
    for (ir, target) in [(instance, "I!RandomElement"), (custom, "Replacement")] {
        let mut cfg = config("");
        let dive_config::ConstantEntry::Substitution { target: rhs, .. } = &mut cfg.constants[0]
        else {
            panic!("substitution")
        };
        rhs.text = target.into();
        assert_stream(&ir, &cfg);
    }
}

#[test]
fn pure_configured_replacement_is_not_random_authority() {
    let ir = lower(&source("n < 3", "Replacement(S) == CHOOSE a \\in S : TRUE"));
    let cfg = config("");
    let mut cfg = cfg;
    let dive_config::ConstantEntry::Substitution { target, .. } = &mut cfg.constants[0] else {
        panic!("substitution")
    };
    target.text = "Replacement".into();
    for profile in [dive_eval::Profile::Dive, dive_eval::Profile::Tlc] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            for workers in [1, 4, 8] {
                for chaos in [None, Some(19)] {
                    let mut opts = CheckOptions::tlc(0, 7);
                    opts.profile = profile;
                    opts.eval = dive_eval::Options::for_profile(profile);
                    opts.tlc_fingerprints = profile == dive_eval::Profile::Tlc;
                    opts.store = store;
                    opts.workers = workers;
                    opts.chaos = chaos;
                    let r = dive_check::check(&ir, &cfg, &opts);
                    assert!(
                        matches!(r.verdict, Verdict::InvariantViolated { .. }),
                        "{r:?}"
                    );
                    let stats = r.stats.expect("concrete search");
                    assert_eq!((stats.generated, stats.distinct, stats.depth), (4, 4, 4));
                    assert_eq!(r.workers.used, workers);
                }
            }
        }
    }
}

#[test]
fn other_native_random_families_refuse_and_invalid_transitive_cfg_keeps_startup_error() {
    for (params, call, target) in [
        ("_,_", "C(2,1..4)", "RandomSubset"),
        ("_,_,_", "C(2,1,1..4)", "RandomSetOfSubsets"),
    ] {
        let text = source("n < 3", "")
            .replace(
                "EXTENDS Naturals, TLC",
                "EXTENDS Naturals, TLC, Randomization",
            )
            .replace("CONSTANT C(_)", &format!("CONSTANT C({params})"))
            .replace("C(1..100)", call);
        let mut cfg = config("");
        let dive_config::ConstantEntry::Substitution { target: rhs, .. } = &mut cfg.constants[0]
        else {
            panic!("substitution")
        };
        rhs.text = target.into();
        assert_refuses(&lower(&text), &cfg);
    }
    // RandomSubsetSet is a native implementation helper, not an exported
    // Randomization source definition. Public typed native authority still
    // must be guarded when that helper is installed on an admitted target.
    let text = source("n < 3", "Replacement(a,b,c) == b")
        .replace("CONSTANT C(_)", "CONSTANT C(_,_,_)")
        .replace("C(1..100)", "C(2,1,1..4)");
    let ir = lower(&text);
    let id = ir
        .defs()
        .iter()
        .position(|d| d.name == "Replacement")
        .expect("replacement");
    let mut value = serde_json::to_value(ir.as_ref()).expect("IR JSON");
    value["defs"][id]["std"] =
        serde_json::to_value(dive_ir::StdOp::RandomizationRandomSubsetSet).expect("native helper");
    let ir = Arc::new(serde_json::from_value(value).expect("IR"));
    dive_eval::validate_ir(&ir).expect("typed native helper authority");
    let mut cfg = config("");
    let dive_config::ConstantEntry::Substitution { target, .. } = &mut cfg.constants[0] else {
        panic!("substitution")
    };
    target.text = "Replacement".into();
    assert_refuses(&ir, &cfg);

    let text = source("n < 3", "CONSTANT D(_)");
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\nCONSTANT C <- D\nCONSTANT D <- RandomElement\n").into_config().expect("transitive cfg");
    let result = dive_check::check(&lower(&text), &cfg, &CheckOptions::tlc(0, 7));
    assert!(
        matches!(result.verdict, Verdict::ModelError { code: 2223, .. }),
        "{result:?}"
    );
    assert_eq!(result.workers.used, 0);
    assert!(result.stats.is_none() && result.output.is_empty());
}

#[test]
fn supplied_bound_models_refuse_native_targets_in_legacy_and_pool_capture() {
    let plain = lower(&source("x # 74", ""));
    let instance = lower(&source("x # 74", "I == INSTANCE TLC"));
    let custom = lower(&source(
        "x # 74",
        "Replacement(S) == CHOOSE a \\in S : TRUE",
    ));
    let id = custom
        .defs()
        .iter()
        .position(|d| d.name == "Replacement")
        .expect("target");
    let mut value = serde_json::to_value(custom.as_ref()).expect("IR JSON");
    value["defs"][id]["std"] =
        serde_json::to_value(dive_ir::StdOp::TlcRandomElement).expect("native authority");
    let custom = Arc::new(serde_json::from_value(value).expect("typed IR"));
    let redirected =
        lower(&source("x # 74", "").replace("CONSTANT C(_)", "C(S) == CHOOSE a \\in S : TRUE"));
    for (ir, target) in [
        (plain, "RandomElement"),
        (instance, "I!RandomElement"),
        (custom, "Replacement"),
        (redirected, "RandomElement"),
    ] {
        let mut cfg = config("");
        let dive_config::ConstantEntry::Substitution { target: rhs, .. } = &mut cfg.constants[0]
        else {
            panic!("substitution")
        };
        rhs.text = target.into();
        for profile in [dive_eval::Profile::Dive, dive_eval::Profile::Tlc] {
            for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                for workers in [1, 4, 8] {
                    for chaos in [None, Some(19)] {
                        let mut opts = CheckOptions::tlc(0, 7);
                        opts.profile = profile;
                        opts.eval = dive_eval::Options::for_profile(profile);
                        opts.tlc_fingerprints = profile == dive_eval::Profile::Tlc;
                        opts.store = store;
                        opts.workers = workers;
                        opts.chaos = chaos;
                        for pooled in [false, true] {
                            let model = dive_model::bind(
                                ir.clone(),
                                &cfg,
                                &dive_model::BindOptions {
                                    eval: opts.eval,
                                    ..dive_model::BindOptions::tlc()
                                },
                            )
                            .expect("supplied native model");
                            let result = if pooled {
                                dive_check::capture::explore_concrete_parallel_with_evidence(
                                    &model,
                                    &opts,
                                    dive_check::capture::Limits::default(),
                                )
                                .expect("capture admission")
                            } else {
                                dive_check::capture::explore(
                                    &model,
                                    &opts,
                                    dive_check::capture::Limits::default(),
                                )
                            };
                            let dive_check::capture::Exploration::Stopped(result) = result else {
                                panic!("native random capture falsely complete")
                            };
                            assert!(
                                matches!(&result.verdict,Verdict::Unsupported {feature} if feature == "graph capture with configured native random replacements"),
                                "{target}: {result:?}"
                            );
                            assert_eq!(
                                (result.workers.requested, result.workers.used),
                                (workers, 0)
                            );
                            assert!(result.stats.is_none() && result.counterexample.is_none());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn pure_native_imports_and_user_replacements_keep_legacy_capture() {
    let text = source("TRUE", "Replacement(S) == CHOOSE a \\in S : TRUE");
    let ir = lower(&text);
    let mut cfg = config("");
    let dive_config::ConstantEntry::Substitution { target, .. } = &mut cfg.constants[0] else {
        panic!("substitution")
    };
    target.text = "Replacement".into();
    for profile in [dive_eval::Profile::Dive, dive_eval::Profile::Tlc] {
        let mut opts = CheckOptions::tlc(0, 7);
        opts.profile = profile;
        opts.eval = dive_eval::Options::for_profile(profile);
        opts.tlc_fingerprints = profile == dive_eval::Profile::Tlc;
        let m = dive_model::bind(
            ir.clone(),
            &cfg,
            &dive_model::BindOptions {
                eval: opts.eval,
                ..dive_model::BindOptions::tlc()
            },
        )
        .expect("pure model");
        let dive_check::capture::Exploration::Complete(c) =
            dive_check::capture::explore(&m, &opts, dive_check::capture::Limits::default())
        else {
            panic!("pure capture refused")
        };
        assert_eq!(c.graph.states.len(), 4);
        assert_eq!(
            c.safety_result().stats.as_ref().expect("stats").generated,
            4
        );
    }
}

#[derive(Default)]
struct NoPrint(std::sync::atomic::AtomicUsize);
impl dive_eval::PrintSink for NoPrint {
    fn print(&self, _: String) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}
#[test]
fn malformed_native_bindings_are_not_constructible_before_names_or_callbacks() {
    let ir = lower(&source("TRUE", "Replacement(S) == CHOOSE a \\in S : TRUE"));
    for redirect in [false, true] {
        let mut m = dive_eval::Model::new(ir.clone());
        if redirect {
            m.redirects.insert(
                ir.root_def("Replacement").expect("definition"),
                dive_ir::DefId(u32::MAX),
            );
        } else {
            m.constants[0] = Some(dive_eval::ConstBinding::Def(dive_ir::DefId(u32::MAX)));
        }
        let names = m.names.clone();
        let before = names.len();
        let sink = Arc::new(NoPrint::default());
        let err = dive_eval::Evaluator::with_sink(m, dive_eval::Options::tlc(), sink.clone())
            .expect_err("malformed context");
        assert_eq!(err.code(), "DIVE-EVAL-C001");
        assert_eq!(names.len(), before);
        assert_eq!(sink.0.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
}
#[test]
fn foreign_native_bound_models_refuse_in_legacy_capture_before_assumptions() {
    let ir = lower(&source("TRUE", "ASSUME PrintT(\"must not print\")"));
    let valid =
        dive_model::bind(ir.clone(), &config(""), &dive_model::BindOptions::tlc()).expect("model");
    let foreign =
        dive_model::bind(ir, &config(""), &dive_model::BindOptions::tlc()).expect("foreign names");
    for field in [false, true] {
        let mut model = valid.clone();
        if field {
            model.ir = lower(&source("TRUE", ""));
        } else {
            model.names = foreign.names.clone();
        }
        let result = dive_check::capture::explore(
            &model,
            &CheckOptions::tlc(0, 7),
            dive_check::capture::Limits::default(),
        );
        let dive_check::capture::Exploration::Stopped(r) = result else {
            panic!("foreign legacy capture complete")
        };
        assert!(
            matches!(&r.verdict,Verdict::Unsupported {feature} if feature.contains("DIVE-EVAL-C004")),
            "{r:?}"
        );
        assert_eq!(r.workers.used, 0);
        assert!(r.output.is_empty() && r.stats.is_none() && r.counterexample.is_none());
        assert_eq!(valid.evaluator.take_output(), Vec::<String>::new());
        assert_eq!(foreign.evaluator.take_output(), Vec::<String>::new());
    }
}

#[test]
fn transformed_keys_with_configured_draws_refuse_before_effects() {
    let ir = lower(&source(
        "TRUE",
        "ASSUME PrintT(\"must not print\")\nStartup == PrintT(\"must not print\")\nS == {}",
    ));
    for (key, reason) in [
        (
            "VIEW V",
            "randomness under VIEW needs transformed-fingerprint seeding and independent replay",
        ),
        (
            "SYMMETRY S",
            "randomness under SYMMETRY needs canonical fingerprint seeding and independent replay",
        ),
    ] {
        let cfg = config(key);
        for profile in [dive_eval::Profile::Dive, dive_eval::Profile::Tlc] {
            for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                for workers in [1, 4, 8] {
                    for chaos in [None, Some(19)] {
                        let mut opts = CheckOptions::tlc(0, 7);
                        opts.profile = profile;
                        opts.eval = dive_eval::Options::for_profile(profile);
                        opts.tlc_fingerprints = profile == dive_eval::Profile::Tlc;
                        opts.store = store;
                        opts.workers = workers;
                        opts.chaos = chaos;
                        let r = dive_check::check(&ir, &cfg, &opts);
                        assert!(
                            matches!(&r.verdict,Verdict::Unsupported {feature} if feature==reason),
                            "{r:?}"
                        );
                        assert_eq!(r.workers.used, 0);
                        assert!(
                            r.stats.is_none()
                                && r.counterexample.is_none()
                                && r.output.is_empty()
                                && r.warnings.is_empty()
                        );
                    }
                }
            }
        }
    }
}
