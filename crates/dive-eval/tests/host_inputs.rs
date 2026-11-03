//! Category admission precedes all host callbacks and argument evaluation.
#![allow(
    clippy::expect_used,
    reason = "finite source fixtures and strict assertions"
)]

use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use dive_eval::inputs::{
    HostAccess, InputRecord, InputReplay, InputRequest, InputResponse, MAX_INPUT_BYTES,
    MAX_INPUT_RECORDS, RecordedInputs,
};
use dive_eval::{ErrorClass, Evaluator, Host, HostError, Model, Options, St};

#[derive(Debug, Default)]
struct Spy {
    hooks: AtomicUsize,
    inputs: AtomicUsize,
}
impl Host for Spy {
    fn check_native_access(&self, _: dive_ir::StdOp) -> Result<(), HostError> {
        self.hooks.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    fn input(&self, request: &InputRequest) -> Option<Result<InputResponse, HostError>> {
        self.inputs.fetch_add(1, Ordering::Relaxed);
        Some(Ok(match request {
            InputRequest::Environment => {
                InputResponse::Environment([("DIVE_LABEL".into(), "outside".into())].into())
            }
            InputRequest::TextFile(path) => {
                assert_eq!(path, "input.txt");
                InputResponse::Text("hello\n".into())
            }
        }))
    }
}

fn evaluator(options: Options, host: Arc<dyn Host>) -> Evaluator {
    let source = "---- MODULE Host ----\nEXTENDS IOUtils, TLC\nKnownKey==\"DIVE_LABEL\"\nKnownValue==\"outside\"\nEnv==IOEnv\nRead==Deserialize(\"input.txt\",[format|->\"TXT\",charset|->\"UTF-8\"])\nUnsupportedOptions==Deserialize(\"input.txt\",[format|->\"JSON\",charset|->\"UTF-8\"])\nFunctionOptions==Deserialize(\"input.txt\",[name \\in {\"format\",\"charset\"} |-> IF name=\"format\" THEN \"TXT\" ELSE \"UTF-8\"])\nDenied==Deserialize(Assert(FALSE,\"must not evaluate path\"),[format|->\"TXT\",charset|->\"UTF-8\"])\n====\n";
    let analysis = dive_sema::analyze_source(
        "Host.tla",
        source,
        &dive_sema::Options {
            search: dive_sema::SearchPath {
                library_paths: vec![
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("tests/fixtures/community-modules"),
                ],
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let evaluator = Evaluator::new(Model::new(ir), options).expect("evaluator");
    evaluator.set_host(host);
    evaluator
}

#[test]
fn empty_or_unrelated_grants_refuse_before_callbacks_in_both_profiles() {
    for options in [Options::tlc(), Options::dive()] {
        assert_eq!(options.host_access, HostAccess::NONE);
        for (grants, name) in [
            (HostAccess::NONE, "Env"),
            (HostAccess::NONE, "Read"),
            (HostAccess::NONE, "Denied"),
            (HostAccess::parse("exec").expect("exec"), "Env"),
            (HostAccess::parse("env").expect("env"), "Read"),
        ] {
            let host = Arc::new(Spy::default());
            let ev = evaluator(
                Options {
                    host_access: grants,
                    ..options
                },
                host.clone(),
            );
            let error = ev
                .eval_def(ev.ir().root_def(name).expect("definition"), &St::constant())
                .expect_err("category refusal");
            assert_eq!(error.class(), ErrorClass::Unsupported);
            assert!(error.message().contains("--allow-io="), "{error}");
            assert_eq!(host.hooks.load(Ordering::Relaxed), 0);
            assert_eq!(host.inputs.load(Ordering::Relaxed), 0);
        }
    }
}

#[test]
fn admitted_operations_use_real_arguments_and_typed_input_kernels() {
    for options in [Options::tlc(), Options::dive()] {
        let host = Arc::new(Spy::default());
        let ev = evaluator(
            Options {
                host_access: HostAccess::parse("env,files").expect("grants"),
                ..options
            },
            host.clone(),
        );
        for name in ["Env", "Read"] {
            let value = ev
                .eval_def(ev.ir().root_def(name).expect("definition"), &St::constant())
                .expect("source input");
            assert!(matches!(value, dive_value::Value::Fcn(_)));
        }
        assert_eq!(host.hooks.load(Ordering::Relaxed), 2);
        assert_eq!(host.inputs.load(Ordering::Relaxed), 2);
    }
}

#[test]
fn unsupported_serializer_options_never_reach_the_input_provider() {
    for options in [Options::tlc(), Options::dive()] {
        for name in ["UnsupportedOptions", "FunctionOptions"] {
            let host = Arc::new(Spy::default());
            let ev = evaluator(
                Options {
                    host_access: HostAccess::parse("files").expect("grant"),
                    ..options
                },
                host.clone(),
            );
            let error = ev
                .eval_def(ev.ir().root_def(name).expect("definition"), &St::constant())
                .expect_err("unsupported options");
            assert_eq!(error.class(), ErrorClass::Unsupported);
            assert!(ev.external_input_refusal().is_some());
            assert_eq!(host.hooks.load(Ordering::Relaxed), 1);
            assert_eq!(host.inputs.load(Ordering::Relaxed), 0);
        }
    }
}

#[derive(Debug)]
struct EmptyEnvironment;
impl Host for EmptyEnvironment {
    fn input(&self, request: &InputRequest) -> Option<Result<InputResponse, HostError>> {
        assert_eq!(request, &InputRequest::Environment);
        Some(Ok(InputResponse::Environment(Default::default())))
    }
}

#[test]
fn empty_environment_does_not_approximate_a_record_as_an_empty_tuple() {
    for options in [Options::tlc(), Options::dive()] {
        let ev = evaluator(
            Options {
                host_access: HostAccess::parse("env").expect("grant"),
                ..options
            },
            Arc::new(EmptyEnvironment),
        );
        let error = ev
            .eval_def(
                ev.ir().root_def("Env").expect("definition"),
                &St::constant(),
            )
            .expect_err("empty record representation");
        assert_eq!(error.class(), ErrorClass::Unsupported);
        assert!(error.message().contains("empty RecordValue"));
    }
}

#[test]
fn replay_bounds_and_environment_consistency_refuse_before_any_source_execution() {
    let record = InputRecord {
        request: InputRequest::TextFile("input.txt".into()),
        response: InputResponse::Text("hello".into()),
    };
    let grants = HostAccess::parse("env,files").expect("grants");
    assert!(
        InputReplay::new(RecordedInputs {
            grants,
            records: vec![record.clone(); MAX_INPUT_RECORDS + 1]
        })
        .is_err()
    );
    assert!(
        InputReplay::new(RecordedInputs {
            grants,
            records: vec![InputRecord {
                response: InputResponse::Text("x".repeat(MAX_INPUT_BYTES + 1)),
                ..record
            }]
        })
        .is_err()
    );
    let env = |value: &str| InputRecord {
        request: InputRequest::Environment,
        response: InputResponse::Environment([("KEY".into(), value.into())].into()),
    };
    assert!(
        InputReplay::new(RecordedInputs {
            grants,
            records: vec![env("one"), env("two")]
        })
        .is_err()
    );
    assert!(
        InputReplay::new(RecordedInputs {
            grants: HostAccess::NONE,
            records: vec![env("one")]
        })
        .is_err()
    );
}
