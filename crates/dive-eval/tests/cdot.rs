//! Action composition, `A \cdot B` (D-022), against the pinned TLC.
//!
//! `tests/fixtures/cdot/` holds minimal specs and compact pinned TLC results, generated
//! with `-Dtlc2.tool.impl.Tool.cdot=true` (`dev capture --feature 'action
//! composition'`) except `CdOff`, captured without it. Under
//! `--action-composition=tlc` the verdict and the generated and distinct counts must equal TLC's;
//! without the switch, TLC's error. The default mode (`tla`) gives the TLA+
//! meaning: it agrees with TLC where TLC's algorithm computes it, finds the
//! deadlock TLC's counting hides, and refuses the composition TLC's algorithm
//! distorts.

#![allow(clippy::panic, clippy::expect_used, reason = "test code")]

mod harness;

use std::path::{Path, PathBuf};

use dive_eval::{ActionComposition, Options};
use harness::model::{Checked, Verdict, bfs, bind_model, on_big_stack};

const ORACLE: &str = "tlc-2026.10.06.014338";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cdot")
}

fn run(name: &str, opts: Options) -> Checked {
    let dir = root().join("specs").join(name);
    let loaded = harness::load(&dir.join(format!("{name}.tla")), &[]).expect("loads");
    let cfg_text = std::fs::read_to_string(dir.join(format!("{name}.cfg"))).expect("cfg");
    let cfg = dive_config::parse(&cfg_text).into_config().expect("cfg");
    on_big_stack(move || {
        let b = bind_model(&loaded, &cfg, opts)?;
        Ok::<_, String>(bfs(&b, 1_000_000))
    })
    .expect("thread")
    .expect("binds")
}

fn tlc_mode() -> Options {
    Options {
        action_composition: ActionComposition::Tlc,
        ..Options::tlc()
    }
}

fn tla_mode() -> Options {
    Options {
        action_composition: ActionComposition::Tla,
        ..Options::tlc()
    }
}

/// `(code, generated, distinct, depth)` from fresh pinned TLC runs.
fn oracle(name: &str) -> (u32, u64, u64, u64) {
    let text = std::fs::read_to_string(root().join(ORACLE).join("results.json"))
        .expect("compact pinned results");
    let results: serde_json::Value = serde_json::from_str(&text).expect("result JSON");
    let result = &results[name];
    (
        u32::try_from(result["code"].as_u64().expect("code")).expect("EC code"),
        result["generated"].as_u64().expect("generated"),
        result["distinct"].as_u64().expect("distinct"),
        result["depth"].as_u64().expect("depth"),
    )
}

#[test]
fn tlc_mode_matches_tlc_counts() {
    // `CdNext`: success; `CdDeadlock`: success (the intermediate state hides
    // the deadlock); `CdConj`: deadlock; `CdEval`: action property violated;
    // `CdEnabled`: invariant violated.
    for (name, verdict) in [
        ("CdNext", Verdict::Success),
        ("CdDeadlock", Verdict::Success),
        ("CdConj", Verdict::Deadlock),
        (
            "CdEval",
            Verdict::ActionProperty {
                name: "Prop".to_owned(),
            },
        ),
        (
            "CdEnabled",
            Verdict::Invariant {
                name: "Inv".to_owned(),
                initial: false,
            },
        ),
    ] {
        let dive = run(name, tlc_mode());
        assert_eq!(dive.verdict, verdict, "{name}");
        let (code, generated, distinct, depth) = oracle(name);
        let expected_code = match dive.verdict {
            Verdict::Success => 2193,
            Verdict::Deadlock => 2114,
            Verdict::Invariant { initial: false, .. } => 2110,
            Verdict::ActionProperty { .. } => 2112,
            ref other => panic!("unexpected control verdict: {other:?}"),
        };
        assert_eq!(code, expected_code, "{name}: verdict class");
        assert_eq!(dive.depth, depth, "{name}: depth");
        assert_eq!(
            (dive.generated, dive.distinct),
            (generated, distinct),
            "{name}: generated/distinct"
        );
    }
}

#[test]
fn off_is_tlc_without_the_switch() {
    // TLCGet of the unset property returns its key, so the ASSUME holds, and
    // the composition is TLC's error (`EC.GENERAL`, 1000).
    let (code, generated, distinct, depth) = oracle("CdOff");
    assert_eq!(code, 1000);
    let dive = run("CdOff", Options::tlc());
    assert_eq!(
        (dive.generated, dive.distinct, dive.depth),
        (generated, distinct, depth),
        "CdOff: work receipt"
    );
    match dive.verdict {
        Verdict::EvalError { message, .. } => {
            assert!(
                message.contains("does not support action composition"),
                "{message}"
            );
        }
        other => panic!("CdOff: {other:?}"),
    }
}

#[test]
fn tla_mode_agrees_where_tlc_computes_the_meaning() {
    for name in ["CdEval", "CdEnabled"] {
        let tlc = run(name, tlc_mode());
        let tla = run(name, tla_mode());
        assert_eq!(tla.verdict, tlc.verdict, "{name}");
        assert_eq!(
            (tla.generated, tla.distinct, tla.depth),
            (tlc.generated, tlc.distinct, tlc.depth),
            "{name}"
        );
    }
}

#[test]
fn tla_mode_reports_the_deadlocks_tlc_hides() {
    // `CdDeadlock`: from x = 0, A leads only to a state where B is disabled.
    let dive = run("CdDeadlock", tla_mode());
    assert_eq!(dive.verdict, Verdict::Deadlock);
    assert_eq!((dive.generated, dive.distinct), (1, 1));
    // `CdNext`: once y = 3 and x < 4, A steps exist but B is disabled after
    // each, and the stuttering disjunct needs x >= 4. TLC reports success
    // (65 generated, intermediate states included).
    let dive = run("CdNext", tla_mode());
    assert_eq!(dive.verdict, Verdict::Deadlock);
    assert!(dive.generated < 65);
}

#[test]
fn tla_mode_refuses_what_tlc_distorts() {
    let dive = run("CdConj", tla_mode());
    match dive.verdict {
        Verdict::Unsupported(m) => assert!(m.contains("action composition"), "{m}"),
        other => panic!("CdConj: {other:?}"),
    }
}
