//! Exact local-class activation and integer behavior of the pinned Hanoi Bits override.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

mod common;

use common::oracle::{Setup, analyze, bind};
use dive_eval::St;
use dive_model::{BindOptions, Model};
use dive_value::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Files(PathBuf);
impl Drop for Files {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn setup(class: Option<&[u8]>, call: &str, config: &str, opts: BindOptions) -> (Files, Model) {
    let (files, model) = setup_bits(class, call, config, opts, "And(x,y,n,m) == 99");
    (files, model.unwrap_or_else(|e| panic!("{e}")))
}
fn setup_bits(
    class: Option<&[u8]>,
    call: &str,
    config: &str,
    opts: BindOptions,
    body: &str,
) -> (Files, Result<Model, Box<dive_model::Failure>>) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let files = loop {
        let path = std::env::temp_dir().join(format!(
            "dive-bits-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        // Claim a new directory: a recycled PID must not reuse another run's files.
        match fs::create_dir(&path) {
            Ok(()) => break Files(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            result => result.expect("private Bits directory"),
        }
    };
    fs::write(
        files.0.join("Bits.tla"),
        format!("---- MODULE Bits ----\n{body}\n====\n"),
    )
    .expect("Bits");
    if let Some(bytes) = class {
        fs::write(files.0.join("Bits.class"), bytes).expect("class");
    }
    fs::write(files.0.join("M.tla"), format!("---- MODULE M ----\nEXTENDS Bits, Integers\nVARIABLE x\nI == INSTANCE Bits\nInit == x = 0\nNext == x' = x\nReplacement(a,b,c,d) == 42\nInv == x = x /\\ {call}\n====\n")).expect("M");
    let ir = analyze(
        &files.0.join("M.tla"),
        &Setup {
            after: vec![],
            dump_trace: false,
            opts: opts.clone(),
        },
    )
    .expect("analyze");
    let cfg = dive_config::parse(&format!("INIT Init\nNEXT Next\nINVARIANT Inv\n{config}"))
        .into_config()
        .expect("config");
    let model = bind(ir, cfg, opts);
    (files, model)
}
const CLASS: &[u8] = include_bytes!("fixtures/bits/Bits.class");
fn inv(m: &Model) -> Result<Value, dive_eval::EvalError> {
    m.eval(
        &m.invariants[0].pred,
        &St::state(vec![Some(Value::Int(0))].into()),
    )
}
#[test]
fn native_bits_exact_integer_controls_and_config_precedence() {
    for opts in [BindOptions::tlc(), BindOptions::dive()] {
        for call in [
            "And(7,3,0,7) = 3",
            "And(-1,(-2147483647-1),17,0) = (-2147483647-1)",
            "And(7,3,-7,-99) = 3",
            "I!And(7,3,0,7) = 3",
        ] {
            let (_files, m) = setup(Some(CLASS), call, "", opts.clone());
            assert!(m.unsupported.is_empty(), "{:?}", m.unsupported);
            assert_eq!(inv(&m).expect("eval"), Value::Bool(true));
        }
        let (_files, m) = setup(
            Some(CLASS),
            "And(7,3,0,7) = 42",
            "CONSTANT And <- Replacement\n",
            opts.clone(),
        );
        assert_eq!(inv(&m).expect("config override"), Value::Bool(true));
        let (_files, m) = setup(None, "And(7,3,0,7) = 99", "", opts);
        assert_eq!(inv(&m).expect("TLA fallback"), Value::Bool(true));
        assert!(
            m.ir.defs()
                .iter()
                .all(|d| d.std != Some(dive_ir::StdOp::BitsAnd))
        );
    }
}
#[test]
fn native_bits_checks_every_intvalue_argument() {
    for opts in [BindOptions::tlc(), BindOptions::dive()] {
        for call in [
            "And(TRUE,3,0,7) = 0",
            "And(7,TRUE,0,7) = 0",
            "And(7,3,TRUE,7) = 0",
            "And(7,3,0,TRUE) = 0",
        ] {
            let (_files, m) = setup(Some(CLASS), call, "", opts.clone());
            assert_eq!(
                inv(&m).expect_err("must refuse").class(),
                dive_eval::ErrorClass::Unsupported,
                "{call}"
            );
        }
        for call in ["And(x,3,0,7) = 0", "And(7,3,0,x) = 0"] {
            let (_files, m) = setup(Some(CLASS), call, "", opts.clone());
            let st = St::state(vec![Some(Value::Int(2_147_483_648))].into());
            assert_eq!(
                m.eval(&m.invariants[0].pred, &st)
                    .expect_err("outside IntValue")
                    .class(),
                dive_eval::ErrorClass::Unsupported
            );
        }
    }
}
#[test]
fn altered_class_hash_is_a_named_refusal() {
    let mut altered = CLASS.to_vec();
    let last = altered.len() - 1;
    altered[last] ^= 1;
    for opts in [BindOptions::tlc(), BindOptions::dive()] {
        let (_files, result) = setup_bits(
            Some(&altered),
            "And(7,3,0,7) = 99",
            "",
            opts,
            "And(x,y,n,m) == 99",
        );
        let Err(failure) = result else {
            panic!("refuse before any fallback evaluation");
        };
        assert!(matches!(failure.error.kind,
            dive_model::ErrorKind::Unsupported { ref feature } if feature.contains("Bits.class")));
    }
}

#[test]
fn checked_ir_setter_rejects_unrelated_definitions_and_ids() {
    let (_files, m) = setup(None, "And(7,3,0,7) = 99", "", BindOptions::tlc());
    let mut ir = (*m.ir).clone();
    let init = ir.root_def("Init").expect("Init");
    assert!(!ir.set_native_override(init, dive_ir::StdOp::BitsAnd));
    assert!(!ir.set_native_override(dive_ir::DefId(u32::MAX), dive_ir::StdOp::BitsAnd));
    assert!(!ir.set_native_override(init, dive_ir::StdOp::BitwiseAnd));
}

#[test]
fn incompatible_value_signature_remains_unsupported() {
    for opts in [BindOptions::tlc(), BindOptions::dive()] {
        let (_files, result) = setup_bits(
            Some(CLASS),
            "And(Replacement,3,0,7) = 99",
            "",
            opts,
            "And(x(_,_,_,_),y,n,m) == 99",
        );
        let Err(failure) = result else {
            panic!("refuse the incompatible local class signature");
        };
        assert!(matches!(failure.error.kind,
            dive_model::ErrorKind::Unsupported { ref feature } if feature.contains("Bits!And")));
    }
}
