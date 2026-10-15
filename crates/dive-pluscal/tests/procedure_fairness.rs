//! Process-owned fairness admission, reachability and resource regressions.
#![allow(
    clippy::expect_used,
    reason = "test assertions report native translation failures"
)]
use dive_pluscal::{Code, FairnessOption, TranslationOptions, translate};
const SOURCE: &str = "--algorithm A { variable x=0; procedure Add(a=0) { Entry: + x:=x+a; Finish: - return; }; fair process (One=1) { Start:call Add(1); Last:skip; } }";
#[test]
fn reachable_owned_fairness_and_nof() {
    let artifact = translate(SOURCE, &TranslationOptions::default()).expect("bounded fair caller");
    assert!(artifact.tla.contains("WF_vars(One)"));
    assert!(
        artifact
            .tla
            .contains("WF_vars((pc[1] # \"Finish\") /\\ Add(1))")
    );
    assert!(artifact.tla.contains("SF_vars(Entry(1))"));
    let options = TranslationOptions {
        fairness: FairnessOption::None,
        ..TranslationOptions::default()
    };
    let artifact = translate(SOURCE, &options).expect("nof still translates actions");
    assert!(!artifact.tla.contains("WF_vars("));
    assert!(!artifact.tla.contains("SF_vars("));
}
#[test]
fn false_branch_call_still_contributes_and_unused_does_not() {
    let source = "--algorithm A { variable x=0; procedure Add(a=0) { Entry:if(FALSE){call More(a);}else{x:=x+a;}; Finish:return; }; procedure More(b=0) { M:x:=x+b; R:return; }; procedure Unused(c=0) { U:x:=x+c; V:return; }; fair process (One=1) { Start:call Add(1); Last:skip; } }";
    let artifact =
        translate(source, &TranslationOptions::default()).expect("syntactic reachability");
    assert!(artifact.tla.contains("WF_vars(More(1))"));
    assert!(!artifact.tla.contains("WF_vars(Unused(1))"));
}
#[test]
fn sequential_modifier_follows_primary_and_small_output_is_bounded() {
    let source = "--algorithm A { variable x=0; procedure Add(a=0) { Entry: + x:=x+a; Finish:return; }; { Start:call Add(1); Last:skip; } }";
    let artifact =
        translate(source, &TranslationOptions::default()).expect("pinned sequential modifier");
    assert!(artifact.program_counter);
    assert!(!artifact.tla.contains("WF_vars(") && !artifact.tla.contains("SF_vars("));
    let options = TranslationOptions {
        max_output_bytes: 80,
        ..TranslationOptions::default()
    };
    assert_eq!(
        translate(SOURCE, &options)
            .expect_err("bounded generated artifact")
            .diagnostic
            .code,
        Code::Resource
    );
}

#[test]
fn shared_procedure_fairness_belongs_only_to_fair_caller() {
    let source = "--algorithm A variable x=0; procedure P(a=0) begin F:x:=a; R:return; end procedure; fair process One=1 begin L:call P(1); E:skip; end process; process Two=2 begin M:call P(2); N:skip; end process; end algorithm";
    let artifact =
        translate(source, &TranslationOptions::default()).expect("newly supported owned fairness");
    assert!(artifact.tla.contains("WF_vars(One) /\\ WF_vars(P(1))"));
    assert!(!artifact.tla.contains("WF_vars(Two)"));
    assert!(!artifact.tla.contains("WF_vars(P(2))"));
}
