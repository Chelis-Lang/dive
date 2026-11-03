//! Bounds and unsupported features never become sampled successes.
#![allow(clippy::unwrap_used, reason = "test setup failures must fail the test")]
use dive_sim::{Options, Outcome, simulate};
use dive_stdlib::random::RandomMode;
use std::sync::Arc;

fn run(source: &str, config: &str, options: &Options) -> dive_sim::Report {
    run_with(source, config, options, |_| {})
}

fn run_with(
    source: &str,
    config: &str,
    options: &Options,
    edit: fn(&mut dive_model::Model),
) -> dive_sim::Report {
    run_with_name("M.tla", source, config, options, edit)
}

fn run_with_name(
    filename: &str,
    source: &str,
    config: &str,
    options: &Options,
    edit: fn(&mut dive_model::Model),
) -> dive_sim::Report {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(scope, || {
                let analysis =
                    dive_sema::analyze_source(filename, source, &dive_sema::Options::default());
                assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
                let ir = Arc::new(dive_ir::lower(&analysis).unwrap());
                let config = dive_config::parse(config).into_config().unwrap();
                let mut model =
                    dive_model::bind(ir, &config, &dive_model::BindOptions::tlc()).unwrap();
                edit(&mut model);
                simulate(&model, options)
            })
            .unwrap()
            .join()
            .unwrap()
    })
}

#[test]
fn configured_view_refuses_without_assumption_init_or_step_effects() {
    for mode in [RandomMode::Tlc, RandomMode::Dive] {
        let report = run_with_name(
            "SimView.tla",
            include_str!("fixtures/SimView.tla"),
            include_str!("fixtures/SimView.cfg"),
            &Options::new(7, mode, 1, 1),
            |_| {},
        );
        assert!(matches!(report.outcome,
            Outcome::Unsupported(ref feature) if feature == "simulation VIEW declarations"
        ));
        assert_eq!(report.stage, "preflight");
        assert_eq!(report.generated, 0);
        assert_eq!(report.traces_started, 0);
        assert_eq!(report.runtime_output, []);
        assert!(report.traces.is_empty());
    }
}

#[test]
fn exhausted_budgets_keep_partial_statistics_and_refuse() {
    let source = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\n====\n";
    let mut options = Options::new(0, RandomMode::Tlc, 10, 1);
    options.max_generated = 2;
    let report = run(source, "INIT Init\nNEXT Next\n", &options);
    assert!(
        matches!(report.outcome, Outcome::Limit(_)),
        "{:?}",
        report.outcome
    );
    assert_eq!(report.generated, 2);
    assert_eq!(report.stage, "sampling");
    assert_eq!(report.traces_started, 1);
    options.max_generated = 100;
    options.max_retained = 2;
    assert!(matches!(
        run(source, "INIT Init\nNEXT Next\n", &options).outcome,
        Outcome::Limit(_)
    ));
    options.max_candidates = 0;
    let report = run(source, "INIT Init\nNEXT Next\n", &options);
    assert_eq!(report.stage, "initialization");
    assert!(matches!(report.outcome, Outcome::Limit(_)));
}

#[test]
fn temporal_properties_are_named_refusals() {
    let source = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\nLive == []<>(x = 0)\n====\n";
    for mode in [RandomMode::Tlc, RandomMode::Dive] {
        let report = run(
            source,
            "INIT Init\nNEXT Next\nPROPERTY Live\n",
            &Options::new(0, mode, 3, 1),
        );
        assert!(
            matches!(&report.outcome, Outcome::Unsupported(f) if f.contains("temporal properties")),
            "{:?}",
            report.outcome
        );
        assert_eq!(report.traces_started, 0);
    }
}

// ADR 0116: compound initial values and fairness-only specifications are
// simulated in both random modes.
#[test]
fn compound_initial_values_and_fairness_only_specs_are_simulated() {
    let compound = "---- MODULE M ----\nEXTENDS Naturals, Sequences\nVARIABLES q, f\nInit == q = <<>> /\\ f = [i \\in {1, 2} |-> {}]\nNext == \\/ /\\ Len(q) < 3\n           /\\ q' = Append(q, Len(q))\n           /\\ f' = [f EXCEPT ![1] = @ \\cup {Len(q)}]\n        \\/ UNCHANGED <<q, f>>\nInv == Len(q) < 3\n====\n";
    let fair = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = (x + 1) % 4\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nInv == x < 3\n====\n";
    for mode in [RandomMode::Tlc, RandomMode::Dive] {
        let options = Options::new(0, mode, 10, 3);
        let report = run(compound, "INIT Init\nNEXT Next\n", &options);
        assert!(
            matches!(report.outcome, Outcome::Sampled),
            "{:?}",
            report.outcome
        );
        assert_eq!(report.traces_started, 3);
        for (source, config, length) in [
            // The stuttering disjunct can delay the violating append.
            (compound, "INIT Init\nNEXT Next\nINVARIANT Inv\n", 4),
            (fair, "SPECIFICATION Spec\nINVARIANT Inv\n", 4),
        ] {
            let report = run(source, config, &options);
            assert!(
                matches!(
                    &report.outcome,
                    Outcome::Violation { kind: "invariant", name: Some(n), trace, .. }
                        if n == "Inv" && report.traces[*trace].states.len() >= length
                ),
                "{:?}",
                report.outcome
            );
        }
    }
}

#[test]
fn initial_enumeration_finishes_before_checking_completeness() {
    let mut source = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLES x, y\nInit == (x = 1) \\/ (x = 0 /\\ y = 1 \\div x)\nNext == x' = x /\\ y' = y\n====\n".to_owned();
    let options = Options::new(0, RandomMode::Tlc, 4, 1);
    let report = run(&source, "INIT Init\nNEXT Next\n", &options);
    assert!(
        matches!(report.outcome, Outcome::Error { code: 1000, .. }),
        "{report:?}"
    );
    assert_eq!(report.generated, 0);
    assert_eq!(report.initial_candidates, 1);
    source = source.replace("y = 1 \\div x", "y = 0");
    let report = run(&source, "INIT Init\nNEXT Next\n", &options);
    assert!(
        matches!(report.outcome, Outcome::Error { code: 2106, .. }),
        "{report:?}"
    );
    assert_eq!(report.generated, 2);
    assert_eq!(report.initial_candidates, 2);
}

#[test]
fn failure_traces_respect_zero_and_tiny_retained_budgets() {
    let source = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = 1\nInv == x = 0\nBadInit == x = 1\n====\n";
    let mut options = Options::new(0, RandomMode::Tlc, 3, 2);
    options.max_retained = 0;
    let report = run(source, "INIT BadInit\nNEXT Next\nINVARIANT Inv\n", &options);
    assert!(matches!(report.outcome, Outcome::Limit(_)), "{report:?}");
    assert!(report.traces.is_empty());
    assert_eq!(report.pending_violation.unwrap().code, 2107);
    options.max_retained = 1;
    let report = run(source, "INIT Init\nNEXT Next\nINVARIANT Inv\n", &options);
    assert!(matches!(report.outcome, Outcome::Limit(_)), "{report:?}");
    assert_eq!(
        report.traces.iter().map(|t| t.states.len()).sum::<usize>(),
        1
    );
    assert_eq!(report.generated, 2);
    assert_eq!(report.pending_violation.unwrap().code, 2110);
    let report = run(source, "INIT BadInit\nNEXT Next\nINVARIANT Inv\n", &options);
    assert!(matches!(
        report.outcome,
        Outcome::Violation { code: 2107, .. }
    ));
    assert_eq!(report.traces[0].states.len(), 1);
    options.max_retained = 2;
    let report = run(source, "INIT Init\nNEXT Next\nINVARIANT Inv\n", &options);
    assert!(matches!(
        report.outcome,
        Outcome::Violation { code: 2110, .. }
    ));
    assert_eq!(report.traces[0].states.len(), 2);
    assert!(report.pending_violation.is_none());
}

#[test]
fn basic_action_flows_count_chosen_transitions_and_keep_initial_provenance() {
    let source = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nA == x = 0 /\\ x' = 1\nB == x = 1 /\\ x' = 2\nDisabled == x < 0 /\\ x' = x\nNext == A \\/ Disabled \\/ B\n====\n";
    for mode in [RandomMode::Dive, RandomMode::Tlc] {
        let mut options = Options::new(7, mode, 2, 3);
        options.action_stats = true;
        let report = run(source, "INIT Init\nNEXT Next\n", &options);
        assert!(matches!(report.outcome, Outcome::Sampled), "{report:?}");
        let flow = report.action_flow.unwrap();
        let group = |name| flow.actions.iter().find(|a| a.name == name).unwrap().id;
        assert_eq!(flow.transitions.len(), 2);
        assert_eq!(flow.transitions[&(group("Init"), group("A"))], 3);
        assert_eq!(flow.transitions[&(group("A"), group("B"))], 3);
        assert!(flow.actions[group("Init")].initial);
        assert!(
            !flow
                .transitions
                .keys()
                .any(|&(a, b)| { a == group("Disabled") || b == group("Disabled") })
        );
    }
}

#[test]
fn basic_action_flows_combine_parameter_contexts_by_predicate_location() {
    let source = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nA(k) == x' = k\nNext == \\E k \\in {1, 2} : A(k)\n====\n";
    let mut options = Options::new(0, RandomMode::Tlc, 3, 4);
    options.action_stats = true;
    let report = run(source, "INIT Init\nNEXT Next\n", &options);
    assert!(matches!(report.outcome, Outcome::Sampled), "{report:?}");
    let flow = report.action_flow.unwrap();
    assert_eq!(flow.actions.len(), 2);
    let action = flow.actions.iter().find(|a| a.name == "A").unwrap();
    assert_eq!(action.bound_action_ids.len(), 2);
    let init = flow.actions.iter().find(|a| a.initial).unwrap().id;
    assert_eq!(flow.transitions[&(init, action.id)], 4);
    assert_eq!(flow.transitions[&(action.id, action.id)], 8);
}

#[test]
fn basic_action_flows_exclude_duplicates_and_constrained_candidates() {
    let source = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nA == x >= 0 /\\ ((x' = 1) \\/ (x' = 1) \\/ (x' = 2))\nNext == A\nLimit == x < 2\nStep == x' = 1\n====\n";
    let mut options = Options::new(0, RandomMode::Tlc, 1, 1);
    options.action_stats = true;
    let report = run(
        source,
        "INIT Init\nNEXT Next\nCONSTRAINT Limit\nACTION_CONSTRAINT Step\n",
        &options,
    );
    assert!(matches!(report.outcome, Outcome::Sampled), "{report:?}");
    // The unsplit guarded action emits 1 twice, then 2; constraints keep both copies of 1.
    assert_eq!(report.generated, 4);
    let flow = report.action_flow.unwrap();
    assert_eq!(flow.transitions.values().sum::<u64>(), 1);
}

#[test]
fn basic_action_flows_preserve_failure_and_budget_prefixes() {
    let source = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\nInv == x < 2\nDead == x > 0 /\\ x' = x\n====\n";
    let mut options = Options::new(0, RandomMode::Tlc, 3, 1);
    options.action_stats = true;
    let report = run(source, "INIT Init\nNEXT Next\nINVARIANT Inv\n", &options);
    assert!(matches!(
        report.outcome,
        Outcome::Violation { code: 2110, .. }
    ));
    assert_eq!(report.generated, 3);
    assert_eq!(
        report
            .action_flow
            .unwrap()
            .transitions
            .values()
            .sum::<u64>(),
        1
    );
    options.max_generated = 2;
    let report = run(source, "INIT Init\nNEXT Next\n", &options);
    assert!(matches!(report.outcome, Outcome::Limit(_)));
    assert_eq!(
        report
            .action_flow
            .unwrap()
            .transitions
            .values()
            .sum::<u64>(),
        1
    );
    let report = run(source, "INIT Init\nNEXT Dead\n", &options);
    assert!(matches!(
        report.outcome,
        Outcome::Violation { code: 2114, .. }
    ));
    assert!(report.action_flow.unwrap().transitions.is_empty());
    let failing = source.replace("x' = x + 1", "x' = 1 \\div x");
    let report = run(&failing, "INIT Init\nNEXT Next\n", &options);
    assert!(!matches!(report.outcome, Outcome::Sampled));
    assert!(report.action_flow.unwrap().transitions.is_empty());
}

#[test]
fn basic_action_flows_are_empty_before_selection_and_absent_on_refusal() {
    let source = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = 1\nInv == x > 0\nNever == x < 0\n====\n";
    let mut options = Options::new(0, RandomMode::Tlc, 0, 2);
    options.action_stats = true;
    for config in [
        "INIT Init\nNEXT Next\n",
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
        "INIT Init\nNEXT Next\nCONSTRAINT Never\n",
    ] {
        let report = run(source, config, &options);
        assert!(report.action_flow.unwrap().transitions.is_empty());
        assert_eq!(report.generated, 1);
    }
    options.action_stats = false;
    assert!(
        run(source, "INIT Init\nNEXT Next\n", &options)
            .action_flow
            .is_none()
    );
    options.action_stats = true;
    options.traces = 0;
    let report = run(source, "INIT Init\nNEXT Next\n", &options);
    assert!(matches!(report.outcome, Outcome::Unsupported(_)));
    assert!(report.action_flow.is_none());
}

#[test]
fn basic_action_flows_refuse_caller_collisions_before_evaluation() {
    let source = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\n====\n";
    let mut options = Options::new(0, RandomMode::Tlc, 3, 1);
    options.action_stats = true;
    let report = run_with(source, "INIT Init\nNEXT Next\n", &options, |model| {
        model.actions[0] = model.init_actions[0].clone();
    });
    assert!(
        matches!(report.outcome, Outcome::Unsupported(ref feature)
        if feature.contains("unique bound action IDs")),
        "{report:?}"
    );
    assert_eq!(report.stage, "preflight");
    assert_eq!(report.generated, 0);
    assert!(report.action_flow.is_none());
}

#[test]
fn basic_action_flows_keep_unused_value_bound_invocations_separate() {
    // The pinned SharedFalse production DOT has Init, both invocation
    // locations and Step. Neither unused invocation receives a chosen edge.
    let source = include_str!("../../dive-model/tests/fixtures/action-inventory/SharedFalse.tla");
    let mut options = Options::new(0, RandomMode::Tlc, 2, 1);
    options.action_stats = true;
    for mode in [RandomMode::Tlc, RandomMode::Dive] {
        options.mode = mode;
        let report = run_with_name(
            "SharedFalse.tla",
            source,
            "INIT Init\nNEXT Next\n",
            &options,
            |_| {},
        );
        assert!(matches!(report.outcome, Outcome::Sampled), "{report:?}");
        let flow = report.action_flow.unwrap();
        assert_eq!(flow.actions.len(), 4);
        assert_eq!(flow.actions[1].name, "FalseAction");
        assert_eq!(flow.actions[2].name, "FalseAction");
        assert_eq!(flow.actions[1].bound_action_ids, [1]);
        assert_eq!(flow.actions[2].bound_action_ids, [2]);
        assert_ne!(flow.actions[1].location, flow.actions[2].location);
        assert_eq!(
            flow.transitions,
            std::collections::BTreeMap::from([((0, 3), 1), ((3, 3), 1)])
        );
    }
}

#[test]
fn basic_action_flows_keep_standard_operator_action_names() {
    let source = include_str!(
        "../../dive-model/tests/fixtures/action-inventory/FailedConstantEvaluation.tla"
    );
    let mut options = Options::new(0, RandomMode::Tlc, 0, 1);
    options.action_stats = true;
    let report = run_with_name(
        "FailedConstantEvaluation.tla",
        source,
        "INIT Init\nNEXT Next\n",
        &options,
        |_| {},
    );
    assert!(matches!(report.outcome, Outcome::Sampled), "{report:?}");
    let flow = report.action_flow.unwrap();
    assert_eq!(flow.actions[1].name, "\\div");
    assert!(flow.actions[1].label.starts_with("<\\div "));
    assert!(
        flow.transitions.is_empty(),
        "depth zero does not evaluate the failing action"
    );
}

#[test]
fn reached_randomness_is_seeded_during_actual_sampling() {
    let source = "---- MODULE M ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == x = 0\nNext == x' = RandomElement({1, 2})\n====\n";
    let report = run(
        source,
        "INIT Init\nNEXT Next\n",
        &Options::new(0, RandomMode::Tlc, 3, 1),
    );
    assert!(matches!(report.outcome, Outcome::Sampled), "{report:?}");
    assert_eq!(report.stage, "sampling");
    assert_eq!(report.traces_started, 1);
    assert_eq!(report.generated, 4);
    assert_eq!(report.runtime_output.len(), 0);
    assert_eq!(report.traces[0].states.len(), 4);
    for row in &report.traces[0].states[1..] {
        assert!(matches!(row[0], Some(dive_value::Value::Int(1 | 2))));
    }
}
