#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "executable positive and negative evidence controls"
)]
use super::*;
use crate::{analyze_source, parse_config_source};
const CFG: &str = "INIT Init\nNEXT Next\nINVARIANT Inv\n";
fn source(next: &str, inv: &str) -> String {
    format!(
        "---- MODULE Test ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == {next}\nInv == {inv}\n====\n"
    )
}
fn program(text: &str, cfg: &str) -> Program {
    let analysis = analyze_source("Test.tla", text, &AnalyzeOptions::default());
    Program::from_reports(&analysis, &parse_config_source("Test.cfg", cfg)).expect("admitted")
}
fn ordinary(p: &Program, limits: Limits) -> dive_explore::Outcome {
    dive_explore::explore_with_storage(
        &p.prepared.plan,
        &p.prepared.store,
        dive_explore::Config {
            max_states: limits.max_states,
            max_depth: limits.max_depth,
            workers: 1,
            warn: true,
            sample: None,
            opts: Opts::default(),
        },
        &|| false,
        None,
        None,
        &dive_explore::storage::Storage {
            exact: true,
            ..Default::default()
        },
    )
}
fn agrees(p: &Program, l: Limits, result: Decision) {
    let oracle = ordinary(p, l);
    assert_eq!(result.counts(), oracle.stats);
    match (result, oracle.verdict) {
        (Decision::Holds(_), dive_explore::Verdict::Success) => {}
        (
            Decision::Refuted {
                property, trace, ..
            },
            dive_explore::Verdict::InvariantViolated {
                property: fresh, ..
            },
        ) => {
            assert_eq!(property, fresh);
            assert_eq!(trace.length, Some(trace.states.len()));
            assert_eq!(trace.variables, p.prepared.plan.vars);
            for (label, step) in trace.labels.iter().zip(
                oracle
                    .trace
                    .as_ref()
                    .expect("ordinary trace")
                    .as_ref()
                    .expect("ordinary states"),
            ) {
                let expected = step
                    .action
                    .and_then(|id| p.prepared.model.actions.get(id))
                    .and_then(|a| {
                        core_engine::action_name(&p.prepared.model, a).map(|name| {
                            dive_trace::Label {
                                name,
                                location: Some(a.label().to_owned()),
                            }
                        })
                    });
                assert_eq!(*label, expected);
            }
            assert!(
                dive_trace::validate_doc(&p.prepared.model, &trace, &[])
                    .0
                    .validation
                    .is_valid()
            );
        }
        (result, verdict) => panic!("recheck {result:?}, fresh {verdict:?}"),
    }
}
const NEXT: &str = "IF x = 0 THEN x' \\in {1, 2} ELSE IF x = 2 THEN x' = 3 ELSE x' = x";
#[test]
fn complete_rechecks_true_false_initial_and_earlier_discovery_counts() {
    let l = Limits::default();
    let p = program(&source(NEXT, "x < 9"), CFG);
    let (rows, first) = p.capture(l).expect("capture");
    agrees(&p, l, first);
    for inv in ["x <= 3", "x /= 0", "x /= 1", "x /= 2", "x /= 3"] {
        let p = program(&source(NEXT, inv), CFG);
        agrees(&p, l, rows.recheck(&p, l).expect("complete recheck"));
    }
}
#[test]
fn edited_refutation_prefix_decides_only_before_or_at_its_stop() {
    let l = Limits {
        max_states: Some(20),
        max_depth: Some(20),
        ..Limits::default()
    };
    let p = program(&source(NEXT, "x /= 3"), CFG);
    let (prefix, first) = p.capture(l).expect("prefix");
    let Decision::Refuted { ref trace, .. } = first else {
        panic!("old refutation required")
    };
    let old_trace = trace.clone();
    agrees(&p, l, first);
    for inv in ["x < 3", "x /= 2", "x /= 1"] {
        let p = program(&source(NEXT, inv), CFG);
        agrees(
            &p,
            l,
            prefix
                .recheck(&p, l)
                .expect("earlier or preserved violation"),
        );
    }
    let holds = program(&source(NEXT, "TRUE"), CFG);
    assert!(prefix.recheck(&holds, l).is_err());
    let erroneous = program(&source(NEXT, "IF x = 1 THEN 1 ELSE x /= 3"), CFG);
    assert!(
        dive_trace::validate_doc(&erroneous.prepared.model, &old_trace, &[])
            .0
            .validation
            .is_valid()
    );
    let fresh = ordinary(&erroneous, l);
    assert_eq!(fresh.verdict, dive_explore::Verdict::Failed);
    assert_eq!(fresh.stats.depth, 2);
    assert!(prefix.recheck(&erroneous, l).is_err());
}
#[test]
fn changed_siblings_limit_and_shared_helpers_decline_reuse() {
    let l = Limits {
        max_states: Some(4),
        max_depth: Some(20),
        ..Limits::default()
    };
    let p = program(&source("IF x = 0 THEN x' = 1 ELSE x' = 2", "x /= 2"), CFG);
    let (old, first) = p.capture(l).expect("prefix");
    let widened = program(
        &source("IF x = 0 THEN x' \\in {1,3,4,5,6,7} ELSE x' = 2", "x /= 2"),
        CFG,
    );
    let Decision::Refuted { trace, .. } = first else {
        panic!("old refutation required")
    };
    assert!(
        dive_trace::validate_doc(&widened.prepared.model, &trace, &[])
            .0
            .validation
            .is_valid()
    );
    let stopped = ordinary(&widened, l);
    assert!(matches!(
        stopped.verdict,
        dive_explore::Verdict::Admission { .. }
    ));
    assert_eq!(
        (stopped.stats.distinct, stopped.stats.left_on_queue),
        (4, 3)
    );
    assert!(widened.capture(l).is_err());
    assert!(old.recheck(&widened, l).is_err());
    assert!(
        old.recheck(
            &p,
            Limits {
                max_states: Some(3),
                ..l
            }
        )
        .is_err()
    );
    let a = source("IF Inv THEN x' = 1 ELSE x' = 2", "x /= 2")
        .replace("Next ==", "Inv == x /= 2\nNext ==")
        .replace("\nInv == x /= 2\n====", "\n====");
    let (old, _) = program(&a, CFG).capture(l).expect("complete");
    assert!(
        old.recheck(&program(&a.replace("x /= 2", "x /= 1"), CFG), l)
            .is_err()
    );
}
#[test]
fn multiple_initials_and_reordered_invariants_keep_exact_first_failure() {
    let l = Limits::default();
    let t = source("x' = x", "TRUE")
        .replace("x = 0", "x \\in {0,1,2}")
        .replace("====", "Alt == TRUE\n====");
    let cfg = "INIT Init\nNEXT Next\nINVARIANT Inv Alt\n";
    let (closure, _) = program(&t, cfg).capture(l).expect("complete");
    let edited = t.replace("== TRUE", "== x /= 1");
    for cfg in [cfg, "INIT Init\nNEXT Next\nINVARIANT Alt Inv\n"] {
        let p = program(&edited, cfg);
        agrees(&p, l, closure.recheck(&p, l).expect("current order"));
    }
}
#[test]
fn configuration_constants_constraints_and_resource_limits_are_bound() {
    let l = Limits::default();
    let t = source("x' = N", "x <= 3").replace("VARIABLE x", "CONSTANT N\nVARIABLE x");
    let cfg = "CONSTANT N = 2\nINIT Init\nNEXT Next\nINVARIANT Inv\n";
    let p = program(&t, cfg);
    assert!(p.capture(Limits { max_rows: 1, ..l }).is_err());
    let (closure, _) = p.capture(l).expect("complete");
    assert!(
        closure
            .recheck(&program(&t, &cfg.replace("= 2", "= 3")), l)
            .is_err()
    );
    let analysis = analyze_source("Test.tla", &t, &AnalyzeOptions::default());
    for role in [
        "CONSTRAINT Inv",
        "ACTION_CONSTRAINT Inv",
        "PROPERTY Inv",
        "VIEW Inv",
        "POSTCONDITION Inv",
    ] {
        assert!(
            Program::from_reports(
                &analysis,
                &parse_config_source("Test.cfg", &format!("{cfg}{role}\n"))
            )
            .is_err()
        );
    }
}

#[test]
fn specification_role_scope_and_non_scalar_rows() {
    let l = Limits::default();
    let s = source(NEXT, "x < 9").replace("====", "Spec == Init /\\ [][Next]_x\n====");
    let cfg = "SPECIFICATION Spec\nINVARIANT Inv\n";
    let (complete, _) = program(&s, cfg).capture(l).expect("complete");
    let edited = program(&s.replace("x < 9", "x < 3"), cfg);
    agrees(&edited, l, complete.recheck(&edited, l).expect("leaf edit"));
    let swapped = program(
        &s.replace("Init /\\ [][Next]_x", "Init /\\ [][x' = x]_x"),
        cfg,
    );
    assert!(complete.recheck(&swapped, l).is_err());
    let renamed = analyze_source(
        "Other.tla",
        &s.replace("MODULE Test", "MODULE Other"),
        &AnalyzeOptions::default(),
    );
    let scoped = Program::from_reports(&renamed, &parse_config_source("Other.cfg", cfg))
        .expect("independent model");
    assert!(complete.recheck(&scoped, l).is_err());
    let aggregate = program(&source("x' = x", "TRUE").replace("x = 0", "x = <<0>>"), CFG);
    assert!(aggregate.capture(l).is_err());
}

#[test]
fn adjacent_override_and_transitive_file_edit_never_reuse_old_rows() {
    use std::{fs, path::PathBuf};

    struct Files(PathBuf);
    impl Drop for Files {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let files = (0u32..)
        .find_map(|i| {
            let p =
                std::env::temp_dir().join(format!("dive-edit-inputs-{}-{i}", std::process::id()));
            match fs::create_dir(&p) {
                Ok(()) => Some(Files(p)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => None,
                Err(e) => panic!("temporary model directory: {e}"),
            }
        })
        .expect("temporary directory");
    let config = files.0.join("Test.cfg");
    let spec = files.0.join("Test.tla");
    let bits = files.0.join("Bits.tla");
    fs::write(&config, "INIT Init\nNEXT Next\nINVARIANT Inv Dummy\n").expect("cfg");
    fs::write(&spec,"---- MODULE Test ----\nEXTENDS Naturals, Bits\nVARIABLE x\nGate == And(7,3,0,7)\nDummy == Gate = Gate\nInit == x = 0\nNext == x' = Gate\nInv == x /= 3\n====\n").expect("root");
    let helper = "---- MODULE Bits ----\nAnd(a,b,c,d) == 99\n====\n";
    fs::write(&bits, helper).expect("helper");
    let l = Limits::default();
    let p = Program::load(&spec, &config).expect("normal binding");
    let (artifact, result) = p.capture(l).expect("ordinary closure");
    assert!(matches!(result, Decision::Holds(_)));
    fs::write(&bits, helper.replace("99", "3")).expect("transitive edit");
    let edited = Program::load(&spec, &config).expect("edited source");
    assert!(artifact.recheck(&edited, l).is_err());
    fs::write(&bits, helper).expect("restore helper");
    fs::write(
        files.0.join("Bits.class"),
        include_bytes!("../../../dive-model/tests/fixtures/bits/Bits.class"),
    )
    .expect("local override");
    assert!(
        matches!(Program::load(&spec,&config),Err(Miss(reason)) if reason.contains("local Java"))
    );
    let req = crate::CheckRequest {
        file: spec,
        config: Some(config),
        store: crate::Store::Exact,
        seed: Some(0),
        fp: Some(0),
        workers: Some(1),
        ..Default::default()
    };
    let fresh = crate::check_file(&req).expect("fresh check").to_json();
    assert_eq!(fresh["verdict"], "invariant-violation");
    assert_eq!(fresh["statistics"]["generated"], 2);
}
