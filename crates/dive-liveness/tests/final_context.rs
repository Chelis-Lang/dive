//! Public property lowerers reject foreign configured context before hooks.
//! Core's final checks of this module are in
//! `crates/dive-api/tests/liveness_final_context.rs` (#791).
#![allow(clippy::expect_used, clippy::panic, reason = "boundary assertions")]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use dive_runtime::CheckOptions;

const CONFIGURED: &str = "---- MODULE ContextGuard ----\nEXTENDS Sequences\nCONSTANT Op(_)\nVARIABLE x\nInit==x=0\nNext==x'=x\nLength(v)==Len(v)\nP==<>(Op(<<1>>)=1)\n====\n";

fn bound(source: &str, configured: bool, options: &CheckOptions) -> dive_model::Model {
    bound_to(source, configured.then_some("Length"), options)
}

fn bound_to(source: &str, target: Option<&str>, options: &CheckOptions) -> dive_model::Model {
    let analysis =
        dive_sema::analyze_source("ContextGuard.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let cfg = format!(
        "INIT Init\nNEXT Next\nPROPERTY P\n{}",
        target.map_or_else(String::new, |name| format!("CONSTANT Op <- {name}\n"))
    );
    dive_model::bind(
        Arc::new(dive_ir::lower(&analysis).expect("IR")),
        &dive_config::parse(&cfg).into_config().expect("config"),
        &dive_model::BindOptions {
            eval: options.eval,
            symmetry_liveness: options.symmetry_liveness,
            no_deadlock: false,
            host: None,
        },
    )
    .expect("model")
}

#[derive(Debug)]
struct Hooks {
    calls: AtomicUsize,
    deny: bool,
}
impl dive_eval::Host for Hooks {
    fn check_native_access(&self, _: dive_ir::StdOp) -> Result<(), dive_eval::HostError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.deny {
            Err(dive_eval::HostError {
                code: 1000,
                message: "valid-model-native-hook-error".into(),
            })
        } else {
            Ok(())
        }
    }
}
fn install(model: &dive_model::Model, deny: bool) -> Arc<Hooks> {
    let host = Arc::new(Hooks {
        calls: AtomicUsize::new(0),
        deny,
    });
    model.evaluator.set_host(host.clone());
    host
}

#[test]
fn public_lowerers_reject_foreign_configured_context_before_hooks() {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        for mutation in ["names", "ir", "scope"] {
            let mut model = bound(CONFIGURED, true, &options);
            let host = install(&model, true);
            let mut property = model.property_claims()[0].pred.clone();
            match mutation {
                "names" => model.names = Arc::new(dive_value::Interner::new()),
                "ir" => model.ir = bound(CONFIGURED, true, &options).ir,
                "scope" => {
                    property.scope = bound(CONFIGURED, true, &options).property_claims()[0]
                        .pred
                        .scope
                        .clone();
                }
                _ => unreachable!(),
            }
            assert!(
                dive_liveness::plan::lower(
                    &model,
                    &property,
                    dive_liveness::formula::Limits::default()
                )
                .is_err()
            );
            assert!(
                dive_liveness::plan::lower_violation(
                    &model,
                    &property,
                    dive_liveness::formula::Limits::default()
                )
                .is_err()
            );
            assert_eq!(host.calls.load(Ordering::SeqCst), 0, "{mutation}");
        }
    }
}
