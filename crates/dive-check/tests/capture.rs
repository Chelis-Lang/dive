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
            (8, 1, true)
        );
        assert!(c.workers.reason.expect("reason").contains("capture"));
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
