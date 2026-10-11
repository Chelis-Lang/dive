//! Independent replay must resolve the actual configured native operator.
#![allow(clippy::expect_used, reason = "regression assertions")]
use std::sync::Arc;
use dive_model::{BindOptions, Model};
use dive_stdlib::random::{RandomMode, Streams};
use dive_trace::{Ending, Replay, Trace, Validation};
use dive_value::Value;

fn model(source: &str, cfg: &str) -> Model {
    let a = dive_sema::analyze_source("Stream.tla", source, &dive_sema::Options::default());
    assert!(a.accepted());
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse(cfg).into_config().expect("cfg");
    dive_model::bind(ir, &cfg, &BindOptions::tlc()).expect("model")
}
fn spec(call: &str, declarations: &str, extra: &str) -> String {
    format!("---- MODULE Stream ----\nEXTENDS Naturals, TLC\n{declarations}\nVARIABLES n,x\n{extra}\nInit == n=0 /\\ x=0\nNext == n<3 /\\ n'=n+1 /\\ x'={call}\nInv == n<3\n====\n")
}
fn trace() -> Trace {
    Trace {
        states: [0,80,94,100].into_iter().enumerate().map(|(n,x)| Arc::from([Some(Value::Int(i64::try_from(n).expect("small n"))),Some(Value::Int(x))])).collect(),
        labels: vec![None;4],
        ending: Ending::Invariant("Inv".into()),
        length: Some(4),
        properties: Vec::new(),
    }
}
#[test]
fn configured_native_root_and_redirect_have_independent_draw_position_proofs() {
    for (decls, extra, target) in [
        ("CONSTANT C(_)", "", "RandomElement"),
        ("CONSTANT C(_)", "I == INSTANCE TLC", "I!RandomElement"),
        ("C(S) == CHOOSE a \\in S : TRUE", "", "RandomElement"),
        ("CONSTANT C(_)", "Unused == RandomElement(1..100)", "RandomElement"),
    ] {
        let source = spec("C(1..100)",decls,extra);
        let a = dive_sema::analyze_source("Stream.tla",&source,&dive_sema::Options::default());
        let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
        let mut cfg = dive_config::parse("CONSTANT C <- RandomElement\nINIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n").into_config().expect("cfg");
        if let dive_config::ConstantEntry::Substitution {target:rhs,..}=&mut cfg.constants[0] {rhs.text=target.into();}
        let m = dive_model::bind(ir,&cfg,&BindOptions::tlc()).expect("model");
        let replay = Replay::new(Streams::new(7,RandomMode::Tlc),0).expect("fp0");
        let mut t=trace();
        let report=dive_trace::validate(&m,&t,Some(&replay));
        assert!(matches!(report.validation,Validation::Valid),"{target}: {report:?}");
        t.states[1]=Arc::from([Some(Value::Int(1)),Some(Value::Int(74))]);
        assert!(matches!(dive_trace::validate(&m,&t,Some(&replay)).validation,Validation::Invalid(_)));
        assert!(matches!(dive_trace::validate(&m,&trace(),None).validation,Validation::Unsupported(_)));
    }
}
#[test]
fn configured_random_guard_is_not_misclassified_as_pure() {
    let m=model(&spec("C(1..100)","CONSTANT C(_)","" ).replace("n<3 /\\ n'", "C(1..100)>0 /\\ n<3 /\\ n'"),"CONSTANT C <- RandomElement\nINIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n");
    let replay=Replay::new(Streams::new(7,RandomMode::Tlc),0).expect("fp0");
    assert!(matches!(dive_trace::validate(&m,&trace(),Some(&replay)).validation,Validation::Unsupported(_)));
}
