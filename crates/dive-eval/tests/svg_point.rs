//! Bounded observable `PointOnLine` controls; compact pinned inputs are consumed.
#![allow(clippy::expect_used, clippy::panic, reason = "functional assertions")]
use dive_eval::{AssumptionOutcome, Ec, ErrorClass, Evaluator, Model, Options};
use dive_ir::{DefId, StdOp};
use std::{fmt::Write, path::PathBuf, sync::Arc};

fn run(body: &str, opts: Options, fallback: bool) -> (Vec<String>, AssumptionOutcome) {
    let source = format!(
        "---- MODULE M ----\nEXTENDS Integers,TLC,SVG\nFallback(a,b,c)==[x |-> 123,y |-> 456]\n{body}\n====\n"
    );
    let mut sema = dive_sema::Options::default();
    sema.search.after_standard =
        vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/svg-point")];
    let analysis = dive_sema::analyze_source("M.tla", &source, &sema);
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let mut model = Model::new(ir.clone());
    if fallback {
        let id = ir
            .defs()
            .iter()
            .position(|d| d.std == Some(StdOp::SvgPointOnLine))
            .expect("registered override");
        model.redirects.insert(
            DefId(u32::try_from(id).expect("id")),
            ir.root_def("Fallback").expect("fallback"),
        );
    }
    let evaluator = Evaluator::new(model, opts).expect("valid model");
    let result = evaluator.check_assumptions(|_| false);
    (evaluator.take_output(), result)
}

#[test]
fn pinned_point_results_and_representation_refusals_in_both_profiles() {
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/svg-point/inputs.json")).expect("inputs");
    for row in rows.as_array().expect("rows") {
        let args = row["args"]
            .as_array()
            .expect("args")
            .iter()
            .map(|x| x.as_str().expect("expression"))
            .collect::<Vec<_>>()
            .join(",");
        for opts in [Options::tlc(), Options::dive()] {
            let body = if let Some(expected) = row.get("expected") {
                format!(
                    "ASSUME PointOnLine({args}) = [x |-> {}, y |-> {}]",
                    expected["x"], expected["y"]
                )
            } else {
                format!("ASSUME PointOnLine({args}) = [x |-> 0,y |-> 0]")
            };
            let body = body.replace("-2147483648", "(-2147483647 - 1)");
            let (_, result) = run(&body, opts, false);
            if row.get("expected").is_some() {
                assert!(
                    matches!(result, AssumptionOutcome::AllTrue),
                    "{row}: {result:?}"
                );
            } else if row["name"] == "ArgumentError" {
                assert!(
                    matches!(&result,AssumptionOutcome::Error{error,..} if error.ec()==Ec::DivisionByZero),
                    "{result:?}"
                );
            } else {
                assert!(
                    matches!(&result,AssumptionOutcome::Error{error,..} if error.class()==ErrorClass::Unsupported && error.to_string().contains("SVG!PointOnLine")),
                    "{result:?}"
                );
            }
        }
    }
}

#[test]
fn deferred_inputs_do_not_materialize_and_later_argument_errors_remain_authoritative() {
    for opts in [Options::tlc(), Options::dive()] {
        let (_, result) = run(
            "ASSUME PointOnLine([k \\in Int |-> 0],[x |-> 0,y |-> 0],1) = [x |-> 0,y |-> 0]",
            opts,
            false,
        );
        assert!(
            matches!(&result,AssumptionOutcome::Error{error,..} if error.class()==ErrorClass::Unsupported && error.to_string().contains("SVG!PointOnLine")),
            "{result:?}"
        );
        let (_, wide) = run(
            "ASSUME PointOnLine([x |-> 0,y |-> 0],[x |-> 0,y |-> 0],2147483648) = [x |-> 0,y |-> 0]",
            opts,
            false,
        );
        assert!(
            matches!(&wide, AssumptionOutcome::Error{error,..} if if opts.is_tlc() {error.ec()==Ec::IntegerTooBig} else {error.class()==ErrorClass::Unsupported}),
            "wide argument retains prior evaluator boundary: {wide:?}"
        );
        let (_, result) = run(
            "ASSUME PointOnLine([k \\in Int |-> 0],[x |-> 0,y |-> 0],1 \\div 0) = [x |-> 0,y |-> 0]",
            opts,
            false,
        );
        assert!(
            matches!(&result,AssumptionOutcome::Error{error,..} if error.ec()==Ec::DivisionByZero),
            "{result:?}"
        );
        let (output, result) = run(
            "ASSUME PointOnLine(Print(17,<<>>),[x |-> 0,y |-> 0],1) = [x |-> 0,y |-> 0]",
            opts,
            false,
        );
        assert!(
            matches!(&result,AssumptionOutcome::Error{error,..} if error.class()==ErrorClass::Unsupported),
            "{result:?}"
        );
        assert_eq!(output, ["17  <<>>"]);
    }
}

#[test]
fn configured_tla_override_keeps_priority() {
    for opts in [Options::tlc(), Options::dive()] {
        let (_, result) = run(
            "ASSUME PointOnLine(<<>>,<<>>,0) = [x |-> 123,y |-> 456]",
            opts,
            true,
        );
        assert!(matches!(result, AssumptionOutcome::AllTrue), "{result:?}");
    }
}

#[test]
fn bounded_records_and_evaluation_depth_keep_resource_errors() {
    for opts in [Options::tlc(), Options::dive()] {
        let point = |n| {
            let mut point = "[x |-> 0,y |-> 0".to_owned();
            for i in 0..n {
                write!(point, ",f{i} |-> 0").expect("String writing");
            }
            point.push(']');
            point
        };
        let (_, positive) = run(
            &format!(
                "ASSUME PointOnLine({},[x |-> 1,y |-> 1],1) = [x |-> 1,y |-> 1]",
                point(254)
            ),
            opts,
            false,
        );
        assert!(
            matches!(positive, AssumptionOutcome::AllTrue),
            "{positive:?}"
        );
        let (_, negative) = run(
            &format!(
                "ASSUME PointOnLine({},[x |-> 1,y |-> 1],1) = [x |-> 1,y |-> 1]",
                point(255)
            ),
            opts,
            false,
        );
        assert!(
            matches!(&negative,AssumptionOutcome::Error{error,..} if error.class()==ErrorClass::ResourceLimit),
            "{negative:?}"
        );
        let body = "ASSUME PointOnLine([x |-> 0,y |-> 0],[x |-> 1,y |-> 1],1) = [x |-> 1,y |-> 1]";
        let (_, negative) = run(
            body,
            Options {
                max_depth: 0,
                ..opts
            },
            false,
        );
        assert!(
            matches!(&negative,AssumptionOutcome::Error{error,..} if error.class()==ErrorClass::ResourceLimit),
            "{negative:?}"
        );
        let (_, positive) = run(
            body,
            Options {
                max_depth: 2,
                ..opts
            },
            false,
        );
        assert!(
            matches!(positive, AssumptionOutcome::AllTrue),
            "{positive:?}"
        );
    }
}
