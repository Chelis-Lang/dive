//! Error endpoint authority comes from fresh independent ordered replay.
use dive_eval::{HostControl, Interrupted, Options, St};
use dive_model::{BindOptions, Model, Recipe};
use dive_trace::{Ending, Session, TemporalStateEndpoint, Trace, TraceDoc, Validation};
use dive_value::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Input {
    ir: Arc<dive_ir::Ir>,
    config: dive_config::ModelConfig,
}
impl Input {
    fn new(body: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let text = format!(
            "---- MODULE Endpoint ----\nEXTENDS Naturals,Sequences,TLC\nVARIABLE x\n{body}\n====\n"
        );
        let source =
            dive_sema::analyze_source("Endpoint.tla", &text, &dive_sema::Options::default());
        assert!(source.accepted(), "{source:?}");
        Ok(Self {
            ir: Arc::new(dive_ir::lower(&source)?),
            config: dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
                .into_config()
                .ok_or("config")?,
        })
    }
    fn model(&self, options: Options) -> Result<Model, Box<dyn std::error::Error>> {
        Ok(dive_model::bind(
            self.ir.clone(),
            &self.config,
            &BindOptions {
                eval: options,
                ..BindOptions::tlc()
            },
        )?)
    }
    fn session(&self, options: Options) -> Session {
        Session::new(
            Recipe::new(self.ir.clone(), self.config.clone(), options, false),
            None,
        )
    }
    fn request(
        &self,
        options: Options,
        name: &str,
    ) -> Result<TraceDoc, Box<dyn std::error::Error>> {
        let model = self.model(options)?;
        let def = self.ir.root_def(name).ok_or("error definition")?;
        let error = model
            .evaluator
            .eval_def(def, &St::state(Arc::from([Some(Value::Int(0))])))
            .err()
            .ok_or("actual core error required")?;
        let endpoint =
            TemporalStateEndpoint::from_error(&model, &error).ok_or("inner source context")?;
        Ok(
            TraceDoc::behavior(vec![serde_json::json!({"x":0})])?.with_ending(
                Ending::TemporalStateError {
                    ec: error.ec().name().into(),
                    endpoint,
                },
            ),
        )
    }
}

#[test]
fn whole_ordered_program_reproduces_priority_and_refutes_mutated_claims() -> TestResult {
    let input = Input::new(
        "Init == x=0\nNext == x'=1-x\nTypeBad == Len(TRUE \\o <<x>>)=0\nDivBad == 1 \\div (x-x)=0\nP == (<>[]TypeBad) \\/ ([]<>DivBad)",
    )?;
    for options in [Options::dive(), Options::tlc()] {
        let doc = input.request(options, "DivBad")?;
        let session = input.session(options);
        assert!(
            matches!(session.read(&doc).0.validation,Validation::Unsupported(feature) if feature.contains("try_read"))
        );
        assert_eq!(session.try_read(&doc)?.0.validation, Validation::Valid);
        assert!(matches!(
            session
                .try_read(&input.request(options, "TypeBad")?)?
                .0
                .validation,
            Validation::Invalid(_)
        ));
        let mut wire = doc.to_json();
        wire["ending"]["endpoint"]["source"]["module"] = serde_json::json!("Wrong");
        assert!(matches!(
            session.try_read(&TraceDoc::from_json(&wire)?)?.0.validation,
            Validation::Invalid(_)
        ));
        let mut wire = doc.to_json();
        wire["states"][0]["values"]["x"] = serde_json::json!(1);
        assert!(matches!(
            session.try_read(&TraceDoc::from_json(&wire)?)?.0.validation,
            Validation::Invalid(_)
        ));
        let old = doc.clone().with_ending(Ending::Error {
            ec: Some("TLC_MODULE_DIVISION_BY_ZERO".into()),
        });
        assert!(matches!(
            session.try_read(&old)?.0.validation,
            Validation::Unsupported(_)
        ));
        let model = input.model(options)?;
        let trace = Trace::decode(&model, &doc).map_err(|error| format!("{error:?}"))?;
        assert!(matches!(
            dive_trace::validate(&model, &trace).validation,
            Validation::Unsupported(_)
        ));
    }
    Ok(())
}

#[test]
fn unused_operands_tautology_and_wrong_scope_never_select_a_target() -> TestResult {
    for formula in ["[]<>((x=0) \\/ Bad)", "([]<>Bad)=>TRUE"] {
        let input = Input::new(&format!(
            "Init == x=0\nNext == x'=1-x\nBad == 1 \\div (x-x)=0\nP == {formula}"
        ))?;
        let document = input.request(Options::tlc(), "Bad")?;
        assert_ne!(
            input
                .session(Options::tlc())
                .try_read(&document)?
                .0
                .validation,
            Validation::Valid
        );
    }
    let body = "Init == x=0\nNext == x'=1-x\n---- MODULE Inner ----\nEXTENDS Naturals\nVARIABLE C\nBad == IF C=0 THEN TRUE ELSE 1 \\div (x-x)=0\n====\nI == INSTANCE Inner WITH C <- x\nJ == INSTANCE Inner WITH C <- x+1\nP == ([]<>I!Bad) /\\ ([]<>J!Bad)";
    // The inner module uses only its own variable, so both instances retain
    // the same source body but different actual bindings.
    let body = body.replace("1 \\div (x-x)", "1 \\div (C-C)");
    let input = Input::new(&body)?;
    let request = input.request(Options::tlc(), "J!Bad")?;
    assert_eq!(
        input
            .session(Options::tlc())
            .try_read(&request)?
            .0
            .validation,
        Validation::Valid
    );
    // A separate same-expression scope mutation changes the actual first EC.
    let priority = "Init == x=0\nNext == x'=1-x\n---- MODULE Inner ----\nEXTENDS Naturals\nVARIABLE C\nBad == (1 \\div C)=0\n====\nI == INSTANCE Inner WITH C <- x\nJ == INSTANCE Inner WITH C <- (x=0)\nP == ([]<>I!Bad) /\\ ([]<>J!Bad)";
    let ordered = Input::new(priority)?;
    let original = ordered.request(Options::tlc(), "I!Bad")?;
    assert_eq!(
        ordered
            .session(Options::tlc())
            .try_read(&original)?
            .0
            .validation,
        Validation::Valid
    );
    let changed = Input::new(&priority.replace("C <- x\nJ", "C <- (x=0)\nJ").replace(
        "J == INSTANCE Inner WITH C <- (x=0)",
        "J == INSTANCE Inner WITH C <- x",
    ))?;
    let observed = changed.session(Options::tlc()).try_read(&original)?.0;
    assert!(
        matches!(&observed.validation, Validation::Invalid(_)),
        "changed-scope replay: {observed:?}"
    );
    let corrected = changed.request(Options::tlc(), "I!Bad")?;
    assert_eq!(
        changed
            .session(Options::tlc())
            .try_read(&corrected)?
            .0
            .validation,
        Validation::Valid
    );
    Ok(())
}

#[test]
fn descriptor_roundtrip_is_additive_and_unknown_fields_fail_closed() -> TestResult {
    let input = Input::new("Init == x=0\nNext == x'=1-x\nBad == 1 \\div (x-x)=0\nP == []<>Bad")?;
    let request = input.request(Options::tlc(), "Bad")?;
    let value = request.to_json();
    assert_eq!(value["version"], 1);
    assert_eq!(value["ending"]["kind"], "error");
    let decoded = TraceDoc::from_json(&value)?;
    assert_eq!(decoded.ending, request.ending);
    let model = input.model(Options::tlc())?;
    assert_eq!(
        Trace::decode(&model, &decoded)
            .map_err(|error| format!("{error:?}"))?
            .to_doc(&model)?
            .ending,
        request.ending
    );
    for key in ["stage", "unknown"] {
        let mut value = value.clone();
        value["ending"]["endpoint"][key] = serde_json::json!("unrecognized");
        assert!(TraceDoc::from_json(&value).is_err());
    }
    let mut value = value;
    value["ending"]["ec"] = serde_json::json!("GENERAL");
    assert!(TraceDoc::from_json(&value).is_err());
    Ok(())
}

#[derive(Debug)]
struct Stop(AtomicUsize);
impl HostControl for Stop {
    fn check(&self) -> Result<(), Interrupted> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(Interrupted::new(927))
    }
}
#[test]
fn private_controlled_reconstruction_preserves_typed_interruption() -> TestResult {
    let input = Input::new("Init == x=0\nNext == x'=1-x\nBad == 1 \\div (x-x)=0\nP == []<>Bad")?;
    let request = input.request(Options::tlc(), "Bad")?;
    let stop = Arc::new(Stop(AtomicUsize::new(0)));
    let error = input
        .session(Options::tlc())
        .read_controlled(&request, stop.clone())
        .err()
        .ok_or("typed stop")?;
    assert!(matches!(
        error,
        dive_trace::ValidationRunError::Interrupted { .. }
    ));
    assert_eq!(stop.0.load(Ordering::SeqCst), 1);
    Ok(())
}

#[derive(Debug)]
struct Provider(Arc<AtomicUsize>);
impl dive_value::DeferredSet for Provider {
    fn kind(&self) -> dive_value::DeferredKind {
        self.0.fetch_add(1, Ordering::SeqCst);
        dive_value::DeferredKind::Filter
    }
    fn elements(&self, _: dive_value::Ctx) -> dive_value::Result<Vec<Value>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(vec![Value::Int(0)])
    }
    fn member(&self, _: &Value, _: dive_value::Ctx) -> Option<dive_value::Result<bool>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Some(Ok(true))
    }
    fn key(&self) -> Vec<Value> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Vec::new()
    }
}
#[test]
fn public_borrowed_provider_cannot_acquire_endpoint_authority() -> TestResult {
    let input = Input::new("Init == x=0\nNext == x'=x\nBad == 1 \\div (x-x)=0\nP == []<>Bad")?;
    let request = input.request(Options::tlc(), "Bad")?;
    let model = input.model(Options::tlc())?;
    let mut trace = Trace::decode(&model, &request).map_err(|error| format!("{error:?}"))?;
    let calls = Arc::new(AtomicUsize::new(0));
    let value = Value::deferred(Arc::new(Provider(calls.clone())), Options::tlc().ctx);
    assert!(dive_value::tlc::member(
        &Value::Int(0),
        &value,
        Options::tlc().ctx
    )?);
    let before = calls.load(Ordering::SeqCst);
    assert!(before > 0);
    trace.states[0] = Arc::from([Some(value)]);
    assert!(matches!(
        dive_trace::validate(&model, &trace).validation,
        Validation::Unsupported(_)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), before);
    Ok(())
}

#[test]
fn finite_path_safety_and_initial_next_checks_precede_endpoint_replay() -> TestResult {
    let mut input = Input::new(
        "Init == x=0\nNext == x'=1-x\nInv == x=1\nBad == 1 \\div (x-x)=0\nP == []<>Bad",
    )?;
    let request = input.request(Options::tlc(), "Bad")?;
    input.config = dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY P\n")
        .into_config()
        .ok_or("config")?;
    assert!(
        matches!(input.session(Options::tlc()).try_read(&request)?.0.validation,Validation::Invalid(failure) if failure.check==dive_trace::Check::Invariant)
    );
    let input = Input::new("Init == x=0\nNext == x'=1-x\nBad == 1 \\div (x-x)=0\nP == []<>Bad")?;
    let ending = input
        .request(Options::tlc(), "Bad")?
        .ending
        .ok_or("ending")?;
    let invalid = TraceDoc::behavior(vec![serde_json::json!({"x":0}), serde_json::json!({"x":2})])?
        .with_ending(ending);
    assert!(
        matches!(input.session(Options::tlc()).try_read(&invalid)?.0.validation,Validation::Invalid(failure) if failure.check==dive_trace::Check::Next)
    );
    Ok(())
}
