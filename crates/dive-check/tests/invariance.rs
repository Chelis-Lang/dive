//! Worker invariance (ADR 0015, milestone CM3): every result field except
//! the thread fields and the timing is identical at every worker count, in
//! chaos mode, and in sequential mode (the inline engine), in both stores
//! and both profiles.
//!
//! The specifications are the TLC probes, the harness's TLC fixtures, and
//! `fixtures/invariance/Wide.tla`, which stops part-way through a wide level
//! with each kind of event while printing from every kind of evaluation.

#![allow(clippy::panic, clippy::expect_used, reason = "test code")]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dive_check::{CheckOptions, CheckResult, Verdict, check};
use dive_store::StoreMode;

/// Everything a result reports except the thread fields and the timing.
fn observable(r: &CheckResult) -> String {
    format!(
        "{:?}\n{:?}\n{}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}\n{:?}",
        r.verdict,
        r.stats,
        r.tlc_prints_stats,
        r.counterexample,
        r.collision,
        r.output,
        r.warnings,
        r.notes,
        r.store
    )
}

fn load(tla: &Path, cfg: &str) -> (Arc<dive_ir::Ir>, dive_config::ModelConfig) {
    let analysis = dive_sema::analyze_file(tla, &dive_sema::Options::default()).expect("readable");
    assert!(analysis.accepted(), "{} rejected", tla.display());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lowers"));
    let cfg = dive_config::parse(cfg).into_config().expect("cfg parses");
    (ir, cfg)
}

/// The run settings compared: `(workers, sequential, chaos)`.
fn settings(chaos_seeds: usize) -> Vec<(usize, bool, Option<u64>)> {
    let mut out = vec![(1, true, None), (8, true, None)];
    for w in [1, 2, 3, 4, 8, 64] {
        out.push((w, false, None));
    }
    for seed in 0..chaos_seeds {
        out.push((1 + seed % 8, false, Some(0x5eed ^ seed as u64)));
    }
    out
}

/// Checks `tla` with `cfg` under every setting; returns the result at 4
/// workers, or the first difference from the sequential (inline) run.
fn invariant(
    name: &str,
    tla: &Path,
    cfg: &str,
    base: &CheckOptions,
    chaos_seeds: usize,
) -> Result<CheckResult, String> {
    let (ir, cfg) = load(tla, cfg);
    let mut reference: Option<(String, CheckResult)> = None;
    let mut parallel: Option<CheckResult> = None;
    for (workers, sequential, chaos) in settings(chaos_seeds) {
        let mut opts = base.clone();
        opts.workers = workers;
        opts.sequential = sequential;
        opts.chaos = chaos;
        let r = check(&ir, &cfg, &opts);
        let got = observable(&r);
        if !sequential && r.workers.used == 1 && workers > 1 && !r.workers.sequential {
            return Err(format!("{name}: {workers} workers requested, 1 used"));
        }
        match &reference {
            None => reference = Some((got, r.clone())),
            Some((want, _)) if *want != got => {
                return Err(format!(
                    "{name} ({:?}, {:?}): workers {workers}, sequential {sequential}, chaos {chaos:?} differs\n--- inline\n{want}\n--- this run\n{got}",
                    base.profile, base.store
                ));
            }
            Some(_) => {}
        }
        if workers == 4 && !sequential && chaos.is_none() {
            parallel = Some(r);
        }
    }
    Ok(parallel.expect("a run at 4 workers"))
}

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn on_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(dive_eval::RECOMMENDED_STACK)
        .spawn(f)
        .expect("thread")
        .join()
        .expect("no panic")
}

fn profiles() -> Vec<CheckOptions> {
    let mut out = Vec::new();
    for store in [StoreMode::Fingerprint, StoreMode::Exact] {
        let mut tlc = CheckOptions::tlc(0, 0);
        tlc.store = store;
        out.push(tlc);
        let mut dive = CheckOptions::dive(0);
        dive.store = store;
        out.push(dive);
    }
    out
}

fn wide_cfg(mode: &str) -> String {
    format!(
        "CONSTANT Mode = \"{mode}\"\nINIT Init\nNEXT Next\nINVARIANT Inv\nCONSTRAINT Constr\nPROPERTY Prop\nCHECK_DEADLOCK {}\n",
        if mode == "deadlock" { "TRUE" } else { "FALSE" }
    )
}

#[test]
fn wide_levels_stop_identically_at_every_worker_count() {
    let failures = on_big_stack(|| {
        let tla = crate_dir().join("tests/fixtures/invariance/Wide.tla");
        let mut failures = Vec::new();
        for (mode, expect) in [
            ("success", "Success"),
            ("invariant", "InvariantViolated"),
            ("error", "EvaluationError"),
            ("deadlock", "Deadlock"),
            ("incomplete", "NotCompletelySpecified"),
            ("constraint", "InvariantViolated"),
            ("implied", "ActionPropertyViolated"),
        ] {
            for base in profiles() {
                match invariant(&format!("Wide {mode}"), &tla, &wide_cfg(mode), &base, 12) {
                    Ok(r) => {
                        let v = format!("{:?}", r.verdict);
                        if !v.starts_with(expect) {
                            failures.push(format!("Wide {mode}: expected {expect}, got {v}"));
                        }
                        if r.workers.sequential {
                            failures.push(format!("Wide {mode}: ran in sequential mode"));
                        }
                    }
                    Err(e) => failures.push(e),
                }
            }
        }
        failures
    });
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn wide_print_output_is_released_in_order() {
    let r = on_big_stack(|| {
        let tla = crate_dir().join("tests/fixtures/invariance/Wide.tla");
        invariant(
            "Wide success",
            &tla,
            &wide_cfg("success"),
            &CheckOptions::tlc(0, 0),
            4,
        )
    })
    .expect("invariant");
    assert_eq!(r.verdict, Verdict::Success);
    // Every kind of evaluation printed.
    for tag in ["\"A\"", "\"inv\"", "\"constr\"", "\"prop\""] {
        assert!(
            r.output.iter().any(|l| l.contains(tag)),
            "no {tag} output: {:?}",
            r.output
        );
    }
}

fn probe_dirs() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let probes = crate_dir().join("tests/fixtures/tlc-probes/specs");
    let mut names: Vec<String> = std::fs::read_dir(&probes)
        .expect("probes")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    for n in names {
        out.push((n.clone(), probes.join(&n)));
    }
    let harness = crate_dir().join("../dive-conformance/tests/fixtures/specs");
    for n in [
        "Success",
        "Deadlock",
        "InvariantViolation",
        "InitInvariant",
        "InitProperty",
        "ActionProperty",
        "AssertFailure",
        "EvalError",
        "Overflow",
        "Assumption",
        "PrintOutput",
    ] {
        out.push((n.to_owned(), harness.join(n)));
    }
    out
}

#[test]
fn probes_are_invariant() {
    let failures = on_big_stack(|| {
        let mut failures = Vec::new();
        for (name, dir) in probe_dirs() {
            let cfg = std::fs::read_to_string(dir.join(format!("{name}.cfg"))).expect("cfg");
            for base in profiles() {
                // With big integers `Overflow` counts up for ever.
                if name == "Overflow" && base.profile == dive_eval::Profile::Dive {
                    continue;
                }
                if let Err(e) = invariant(&name, &dir.join(format!("{name}.tla")), &cfg, &base, 6) {
                    failures.push(e);
                }
            }
        }
        failures
    });
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

fn check_source(src: &str, cfg: &str, workers: usize) -> CheckResult {
    let analysis = dive_sema::analyze_source("M.tla", src, &dive_sema::Options::default());
    assert!(analysis.accepted(), "rejected: {src}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lowers"));
    let cfg = dive_config::parse(cfg).into_config().expect("cfg");
    let mut opts = CheckOptions::tlc(0, 0);
    opts.workers = workers;
    check(&ir, &cfg, &opts)
}

#[test]
fn sequential_mode_is_detected_and_reported() {
    let cases: Vec<(&str, &str)> = vec![
        // Writes a register.
        (
            "EXTENDS Naturals, TLC\nVARIABLE x\nInit == x = 0\nNext == x < 3 /\\ x' = x + 1 /\\ TLCSet(1, x)\n",
            "TLCSet",
        ),
        // Reads a live counter.
        (
            "EXTENDS Naturals, TLC\nVARIABLE x\nInit == x = 0\nNext == x < 3 /\\ x' = x + 1\nInv == TLCGet(\"distinct\") < 100\n",
            "TLCGet(\"distinct\")",
        ),
        // Draws random values.
        (
            "EXTENDS Naturals, TLC\nVARIABLE x\nInit == x = 0\nNext == x < 3 /\\ x' = RandomElement({x + 1})\n",
            "RandomElement",
        ),
        // Prints inside a lazily evaluated set filter.
        (
            "EXTENDS Naturals, TLC\nVARIABLE x\nInit == x = 0\nNext == x < 3 /\\ x' \\in {y \\in {x + 1} : PrintT(y)}\n",
            "set filter",
        ),
    ];
    let failures = on_big_stack(move || {
        let mut failures = Vec::new();
        for (body, feature) in cases {
            let src = format!("---- MODULE M ----\n{body}====\n");
            let cfg = if body.contains("Inv ==") {
                "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n"
            } else {
                "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n"
            };
            let r = check_source(&src, cfg, 4);
            let w = &r.workers;
            let reported = w.sequential
                && w.used == 1
                && w.requested == 4
                && w.reason.as_deref().is_some_and(|why| why.contains(feature));
            if !reported {
                failures.push(format!("{feature}: {w:?} ({:?})", r.verdict));
            }
        }
        // `TLCGet("level")` and `Print` in the next-state relation do not
        // force sequential mode.
        let src = "---- MODULE M ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == x = 0\nNext == x < 3 /\\ x' = x + 1 /\\ PrintT(x) /\\ TLCGet(\"level\") > 0\nInv == x < 10\n====\n";
        let r = check_source(
            src,
            "INIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n",
            4,
        );
        if r.workers.sequential || r.workers.used != 4 {
            failures.push(format!("Print: {:?}", r.workers));
        }
        failures
    });
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Runtime strings must be created in inline TLC encounter order. A pool
/// of one is still speculative under chaos scheduling, so width alone is
/// not the admission condition (issue #122).
#[test]
fn runtime_names_preserve_inline_safety() {
    on_big_stack(|| {
        for name in ["DynamicStringOrder"] {
            let tla = crate_dir().join(format!("tests/fixtures/runtime-names/{name}.tla"));
            let cfg = std::fs::read_to_string(tla.with_extension("cfg")).expect("fixture");
            for mut base in profiles() {
                base.seed = 7;
                let reference = invariant(name, &tla, &cfg, &base, 8).expect("ordered result");
                assert!(matches!(
                    reference.verdict,
                    Verdict::InvariantViolated { .. }
                ));
                let stats = reference.stats.as_ref().expect("counts");
                if base.profile == dive_eval::Profile::Tlc {
                    assert_eq!((stats.generated, stats.distinct, stats.depth), (23, 23, 2));
                    assert!(reference.workers.sequential);
                    assert_eq!(reference.workers.used, 1);
                    assert!(
                        reference
                            .workers
                            .reason
                            .as_ref()
                            .expect("reason")
                            .contains("runtime names")
                    );
                } else {
                    assert!(
                        !reference.workers.sequential,
                        "canonical order remains parallel"
                    );
                    assert_eq!(reference.workers.used, 4);
                }
            }
        }
    });
}

#[test]
fn runtime_name_wrappers_aliases_and_operator_arguments_are_reachable() {
    on_big_stack(|| {
        for (defs, expression, extra_cfg) in [
            ("", "ToString(x)", ""),
            ("Wrap(v) == ToString(v)\n", "Wrap(x)", ""),
            ("Wrap(v) == v\n", "Wrap(x)", "CONSTANT Wrap <- ToString\n"),
            ("Call(F(_),v) == F(v)\n", "Call(ToString,x)", ""),
            (
                "Wrap(v) == ToString(v)\nCall(F(_),v) == F(v)\n",
                "Call(Wrap,x)",
                "",
            ),
            ("", "Tail(ToString(x))", ""),
            ("", "SubSeq(ToString(x),1,1)", ""),
            ("", "ToString(x) \\o \"suffix\"", ""),
        ] {
            let source = format!(
                "---- MODULE RuntimeNames ----\nEXTENDS Naturals, TLC, Sequences\nVARIABLE x\n{defs}Init == x = 0\nNext == x' = x /\\ ({expression} = {expression})\n====\n"
            );
            let analysis = dive_sema::analyze_source(
                "RuntimeNames.tla",
                &source,
                &dive_sema::Options::default(),
            );
            assert!(
                analysis.accepted(),
                "{source}: {:?}",
                analysis.diagnostics()
            );
            let ir = Arc::new(dive_ir::lower(&analysis).expect("lowers"));
            let cfg = dive_config::parse(&format!("INIT Init\nNEXT Next\n{extra_cfg}"))
                .into_config()
                .expect("cfg");
            let m = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("bind");
            assert!(m.sequential_names_only, "{source}: {:?}", m.sequential);
            assert!(m.sequential.iter().any(|s| s.contains("runtime names")));
        }
    });
}

#[test]
fn runtime_choice_matches_clean_pinned_tlc_at_fixed_seeds() {
    on_big_stack(|| {
        let tla = crate_dir().join("tests/fixtures/runtime-names/DynamicStringOrder.tla");
        let cfg = std::fs::read_to_string(tla.with_extension("cfg")).expect("cfg");
        let (ir, cfg) = load(&tla, &cfg);
        for (seed, count, x) in [(0, 30, 13), (7, 23, 6), (14, 30, 13)] {
            let transcript = std::fs::read_to_string(
                tla.parent()
                    .expect("parent")
                    .join(format!("tlc-2026.10.06.014338/seed-{seed}.stdout")),
            )
            .expect("raw pinned TLC");
            assert!(transcript.contains(&format!(
                "{count} states generated, {count} distinct states found, 15 states left on queue."
            )));
            assert!(transcript.contains(&format!("x = {x}")));
            for store in [StoreMode::Fingerprint, StoreMode::Exact] {
                for workers in [1, 4, 8] {
                    for chaos in [None, Some(0), Some(91)] {
                        let mut opts = CheckOptions::tlc(0, seed);
                        opts.store = store;
                        opts.workers = workers;
                        opts.chaos = chaos;
                        let r = check(&ir, &cfg, &opts);
                        assert!(matches!(r.verdict, Verdict::InvariantViolated { .. }));
                        let stats = r.stats.as_ref().expect("stats");
                        assert_eq!(
                            (
                                stats.generated,
                                stats.distinct,
                                stats.left_on_queue,
                                stats.depth
                            ),
                            (count, count, 15, 2)
                        );
                        let trace = r.counterexample.as_ref().expect("trace");
                        assert_eq!(trace.states.len(), 2);
                        assert_eq!(trace.states[0].values[0], dive_value::Value::Int(x));
                        assert_eq!(trace.states[1].values[0], dive_value::Value::Int(x));
                        assert_eq!(
                            format!("{:?}", trace.states[1].values[1]),
                            format!("\"{}\"", x + 1)
                        );
                        assert_eq!((r.workers.used, r.workers.sequential), (1, true));
                    }
                }
            }
        }
    });
}
