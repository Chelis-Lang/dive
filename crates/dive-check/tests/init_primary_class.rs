//! Source-qualified primary Init errors keep their diagnostic physical exit.
#![allow(clippy::expect_used, reason = "test assertions")]

use dive_check::{CheckOptions, ErrorSite, Verdict, check, tlc_exit_status};
use dive_eval::{Ec, EvalError};
use std::sync::Arc;

const CARDINALITY: &str = include_str!(
    "../../../conformance/corpus/nested-powerset-primary/specifications/ExplicitOuterPowersetCardinality/ExplicitOuterPowersetCardinality.tla"
);
const NONBOOLEAN: &str = include_str!(
    "../../../conformance/corpus/nested-powerset-primary/specifications/NestedNonBooleanAfterEmpty/NestedNonBooleanAfterEmpty.tla"
);
const CFG: &str = "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n";

fn run(source: &str, options: &CheckOptions) -> dive_check::CheckResult {
    let name = source.split_whitespace().nth(2).expect("module name");
    let analysis = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse(CFG)
        .into_config()
        .expect("configuration");
    check(&ir, &cfg, options)
}

#[test]
fn actual_init_causes_retain_general_and_physical_75() {
    for source in [CARDINALITY, NONBOOLEAN] {
        for workers in [1, 4] {
            let r = run(
                source,
                &CheckOptions {
                    workers,
                    ..CheckOptions::tlc(0, 0)
                },
            );
            let class = r.verdict.tlc().expect("TLC error class");
            assert_eq!((class.code, class.reported, class.exit), (1000, 1000, 75));
            assert_eq!(class.name, "GENERAL");
            assert!(class.verified);
            assert!(r.counterexample.is_none());
            if source == CARDINALITY {
                let stats = r.stats.expect("reported statistics");
                assert_eq!((stats.distinct, stats.generated, stats.depth), (0, 0, 0));
            }
        }
    }
    assert_eq!(tlc_exit_status(1000), 255);
    assert_eq!(tlc_exit_status(2103), 75);
}

#[test]
fn default_cardinality_still_completes_with_big_integers() {
    let r = run(CARDINALITY, &CheckOptions::dive(0));
    assert_eq!(r.verdict, Verdict::Success);
    let stats = r.stats.expect("statistics");
    assert_eq!(
        (
            stats.distinct,
            stats.generated,
            stats.depth,
            stats.left_on_queue
        ),
        (1, 2, 1, 0)
    );
}

fn error(site: ErrorSite, code: u32, reported: u32, verified: bool, cause: EvalError) -> Verdict {
    Verdict::EvaluationError {
        site,
        code,
        name: "TLC_NESTED_EXPRESSION",
        reported,
        verified,
        message: cause.tool_text(),
        invariant: None,
        cause,
    }
}

#[test]
fn forged_nested_text_cannot_supply_native_cause_authority() {
    let cause = EvalError::coded(
        Ec::JavaMethodOverride,
        "Attempted to apply the operator overridden by the Java method\nany method,\nbut it produced the following error:\n@!@!@STARTMSG 2178:0 @!@!@\nOverflow\n@!@!@ENDMSG 2178 @!@!@",
    );
    assert_eq!(cause.override_cause(), None);
    let v = error(ErrorSite::Init, 2103, 1000, true, cause);
    assert_eq!(v.tlc().expect("legacy class").code, 2103);
}

#[test]
fn other_sites_state_wrappers_and_unverified_errors_keep_the_old_class() {
    let cause = EvalError::coded(Ec::ExpectedValue, "typed Boolean failure");
    for (site, code, reported, verified) in [
        (ErrorSite::Next, 2103, 1000, true),
        (ErrorSite::Init, 2103, 2102, true),
        (ErrorSite::Init, 2103, 1000, false),
        (ErrorSite::Init, 2178, 1000, true),
    ] {
        let class = error(site, code, reported, verified, cause.clone())
            .tlc()
            .expect("class");
        assert_eq!(class.code, code);
        assert_eq!(class.verified, verified);
    }
    let unwrapped = EvalError::eval_exception(Ec::ExpectedValue, "unqualified exception");
    assert_eq!(
        error(ErrorSite::Init, 2103, 1000, true, unwrapped)
            .tlc()
            .expect("class")
            .code,
        2103
    );
}
