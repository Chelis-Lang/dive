//! Private domain admission and publication controls.
#![allow(clippy::expect_used, clippy::panic, reason = "finite domain controls")]
use super::*;

fn inputs(source: &str, extra: &str) -> (Arc<Ir>, dive_config::ModelConfig, CheckOptions) {
    let analysis = dive_sema::analyze_source("Names.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse(&format!(
        "INIT Init\nNEXT Next\nPROPERTY P\nCHECK_DEADLOCK FALSE\n{extra}"
    ))
    .into_config()
    .expect("config");
    let mut options = CheckOptions::tlc(0, 0);
    options.workers = 8;
    (ir, config, options)
}
const BODY: &str = "---- MODULE Names ----\nEXTENDS Naturals,Sequences,TLC\nVARIABLE x\nInit == x = <<1,2>>\nNext == x'=SubSeq(x,1,Len(x))\nP == <>[](Len(x)<=3)\n";
fn prepare(
    source: &str,
    extra: &str,
) -> (Arc<Ir>, dive_config::ModelConfig, CheckOptions, Prepared) {
    let (ir, config, options) = inputs(source, extra);
    let model = crate::core_engine::prepare(
        &ir,
        &config,
        &options,
        true,
        dive_core::Opts::default(),
        true,
    )
    .expect("prepare");
    (ir, config, options, model)
}
#[test]
fn actual_domains_and_all_scope_owners_are_required() {
    let source = format!("{BODY}====\n");
    let (ir, config, options, mut prepared) = prepare(&source, "");
    assert!(fixed_name_domain(&prepared));
    assert!(dive_runtime::capture::runtime_names_only(&prepared.model));
    assert!(static_refusal(&ir, &config, &options, &prepared).is_none());
    // Reference pool policy is unchanged even though native demand is guarded.
    assert!(dive_runtime::capture::refusal(&prepared.model, &options, true).is_some());
    let other = dive_model::bind(ir.clone(), &config, &dive_model::BindOptions::tlc())
        .expect("foreign model");
    prepared.model.implied_temporals[0].pred.scope = other.implied_temporals[0].pred.scope.clone();
    assert!(!fixed_name_domain(&prepared));
    assert!(static_refusal(&ir, &config, &options, &prepared).is_some());
    let (_, _, _, mut prepared) = prepare(&source, "");
    prepared.model.names = Arc::new(dive_value::Interner::new());
    assert!(!fixed_name_domain(&prepared));
    let (_, _, _, mut prepared) = prepare(&source, "");
    prepared.store = Arc::new(dive_core::Store::default());
    assert!(!fixed_name_domain(&prepared));
}
#[test]
fn unused_and_configured_effects_do_not_gain_native_admission() {
    for (suffix, extra) in [
        ("Unused(v) == Print(v,TRUE)\n====\n", ""),
        (
            "CONSTANT Op(_)\nDirty(v) == Print(v,TRUE)\nGuard == Op(x)\n====\n",
            "CONSTRAINT Guard\nCONSTANT Op <- Dirty\n",
        ),
    ] {
        let (ir, config, options, prepared) = prepare(&format!("{BODY}{suffix}"), extra);
        assert!(!dive_runtime::capture::runtime_names_only(&prepared.model));
        // Effects gain no parallel width: the requested eight workers
        // capture inline with one, exactly as one requested worker does.
        let width = capture_width(&options, &prepared);
        assert_eq!((width.workers, width.pool), (1, false));
        assert!(
            width
                .one_worker
                .is_some_and(|why| why.contains("one worker"))
        );
        let mut one = options.clone();
        one.workers = 1;
        assert_eq!(
            capture_width(&one, &prepared),
            CaptureWidth {
                workers: 1,
                pool: false,
                one_worker: None
            }
        );
        assert_eq!(
            static_refusal(&ir, &config, &options, &prepared),
            static_refusal(&ir, &config, &one, &prepared)
        );
    }
}

#[test]
fn all_semantic_publication_paths_refuse_late_reference_names() {
    for workers in [1, 8] {
        for route in [
            None,
            Some(Schedule {
                min_states: 1,
                growth_factor: 1,
            }),
        ] {
            for expression in [
                "ToString(x+1)#\"never\"",
                "ToString(x+1)=\"never\"",
                "IF ToString(x+1)=\"never\" THEN TRUE ELSE 1 \\div (x-x)=0",
            ] {
                let source = format!(
                    "---- MODULE Names ----\nEXTENDS Naturals,TLC\nVARIABLE x\nInit == x=37\nNext == UNCHANGED x\nP == <>[]({expression})\n====\n"
                );
                let (ir, config, mut options) = inputs(&source, "");
                options.workers = workers;
                let prepared = crate::core_engine::prepare(
                    &ir,
                    &config,
                    &options,
                    true,
                    dive_core::Opts::default(),
                    true,
                )
                .expect("prepared");
                assert!(fixed_name_domain(&prepared));
                let mut run = CoreRun::default();
                let outcome = check_configured(
                    &ir,
                    &config,
                    &options,
                    (true, dive_core::Opts::default()),
                    prepared,
                    route.map(|schedule| (schedule, crate::TemporalMode::Ordinary, false)),
                    &mut run,
                    crate::temporal::core_file_configuration(),
                );
                assert!(
                    matches!(outcome, CoreOutcome::Checked(ref result) if matches!(result.verdict, Verdict::Unsupported { ref feature } if feature.contains("outside the proved startup domain"))),
                    "{workers} {route:?} {expression}: {outcome:?}"
                );
            }
        }
    }
}

#[test]
fn real_domain_growth_refuses_semantics_but_preserves_nonsemantic_stop_phase() {
    let (ir, _, options, prepared) = prepare(&format!("{BODY}====\n"), "");
    let count = prepared.model.names.len();
    prepared
        .model
        .names
        .intern("a-new-name-not-in-the-startup-domain");
    let stopped = dive_runtime::budget::Stopped {
        resource: dive_runtime::budget::Resource::Time,
        limit: 5,
        observed: 6,
        elapsed_ms: 6,
        phase: Phase::Capture,
    };
    for verdict in [
        Verdict::Stopped(Box::new(stopped.clone())),
        Verdict::ResourceLimit {
            message: "allocation refused".into(),
        },
        Verdict::InternalError {
            message: "actual internal failure".into(),
        },
    ] {
        let expected = verdict.clone();
        let outcome = guard_outcome(
            true,
            &prepared.model.names,
            count,
            CoreOutcome::Checked(Box::new(crate::core_engine::result(verdict, &options, &ir))),
            &options,
            &ir,
        );
        assert!(matches!(outcome,CoreOutcome::Checked(ref result) if result.verdict==expected));
    }
    let outcome = guard_outcome(
        true,
        &prepared.model.names,
        count,
        CoreOutcome::Checked(Box::new(crate::core_engine::result(
            Verdict::Success,
            &options,
            &ir,
        ))),
        &options,
        &ir,
    );
    assert!(
        matches!(outcome,CoreOutcome::Checked(ref result) if matches!(result.verdict,Verdict::Unsupported{..}))
    );
}
