//! The trace validator replays a run's random draws (ADR 0016, amendment 2;
//! issue #52): `dive check` draws `RandomElement` in `Init` and `Next`
//! from the streams of its `--seed` and `--fp`, and the validator, given
//! the same seed and polynomial, draws the same values while it evaluates
//! the initial predicate and each step as predicates, in both profiles.
//! Without them the draws are refused by name; with another seed the trace
//! is invalid; and a value changed to another one the operator could have
//! drawn is rejected (the `redraw` mutants). Where the successor generator
//! drew for another successor first, an order a predicate does not follow,
//! the validator refuses by name instead of guessing.

#![allow(clippy::panic, clippy::expect_used, reason = "test code")]

use std::path::{Path, PathBuf};

use dive_api::trace::{Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, check_file};

/// Draws in `Init` and in both actions. The guards are on the unprimed
/// state, so a predicate draws exactly where the successor generator does:
/// the counterexample is Up, Up, Jump.
const SPEC: &str = "---- MODULE RndSteps ----
EXTENDS Naturals, TLC
VARIABLES x, n
Init == x = RandomElement(1..1000000) /\\ n = 0
Up == n < 2 /\\ x' = RandomElement(1..1000000) /\\ n' = n + 1
Jump == n >= 2 /\\ n' = n + 2 /\\ x' = RandomElement(1..1000000)
Next == Up \\/ Jump
Inv == n < 3
====
";

/// Here `Up`'s primed conjunct comes before its draw: the successor
/// generator draws for `Up` (it assigns `n'`) before it draws for `Jump`,
/// but a predicate on a `Jump` step stops at `n' = n + 1` and draws only
/// `Jump`'s value. The counterexample is Up, Jump.
const UNORDERED: &str = "---- MODULE RndOrder ----
EXTENDS Naturals, TLC
VARIABLES x, n
Init == x = 0 /\\ n = 0
Up == n' = n + 1 /\\ x' = RandomElement(1..1000000)
Jump == n' = n + 2 /\\ x' = RandomElement(1..1000000)
Next == Up \\/ Jump
Inv == n < 3
====
";

const CFG: &str = "INIT Init\nNEXT Next\nINVARIANT Inv\n";

fn write_spec(name: &str, spec: &str, cfg: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("dive-trace-random-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let module = spec
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(2))
        .expect("module name");
    let tla = dir.join(format!("{module}.tla"));
    let cfg_path = dir.join(format!("{module}.cfg"));
    std::fs::write(&tla, spec).expect("write spec");
    std::fs::write(&cfg_path, cfg).expect("write cfg");
    (tla, cfg_path)
}

/// `dive check` with `seed` and polynomial 0; the counterexample in
/// dive's trace format.
fn counterexample(tla: &Path, compat_tlc: bool, seed: i64) -> TraceDoc {
    let report = check_file(&CheckRequest {
        file: tla.to_path_buf(),
        compat_tlc,
        seed: Some(seed),
        fp: Some(0),
        workers: Some(1),
        ..CheckRequest::default()
    })
    .expect("check");
    let j = report.to_json();
    assert_eq!(j["verdict"], "invariant-violation", "{j:#}");
    TraceDoc::from_json(&j["counterexample"]["trace"]).expect("dive trace document")
}

fn model(tla: &Path, cfg: &Path, compat_tlc: bool, run: Option<(i64, usize)>) -> SpecModel {
    SpecModel::load(
        tla,
        cfg,
        &TraceOptions {
            compat_tlc,
            run,
            ..TraceOptions::default()
        },
    )
    .expect("bind")
}

#[test]
fn draws_in_init_and_next_replay_in_both_profiles() {
    let (tla, cfg) = write_spec("both", SPEC, CFG);
    for compat_tlc in [true, false] {
        for seed in [0, 7, -3] {
            let doc = counterexample(&tla, compat_tlc, seed);
            assert!(doc.states.len() >= 3, "{doc:?}");

            let replayed = model(&tla, &cfg, compat_tlc, Some((seed, 0))).validate(&doc);
            assert_eq!(
                replayed.validation,
                Validation::Valid,
                "compat {compat_tlc} seed {seed}: {replayed:?}"
            );

            // Without the run's seed, the draws are refused by name.
            let unseeded = model(&tla, &cfg, compat_tlc, None).validate(&doc);
            let Validation::Unsupported(why) = &unseeded.validation else {
                panic!("without a seed: {unseeded:?}");
            };
            assert!(why.contains("RandomElement"), "{why}");

            // Another seed draws other values: the first state is not initial.
            let other = model(&tla, &cfg, compat_tlc, Some((seed + 1, 0))).validate(&doc);
            assert!(
                matches!(other.validation, Validation::Invalid(_)),
                "another seed: {other:?}"
            );
            // Another polynomial reseeds every step (the initial state still
            // comes from the start-up stream, which only the seed decides).
            let other_fp = model(&tla, &cfg, compat_tlc, Some((seed, 1))).validate(&doc);
            let Validation::Invalid(f) = &other_fp.validation else {
                panic!("another polynomial: {other_fp:?}");
            };
            assert_eq!(f.state, 1, "{f:?}");
        }
    }
}

#[test]
fn redraw_mutants_are_rejected() {
    let (tla, cfg) = write_spec("mutants", SPEC, CFG);
    for compat_tlc in [true, false] {
        let doc = counterexample(&tla, compat_tlc, 7);
        let controls = model(&tla, &cfg, compat_tlc, Some((7, 0))).controls(&doc);
        assert!(controls.failures().is_empty(), "{:?}", controls.failures());
        let redraws: Vec<&str> = controls
            .mutants
            .iter()
            .map(|(n, _)| n.as_str())
            .filter(|n| n.starts_with("redraw-"))
            .collect();
        // One per state: x is drawn in Init and in every step.
        assert_eq!(redraws.len(), doc.states.len(), "{redraws:?}");
        assert_eq!(doc.states.len(), 4);
        assert!(redraws.contains(&"redraw-0-x"), "{redraws:?}");
        for (name, v) in &controls.mutants {
            assert!(matches!(v, Validation::Invalid(_)), "{name}: {v:?}");
        }
    }
}

#[test]
fn draws_out_of_the_generators_order_are_refused_not_judged() {
    let (tla, cfg) = write_spec("order", UNORDERED, CFG);
    for compat_tlc in [true, false] {
        let doc = counterexample(&tla, compat_tlc, 7);
        assert_eq!(doc.states.len(), 3);
        let r = model(&tla, &cfg, compat_tlc, Some((7, 0))).validate(&doc);
        let Validation::Unsupported(why) = &r.validation else {
            panic!("compat {compat_tlc}: {r:?}");
        };
        assert!(why.contains("stream position"), "{why}");
        // A different seed does not establish invalidity either: the
        // generator's draw position is still unproved.
        let other = model(&tla, &cfg, compat_tlc, Some((8, 0))).validate(&doc);
        let Validation::Unsupported(_) = &other.validation else {
            panic!("compat {compat_tlc}, another seed: {other:?}");
        };
    }
}

#[test]
fn wide_random_expansions_are_refused_without_an_offset_search() {
    let spec = include_str!("fixtures/random-traces/RndWide/RndWide.tla");
    let (tla, cfg) = write_spec("wide", spec, CFG);
    for compat in [true, false] {
        let doc = counterexample(&tla, compat, 7);
        assert_eq!(doc.states.len(), 2);
        let report = model(&tla, &cfg, compat, Some((7, 0))).validate(&doc);
        assert!(
            matches!(report.validation, Validation::Unsupported(_)),
            "{report:?}"
        );
    }
}

#[test]
fn a_coincidental_random_match_is_not_a_stream_position_proof() {
    let spec = include_str!("fixtures/random-traces/RndCoincidence/RndCoincidence.tla");
    let (tla, cfg) = write_spec("coincidence", spec, CFG);
    let doc = counterexample(&tla, true, 0);
    let report = model(&tla, &cfg, true, Some((0, 0))).validate(&doc);
    assert!(
        matches!(report.validation, Validation::Unsupported(_)),
        "{report:?}"
    );
}

#[test]
fn tlc_p9random_counterexample_replays() {
    // The TLC golden of probe P9Random (`-seed 0 -fp 0`): x = 730968.
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/probes/specifications/P9Random");
    let tla = dir.join("P9Random.tla");
    let cfg = dir.join("P9Random.cfg");
    let doc = TraceDoc::from_json(&serde_json::json!({
        "counterexample": {"action": [], "state": [[1, {"x": 730_968}]]},
        "vars": ["x"]
    }))
    .expect("TLC document")
    .with_ending(Ending::Invariant("Inv".to_owned()));
    let controls = model(&tla, &cfg, true, Some((0, 0))).controls(&doc);
    assert_eq!(controls.positive.validation, Validation::Valid);
    assert!(controls.failures().is_empty(), "{:?}", controls.failures());
    assert!(
        controls.mutants.iter().any(|(n, _)| n == "redraw-0-x"),
        "{:?}",
        controls.mutants
    );
    // Another seed's draw is not this trace's.
    let r = model(&tla, &cfg, true, Some((1, 0))).validate(&doc);
    assert!(matches!(r.validation, Validation::Invalid(_)), "{r:?}");
}

#[test]
fn a_draw_the_validator_cannot_place_is_refused() {
    // The checker draws an invariant's value from the stream of the state
    // it expanded, after its successors were generated: only the successor
    // generator knows that position.
    let spec = "---- MODULE RndInv ----
EXTENDS Naturals, TLC
VARIABLE n
Init == n = 0
Next == n' = n + 1
Inv == n < RandomElement({3})
====
";
    let (tla, cfg) = write_spec("inv", spec, "INIT Init\nNEXT Next\nINVARIANT Inv\n");
    let doc = counterexample(&tla, true, 0);
    let r = model(&tla, &cfg, true, Some((0, 0))).validate(&doc);
    let Validation::Unsupported(why) = &r.validation else {
        panic!("{r:?}");
    };
    assert!(why.contains(dive_trace_unplaced()), "{why}");
}

fn dive_trace_unplaced() -> &'static str {
    "the run draws it at a position of its stream that only the successor generator determines"
}
