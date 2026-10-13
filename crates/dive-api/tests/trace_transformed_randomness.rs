//! Transformed randomness refuses before real private or API startup effects.
#![allow(clippy::expect_used, reason = "test fixture construction")]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_eval::{Host, HostError, Options};
use serde_json::json;

#[derive(Debug, Default)]
struct StartupCalls(AtomicUsize);

impl Host for StartupCalls {
    fn check_native_access(&self, _op: dive_ir::StdOp) -> Result<(), HostError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn check_register_access(&self) -> Result<(), HostError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

fn source(init: &str, extra: &str, transform: &str) -> String {
    format!(
        "---- MODULE Transform ----\nEXTENDS Naturals, TLC\nVARIABLE x\n\
         {extra}\nWrite == TLCSet(1,42)\nASSUME Write\n\
         Init == {init}\nNext == x'=x\nV == {transform}\n====\n"
    )
}

#[test]
fn all_source_and_config_routes_refuse_before_startup_callbacks() {
    let mut controls = 0;
    for compat_tlc in [false, true] {
        for (role, transform, feature) in [
            ("VIEW", "x", "randomness under VIEW is not supported"),
            (
                "SYMMETRY",
                "{[i \\in {0} |-> i]}",
                "randomness under SYMMETRY is not supported",
            ),
        ] {
            for (init, extra, constants) in [
                ("x=RandomElement({0})", "", ""),
                ("x=IF FALSE THEN RandomElement({0}) ELSE 0", "", ""),
                ("x=0", "Unused == RandomElement({0})", ""),
                (
                    "x=Draw({0})",
                    "CONSTANT Draw(_)",
                    "CONSTANT Draw <- RandomElement\n",
                ),
                (
                    "x=0",
                    "CONSTANT Draw(_)",
                    "CONSTANT Draw <- RandomElement\n",
                ),
            ] {
                let source = source(init, extra, transform);
                let cfg = format!("{constants}INIT Init\nNEXT Next\n{role} V\n");
                let opts = TraceOptions {
                    compat_tlc,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                };
                let analysis = dive_sema::analyze_source(
                    "Transform.tla",
                    &source,
                    &dive_sema::Options::default(),
                );
                assert!(
                    analysis.accepted(),
                    "{source}: {:?}",
                    analysis.diagnostics()
                );
                let error = SpecModel::from_source("Transform.tla", &source, &cfg, &opts)
                    .expect_err("transform and randomness must refuse before binding");
                assert!(
                    matches!(error, dive_api::ApiError::Unsupported { feature: ref f } if f == feature),
                    "{error}"
                );

                let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
                let cfg = dive_config::parse(&cfg).into_config().expect("config");
                let eval = if compat_tlc {
                    Options::tlc()
                } else {
                    Options::dive()
                };
                let recipe = dive_model::Recipe::new(ir, cfg, eval, false);
                let calls = Arc::new(StartupCalls::default());
                let failure = recipe.bind(calls.clone()).expect_err("no private startup");
                assert!(
                    matches!(failure.error.kind, dive_model::ErrorKind::Unsupported { feature: ref f } if f == feature)
                );
                assert_eq!(failure.output, [] as [dive_model::Message; 0]);
                assert_eq!(calls.0.load(Ordering::Relaxed), 0);

                let doc = TraceDoc::behavior(vec![json!({"x":0})]).expect("document");
                let (report, transport) = dive_trace::Session::new(recipe, None).read(&doc);
                assert_eq!(
                    report.validation,
                    Validation::Unsupported(format!("unsupported: {feature}"))
                );
                assert_eq!(report.notes, [] as [String; 0]);
                assert!(transport.is_none());
                controls += 1;
            }
        }
    }
    assert_eq!(controls, 20);
}

#[test]
fn pure_view_keeps_actual_startup_register_execution() {
    let source = source("x=0 /\\ TLCGet(1)=42", "", "x");
    let config = "INIT Init\nNEXT Next\nVIEW V\n";
    for compat_tlc in [false, true] {
        let analysis =
            dive_sema::analyze_source("Transform.tla", &source, &dive_sema::Options::default());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let cfg = dive_config::parse(config).into_config().expect("config");
        let eval = if compat_tlc {
            Options::tlc()
        } else {
            Options::dive()
        };
        let calls = Arc::new(StartupCalls::default());
        let bound = dive_model::Recipe::new(ir, cfg, eval, false)
            .bind(calls.clone())
            .expect("ordinary startup remains available");
        assert!(calls.0.load(Ordering::Relaxed) > 0);
        assert_eq!(bound.output, [] as [dive_model::Message; 0]);
        let model = SpecModel::from_source(
            "Transform.tla",
            &source,
            config,
            &TraceOptions {
                compat_tlc,
                ..TraceOptions::default()
            },
        )
        .expect("pure VIEW with startup register");
        let doc = TraceDoc::behavior(vec![json!({"x":0})]).expect("document");
        for _ in 0..2 {
            assert_eq!(model.validate(&doc).validation, Validation::Valid);
        }
        let altered = TraceDoc::behavior(vec![json!({"x":1})]).expect("altered document");
        assert!(matches!(
            model.validate(&altered).validation,
            Validation::Invalid(_)
        ));
    }
}
