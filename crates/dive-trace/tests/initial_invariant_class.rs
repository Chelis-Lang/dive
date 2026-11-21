//! Named cause validity alone grants no diagnostic receiver wrapper identity.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use dive_eval::{HostControl, Interrupted, Options};
use dive_model::{BindOptions, Recipe};
use dive_trace::{Ending, Session, Trace, TraceDoc, Validation, ValidationRunError};
use serde_json::json;

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct Input {
    ir: Arc<dive_ir::Ir>,
    config: dive_config::ModelConfig,
}

impl Input {
    fn new(body: &str, config: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let source = format!(
            "---- MODULE ReceiverClass ----\nEXTENDS Naturals,TLC,FiniteSets\nVARIABLE x\n{body}\n====\n"
        );
        let analysis =
            dive_sema::analyze_source("ReceiverClass.tla", &source, &dive_sema::Options::default());
        assert!(analysis.accepted(), "{analysis:?}");
        Ok(Self {
            ir: Arc::new(dive_ir::lower(&analysis)?),
            config: dive_config::parse(config)
                .into_config()
                .ok_or("configuration")?,
        })
    }
    fn session(&self) -> Session {
        Session::new(
            Recipe::new(self.ir.clone(), self.config.clone(), Options::tlc(), false),
            None,
        )
    }
    fn document(&self, name: &str) -> Result<TraceDoc, Box<dyn std::error::Error>> {
        let model = dive_model::bind(self.ir.clone(), &self.config, &BindOptions::tlc())?;
        let mut state = None;
        model
            .evaluator
            .init_states(&model.init_actions, &mut |candidate| {
                state = Some(Arc::from(candidate.state));
                dive_eval::next::Flow::Stop
            })
            .map_err(|failure| failure.error)?;
        let trace = Trace {
            states: vec![state.ok_or("first initial state")?],
            labels: vec![None],
            ending: Ending::InvariantError {
                name: name.into(),
                ec: "GENERAL".into(),
            },
            length: None,
            properties: Vec::new(),
            concrete_closure: false,
            witnesses: Vec::new(),
        };
        Ok(trace.to_doc(&model)?)
    }
}

#[test]
fn original_eight_initial_words_get_independent_class_evidence() -> TestResult {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/comparison-receiver-primary/specifications");
    for name in [
        "InitialLiteral",
        "Issue620",
        "InitialStateLeft",
        "InitialStateRight",
        "InitialTupleNested",
        "RecordLiteral",
        "LetTuple",
        "InstanceTuple",
    ] {
        let file = root.join(name).join(format!("{name}.tla"));
        let analysis = dive_sema::analyze_file(&file, &dive_sema::Options::default())?;
        assert!(analysis.accepted());
        let input = Input {
            ir: Arc::new(dive_ir::lower(&analysis)?),
            config: dive_config::parse(&std::fs::read_to_string(file.with_extension("cfg"))?)
                .into_config()
                .ok_or("cfg")?,
        };
        let doc = input.document("Inv")?;
        assert_eq!(doc.to_json()["version"], 2);
        assert_eq!(
            doc.to_json()["ending"],
            json!({"kind":"invariant-error","name":"Inv","ec":"GENERAL"})
        );
        let fresh = input.session();
        assert_eq!(
            fresh.initial_error_stage(None)?.1,
            Some(dive_trace::InitialErrorStage::InvariantReceiver)
        );
        assert_eq!(
            fresh.try_read(&doc)?.0.validation,
            Validation::Valid,
            "{name}"
        );
        for _ in 0..2 {
            let (report, class) = fresh.initial_invariant_class(&doc, None)?;
            assert_eq!(report.validation, Validation::Valid, "{name}: {report:?}");
            let class = class.ok_or("missing independently reconstructed wrapper")?;
            assert_eq!((class.reported_code(), class.deciding_code()), (2102, 2147));
        }
    }
    Ok(())
}

#[test]
fn actual_no_word_generation_and_function_routes_reconstruct_diagnostic_catches() -> TestResult {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/corpus");
    for (corpus, name, expected) in [
        (
            "comparison-receiver-primary",
            "InitGeneration",
            dive_trace::InitialErrorStage::Generation,
        ),
        (
            "constant-retry-lazy",
            "WholeFunctionError",
            dive_trace::InitialErrorStage::LegacyInvariant,
        ),
    ] {
        let file = root
            .join(corpus)
            .join("specifications")
            .join(name)
            .join(format!("{name}.tla"));
        let analysis = dive_sema::analyze_file(&file, &dive_sema::Options::default())?;
        let input = Input {
            ir: Arc::new(dive_ir::lower(&analysis)?),
            config: dive_config::parse(&std::fs::read_to_string(file.with_extension("cfg"))?)
                .into_config()
                .ok_or("cfg")?,
        };
        let (report, stage) = input.session().initial_error_stage(None)?;
        assert_eq!(report.validation, Validation::Valid, "{name}: {report:?}");
        assert_eq!(stage, Some(expected));
        let count = Arc::new(Control {
            calls: AtomicUsize::new(0),
            stop: usize::MAX,
        });
        assert_eq!(
            input.session().initial_error_stage(Some(count.clone()))?.1,
            Some(expected)
        );
        let total = count.calls.load(Ordering::Relaxed);
        for stop in [total / 2, total] {
            let error = input
                .session()
                .initial_error_stage(Some(Arc::new(Control {
                    calls: AtomicUsize::new(0),
                    stop,
                })))
                .err()
                .ok_or("typed routing stop")?;
            assert!(matches!(error, ValidationRunError::Interrupted { .. }));
        }
    }
    Ok(())
}

#[test]
fn wrong_claims_and_valid_words_without_source_lifetime_get_no_wrapper() -> TestResult {
    let input = Input::new(
        "Init == x = 0\nNext == UNCHANGED x\nInv == <<x>> = TRUE",
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    )?;
    let doc = input.document("Inv")?;
    for (pointer, value) in [
        ("/ending/name", json!("Missing")),
        ("/ending/ec", json!("TLC_NESTED_EXPRESSION")),
        ("/states/0/values/x", json!(1)),
    ] {
        let mut wrong = doc.to_json();
        *wrong.pointer_mut(pointer).ok_or("field")? = value;
        let (report, class) = input
            .session()
            .initial_invariant_class(&TraceDoc::from_json(&wrong)?, None)?;
        assert!(!report.validation.is_valid());
        assert!(class.is_none());
    }
    for (body, config, state) in [
        (
            "Init == x \\in {0,1}\nNext == UNCHANGED x\nInv == IF x = 0 THEN TRUE ELSE <<x>> = TRUE",
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
            json!(1),
        ),
        (
            "Init == x = 0\nNext == UNCHANGED x\nEarlier == x >= 0\nInv == <<x>> = TRUE",
            "INIT Init\nNEXT Next\nINVARIANTS Earlier Inv\n",
            json!(0),
        ),
        (
            "C == <<1>>\nInit == x = 0\nNext == UNCHANGED x\nInv == C = TRUE",
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
            json!(0),
        ),
        (
            "Init == x = {0}\nNext == UNCHANGED x\nInv == <<Cardinality(x)>> = TRUE",
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
            serde_json::Value::Null,
        ),
        (
            "Init == x = 0\nNext == UNCHANGED x\nC == x >= 0\nInv == <<x>> = TRUE",
            "INIT Init\nNEXT Next\nINVARIANT Inv\nCONSTRAINT C\n",
            json!(0),
        ),
        (
            "Init == x = 0\nNext == UNCHANGED x\nV == x\nInv == <<x>> = TRUE",
            "INIT Init\nNEXT Next\nINVARIANT Inv\nVIEW V\n",
            json!(0),
        ),
    ] {
        let input = Input::new(body, config)?;
        let mut doc = input.document("Inv")?;
        if !state.is_null() {
            doc.states[0].insert("x".into(), state);
        }
        assert_eq!(
            input.session().try_read(&doc)?.0.validation,
            Validation::Valid,
            "the underlying named word still validates: {body}"
        );
        let (_, class) = input.session().initial_invariant_class(&doc, None)?;
        assert!(class.is_none(), "{body}");
    }
    let mut longer = doc.clone();
    longer.states.push(longer.states[0].clone());
    longer.labels.push(None);
    assert!(
        input
            .session()
            .initial_invariant_class(&longer, None)?
            .1
            .is_none()
    );
    assert!(
        input
            .session()
            .initial_invariant_class(
                &doc.clone().with_ending(Ending::Error {
                    ec: Some("GENERAL".into())
                }),
                None
            )?
            .1
            .is_none()
    );
    Ok(())
}

#[derive(Debug)]
struct Control {
    calls: AtomicUsize,
    stop: usize,
}
impl HostControl for Control {
    fn check(&self) -> Result<(), Interrupted> {
        if self.calls.fetch_add(1, Ordering::Relaxed) + 1 >= self.stop {
            Err(Interrupted::new(988))
        } else {
            Ok(())
        }
    }
}

#[test]
fn actual_mid_and_final_control_stops_cannot_return_a_certificate() -> TestResult {
    let input = Input::new(
        "Init == x = 0\nNext == UNCHANGED x\nInv == <<x>> = TRUE",
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    )?;
    let doc = input.document("Inv")?;
    let count = Arc::new(Control {
        calls: AtomicUsize::new(0),
        stop: usize::MAX,
    });
    assert!(
        input
            .session()
            .initial_invariant_class(&doc, Some(count.clone()))?
            .1
            .is_some()
    );
    let total = count.calls.load(Ordering::Relaxed);
    for stop in [1, total / 2, total - 1, total] {
        let control = Arc::new(Control {
            calls: AtomicUsize::new(0),
            stop,
        });
        let error = input
            .session()
            .initial_invariant_class(&doc, Some(control))
            .err()
            .ok_or("typed interruption")?;
        let ValidationRunError::Interrupted { error, .. } = error else {
            return Err("stop lost original typed channel".into());
        };
        assert_eq!(error.interruption().ok_or("owner")?.token(), 988);
    }
    Ok(())
}
