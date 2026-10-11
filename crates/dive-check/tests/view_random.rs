//! VIEW/RNG refuses before binding effects until transformed seeding is proved.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_check::{CheckOptions, Verdict};
use dive_ir::Ir;
use dive_store::StoreMode;
use std::sync::Arc;

const REFUSAL: &str =
    "randomness under VIEW needs transformed-fingerprint seeding and independent replay";

fn lower(source: &str) -> Arc<Ir> {
    let analysis =
        dive_sema::analyze_source("RandomView.tla", source, &dive_sema::Options::default());
    Arc::new(dive_ir::lower(&analysis).expect("valid test IR"))
}
fn config(extra: &str) -> dive_config::ModelConfig {
    dive_config::parse(&format!("INIT Init\nNEXT Next\nINVARIANT Inv\n{extra}"))
        .into_config()
        .expect("config")
}
fn source(definitions: &str, next_value: &str) -> String {
    format!(
        "---- MODULE RandomView ----\nEXTENDS Naturals, TLC, Randomization\nVARIABLES n, x\n{definitions}\nInit == /\\ n = 0 /\\ x = 0\nNext == /\\ n' = n + 1 /\\ x' = {next_value}\nInv == n < 3\nV == n\n====\n"
    )
}
fn variants() -> Vec<Arc<Ir>> {
    let mut variants = vec![
        lower(&source("", "RandomElement(1..100)")),
        lower(&source("", "RandomSubset(2, 1..4)")),
        lower(&source("", "RandomSetOfSubsets(2, 1, 1..4)")),
        lower(&source("Alias(S) == RandomElement(S)", "Alias(1..100)")),
        lower(&source(
            "CONSTANT C\nReplacement == RandomElement(1..100)",
            "C",
        )),
        lower(&source("Unused == RandomElement(1..100)", "n")),
        lower(&source(
            "ASSUME PrintT(\"must not print\")",
            "RandomElement(1..100)",
        )),
        lower(&source(
            "Startup == PrintT(\"must not print\")",
            "RandomElement(1..100)",
        )),
        lower(&source(
            "",
            "IF PrintT(\"must not print\") THEN RandomElement(1..100) ELSE 0",
        )),
        lower(
            &source("", "RandomElement(1..100)")
                .replace("x = 0", "x = IF PrintT(\"must not print\") THEN 0 ELSE 1"),
        ),
    ];
    // A public native override can carry RNG authority without a random node
    // in its user body. The guard must inspect its typed call reference too.
    let native = lower(&source(
        "Alias(S) == CHOOSE a \\in S : TRUE",
        "Alias(1..100)",
    ));
    let id = native
        .defs()
        .iter()
        .position(|def| def.name == "Alias")
        .expect("Alias");
    let mut json = serde_json::to_value(native.as_ref()).expect("IR JSON");
    json["defs"][id]["std"] =
        serde_json::to_value(dive_ir::StdOp::TlcRandomElement).expect("StdOp JSON");
    let native: Arc<Ir> = Arc::new(serde_json::from_value(json).expect("native alias IR"));
    dive_eval::validate_ir(&native).expect("typed native alias is valid IR");
    variants.push(native);
    variants
}

#[test]
fn transformed_randomness_refuses_before_any_binding_or_search() {
    let variants = variants();
    for (index, ir) in variants.iter().enumerate() {
        let cfg = config(if index == 4 {
            "CONSTANT C <- Replacement\nVIEW V\n"
        } else {
            "VIEW V\n"
        });
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
                        let result = dive_check::check(ir, &cfg, &opts);
                        assert!(
                            matches!(&result.verdict, Verdict::Unsupported {feature} if feature == REFUSAL),
                            "variant {index}: {result:?}"
                        );
                        assert_eq!(result.workers.used, 0);
                        assert_eq!(result.workers.requested, width);
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
}

#[test]
fn pure_view_and_untransformed_randomness_still_check_concretely() {
    for (ir, cfg) in [
        (
            lower(&source("", "n").replace(", Randomization", "")),
            config("VIEW V\n"),
        ),
        (lower(&source("", "RandomElement(1..100)")), config("")),
    ] {
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
                        let result = dive_check::check(&ir, &cfg, &opts);
                        assert!(
                            matches!(&result.verdict,Verdict::InvariantViolated {property, ..} if property == "Inv"),
                            "{result:?}"
                        );
                        let stats = result.stats.as_ref().expect("search evidence");
                        assert_eq!((stats.generated, stats.distinct, stats.depth), (4, 4, 4));
                        assert_eq!(
                            result
                                .counterexample
                                .as_ref()
                                .expect("concrete witness")
                                .states
                                .len(),
                            4
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn configured_native_targets_refuse_without_source_calls_or_effects() {
    let text = source(
        "CONSTANT C(_)\nASSUME PrintT(\"must not print\")",
        "C(1..100)",
    )
    .replace(", Randomization", "");
    let plain = lower(&text);
    let instance = lower(&text.replace("CONSTANT C(_)", "CONSTANT C(_)\nI == INSTANCE TLC"));
    let custom = lower(&text.replace(
        "CONSTANT C(_)",
        "CONSTANT C(_)\nReplacement(S) == CHOOSE a \\in S : TRUE",
    ));
    let id = custom
        .defs()
        .iter()
        .position(|d| d.name == "Replacement")
        .expect("replacement");
    let mut value = serde_json::to_value(custom.as_ref()).expect("IR JSON");
    value["defs"][id]["std"] =
        serde_json::to_value(dive_ir::StdOp::TlcRandomElement).expect("native op");
    let custom: Arc<Ir> = Arc::new(serde_json::from_value(value).expect("typed native override"));
    dive_eval::validate_ir(&custom).expect("admitted custom authority");
    for (ir, target) in [
        (plain, "RandomElement"),
        (instance, "I!RandomElement"),
        (custom, "Replacement"),
    ] {
        let mut cfg = config("CONSTANT C <- RandomElement\nVIEW V\n");
        // Public typed cfg names can designate an instantiated context alias;
        // the textual cfg parser deliberately retains TLC's token boundaries.
        let dive_config::ConstantEntry::Substitution { target: rhs, .. } = &mut cfg.constants[0]
        else {
            panic!("substitution")
        };
        rhs.text = target.into();
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
                        let result = dive_check::check(&ir, &cfg, &opts);
                        assert!(
                            matches!(&result.verdict, Verdict::Unsupported { feature } if feature == REFUSAL),
                            "{target}: {result:?}"
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
}
