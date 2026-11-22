//! Actual bound roles are startup inputs; configured roots remain attribution inputs.
use super::*;
use std::sync::Arc;

type TestResult = Result<(), Box<dyn std::error::Error>>;
fn bound(
    body: &str,
    cfg: &str,
    options: &CheckOptions,
) -> Result<Model, Box<dyn std::error::Error>> {
    let text = format!(
        "---- MODULE StartupRoles ----\nEXTENDS Naturals,TLC\nVARIABLE x\nInit == x=0\nNext == x'=1-x\n{body}\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("StartupRoles.tla", &text, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let cfg = dive_config::parse(cfg)
        .into_config()
        .ok_or("configuration")?;
    Ok(dive_model::bind(
        ir,
        &cfg,
        &dive_model::BindOptions {
            eval: options.eval,
            symmetry_liveness: options.symmetry_liveness,
            no_deadlock: options.no_deadlock,
            host: None,
        },
    )?)
}
const CONFIG: &str = "INIT Init\nNEXT Next\nPROPERTY P\nCHECK_DEADLOCK FALSE\n";
const BAD: &str = "Bad == IF x=0 THEN 1 \\div 0=0 ELSE TRUE\n";

#[test]
fn actual_role_split_proves_zero_without_interpreting_the_original_initial_role() -> TestResult {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let model = bound(
            &format!("{BAD}P == (x=0) /\\ (([]<>Bad)=>TRUE)"),
            CONFIG,
            &options,
        )?;
        assert_eq!(model.implied_inits.len(), 1);
        assert_eq!(model.implied_temporals.len(), 1);
        let control = evaluator_control(&options)?;
        let error = prepare(&model, Limits::default(), &control)
            .err()
            .ok_or("startup error")?;
        assert!(matches!(error, AnalysisError::StartupTautology { .. }));
        let original = plan::lower_violation(
            &model,
            &model.property_claims()[0].pred,
            Limits::default().formula,
        )?;
        let normalized = crate::tlc_plan::demand::normalize(
            &[(&original.formula, original.atoms.len())],
            model.temporals.len(),
            Limits::default().formula,
        )?
        .ok_or("original envelope")?;
        assert!(
            !normalized.zero_solutions(),
            "an original configured root is not the startup role"
        );
    }
    Ok(())
}

#[test]
fn nonzero_continuation_keeps_legacy_plans_and_split_initial_atoms_cannot_certify_rows()
-> TestResult {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let model = bound(
            "InitialOnly == IF x=0 THEN TRUE ELSE (1 \\div 0=0)\nP == InitialOnly /\\ []<>(x=0)",
            CONFIG,
            &options,
        )?;
        assert_eq!(model.implied_inits.len(), 1);
        let control = evaluator_control(&options)?;
        let plans = prepare(&model, Limits::default(), &control)?;
        let original = plan::lower_violation(
            &model,
            &model.property_claims()[0].pred,
            Limits::default().formula,
        )?;
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].plan.formula, original.formula);
        assert_eq!(plans[0].plan.atoms.len(), original.atoms.len());
        for (actual, expected) in plans[0].plan.atoms.iter().zip(&original.atoms) {
            assert_eq!(
                (
                    actual.kind,
                    actual.test,
                    actual.pred.expr,
                    actual.pred.location
                ),
                (
                    expected.kind,
                    expected.test,
                    expected.pred.expr,
                    expected.pred.location
                )
            );
            for x in [0, 1] {
                let state = St::state(Arc::from([Some(dive_value::Value::Int(x))]));
                assert_eq!(
                    actual.evaluate(&model, &state),
                    expected.evaluate(&model, &state)
                );
            }
        }
        let normalized = crate::tlc_plan::demand::normalize(
            &[(&original.formula, original.atoms.len())],
            model.temporals.len(),
            Limits::default().formula,
        )?
        .ok_or("original envelope")?;
        assert!(!normalized.zero_solutions());
        assert!(
            normalized.into_source_program()?.is_none(),
            "standalone initial atom is not an EA/AE bin"
        );
    }
    Ok(())
}

#[test]
fn shared_actual_conversion_keeps_legacy_errors_and_effect_refusals() -> TestResult {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        for (right, evaluation) in [
            ("(1 \\div 0=0)", true),
            ("PrintT(\"not permitted\")", false),
        ] {
            let model = bound(&format!("{BAD}P == ([]<>Bad)=>{right}"), CONFIG, &options)?;
            let reference = plan::lower_violation(
                &model,
                &model.property_claims()[0].pred,
                Limits::default().formula,
            )
            .err()
            .ok_or("original planning error")?;
            let claims = model
                .implied_temporals
                .iter()
                .map(|p| p.pred.clone())
                .collect::<Vec<_>>();
            let actual = plan::lower_claims(&model, &claims, Limits::default().formula)
                .err()
                .ok_or("actual-role planning error")?;
            assert_eq!(actual.to_string(), reference.to_string());
            if evaluation {
                let (
                    PlanningError::Evaluation {
                        error: actual,
                        source: a,
                    },
                    PlanningError::Evaluation {
                        error: expected,
                        source: b,
                    },
                ) = (actual, reference)
                else {
                    return Err(
                        "constant failure must retain its original evaluation variant".into(),
                    );
                };
                assert_eq!(actual, expected);
                assert_eq!(a, b);
            } else {
                assert!(matches!(actual, PlanningError::Unsupported(_)));
                assert_eq!(model.evaluator.take_output(), Vec::<String>::new());
            }
        }
    }
    // A first tautological claim cannot suppress conversion of a later claim.
    let options = CheckOptions::tlc(0, 0);
    let model = bound(
        &format!("{BAD}P == ([]<>Bad)=>TRUE\nQ == ([]<>Bad)=>(1 \\div 0=0)"),
        "INIT Init\nNEXT Next\nPROPERTIES P Q\nCHECK_DEADLOCK FALSE\n",
        &options,
    )?;
    let control = evaluator_control(&options)?;
    let error = prepare(&model, Limits::default(), &control)
        .err()
        .ok_or("later conversion error")?;
    assert!(matches!(
        error,
        AnalysisError::Planning(PlanningError::Evaluation { .. })
    ));
    Ok(())
}

#[test]
fn expired_preparation_cannot_publish_a_startup_zero_solution() -> TestResult {
    let options = CheckOptions::tlc(0, 0);
    let model = bound(&format!("{BAD}P == ([]<>Bad)=>TRUE"), CONFIG, &options)?;
    let control = evaluator_control(&options)?;
    let watchdog = crate::budget::expired_for_test();
    let _scope = crate::budget::enter(watchdog.budget());
    let error = prepare(&model, Limits::default(), &control)
        .err()
        .ok_or("budget failure")?;
    assert!(matches!(
        error,
        AnalysisError::Limit(crate::budget::EXHAUSTED)
    ));
    Ok(())
}

#[test]
fn intrinsic_dnf_overflow_refuses_but_incomplete_optional_work_does_not_prove_zero() -> TestResult {
    let mut nodes = Vec::new();
    let mut factors = Vec::new();
    for factor in 0..31 {
        let mut choices = Vec::new();
        for branch in 0..2 {
            let atom = nodes.len();
            nodes.push(formula::Node::Atom(crate::formula::Atom {
                kind: AtomKind::State,
                index: factor * 2 + branch,
            }));
            nodes.push(formula::Node::Always(atom));
            nodes.push(formula::Node::Eventually(atom + 1));
            choices.push(atom + 2);
        }
        factors.push(nodes.len());
        nodes.push(formula::Node::Or(choices));
    }
    let product = nodes.len();
    nodes.push(formula::Node::And(factors));
    nodes.push(formula::Node::Not(product));
    nodes.push(formula::Node::Not(product + 1));
    nodes.push(formula::Node::And(vec![product + 2]));
    let root = nodes.len() - 1;
    let formula = formula::Formula::new(nodes, root)?;
    // 31 genuine binary DNF factors exceed Java int before output allocation.
    let error = startup_normalization(&[(&formula, 62)], 0, formula::Limits::default())
        .err()
        .ok_or("intrinsic DNF overflow")?;
    assert!(matches!(
        error,
        AnalysisError::Tableau(formula::FormulaError::Limit("TLC DNF cardinality"))
    ));
    let incomplete = formula::Limits {
        expansion_steps: 0,
        ..formula::Limits::default()
    };
    assert!(startup_normalization(&[(&formula, 62)], 0, incomplete)?.is_none());
    Ok(())
}

#[test]
fn component_choice_requires_three_positive_pairs_in_one_wide_group() -> TestResult {
    use formula::{Atom, Formula, Node};

    // Each distinct SF choice doubles the ordinary group's alternatives.
    // Negating the conjunction moves its clauses outside the eligible context.
    for (pairs, width, negated, expect_pairs) in [
        (0, 8, false, 0),
        (1, 8, false, 0),
        (2, 8, false, 0),
        (3, 1, false, 3),
        (3, 70, false, 3),
        (6, 8, false, 6),
        (4, 8, true, 0),
    ] {
        let mut nodes = Vec::new();
        let mut clauses = Vec::new();
        for i in 0..pairs {
            let n = nodes.len();
            nodes.extend([
                Node::Atom(Atom {
                    kind: AtomKind::State,
                    index: i,
                }),
                Node::Always(n),
                Node::Eventually(n + 1),
                Node::Atom(Atom {
                    kind: AtomKind::Action,
                    index: i,
                }),
                Node::Eventually(n + 3),
                Node::Always(n + 4),
                Node::Or(vec![n + 2, n + 5]),
            ]);
            clauses.push(n + 6);
        }
        // Unrelated FG alternatives alone do not justify factoring. The wide case
        // exceeds the optional one-word condition index.
        let mut choices = Vec::new();
        for i in 0..width {
            let n = nodes.len();
            nodes.extend([
                Node::Atom(Atom {
                    kind: AtomKind::State,
                    index: pairs + i,
                }),
                Node::Always(n),
                Node::Eventually(n + 1),
            ]);
            choices.push(n + 2);
        }
        clauses.push(nodes.len());
        nodes.push(Node::Or(choices));
        nodes.push(Node::And(clauses));
        if negated {
            nodes.push(Node::Not(nodes.len() - 1));
        }
        let root = nodes.len() - 1;
        let formula = Formula::new(nodes, root)?;
        let limits = formula::Limits::default();
        let mut property = PreparedProperty {
            groups: prepare_groups(formula::decompose(&formula, limits)?, limits)?,
            streett: Vec::new(),
            plan: Plan {
                formula,
                atoms: Vec::new(),
            },
            frozen_prefix: None,
        };
        let original = property
            .groups
            .iter()
            .map(|g| g.alternatives.len())
            .sum::<usize>();
        if !negated {
            assert_eq!(original, width << pairs);
            property.prefer_component_search(formula::Limits {
                expansion_steps: 0,
                ..limits
            })?;
            assert!(
                property.streett.is_empty(),
                "optional planning limit retains the original"
            );
        }
        if pairs == 3 {
            let watchdog = crate::budget::expired_for_test();
            let _scope = crate::budget::enter(watchdog.budget());
            assert!(matches!(
                property.prefer_component_search(limits),
                Err(AnalysisError::Limit(crate::budget::EXHAUSTED))
            ));
        }
        property.prefer_component_search(limits)?;
        assert_eq!(property.streett.len(), expect_pairs);
        let selected = property
            .groups
            .iter()
            .map(|g| g.alternatives.len())
            .sum::<usize>();
        assert_eq!(selected, original >> expect_pairs);
    }
    Ok(())
}
