//! Symmetry safety keys against clean pinned TLC captures.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test assertions"
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dive_check::{CheckOptions, CheckResult, Verdict, check};
use dive_store::StoreMode;

fn root(corpus: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../{corpus}/tests/fixtures"))
}

fn run(root: &Path, name: &str, options: &CheckOptions) -> CheckResult {
    let source = root.join("specs").join(name).join(format!("{name}.tla"));
    let analysis = dive_sema::analyze_file(&source, &dive_sema::Options::default()).unwrap();
    assert!(analysis.accepted(), "{name}: {:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).unwrap());
    let cfg = dive_config::parse(&std::fs::read_to_string(source.with_extension("cfg")).unwrap())
        .into_config()
        .unwrap();
    check(&ir, &cfg, options)
}

fn message(transcript: &str, code: u32) -> Option<String> {
    let head = format!("@!@!@STARTMSG {code}:");
    let body = transcript.split_once(&head)?.1.split_once('\n')?.1;
    Some(body.split("@!@!@ENDMSG").next()?.trim_end().to_owned())
}

fn assert_tlc(result: &CheckResult, transcript: &str, name: &str) {
    let generated = message(transcript, 2199).unwrap();
    let counts: Vec<u64> = generated
        .split_whitespace()
        .filter_map(|word| word.parse().ok())
        .collect();
    let depth: u64 = message(transcript, 2194)
        .unwrap()
        .trim_end_matches('.')
        .split_whitespace()
        .last()
        .unwrap()
        .parse()
        .unwrap();
    let stats = result
        .stats
        .as_ref()
        .unwrap_or_else(|| panic!("{name}: {:?}", result.verdict));
    assert_eq!(
        (
            stats.generated,
            stats.distinct,
            stats.left_on_queue,
            stats.depth
        ),
        (counts[0], counts[1], counts[2], depth),
        "{name}: {:?}",
        result.verdict
    );
    if message(transcript, 2110).is_some() {
        assert!(
            matches!(result.verdict, Verdict::InvariantViolated { .. }),
            "{name}: {:?}",
            result.verdict
        );
    } else {
        assert_eq!(result.verdict, Verdict::Success, "{name}");
    }
    let expected_warning = message(transcript, 2300);
    assert_eq!(
        result
            .warnings
            .iter()
            .find(|(code, _)| *code == 2300)
            .map(|(_, text)| text.clone()),
        expected_warning,
        "{name}"
    );
}

#[test]
fn unimplemented_infinite_permutation_refuses_instead_of_becoming_a_key() {
    let root = root("dive-model").join("symmetry-group");
    let transcript =
        std::fs::read_to_string(root.join("tlc-2026.10.06.014338").join("SyInfinite.stdout"))
            .unwrap();
    assert!(transcript.contains("contains Nat"));
    for store in [StoreMode::Fingerprint, StoreMode::Exact] {
        for mut options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            options.store = store;
            for workers in [1, 4, 8] {
                options.workers = workers;
                let result = run(&root, "SyInfinite", &options);
                let Verdict::Unsupported { feature } = result.verdict else {
                    panic!("{:?}", result.verdict);
                };
                assert!(feature.contains("SYMMETRY"), "{feature}");
                assert!(feature.contains("Nat"), "{feature}");
            }
        }
    }
}

#[test]
fn pure_symmetry_probes_match_in_both_stores_at_every_worker_width() {
    let root = root("dive-liveness").join("tlc-probes");
    for name in [
        "SymCounts",
        "SymCountsNone",
        "SymAsym",
        "SymAsymLate",
        "SymOrder",
        "SymTwoSets",
        "SymView",
        "SymViolation",
    ] {
        let transcript = std::fs::read_to_string(
            root.join("tlc-2026.10.06.014338")
                .join(format!("{name}.stdout")),
        )
        .unwrap();
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            for workers in [1, 4, 8] {
                let options = CheckOptions {
                    workers,
                    store,
                    chaos: Some(7),
                    ..CheckOptions::tlc(0, 0)
                };
                let result = run(&root, name, &options);
                assert_tlc(&result, &transcript, name);
                if let Some(trace) = &result.counterexample {
                    let count = transcript
                        .lines()
                        .filter(|line| line.starts_with("@!@!@STARTMSG 2217:"))
                        .count();
                    assert_eq!(trace.states.len(), count, "{name}");
                }
            }
        }
    }
}

#[test]
fn disabled_and_unused_groups_keep_exact_startup_warnings() {
    let root = root("dive-model").join("symmetry-group");
    for name in [
        "SySmall",
        "SyUnused",
        "SySubstitution",
        "SyNonBiject",
        "SyWarnConditional",
        "SyWarnOverride",
        "SyWarnNonModel",
    ] {
        let transcript = std::fs::read_to_string(
            root.join("tlc-2026.10.06.014338")
                .join(format!("{name}.stdout")),
        )
        .unwrap();
        assert_tlc(
            &run(&root, name, &CheckOptions::tlc(0, 0)),
            &transcript,
            name,
        );
    }
    let bad = run(&root, "SyNonBiject", &CheckOptions::dive(0));
    assert!(matches!(bad.verdict, Verdict::Unsupported { .. }));
    assert!(bad.stats.is_none());
}

#[test]
fn invalid_representation_diagnostics_precede_assumptions_and_search() {
    let root = root("dive-model").join("symmetry-group");
    for name in [
        "SyMissing",
        "SyConstant",
        "SyNotSet",
        "SyTuple",
        "SyAssignment",
        "SyNonModel",
    ] {
        let result = run(&root, name, &CheckOptions::tlc(0, 0));
        let Verdict::ModelError { code, message, .. } = result.verdict else {
            panic!("{name}: {:?}", result.verdict);
        };
        let transcript = std::fs::read_to_string(
            root.join("tlc-2026.10.06.014338")
                .join(format!("{name}.stdout")),
        )
        .unwrap();
        assert_eq!(Some(message), self::message(&transcript, code), "{name}");
        assert!(result.stats.is_none());
        assert_eq!(result.workers.used, 0);
    }
}

#[test]
fn default_profile_preserves_worker_determinism_and_reports_symmetry_order_policy() {
    let root = root("dive-liveness").join("tlc-probes");
    for name in [
        "SymCounts",
        "SymAsym",
        "SymOrder",
        "SymTwoSets",
        "SymView",
        "SymViolation",
    ] {
        for store in [StoreMode::Fingerprint, StoreMode::Exact] {
            let first = run(
                &root,
                name,
                &CheckOptions {
                    store,
                    ..CheckOptions::dive(0)
                },
            );
            assert!(first.verdict.is_verdict(), "{name}: {:?}", first.verdict);
            for workers in [4, 8] {
                let next = run(
                    &root,
                    name,
                    &CheckOptions {
                        store,
                        workers,
                        chaos: Some(17),
                        ..CheckOptions::dive(0)
                    },
                );
                assert_eq!(first.verdict, next.verdict, "{name}");
                assert_eq!(first.stats, next.stats, "{name}");
                assert_eq!(
                    format!("{:?}", first.counterexample),
                    format!("{:?}", next.counterexample),
                    "{name}"
                );
                assert_eq!(first.warnings, next.warnings, "{name}");
                assert_eq!(first.notes, next.notes, "{name}");
            }
            assert!(
                first
                    .notes
                    .iter()
                    .any(|n| n.contains("canonicalizes symmetry keys")),
                "{name}"
            );
        }
    }
}
