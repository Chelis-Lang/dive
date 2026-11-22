//! A run that ends in Init reports the one worker that evaluated it.
#![allow(clippy::expect_used, reason = "worker report regression assertions")]

use dive_api::{CheckRequest, check_file};
use std::path::Path;

fn request(workers: usize) -> CheckRequest {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/native-checkpoint-keys/CpSymView/CpSymView.tla");
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        workers: Some(workers),
        seed: Some(7),
        fp: Some(0),
        ..CheckRequest::default()
    }
}

#[test]
fn a_spill_failure_in_init_reports_one_worker_and_a_full_search_reports_the_width() {
    let dir = std::env::temp_dir().join(format!("dive-init-workers-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("private directory");
    let blocked = dir.join("blocked");
    std::fs::write(&blocked, b"not a directory").expect("block spill parent");
    for compat_tlc in [false, true] {
        let full = check_file(&CheckRequest {
            compat_tlc,
            ..request(8)
        })
        .expect("full search")
        .to_json();
        assert!(
            full["statistics"]["depth"].as_u64().expect("depth") > 1,
            "{full}"
        );
        assert_eq!(full["options"]["workers"]["requested"], 8);
        assert_eq!(full["options"]["workers"]["used"], 8, "{full}");
        for (frontier, predecessor) in [(true, false), (false, true), (true, true)] {
            let failed = check_file(&CheckRequest {
                compat_tlc,
                frontier_memory_bytes: frontier.then_some(0),
                frontier_directory: frontier.then(|| blocked.clone()),
                predecessor_memory_bytes: predecessor.then_some(0),
                predecessor_directory: predecessor.then(|| blocked.clone()),
                ..request(8)
            })
            .expect("storage failure")
            .to_json();
            assert_eq!(failed["verdict"], "resource-limit", "{failed}");
            assert_eq!(
                failed["statistics"]["generated"],
                failed["statistics"]["initial_generated"]
            );
            assert_eq!(failed["options"]["workers"]["requested"], 8);
            // Init runs on one worker; no parallel level started.
            assert_eq!(failed["options"]["workers"]["used"], 1, "{failed}");
            assert!(
                failed["options"]["workers"]["reason"]
                    .as_str()
                    .is_some_and(|reason| reason.contains("ended in Init")),
                "{failed}"
            );
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn temporal_capture_reports_inline_width_only_for_wide_cheap_initial_frontiers() {
    let dir = std::env::temp_dir().join(format!("dive-temporal-width-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("private directory");
    let file = dir.join("Cheap.tla");
    let config = dir.join("Cheap.cfg");
    std::fs::write(&config, "INIT Init\nNEXT Next\nPROPERTY Holds\n").expect("config");
    for (max, next, expected) in [
        (4095, "x' = x", 1),
        (4094, "x' = x", 8),
        (4095, "x' \\in {x, 0}", 8),
    ] {
        std::fs::write(&file, format!("---- MODULE Cheap ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..{max}\nNext == {next}\nHolds == []<>(x >= 0)\n====\n")).expect("module");
        let check = |width| {
            check_file(&CheckRequest {
                file: file.clone(),
                config: Some(config.clone()),
                workers: Some(width),
                seed: Some(0),
                fp: Some(0),
                ..CheckRequest::default()
            })
            .expect("finite check")
            .to_json()
        };
        let one = check(1);
        let eight = check(8);
        assert_eq!(eight["check_kind"], "temporal", "{eight}");
        assert_eq!(eight["verdict"], "success", "{eight}");
        assert_eq!(eight["statistics"], one["statistics"]);
        let workers = &eight["options"]["workers"];
        assert_eq!(workers["used"], expected, "{workers}");
        let scope = &eight["core"]["temporal_workers"];
        assert_eq!(scope["width_applies_to"], "source-expansion");
        assert_eq!(
            scope["serial_phases"],
            serde_json::json!([
                "initial-states",
                "ordered-commit",
                "temporal-analysis",
                "trace-validation"
            ])
        );
        if expected == 1 {
            assert_eq!(workers["sequential"], true);
            assert!(
                workers["reason"]
                    .as_str()
                    .is_some_and(|why| why.contains("4096 initial states"))
            );
        } else {
            assert!(workers["reason"].is_null());
        }
    }
    std::fs::remove_dir_all(dir).expect("remove fixture");
}

#[test]
fn conditional_diagnostics_preserve_source_order_and_membership_demand() {
    let dir = std::env::temp_dir().join(format!("dive-ordered-prints-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("private directory");
    let file = dir.join("Messages.tla");
    let config = file.with_extension("cfg");
    for (generator, invariant, should_print) in [
        (r"x' \in {x, 16}", "x >= 0", false),
        (
            r#"PrintT(<<"source",x>>) /\ x' \in {x, 16}"#,
            "x >= 0",
            true,
        ),
        (
            r"x' \in {x, 16}",
            r#"IF x = 16 THEN PrintT("one new state") ELSE TRUE"#,
            true,
        ),
        (
            r#"IF x = 4 THEN PrintT("late") /\ x'=16 ELSE x' \in {x, 16}"#,
            "x >= 0",
            true,
        ),
    ] {
        std::fs::write(&file, format!("---- MODULE Messages ----\nEXTENDS Naturals,TLC\nVARIABLE x\nInit == x \\in 0..7\nNext == {generator}\nInv == {invariant} /\\ (IF x < 0 THEN PrintT(x) ELSE TRUE)\n====\n")).expect("module");
        std::fs::write(&config, "INIT Init\nNEXT Next\nINVARIANT Inv\n").expect("config");
        for compat_tlc in [false, true] {
            for max_states in [None, Some(8)] {
                let check = |width, sequential| {
                    check_file(&CheckRequest {
                        file: file.clone(),
                        config: Some(config.clone()),
                        workers: Some(width),
                        sequential,
                        seed: Some(0),
                        fp: Some(0),
                        compat_tlc,
                        max_states,
                        ..CheckRequest::default()
                    })
                    .expect("model check")
                    .to_json()
                };
                let serial = check(1, false);
                let inline = check(8, true);
                assert_eq!(inline["options"]["workers"]["used"], 1);
                assert_eq!(inline["options"]["workers"]["sequential"], true);
                assert_eq!(inline["output"], serial["output"]);
                if max_states.is_none() {
                    assert_eq!(serial["verdict"], "success", "{serial}");
                    assert_eq!(serial["statistics"]["distinct"], 9);
                    assert_eq!(
                        !serial["output"].as_array().expect("output").is_empty(),
                        should_print
                    );
                }
                for width in [2, 8] {
                    let parallel = check(width, false);
                    for key in [
                        "verdict",
                        "statistics",
                        "output",
                        "warnings",
                        "property",
                        "exit_code",
                        "counterexample",
                    ] {
                        assert_eq!(serial[key], parallel[key], "{generator} {invariant} {key}");
                    }
                    assert_eq!(parallel["options"]["workers"]["used"], width);
                }
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn ordered_print_workers_publish_only_warnings_reached_at_commit() {
    let dir = std::env::temp_dir().join(format!("dive-print-warnings-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("private directory");
    let file = dir.join("WarningCases.tla");
    let config = file.with_extension("cfg");
    for (init, next, inv, view, expected) in [
        (
            r"x \in 0..3 /\ y=0",
            r#"IF x<0 THEN PrintT("inactive") /\ UNCHANGED <<x,y>> ELSE UNCHANGED <<x,y>>"#,
            r"ENABLED (x'=x+1 /\ UNCHANGED x /\ y'=y)",
            "",
            "invariant-violation",
        ),
        (
            r"x \in 0..3 /\ y=0 /\ PrintT(x)",
            r"x'=x /\ y'=1",
            r"IF y=1 THEN ~(ENABLED (x'=x+1 /\ UNCHANGED x /\ y'=y)) ELSE TRUE",
            "VIEW V\n",
            "success",
        ),
    ] {
        std::fs::write(&file, format!("---- MODULE WarningCases ----\nEXTENDS Naturals,TLC\nVARIABLES x,y\nInit == {init}\nNext == {next}\nInv == {inv}\nV == x\n====\n")).expect("module");
        std::fs::write(
            &config,
            format!("INIT Init\nNEXT Next\nINVARIANT Inv\n{view}"),
        )
        .expect("config");
        for compat_tlc in [false, true] {
            let check = |width| {
                check_file(&CheckRequest {
                    file: file.clone(),
                    config: Some(config.clone()),
                    workers: Some(width),
                    seed: Some(0),
                    fp: Some(0),
                    compat_tlc,
                    store: dive_api::Store::Exact,
                    ..CheckRequest::default()
                })
                .expect("model check")
                .to_json()
            };
            let serial = check(1);
            let parallel = check(8);
            assert_eq!(serial["verdict"], expected, "{serial}");
            for key in [
                "verdict",
                "statistics",
                "output",
                "warnings",
                "exit_code",
                "property",
                "counterexample",
            ] {
                assert_eq!(parallel[key], serial[key], "{inv} {key}");
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn finite_initial_products_preserve_admission_order_multiplicity_and_first_failure() {
    use dive_api::Store;
    use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};

    const SOURCE: &str = r"---- MODULE InitialProduct ----
EXTENDS Naturals
VARIABLES x, y, z
Init == /\ x \in 0..7
        /\ y \in 0..127
        /\ z \in 0..7
Overlap == Init \/ (x \in 0..1 /\ y \in 0..127 /\ z \in 0..7)
Next == UNCHANGED <<x, y, z>>
Holds == x + y + z < 160
FirstFailure == x # 2
LaterFailure == y # 32
====";
    const GOOD: &str = "INIT Init\nNEXT Next\nINVARIANT Holds\n";
    const DUPLICATES: &str = "INIT Overlap\nNEXT Next\nINVARIANT Holds\n";
    const BAD: &str = "INIT Init\nNEXT Next\nINVARIANTS FirstFailure LaterFailure\n";
    let dir = std::env::temp_dir().join(format!("dive-initial-product-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("fixture directory");
    let file = dir.join("InitialProduct.tla");
    let config = file.with_extension("cfg");
    std::fs::write(&file, SOURCE).expect("module");
    for compat_tlc in [false, true] {
        for store in [Store::Exact, Store::Fingerprint] {
            for (cfg, limit, initial_generated, distinct) in [
                (GOOD, None, 8192, 8192),
                (GOOD, Some(4096), 4097, 4096),
                (DUPLICATES, None, 10_240, 8192),
                (BAD, None, 257, 257),
            ] {
                std::fs::write(&config, cfg).expect("config");
                let check = |workers| {
                    check_file(&CheckRequest {
                        file: file.clone(),
                        config: Some(config.clone()),
                        max_states: limit,
                        workers: Some(workers),
                        compat_tlc,
                        store,
                        seed: Some(0),
                        fp: Some(0),
                        ..CheckRequest::default()
                    })
                    .expect("check")
                    .to_json()
                };
                let serial = check(1);
                let parallel = check(8);
                for key in [
                    "verdict",
                    "statistics",
                    "warnings",
                    "property",
                    "counterexample",
                    "error_site",
                    "message",
                ] {
                    assert_eq!(parallel[key], serial[key], "{cfg} {key}");
                }
                assert_eq!(
                    parallel["statistics"]["initial_generated"], initial_generated,
                    "{cfg} {parallel}"
                );
                assert_eq!(parallel["statistics"]["distinct"], distinct);
                if cfg == GOOD {
                    assert_eq!(parallel["core"]["metrics"]["parallel_init_workers"], 8);
                } else {
                    assert!(parallel["core"]["metrics"]["parallel_init_workers"].is_null());
                }
                if limit.is_some() {
                    assert_eq!(parallel["verdict"], "stopped");
                    assert_eq!(parallel["statistics"]["generated"], initial_generated);
                    assert_eq!(parallel["statistics"]["left_on_queue"], distinct);
                    assert_eq!(parallel["options"]["workers"]["used"], 8);
                } else if cfg == BAD {
                    assert_eq!(parallel["verdict"], "invariant-violation");
                    assert_eq!(parallel["property"], "LaterFailure");
                    let trace =
                        TraceDoc::from_json(&parallel["counterexample"]["trace"]).expect("trace");
                    assert_eq!(trace.states.len(), 1);
                    let model = SpecModel::from_source(
                        "InitialProduct.tla",
                        SOURCE,
                        cfg,
                        &TraceOptions::default(),
                    )
                    .expect("independent model");
                    assert_eq!(model.validate(&trace).validation, Validation::Valid);
                } else {
                    assert_eq!(parallel["verdict"], "success");
                    assert_eq!(
                        parallel["statistics"]["generated"],
                        initial_generated + distinct
                    );
                }
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}
