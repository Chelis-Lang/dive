//! Configured native RNG authority must refuse until both stream routes agree.
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
fn both_concrete_wrong_verdicts_refuse_before_any_search() {
    for invariant in ["x # 74", "x # 80"] {
        assert_refuses(&lower(&source(invariant, "")), &config(""));
    }
}
#[test]
fn configured_native_authority_refuses_before_startup_or_initial_effects() {
    for extra in [
        "ASSUME PrintT(\"must not print\")",
        "Startup == PrintT(\"must not print\")",
        // Presence of a visible random node must not hide the cfg authority:
        // the scheduling route still needs to be proved against that authority.
        "Unused == RandomElement(1..100)",
    ] {
        assert_refuses(&lower(&source("x # 74", extra)), &config(""));
    }
    let init_effect =
        source("x # 74", "").replace("x=0", "x=IF PrintT(\"must not print\") THEN 0 ELSE 1");
    assert_refuses(&lower(&init_effect), &config(""));
}
#[test]
fn configured_context_alias_and_typed_native_replacement_refuse() {
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
        assert_refuses(&ir, &cfg);
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
fn each_native_random_family_and_transitive_constant_target_refuses() {
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
    assert_refuses(&lower(&text), &cfg);
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
            for workers in [1, 4, 8] {
                for chaos in [None, Some(19)] {
                    let mut opts = CheckOptions::tlc(0, 7);
                    opts.profile = profile;
                    opts.eval = dive_eval::Options::for_profile(profile);
                    opts.tlc_fingerprints = profile == dive_eval::Profile::Tlc;
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
                                Default::default(),
                            )
                            .expect("capture admission")
                        } else {
                            dive_check::capture::explore(&model, &opts, Default::default())
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
            dive_check::capture::explore(&m, &opts, Default::default())
        else {
            panic!("pure capture refused")
        };
        assert_eq!(c.graph.states.len(), 4);
        assert_eq!(c.safety_result.stats.expect("stats").generated, 4);
    }
}
