//! A configured user wrapper of a property's constant operator holds on Core,
//! and a configured native operator target stays a named refusal (the reference
//! analysis named "configured predicate operator target"; Core refuses the
//! temporal validation failure's report). These ran
//! on the retired reference final analysis until #791, with a caller-installed
//! evaluator host Core does not accept.
#![allow(clippy::expect_used, clippy::panic, reason = "boundary assertions")]

#[path = "support/core_final.rs"]
mod core_final;
#[path = "support/refused.rs"]
mod refused;

use core_final::Outcome;
use dive_runtime::CheckOptions;

const CONFIGURED: &str = "---- MODULE ContextGuard ----\nEXTENDS Sequences\nCONSTANT Op(_)\nVARIABLE x\nInit==x=0\nNext==x'=x\nLength(v)==Len(v)\nP==<>(Op(<<1>>)=1)\n====\n";

fn cfg(target: &str) -> String {
    format!("INIT Init\nNEXT Next\nPROPERTY P\nCONSTANT Op <- {target}\n")
}

#[test]
fn configured_user_wrapper_holds() {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let run = core_final::check(CONFIGURED, cfg("Length"), &options)
            .unwrap_or_else(|s| panic!("{s}"));
        assert!(
            matches!(run.outcome(), Outcome::Holds(_)),
            "{:?}",
            run.outcome()
        );
    }
}

#[test]
fn configured_native_target_is_a_named_refusal() {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let stop = core_final::check(CONFIGURED, cfg("Len"), &options)
            .expect_err("configured native target stays unsupported");
        refused::assert_refused(
            &stop.json(),
            stop.exit_code(),
            refused::TEMPORAL_VALIDATION_FAILURE_REPORT,
        );
    }
}
