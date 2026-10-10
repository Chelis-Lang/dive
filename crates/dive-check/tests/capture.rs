//! Graph capture shares the safety engine without claiming temporal truth.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use dive_check::{
    CheckOptions, Verdict,
    capture::{self, Exploration, Limits},
};
use dive_store::StoreMode;
use dive_value::Value;
use std::sync::Arc;

fn model(source: &str, config: &str, opts: &CheckOptions) -> dive_model::Model {
    let name = source.split_whitespace().nth(2).expect("module");
    let analysis = dive_sema::analyze_source(
        &format!("{name}.tla"),
        source,
        &dive_sema::Options::default(),
    );
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse(config).into_config().expect("config");
    dive_model::bind(
        ir,
        &cfg,
        &dive_model::BindOptions {
            eval: opts.eval,
            no_deadlock: opts.no_deadlock,
            host: None,
        },
    )
    .expect("bind")
}

fn complete(model: &dive_model::Model, opts: &CheckOptions) -> capture::Complete {
    match capture::explore(model, opts, Limits::default()) {
        Exploration::Complete(c) => *c,
        Exploration::Stopped(v) => panic!("{v:?}"),
    }
}

#[test]
fn duplicates_seen_edges_roots_and_stutters_retain_reference_order() {
    let source = "---- MODULE Capture ----\nVARIABLE x\nInit == x \\in {0,1}\n\
        Next == x' = 0 \\/ x' = 0 \\/ x' = 1\nProp == []<>(x = 0)\n====\n";
    for store in [StoreMode::Fingerprint, StoreMode::Exact] {
        let opts = CheckOptions {
            store,
            workers: 8,
            ..CheckOptions::tlc(0, 0)
        };
        let m = model(source, "INIT Init\nNEXT Next\nPROPERTY Prop\n", &opts);
        assert!(
            m.unsupported
                .iter()
                .any(|u| u.kind == dive_model::UnsupportedKind::TemporalProperty)
        );
        let c = complete(&m, &opts);
        assert_eq!(
            (c.stats.generated, c.stats.distinct, c.stats.depth),
            (8, 2, 1)
        );
        assert_eq!(c.graph.roots, [0, 1]);
        assert_eq!(c.graph.states[0][0], Some(Value::Int(0)));
        assert_eq!(c.graph.states[1][0], Some(Value::Int(1)));
        for (from, row) in c.graph.edges.iter().enumerate() {
            assert_eq!(
                row.iter().map(|e| e.to).collect::<Vec<_>>(),
                [0, 0, 1, from]
            );
            assert_eq!(
                row.iter().map(|e| e.ordinal).collect::<Vec<_>>(),
                [Some(0), Some(1), Some(2), None]
            );
            assert!(row.last().expect("stutter").label.is_none());
        }
        assert_eq!(
            (c.workers.requested, c.workers.used, c.workers.sequential),
            (8, 8, false)
        );
        assert!(c.workers.reason.is_none());
    }
}

#[test]
fn constraints_filter_generated_edges_without_filtering_implicit_stutters() {
    let opts = CheckOptions::tlc(0, 0);
    let m = model(
        "---- MODULE Capture ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\n\
        Next == x' = (x + 1) % 3\nKeep == x < 2\nChanges == x' # x\n====\n",
        "INIT Init\nNEXT Next\nCONSTRAINT Keep\nACTION_CONSTRAINT Changes\n",
        &opts,
    );
    let c = complete(&m, &opts);
    assert_eq!(
        (c.stats.generated, c.stats.distinct, c.stats.depth),
        (3, 2, 2)
    );
    assert_eq!(
        c.graph.edges[0].iter().map(|e| e.to).collect::<Vec<_>>(),
        [1, 0]
    );
    assert_eq!(
        c.graph.edges[1].iter().map(|e| e.to).collect::<Vec<_>>(),
        [1]
    );
    assert!(c.graph.edges[1][0].ordinal.is_none());
}

#[test]
fn no_safety_stop_or_resource_bound_exposes_a_complete_graph() {
    let opts = CheckOptions::tlc(0, 0);
    let source = "---- MODULE Capture ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nInv == ~x\n====\n";
    let m = model(source, "INIT Init\nNEXT Next\nINVARIANT Inv\n", &opts);
    assert!(matches!(
        capture::explore(&m, &opts, Limits::default()),
        Exploration::Stopped(r) if matches!(r.verdict, Verdict::InvariantViolated { .. })
    ));
    for limits in [
        Limits {
            generated: 0,
            ..Limits::default()
        },
        Limits {
            states: 0,
            ..Limits::default()
        },
        Limits {
            edges: 0,
            ..Limits::default()
        },
    ] {
        let m = model(source, "INIT Init\nNEXT Next\n", &opts);
        assert!(matches!(
            capture::explore(&m, &opts, limits),
            Exploration::Stopped(r) if matches!(r.verdict, Verdict::ResourceLimit { .. })
        ));
    }
    let dead = model(
        "---- MODULE Capture ----\nVARIABLE x\nInit == x = TRUE\nNext == ~x /\\ x' = TRUE\n====\n",
        "INIT Init\nNEXT Next\n",
        &opts,
    );
    assert!(matches!(
        capture::explore(&dead, &opts, Limits::default()),
        Exploration::Stopped(r) if r.verdict == Verdict::Deadlock
    ));
}

#[test]
fn every_other_unsupported_reason_and_profile_mismatch_is_retained() {
    let opts = CheckOptions::tlc(0, 0);
    let mut m = model(
        "---- MODULE Capture ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\n====\n",
        "INIT Init\nNEXT Next\n",
        &opts,
    );
    // A matching-looking message is not a temporal capability certificate.
    m.unsupported.push(dive_model::Unsupported {
        feature: "temporal properties (liveness checking): Fake".into(),
        kind: dive_model::UnsupportedKind::Feature,
    });
    assert!(
        matches!(capture::explore(&m,&opts,Limits::default()),Exploration::Stopped(r) if matches!(&r.verdict, Verdict::Unsupported { feature } if feature.contains("Fake")))
    );
    assert!(
        matches!(capture::explore(&m,&CheckOptions::dive(0),Limits::default()),Exploration::Stopped(r) if matches!(&r.verdict, Verdict::Unsupported { feature } if feature.contains("profile mismatch")))
    );
}

#[test]
fn failing_assumptions_keep_prints_and_completed_safety_stops_keep_all_evidence() {
    let opts = CheckOptions::tlc(0, 17);
    for source in [
        "---- MODULE Capture ----\nEXTENDS TLC\nVARIABLE x\nASSUME PrintT(\"before-failure\") /\\ FALSE\nInit == x = TRUE\nNext == x' = x\n====\n",
        "---- MODULE Capture ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nInv == ~x\n====\n",
    ] {
        let cfg_text = if source.contains("Inv ==") {
            "INIT Init\nNEXT Next\nINVARIANT Inv\n"
        } else {
            "INIT Init\nNEXT Next\n"
        };
        let m = model(source, cfg_text, &opts);
        let cfg = dive_config::parse(cfg_text).into_config().expect("cfg");
        let normal = dive_check::check(&m.ir, &cfg, &opts);
        let Exploration::Stopped(captured) = capture::explore(&m, &opts, Limits::default()) else {
            panic!("safety stop must retain evidence, never a complete graph");
        };
        assert_eq!(captured.verdict, normal.verdict);
        assert_eq!(captured.stats, normal.stats);
        assert_eq!(captured.tlc_prints_stats, normal.tlc_prints_stats);
        assert_eq!(captured.output, normal.output);
        assert_eq!(captured.warnings, normal.warnings);
        assert_eq!(captured.var_names, normal.var_names);
        assert_eq!(captured.tlc_var_order, normal.tlc_var_order);
        assert_eq!(captured.fp_index, normal.fp_index);
        assert!(captured.elapsed > std::time::Duration::ZERO);
        let states = |r: &dive_check::CheckResult| {
            r.counterexample.as_ref().map(|c| {
                c.states
                    .iter()
                    .map(|s| s.values.clone())
                    .collect::<Vec<_>>()
            })
        };
        assert_eq!(states(&captured), states(&normal));
        if source.contains("PrintT") {
            assert_ne!(captured.output, Vec::<String>::new());
        }
    }
}

// Inline capture is an independent ordered-commit control. Compare the whole
// retained graph, not just the new-state frontier, on waves crossing multiple
// levels and with duplicate, previously seen and constrained successors.
#[test]
fn ordered_parallel_capture_matches_inline_across_profiles_stores_and_chaos() {
    let source = "---- MODULE Capture ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..11\n\
        A == x' = (x + 12) % 24\nB == x' = x\nC == x' = (x + 12) % 24\n\
        Next == A \\/ B \\/ C\nKeep == x < 20\n====\n";
    for profile in [CheckOptions::tlc(0, 17), CheckOptions::dive(17)] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            let inline = CheckOptions {
                store,
                sequential: true,
                ..profile.clone()
            };
            let reference = complete(
                &model(source, "INIT Init\nNEXT Next\nCONSTRAINT Keep\n", &inline),
                &inline,
            );
            assert_eq!(reference.stats.distinct, 20);
            for workers in [1, 4, 8] {
                for chaos in [None, Some(0), Some(5), Some(91)] {
                    let opts = CheckOptions {
                        store,
                        workers,
                        chaos,
                        ..profile.clone()
                    };
                    let c = complete(
                        &model(source, "INIT Init\nNEXT Next\nCONSTRAINT Keep\n", &opts),
                        &opts,
                    );
                    assert_eq!(c.graph.states, reference.graph.states);
                    assert_eq!(c.graph.edges, reference.graph.edges);
                    assert_eq!(c.graph.roots, reference.graph.roots);
                    assert_eq!(c.stats, reference.stats);
                    assert_eq!(c.output, reference.output);
                    assert_eq!(c.workers.used, workers);
                    assert!(!c.workers.sequential);
                }
            }
        }
    }
}

fn stopped(m: &dive_model::Model, opts: &CheckOptions, limits: Limits) -> dive_check::CheckResult {
    match capture::explore(m, opts, limits) {
        Exploration::Stopped(r) => *r,
        Exploration::Complete(_) => panic!("a bound or safety stop cannot expose a complete graph"),
    }
}

#[test]
fn tiny_bounds_choose_the_inline_prefix_at_every_width() {
    let source = "---- MODULE Capture ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..9\n\
        Next == x' = (x + 10) % 20 \\/ x' = x \\/ x' = (x + 10) % 20\n====\n";
    for profile in [CheckOptions::tlc(0, 17), CheckOptions::dive(17)] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            let inline = CheckOptions {
                store,
                sequential: true,
                ..profile.clone()
            };
            for limits in [
                Limits {
                    generated: 10,
                    ..Limits::default()
                },
                Limits {
                    generated: 14,
                    ..Limits::default()
                },
                Limits {
                    generated: 35,
                    ..Limits::default()
                },
                Limits {
                    states: 11,
                    ..Limits::default()
                },
                Limits {
                    edges: 2,
                    ..Limits::default()
                },
                Limits {
                    edges: 3,
                    ..Limits::default()
                },
                Limits {
                    edges: 4,
                    ..Limits::default()
                },
                Limits {
                    edges: 33,
                    ..Limits::default()
                },
                Limits {
                    successors_per_unit: 0,
                    ..Limits::default()
                },
                Limits {
                    successors_per_unit: 2,
                    ..Limits::default()
                },
            ] {
                let r = stopped(
                    &model(source, "INIT Init\nNEXT Next\n", &inline),
                    &inline,
                    limits,
                );
                assert!(matches!(r.verdict, Verdict::ResourceLimit { .. }));
                for workers in [1, 4, 8] {
                    for chaos in [None, Some(0), Some(6)] {
                        let opts = CheckOptions {
                            store,
                            workers,
                            chaos,
                            ..profile.clone()
                        };
                        let p = stopped(
                            &model(source, "INIT Init\nNEXT Next\n", &opts),
                            &opts,
                            limits,
                        );
                        assert_eq!(p.verdict, r.verdict, "{limits:?} {workers} {chaos:?}");
                        assert_eq!(p.stats, r.stats, "{limits:?} {workers} {chaos:?}");
                        assert_eq!(
                            p.counterexample.as_ref().map(|c| c
                                .states
                                .iter()
                                .map(|s| (&s.values, &s.label, &s.action))
                                .collect::<Vec<_>>()),
                            r.counterexample.as_ref().map(|c| c
                                .states
                                .iter()
                                .map(|s| (&s.values, &s.label, &s.action))
                                .collect::<Vec<_>>())
                        );
                    }
                }
            }
        }
    }
}

fn observable(r: &dive_check::CheckResult) -> String {
    format!(
        "{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}",
        r.verdict,
        r.stats,
        r.counterexample,
        r.output,
        r.warnings,
        r.collision,
        r.notes,
        r.tlc_prints_stats
    )
}

#[test]
fn mid_wave_safety_stops_preserve_ordered_prints_counts_and_predecessors() {
    // The existing safety control uses live level reads. Replacing only that
    // read with a pure constant isolates graph retention's contract; stateful
    // reads remain explicitly refused by capture (tested separately below).
    let source = include_str!("fixtures/invariance/Wide.tla").replace("TLCGet(\"level\")", "0");
    for profile in [CheckOptions::tlc(0, 17), CheckOptions::dive(17)] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            let inline = CheckOptions {
                store,
                sequential: true,
                ..profile.clone()
            };
            for mode in [
                "invariant",
                "error",
                "deadlock",
                "incomplete",
                "constraint",
                "implied",
            ] {
                let cfg_text = format!(
                    "CONSTANT Mode = \"{mode}\"\nINIT Init\nNEXT Next\nINVARIANT Inv\nCONSTRAINT Constr\nPROPERTY Prop\nCHECK_DEADLOCK {}\n",
                    if mode == "deadlock" { "TRUE" } else { "FALSE" }
                );
                let reference = stopped(
                    &model(&source, &cfg_text, &inline),
                    &inline,
                    Limits::default(),
                );
                let cfg = dive_config::parse(&cfg_text).into_config().expect("cfg");
                let normal =
                    dive_check::check(&model(&source, &cfg_text, &inline).ir, &cfg, &inline);
                assert_eq!(observable(&reference), observable(&normal), "{mode}");
                for workers in [1, 4, 8] {
                    for chaos in [None, Some(0), Some(33)] {
                        let opts = CheckOptions {
                            store,
                            workers,
                            chaos,
                            ..profile.clone()
                        };
                        let r =
                            stopped(&model(&source, &cfg_text, &opts), &opts, Limits::default());
                        assert_eq!(
                            observable(&r),
                            observable(&reference),
                            "{mode} {workers} {chaos:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn capture_still_refuses_projection_randomness_and_stateful_effects() {
    let opts = CheckOptions {
        workers: 8,
        ..CheckOptions::tlc(0, 17)
    };
    for (source, cfg, reason) in [
        (
            "---- MODULE Capture ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\nV == x\n====\n",
            "INIT Init\nNEXT Next\nVIEW V\n",
            "VIEW",
        ),
        (
            "---- MODULE Capture ----\nEXTENDS TLC\nCONSTANTS a, b\nVARIABLE x\nInit == x = a\nNext == x' = x\nSyms == Permutations({a,b})\n====\n",
            "CONSTANTS a=a b=b\nINIT Init\nNEXT Next\nSYMMETRY Syms\n",
            "SYMMETRY",
        ),
        (
            "---- MODULE Capture ----\nEXTENDS TLC\nVARIABLE x\nInit == x = RandomElement({TRUE,FALSE})\nNext == x' = x\n====\n",
            "INIT Init\nNEXT Next\n",
            "random",
        ),
        (
            "---- MODULE Capture ----\nEXTENDS TLC\nVARIABLE x\nInit == x = TRUE\nNext == TLCSet(0, 7) /\\ x' = x\n====\n",
            "INIT Init\nNEXT Next\n",
            "stateful",
        ),
    ] {
        let r = stopped(&model(source, cfg, &opts), &opts, Limits::default());
        assert!(
            matches!(&r.verdict, Verdict::Unsupported { feature } if feature.contains(reason)),
            "{:?}",
            r.verdict
        );
        assert!(r.stats.is_none());
    }
}

#[test]
fn pre_pool_initial_stop_reports_the_one_thread_that_ran() {
    let source =
        "---- MODULE Capture ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\nInv == ~x\n====\n";
    let opts = CheckOptions {
        workers: 8,
        ..CheckOptions::tlc(0, 17)
    };
    for config in [
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
        "INIT Init\nNEXT Next\n",
    ] {
        let limits = if config.contains("INVARIANT") {
            Limits::default()
        } else {
            Limits {
                states: 0,
                ..Limits::default()
            }
        };
        let stop = stopped(&model(source, config, &opts), &opts, limits);
        assert_eq!((stop.workers.requested, stop.workers.used), (8, 1));
        assert!(stop.workers.sequential);
        assert!(
            stop.workers
                .reason
                .expect("observed reason")
                .contains("before the worker pool")
        );
    }
}

#[test]
fn runtime_string_choice_respects_reference_intern_order() {
    let source = r#"---- MODULE Capture ----
EXTENDS Naturals, TLC
VARIABLES x, s
Init == x \in 0..15 /\ s = "unset"
Next == x' = x /\ s' = (CHOOSE t \in {ToString(x), ToString(x+1)} : TRUE)
====
"#;
    for store in [StoreMode::Fingerprint, StoreMode::Exact] {
        for base in [CheckOptions::tlc(0, 17), CheckOptions::dive(17)] {
            let inline = CheckOptions {
                store,
                sequential: true,
                ..base.clone()
            };
            let r = complete(&model(source, "INIT Init\nNEXT Next\n", &inline), &inline);
            for workers in [1, 4, 8] {
                for chaos in [None, Some(0), Some(3), Some(33)] {
                    let opts = CheckOptions {
                        store,
                        workers,
                        chaos,
                        ..base.clone()
                    };
                    let p = complete(&model(source, "INIT Init\nNEXT Next\n", &opts), &opts);
                    if base.profile == dive_eval::Profile::Tlc {
                        assert_eq!((p.workers.used, p.workers.sequential), (1, true));
                        assert!(
                            p.workers
                                .reason
                                .as_ref()
                                .expect("reason")
                                .contains("runtime names")
                        );
                    } else {
                        assert_eq!((p.workers.used, p.workers.sequential), (workers, false));
                    }
                    assert_eq!(p.graph.states, r.graph.states, "{workers} {chaos:?}");
                    assert_eq!(p.graph.edges, r.graph.edges, "{workers} {chaos:?}");
                    assert_eq!(p.stats, r.stats, "{workers} {chaos:?}");
                }
            }
        }
    }
}

#[test]
fn runtime_string_ordinary_safety_probe() {
    let source = r#"---- MODULE Capture ----
EXTENDS Naturals, TLC
VARIABLES x, s
Init == x \in 0..15 /\ s = "unset"
Next == x' = x /\ s' = (CHOOSE t \in {ToString(x), ToString(x+1)} : TRUE)
Inv == x >= 14 \/ s = "unset" \/ s = ToString(x)
====
"#;
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\n")
        .into_config()
        .expect("cfg");
    let ir = model(
        source,
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
        &CheckOptions::tlc(0, 17),
    )
    .ir;
    let inline = CheckOptions {
        sequential: true,
        ..CheckOptions::tlc(0, 17)
    };
    let r = dive_check::check(&ir, &cfg, &inline);
    let parallel = CheckOptions {
        workers: 4,
        chaos: Some(0),
        ..CheckOptions::tlc(0, 17)
    };
    let p = dive_check::check(&ir, &cfg, &parallel);
    assert_eq!((p.workers.used, p.workers.sequential), (1, true));
    assert_eq!(observable(&r), observable(&p));
}
