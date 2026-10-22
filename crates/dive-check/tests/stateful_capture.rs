//! Structural sequential requirements must not become stateful capture refusals.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "parsed model control assertions"
)]
use dive_check::{
    CheckOptions, Verdict,
    capture::{self, Exploration, Limits},
};
use dive_eval::{ActionComposition, Host, HostError};
use dive_ir::StdOp;
use dive_model::Model;
use dive_value::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn options(workers: usize) -> CheckOptions {
    let mut opts = CheckOptions::tlc(0, 0);
    opts.eval.action_composition = ActionComposition::Tlc;
    opts.workers = workers;
    opts
}
fn input(source: &str, config: &str, opts: &CheckOptions) -> Model {
    let name = source.split_whitespace().nth(2).expect("module");
    let mut analysis_options = dive_sema::Options::default();
    analysis_options.search.after_standard.push(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../dive-eval/tests/fixtures/community-modules"),
    );
    let analysis = dive_sema::analyze_source(&format!("{name}.tla"), source, &analysis_options);
    assert!(
        analysis.accepted(),
        "{:?} {:?}",
        analysis.diagnostics(),
        analysis.syntax_diagnostics()
    );
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse(config).into_config().expect("config");
    dive_model::bind(
        ir,
        &cfg,
        &dive_model::BindOptions {
            eval: opts.eval,
            symmetry_liveness: opts.symmetry_liveness,
            no_deadlock: false,
            host: None,
        },
    )
    .expect("bind")
}
fn source(first: &str, composition: bool) -> String {
    let next = if composition {
        "First \\cdot Last"
    } else {
        "First"
    };
    format!(
        "---- MODULE Names ----\nEXTENDS Naturals, Sequences, TLC, IOUtils\nVARIABLE s\nInit == s = <<0,1>>\nFirst == {first}\nLast == s'=s\nNext == {next}\n====\n"
    )
}
#[test]
fn parsed_classification_preserves_structural_flags_and_clone_metadata() {
    for (first, names) in [("s'=s", false), ("s'=SubSeq(s,1,1)", true)] {
        for composition in [false, true] {
            let m = input(
                &source(first, composition),
                "INIT Init\nNEXT Next\n",
                &options(1),
            );
            let mut expected = Vec::new();
            if composition {
                expected.push(dive_model::TLC_CDOT.to_owned());
            }
            if names {
                expected.push(
                    "Sequences!SubSeq (runtime names require TLC encounter order)".to_owned(),
                );
            }
            expected.sort();
            assert_eq!(m.sequential, expected);
            assert_eq!(m.sequential_names_only, names && !composition);
            assert_eq!(m.runtime_name_order, names);
            assert_eq!(m.stateful_checking(), None);
            assert_eq!(m.clone().stateful_checking(), None);
        }
    }
}
#[derive(Default, Debug)]
struct Calls(AtomicUsize);
impl Host for Calls {
    fn check_native_access(&self, _op: StdOp) -> Result<(), HostError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
#[test]
fn mixed_real_effects_still_refuse_before_host_or_output_changes() {
    for effect in [
        "TLCSet(0,7)",
        "TLCGet(0)=7",
        "TLCGet(\"generated\")>=0",
        "RandomElement({TRUE,FALSE})",
        "IOExec(<<s>>)=TRUE",
        "Len({v \\in {0} : PrintT(v)})=1",
        "[v \\in {0} |-> PrintT(v)][0]",
        "LET Lazy=={v \\in {0}:PrintT(v)} IN Len(Lazy)=1",
    ] {
        let first = format!("{effect} /\\ s'=SubSeq(s,1,1)");
        let m = input(&source(&first, true), "INIT Init\nNEXT Next\n", &options(1));
        assert!(
            m.stateful_checking().is_some(),
            "{effect}: {:?}",
            m.sequential
        );
        assert_eq!(m.clone().stateful_checking(), m.stateful_checking());
        assert!(m.runtime_name_order && !m.sequential_names_only);
        assert!(m.sequential.iter().any(|r| r == dive_model::TLC_CDOT));
        let calls = Arc::new(Calls::default());
        let original: Arc<dyn Host> = calls.clone();
        m.evaluator.set_host(original.clone());
        let Exploration::Stopped(r) = capture::explore(&m, &options(1), Limits::default()) else {
            panic!("effect admitted: {effect}")
        };
        assert!(matches!(r.verdict, Verdict::Unsupported { .. }), "{r:?}");
        assert!(r.stats.is_none());
        assert_eq!(calls.0.load(Ordering::SeqCst), 0);
        assert!(Arc::ptr_eq(&m.evaluator.host().expect("host"), &original));
        assert_eq!(m.evaluator.take_output(), [] as [String; 0]);
    }
}
fn word(c: &capture::Complete) -> Vec<&str> {
    c.graph
        .states
        .iter()
        .map(|s| match &s[0] {
            Some(Value::Str(a)) => a.as_str(),
            _ => panic!("string state"),
        })
        .collect()
}
#[test]
fn actual_runtime_names_and_composition_keep_primary_values_counts_and_inline_order() {
    let primary = include_str!("../../dive-api/tests/fixtures/name-cdot/NameCdot.tla");
    let original = include_str!("../../dive-api/tests/fixtures/name-cdot/original/NameCdot.tla");
    for (source, expected, growth) in [(original, "a", false), (primary, "ba", true)] {
        for width in [1, 4] {
            let opts = options(width);
            let m = input(source, "INIT Init\nNEXT Next\n", &opts);
            assert!(!m.has_runtime_names());
            let names_before = m.names.len();
            if growth {
                assert!(m.names.get("ba").is_none() && m.names.get("ab").is_none());
            } else {
                assert!(m.names.get("a").is_some() && m.names.get("b").is_some());
            }
            for _ in 0..2 {
                let Exploration::Complete(c) = capture::explore(&m, &opts, Limits::default())
                else {
                    panic!("mixed capture refused")
                };
                assert_eq!(word(&c), ["baba", expected], "actual primary word");
                assert_eq!(
                    (c.stats.generated, c.stats.distinct, c.stats.depth),
                    (5, 2, 2)
                );
                assert_eq!(
                    (c.workers.requested, c.workers.used, c.workers.sequential),
                    (width, 1, true)
                );
                assert_eq!(c.output, [] as [String; 0]);
                assert_eq!(m.has_runtime_names(), growth);
                if growth {
                    assert!(m.names.len() > names_before);
                    assert!(m.names.get("ba").is_some() && m.names.get("ab").is_some());
                }
            }
            // The ordinary safety engine stops on the same actual successor.
            let original = source.replace("====", "Original == Len(s)=4\n====");
            let safety = input(
                &original,
                "INIT Init\nNEXT Next\nINVARIANT Original\n",
                &opts,
            );
            let cfg = dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Original\n")
                .into_config()
                .expect("config");
            let result = dive_check::check(&safety.ir, &cfg, &opts);
            assert!(matches!(result.verdict, Verdict::InvariantViolated { .. }));
            let states = &result.counterexample.expect("ordinary witness").states;
            assert!(matches!(&states[1].values[0],Value::Str(a) if a.as_str()==expected));
        }
    }
}

#[test]
fn mixed_structural_requirements_do_not_admit_parallel_or_dfid() {
    let opts = options(1);
    let m = input(
        &source("s'=SubSeq(s,1,1)", true),
        "INIT Init\nNEXT Next\n",
        &opts,
    );
    let parallel = capture::explore_concrete_parallel_with_evidence(&m, &opts, Limits::default())
        .expect("typed stop");
    assert!(
        matches!(parallel,Exploration::Stopped(ref r) if matches!(r.verdict,Verdict::Unsupported{..}) && r.workers.used==0 && r.stats.is_none())
    );
    let report = dive_check::dfid::check(&m, &dive_check::dfid::Options::new(opts, 2));
    assert!(matches!(
        report.outcome,
        dive_check::dfid::Outcome::Unsupported(_)
    ));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    let path = std::env::temp_dir().join(format!(
        "dive-mixed-refuse-{}-{}.cp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ));
    let result = dive_check::checkpoint::check(
        &m.ir,
        &cfg,
        &options(1),
        &dive_check::checkpoint::Request {
            path: path.clone(),
            action: dive_check::checkpoint::Action::PauseAfterLevels(0),
            counters: dive_check::checkpoint::Counters::Tlc,
            config_source_sha256: None,
        },
    );
    assert!(matches!(result.check.verdict, Verdict::Unsupported { .. }));
    assert!(result.receipt.is_none() && !path.exists());
    assert_eq!(m.evaluator.take_output(), [] as [String; 0]);
}
