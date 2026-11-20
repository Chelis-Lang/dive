#![allow(
    clippy::expect_used,
    reason = "executable projection acceptance controls"
)]
use super::*;

const SRC: &str = r"---- MODULE Test ----
EXTENDS Naturals, Sequences
CONSTANT N
ASSUME N \in Nat
Domain == 0..N
VARIABLE x
Init == x = <<0>>
Next == IF x[1] < N THEN x' = <<x[1]+1>> ELSE x' = x
Spec == Init /\ [][Next]_x
Bound == x[1] <= N
Other == x[1] < N
A == x[1] \in Nat
B == Len(x) = 1
C == x[1] < 8
D == x[1] < 9
====";
const CFG: &str = "CONSTANT N = 3\nCONSTANT Nat <- Domain\nINVARIANT A B C\nSPECIFICATION Spec\nCONSTRAINT Bound\n";
fn load(src: &str, cfg: &str) -> Result<Program, Miss> {
    let a = crate::analyze_source("Test.tla", src, &AnalyzeOptions::default());
    Program::from_reports(&a, &crate::parse_config_source("Test.cfg", cfg))
}

#[test]
fn retained_success_projects_an_ordered_subset_after_spans_move() {
    let l = Limits::default();
    let prior = load(SRC, CFG).expect("old");
    let edited = load(SRC, &CFG.replace("A B C", "A C")).expect("current");
    assert_eq!(
        prior
            .subset(&edited, l, l)
            .expect("bound subset")
            .invariants(),
        ["A", "C"]
    );
    let c = prior.capture(l).expect("actual complete success");
    let projected = c.project(&edited, l).expect("holds");
    assert_eq!(
        projected,
        edited.capture(l).expect("fresh holds").original_counts()
    );
    assert_eq!((projected.distinct, projected.depth), (4, 4));
}

#[test]
fn other_inputs_limits_and_unproved_inventories_cannot_decide() {
    let l = Limits::default();
    let p = load(SRC, CFG).expect("old");
    for cfg in [
        CFG.replace("A B C", "C A"),
        CFG.replace("A B C", "A D"),
        CFG.replace("N = 3", "N = 4"),
        CFG.replace("CONSTRAINT Bound", "CONSTRAINT Other"),
        CFG.replace("CONSTANT Nat <- Domain\n", ""),
        CFG.replace("SPECIFICATION Spec", "INIT Init\nNEXT Next"),
    ] {
        let edited = load(SRC, &cfg).expect("bind changed request");
        assert!(p.subset(&edited, l, l).is_err());
    }
    let changed = load(&SRC.replace("x[1]+1", "x[1]+2"), CFG).expect("edited plan");
    assert!(p.subset(&changed, l, l).is_err());
    assert!(
        p.subset(
            &p,
            l,
            Limits {
                max_depth: Some(10),
                ..l
            }
        )
        .is_err()
    );
    assert!(
        load(SRC, CFG)
            .expect("bounded")
            .capture(Limits {
                max_states: Some(1),
                ..l
            })
            .is_err()
    );
    let false_prior =
        load(&SRC.replace("x[1] < 8", "x[1] < 2"), CFG).expect("false property bound");
    let new = load(
        &SRC.replace("x[1] < 8", "x[1] < 2"),
        &CFG.replace("A B C", "A B"),
    )
    .expect("removed false property");
    assert!(false_prior.subset(&new, l, l).is_ok());
    assert!(false_prior.capture(l).is_err());
}

#[test]
fn source_native_overrides_are_refused_even_for_relation_only_callers() {
    let text = r"---- MODULE TLC ----
EXTENDS Naturals
RandomElement(S) == 999
VARIABLE x
Draw == RandomElement(0..1)
Init == x=Draw
Next == UNCHANGED x
Q == x>=0
P == x<2
====";
    let a = crate::analyze_source("TLC.tla", text, &AnalyzeOptions::default());
    for names in ["Q P", "P"] {
        let cfg = crate::parse_config_source(
            "TLC.cfg",
            &format!("INIT Init\nNEXT Next\nINVARIANT {names}\n"),
        );
        assert!(Program::from_reports(&a, &cfg).is_err_and(|e| e.to_string().contains("shadows")));
    }
}
