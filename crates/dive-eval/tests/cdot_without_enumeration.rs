//! Action composition never pulls the checker enumerator into independent replay.
#![cfg(not(feature = "enumerate"))]
#![allow(clippy::expect_used, reason = "test fixture construction")]

use std::sync::Arc;

use dive_eval::{ActionComposition, Ec, ErrorClass, Evaluator, Model, Options, St};

#[test]
fn composition_without_enumeration_has_an_explicit_mode_aware_error() {
    let source = "---- MODULE CdotCore ----\nVARIABLE x\nP == (x' = 0) \\cdot (x' = 0)\n====\n";
    let analysis =
        dive_sema::analyze_source("CdotCore.tla", source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let property = ir.root_def("P").expect("P");
    for (mode, class) in [
        (ActionComposition::Off, ErrorClass::Eval),
        (ActionComposition::Tlc, ErrorClass::Unsupported),
        (ActionComposition::Tla, ErrorClass::Unsupported),
    ] {
        let evaluator = Evaluator::new(
            Model::new(ir.clone()),
            Options {
                action_composition: mode,
                ..Options::tlc()
            },
        )
        .expect("evaluator");
        let error = evaluator
            .eval_def(
                property,
                &St::action(Arc::from([Some(0.into())]), Arc::from([Some(0.into())])),
            )
            .expect_err("composition needs independent intermediate-state support");
        assert_eq!(error.class(), class);
        if mode == ActionComposition::Off {
            assert_eq!(error.ec(), Ec::General);
            assert!(
                error
                    .message()
                    .contains("does not support action composition")
            );
        } else {
            assert!(error.message().contains("without the successor enumerator"));
        }
        assert_eq!(evaluator.take_output(), Vec::<String>::new());
    }
}
