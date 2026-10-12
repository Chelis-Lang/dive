//! The `_PERIODIC` configuration entry binds with TLC's errors, and running
//! the hook is a named refusal. The precedence cases are TLC
//! `tlc-2026.10.06.014338` results recorded before the rescue.

#![allow(clippy::expect_used, clippy::panic, reason = "test code")]

use std::sync::Arc;

use dive_model::{BindOptions, ErrorKind, bind};

const SPEC: &str = "---- MODULE M ----\nEXTENDS Naturals\nCONSTANT Token\nVARIABLE x\nInit == x = 0\n\
    Next == x' = x\nInv == x >= 0\nHook == TRUE\nArg(a) == a\n====\n";

fn bind_error(cfg: &str) -> ErrorKind {
    let analysis = dive_sema::analyze_source("M.tla", SPEC, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lowers"));
    let cfg = dive_config::parse(cfg).into_config().expect("cfg");
    match bind(ir, &cfg, &BindOptions::tlc()) {
        Ok(_) => panic!("_PERIODIC must not bind silently"),
        Err(failure) => {
            assert_eq!(failure.output, [] as [dive_model::Message; 0]);
            failure.error.kind
        }
    }
}

fn tlc_code(kind: &ErrorKind) -> Option<u32> {
    match kind {
        ErrorKind::Tlc(code) => Some(code.code()),
        _ => None,
    }
}

#[test]
fn periodic_binds_with_tlc_errors_then_is_refused() {
    let base = "CONSTANT Token = Atom\nINIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n";
    for (extra, code) in [
        // TlcConfigSpecifiedNotDefined.
        ("_PERIODIC Missing\n", Some(2229)),
        // TlcConfigIdRequiresNoArg.
        ("_PERIODIC Arg\n", Some(2228)),
        // Binding the periodic definition precedes the `_POSSIBLE` refusal.
        ("_POSSIBLE Inv\n_PERIODIC Missing\n", Some(2229)),
    ] {
        let kind = bind_error(&format!("{base}{extra}"));
        assert_eq!(tlc_code(&kind), code, "{extra}: {kind:?}");
    }
    // Earlier configuration errors take precedence.
    for (cfg, code) in [
        (
            "CONSTANT Token = Atom\nNEXT Next\n_PERIODIC Missing\n",
            2231,
        ),
        (
            "CONSTANT Token = Atom\nINIT Init\nINVARIANT Inv\n_PERIODIC Missing\n",
            2232,
        ),
        (
            "CONSTANT Token = Atom\nINIT Init\nNEXT Missing\n_PERIODIC Missing\n",
            2229,
        ),
        ("INIT Init\nNEXT Next\n_PERIODIC Missing\n", 2222),
    ] {
        let kind = bind_error(cfg);
        assert_eq!(tlc_code(&kind), Some(code), "{cfg}: {kind:?}");
    }
    let refused = bind_error(&format!("{base}_PERIODIC Hook\n"));
    assert!(
        matches!(&refused, ErrorKind::Unsupported { feature } if feature == "_PERIODIC"),
        "{refused:?}"
    );
}
