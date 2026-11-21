//! A prefix proves a CHOOSE witness without claiming an exhausted powerset.
#![allow(clippy::expect_used, reason = "trace fixture assertions")]
#[path = "support/core_final.rs"]
mod core_final;

use dive_runtime::CheckOptions;

const SOURCE: &str = r"---- MODULE WideChoice ----
EXTENDS Naturals, FiniteSets
VARIABLES x, n
Init == x = {} /\ n = 26
Next == x' = (CHOOSE s \in SUBSET (1..n) : s = {n}) /\ n' = n
Inv == x = {} \/ x = {n}
====";
const CFG: &str = "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n";

#[test]
fn choice_finds_the_last_singleton_in_either_profile() {
    for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let result = core_final::run(SOURCE, CFG, &opts).to_json();
        assert_eq!(result["verdict"], "success", "{result}");
        assert_eq!(result["statistics"]["distinct"], 2);
    }
}

#[test]
fn choice_requiring_a_larger_subset_remains_open() {
    let source = SOURCE.replace("s = {n}", "s = {1,n}");
    for opts in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let result = core_final::run(&source, CFG, &opts).to_json();
        assert_eq!(result["verdict"], "unsupported", "{result}");
        assert!(
            result["message"]
                .as_str()
                .is_some_and(|s| s.contains("beyond the empty set and singletons"))
        );
    }
}

#[test]
fn compat_cannot_establish_a_witness_when_tlc_cannot_size_the_base() {
    let source = SOURCE
        .replace("n = 26", "n = 2147483647")
        .replace("1..n", "0..n")
        .replace("s = {n}", "s = {}");
    let row = core_final::run(&source, CFG, &CheckOptions::tlc(0, 0)).to_json();
    assert_eq!(row["verdict"], "unsupported", "{row}");
    let row = core_final::run(&source, CFG, &CheckOptions::dive(0)).to_json();
    assert_eq!(row["verdict"], "success", "{row}");
}

#[test]
fn independent_replay_accepts_a_wide_choice_and_rejects_a_changed_successor() {
    let source = SOURCE.replace("Inv == x = {} \\/ x = {n}", "Inv == x = {}");
    for (opts, compat) in [
        (CheckOptions::dive(0), false),
        (CheckOptions::tlc(0, 0), true),
    ] {
        let files = core_final::Files::new(&source, CFG);
        let report = dive_api::check_file(&files.request(&opts))
            .expect("check")
            .to_json();
        assert_eq!(report["verdict"], "invariant-violation");
        let model = dive_api::trace::SpecModel::load(
            &files.file,
            &files.file.with_extension("cfg"),
            &dive_api::trace::TraceOptions {
                compat_tlc: compat,
                run: Some((0, 0)),
                ..Default::default()
            },
        )
        .expect("trace model");
        let mut document = report["counterexample"]["trace"].clone();
        let trace = dive_api::trace::TraceDoc::parse(&document.to_string()).expect("trace");
        assert_eq!(
            model.validate(&trace).validation,
            dive_api::trace::Validation::Valid
        );
        document["states"][1] = document["states"][0].clone();
        let changed =
            dive_api::trace::TraceDoc::parse(&document.to_string()).expect("changed trace");
        assert!(matches!(
            model.validate(&changed).validation,
            dive_api::trace::Validation::Invalid(_)
        ));
    }
}
