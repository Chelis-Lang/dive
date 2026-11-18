//! The trace validator's controls (ADR 0016 §3, M5 WP-E1) on the
//! counterexamples available without a corpus checkout:
//! - the TLC golden counterexamples of the small-tier Examples models whose
//!   files are vendored in `dive-eval`'s fixtures;
//! - the TLC probes in `tests/fixtures/checker` that carry a `-dumpTrace
//!   json` document.
//!
//! Every one must validate under `--compat=tlc` (deadlocks: every check but
//! the claim, which the validator refuses by name), and every
//! by-construction mutant must be rejected. The conformance harness runs
//! the same controls over the whole corpus (`dive-conformance
//! trace-controls`).

#![allow(clippy::panic, clippy::expect_used, reason = "test code")]

use std::path::{Path, PathBuf};

use dive_api::trace::{Controls, Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use serde_json::Value as J;

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn tlc() -> TraceOptions {
    TraceOptions {
        compat_tlc: true,
        ..TraceOptions::default()
    }
}

fn controls(tla: &Path, cfg: &Path, doc: &TraceDoc) -> Result<Controls, String> {
    let model = SpecModel::load(tla, cfg, &tlc()).map_err(|e| e.to_string())?;
    Ok(model.controls(doc))
}

/// Golden counterexamples whose model files are vendored.
fn golden_controls() -> Vec<(String, Result<Controls, String>)> {
    let goldens = crate_dir().join("../../conformance/golden/tlc-2026.10.06.014338/examples");
    let vendored = crate_dir().join("../dive-eval/tests/fixtures/examples");
    let mut out = Vec::new();
    let mut stack = vec![goldens.clone()];
    let mut files = Vec::new();
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).expect("golden dir").flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "json") {
                files.push(p);
            }
        }
    }
    files.sort();
    for f in files {
        let g: J = serde_json::from_str(&std::fs::read_to_string(&f).expect("read")).expect("json");
        let r = &g["result"];
        // Worker-specific goldens (ADR 0004) are TLC at several workers,
        // whose counterexamples are only approximately shortest.
        if g["workers"] != 1 || r["verdict"] != "safety-violation" {
            continue;
        }
        let id = g["model"]["id"].as_str().expect("id").to_owned();
        let tla = vendored.join(g["model"]["module"].as_str().expect("module"));
        let cfg = vendored.join(&id);
        if !tla.is_file() || !cfg.is_file() {
            continue;
        }
        let ending = Ending::from_tlc(
            r["error_class"].as_str().expect("class"),
            r["violated_property"].as_str(),
            None,
        )
        .expect("ending");
        let doc = TraceDoc::from_json(&r["trace"]["json"])
            .expect("TLC document")
            .with_ending(ending);
        assert_eq!(
            doc.states.len() as u64,
            r["trace"]["length"].as_u64().expect("length")
        );
        out.push((id, controls(&tla, &cfg, &doc)));
    }
    out
}

/// The probes with a `-dumpTrace json` document, with the claim read from
/// TLC's message stream.
fn probe_controls() -> Vec<(String, Result<Controls, String>)> {
    let root = crate_dir().join("tests/fixtures/checker/tlc-probes");
    let runs = root.join("tlc-2026.10.06.014338");
    let mut names: Vec<String> = std::fs::read_dir(&runs)
        .expect("probe dir")
        .flatten()
        .filter_map(|e| {
            e.file_name()
                .to_str()
                .and_then(|n| n.strip_suffix(".trace.json"))
                .map(str::to_owned)
        })
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let stdout =
                std::fs::read_to_string(runs.join(format!("{name}.stdout"))).expect("stdout");
            let ending =
                probe_ending(&stdout).unwrap_or_else(|| panic!("{name}: no claim in stdout"));
            let doc = TraceDoc::parse(
                &std::fs::read_to_string(runs.join(format!("{name}.trace.json"))).expect("trace"),
            )
            .expect("TLC document")
            .with_ending(ending);
            let dir = root.join("specs").join(&name);
            let c = controls(
                &dir.join(format!("{name}.tla")),
                &dir.join(format!("{name}.cfg")),
                &doc,
            );
            (name, c)
        })
        .collect()
}

/// The violation TLC reports, from its `-tool` message stream.
fn probe_ending(stdout: &str) -> Option<Ending> {
    let mut lines = stdout.lines();
    while let Some(l) = lines.next() {
        let Some(code) = l
            .strip_prefix("@!@!@STARTMSG ")
            .and_then(|r| r.split(':').next())
        else {
            continue;
        };
        let text = lines.next().unwrap_or("");
        let word = |prefix: &str| {
            text.strip_prefix(prefix)
                .and_then(|r| r.split_whitespace().next())
                .map(str::to_owned)
        };
        match code {
            "2107" | "2110" => return word("Invariant ").map(Ending::Invariant),
            "2112" => return word("Action property ").map(Ending::Property),
            "2114" => return Some(Ending::Deadlock),
            _ => {}
        }
    }
    None
}

fn check(all: &[(String, Result<Controls, String>)], expected: usize) {
    let mut failures = Vec::new();
    let mut mutants = 0;
    for (id, c) in all {
        match c {
            Ok(c) => {
                mutants += c.mutants.len();
                failures.extend(c.failures().into_iter().map(|f| format!("{id}: {f}")));
                assert!(
                    !c.mutants.is_empty() || c.positive.length == 1,
                    "{id}: no mutants"
                );
            }
            Err(e) => failures.push(format!("{id}: {e}")),
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    assert_eq!(
        all.len(),
        expected,
        "{:?}",
        all.iter().map(|x| &x.0).collect::<Vec<_>>()
    );
    assert!(mutants > expected, "{mutants} mutants");
}

#[test]
fn tlc_golden_counterexamples_validate_and_their_mutants_do_not() {
    // DieHard, MCDieHarder, MissionariesAndCannibals, FourQueens, spanning,
    // Hanoi and ACP_NB_WRONG_TLC.
    check(&golden_controls(), 7);
}

#[test]
fn tlc_probe_counterexamples_validate_and_their_mutants_do_not() {
    let all = probe_controls();
    for (id, c) in &all {
        let Ok(c) = c else { continue };
        // Both finite deadlock probes now prove absence independently.
        assert_eq!(
            c.positive.validation,
            Validation::Valid,
            "{id}: {:?}",
            c.positive
        );
    }
    check(&all, 12);
}

#[test]
fn deadlock_needs_the_model_to_check_it() {
    let spec = "---- MODULE D ----\nVARIABLE x\nInit == x = 0\nNext == x = 0 /\\ x' = 1\n====\n";
    let doc = TraceDoc::behavior(vec![
        serde_json::json!({"x": 0}),
        serde_json::json!({"x": 1}),
    ])
    .expect("doc")
    .with_ending(Ending::Deadlock);
    let checks =
        SpecModel::from_source("D.tla", spec, "INIT Init\nNEXT Next\n", &tlc()).expect("model");
    assert_eq!(checks.validate(&doc).validation, Validation::Valid);
    let scoped = spec
        .replace(
            "Next == x = 0 /\\ x' = 1",
            "Step(k) == x < k /\\ x' = x + 1\nNext == Step(1)",
        )
        .replace("VARIABLE x", "EXTENDS Naturals\nVARIABLE x");
    let scoped = SpecModel::from_source("D.tla", &scoped, "INIT Init\nNEXT Next\n", &tlc())
        .expect("scoped model");
    assert_eq!(scoped.validate(&doc).validation, Validation::Valid);
    let fixtures =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../dive-trace/tests/fixtures");
    let source =
        std::fs::read_to_string(fixtures.join("specs/DeadlockScopedCover/DeadlockScopedCover.tla"))
            .expect("pinned source");
    let config =
        std::fs::read_to_string(fixtures.join("specs/DeadlockScopedCover/DeadlockScopedCover.cfg"))
            .expect("pinned config");
    let code = std::fs::read_to_string(
        fixtures.join("tlc/tlc-2026.10.06.014338/DeadlockScopedCover.exit"),
    )
    .expect("pinned exit");
    assert_eq!(code.trim(), "11");
    let pinned = SpecModel::from_source("DeadlockScopedCover.tla", &source, &config, &tlc())
        .expect("pinned model");
    assert_eq!(pinned.validate(&doc).validation, Validation::Valid);
    let opaque = spec
        .replace(
            "Next == x = 0 /\\ x' = 1",
            "Step(k) == IF x < k THEN x' = x + 1 ELSE x' > x /\\ x' < x + 1\nNext == Step(1)",
        )
        .replace("VARIABLE x", "EXTENDS Naturals\nVARIABLE x");
    let unknown = SpecModel::from_source("D.tla", &opaque, "INIT Init\nNEXT Next\n", &tlc())
        .expect("missing-cover model");
    assert_eq!(
        unknown.validate(&doc).validation,
        Validation::Unsupported(dive_trace_deadlock().to_owned())
    );
    let no = SpecModel::from_source(
        "D.tla",
        spec,
        "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n",
        &tlc(),
    )
    .expect("model");
    assert!(matches!(
        no.validate(&doc).validation,
        Validation::Invalid(_)
    ));
}

fn dive_trace_deadlock() -> &'static str {
    // The reason `dive-trace` gives; spelled out so a change is deliberate.
    "deadlock: ENABLED of the next-state relation could not be decided on the last state; every other check passed"
}

#[test]
fn mutation_controls_preserve_names_and_still_refute_changed_values() {
    let source = "---- MODULE MutationNames ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=x+1\nInv == x<2\n====\n";
    for compat_tlc in [false, true] {
        let model = SpecModel::from_source(
            "MutationNames.tla",
            source,
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
            &TraceOptions {
                compat_tlc,
                ..TraceOptions::default()
            },
        )
        .expect("model");
        let doc = TraceDoc::behavior(vec![
            serde_json::json!({"x":0}),
            serde_json::json!({"x":1}),
            serde_json::json!({"x":2}),
        ])
        .expect("doc")
        .with_ending(Ending::Invariant("Inv".to_owned()));
        let before = model.model().names.len();
        let controls = model.controls(&doc);
        assert!(controls.failures().is_empty(), "{:?}", controls.failures());
        assert!(controls.mutants.iter().any(
            |(name, verdict)| name == "change-1-x" && matches!(verdict, Validation::Invalid(_))
        ));
        assert_eq!(model.model().names.len(), before);
        assert!(!model.model().has_runtime_names());
        assert_eq!(model.validate(&doc).validation, Validation::Valid);
    }
}

#[test]
fn accepted_or_refused_mutants_are_control_failures() {
    for verdict in [
        Validation::Valid,
        Validation::Unsupported("unproved".to_owned()),
    ] {
        let controls = Controls {
            positive: dive_api::trace::Report {
                validation: Validation::Valid,
                notes: Vec::new(),
                length: 2,
            },
            dive_format: Some(Validation::Valid),
            mutants: vec![("forged-step".to_owned(), verdict)],
        };
        assert_eq!(controls.failures().len(), 1);
        assert!(controls.failures()[0].contains("forged-step"));
    }
}

#[test]
fn alternate_label_mutation_does_not_run_untaken_effects() {
    let source = "---- MODULE MutationEffects ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == x=0\nA == x'=x+1 /\\ (IF x=0 THEN TRUE ELSE PrintT(\"poison\"))\nB == x'=x+1\nNext == IF x=0 THEN A ELSE B\nInv == x<2\n====\n";
    for compat_tlc in [false, true] {
        let model = SpecModel::from_source(
            "MutationEffects.tla",
            source,
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
            &TraceOptions {
                compat_tlc,
                run: Some((0, 0)),
                ..TraceOptions::default()
            },
        )
        .expect("model");
        let doc = TraceDoc::parse(
            r#"{"format":"dive-trace","version":1,"variables":["x"],
            "states":[{"action":null,"values":{"x":0}},
            {"action":{"name":"A"},"values":{"x":1}},
            {"action":{"name":"B"},"values":{"x":2}}],
            "ending":{"kind":"invariant","name":"Inv"}}"#,
        )
        .expect("doc");
        assert_eq!(model.validate(&doc).validation, Validation::Valid);
        let controls = model.controls(&doc);
        assert!(controls.failures().is_empty(), "{:?}", controls.failures());
        assert_eq!(model.model().evaluator.take_output(), Vec::<String>::new());
        assert_eq!(model.validate(&doc).validation, Validation::Valid);
    }
}

#[test]
fn a_label_must_name_an_action_that_holds() {
    let spec = "---- MODULE L ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\n\
                Up == x' = x + 1\nDown == x > 0 /\\ x' = x - 1\nNext == Up \\/ Down\n\
                Inv == x < 2\n====\n";
    let model = SpecModel::from_source(
        "L.tla",
        spec,
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
        &tlc(),
    )
    .expect("model");
    let doc = |label: &str| {
        TraceDoc::parse(&format!(
            r#"{{"format":"dive-trace","version":1,"variables":["x"],
            "states":[{{"action":null,"values":{{"x":0}}}},
                      {{"action":{{"name":"Up"}},"values":{{"x":1}}}},
                      {{"action":{{"name":"{label}"}},"values":{{"x":2}}}}],
            "ending":{{"kind":"invariant","name":"Inv"}}}}"#
        ))
        .expect("doc")
    };
    assert_eq!(model.validate(&doc("Up")).validation, Validation::Valid);
    let Validation::Invalid(f) = model.validate(&doc("Down")).validation else {
        panic!("a wrong label validates");
    };
    assert_eq!((f.state, f.check), (2, dive_api::trace::Check::Label));
    // An unknown name is noted, not trusted.
    let r = model.validate(&doc("Sideways"));
    assert_eq!(r.validation, Validation::Valid);
    assert!(
        r.notes.iter().any(|n| n.contains("Sideways")),
        "{:?}",
        r.notes
    );
}

#[test]
fn constraints_bound_every_state_but_a_violating_last_one() {
    // The probe `ConstrainedViolation`: TLC reports a violation in a state
    // outside CONSTRAINT. The same trace claiming a different ending is
    // invalid there.
    let spec = "---- MODULE C ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\n\
                Next == x' = x + 1\nInv == x # 2\nP == [][x' > x]_x\nConstr == x < 2\n====\n";
    let cfg = "INIT Init\nNEXT Next\nINVARIANT Inv\nCONSTRAINT Constr\n";
    let model = SpecModel::from_source("C.tla", spec, cfg, &tlc()).expect("model");
    let states = vec![
        serde_json::json!({"x": 0}),
        serde_json::json!({"x": 1}),
        serde_json::json!({"x": 2}),
    ];
    let inv = TraceDoc::behavior(states.clone())
        .expect("doc")
        .with_ending(Ending::Invariant("Inv".into()));
    assert_eq!(model.validate(&inv).validation, Validation::Valid);
    let mut four = states;
    four.push(serde_json::json!({"x": 3}));
    let past = TraceDoc::behavior(four)
        .expect("doc")
        .with_ending(Ending::Invariant("Inv".into()));
    let Validation::Invalid(f) = model.validate(&past).validation else {
        panic!("a trace past the constraint validates");
    };
    assert_eq!(f.state, 2);
}
