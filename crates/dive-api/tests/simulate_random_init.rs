//! Primary startup draws remain separate from selection and independent membership.
//!
//! Only the retired reference engine sampled `RandomElement` in simulation
//! (#791). Core refuses each of these requests by name; the restoring
//! coverage is tracked in #910, which points to this file's earlier form.
#![allow(clippy::expect_used, reason = "bounded fixture setup and assertions")]
#[path = "support/refused.rs"]
mod refused;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use dive_api::SimulateRequest;
use dive_api::trace::{Check, Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use serde_json::{Value, json};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-sim/tests/fixtures/random-init")
}
fn request(row: &Value, compat_tlc: bool) -> SimulateRequest {
    SimulateRequest {
        file: fixtures().join(format!("{}.tla", row["model"].as_str().expect("model"))),
        seed: row["seed"].as_i64(),
        aril: row["aril"].as_u64().expect("aril"),
        depth: Some(usize::try_from(row["depth"].as_u64().expect("depth")).expect("depth bound")),
        traces: row["requested_traces"].as_u64().expect("traces"),
        compat_tlc,
        ..Default::default()
    }
}
fn assert_refused(request: &SimulateRequest) {
    let report = dive_api::simulate_file(request).expect("report");
    refused::assert_refused(
        &report.to_json(),
        report.exit_code(),
        refused::FINITE_RANDOM,
    );
}

#[test]
fn actual_primary_init_words_errors_and_output_keep_their_boundaries() {
    let oracle: Value = serde_json::from_str(
        &std::fs::read_to_string(fixtures().join("oracle.json")).expect("generated primary data"),
    )
    .expect("primary JSON");
    assert_eq!(oracle["oracle"], "tlc-2026.10.06.014338");
    let mut runs = 0;
    for row in oracle["primary_probes"].as_array().expect("primary cases") {
        for compat_tlc in [true, false] {
            if !compat_tlc && row["aril"] != 0 {
                continue;
            }
            assert_refused(&request(row, compat_tlc));
            runs += 1;
        }
    }
    assert_eq!(runs, 20);
}

#[test]
fn seed_prefix_statistics_and_true_word_mutations_remain_independent() {
    let file = fixtures().join("InitRandomInterval.tla");
    for compat_tlc in [false, true] {
        assert_refused(&SimulateRequest {
            file: file.clone(),
            seed: Some(0),
            depth: Some(3),
            traces: 2,
            compat_tlc,
            ..Default::default()
        });
        // The independent validator still admits any drawn initial value and
        // rejects forged Init, Next and ending claims.
        let model = SpecModel::load(
            &file,
            &file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc,
                ..Default::default()
            },
        )
        .expect("fresh model");
        let doc = TraceDoc::behavior(vec![json!({"x":17}), json!({"x":18})]).expect("word");
        assert_eq!(model.validate(&doc).validation, Validation::Valid);
        for (state, value, check) in [(0, -1, Check::Init), (1, -999, Check::Next)] {
            let mut forged = doc.clone();
            forged.states[state]["x"] = json!(value);
            assert!(
                matches!(model.validate(&forged).validation,Validation::Invalid(f) if f.check==check)
            );
        }
        let false_ending = doc.clone().with_ending(Ending::Invariant("Safe".into()));
        assert!(
            matches!(model.validate(&false_ending).validation,Validation::Invalid(f) if f.check==Check::Ending)
        );
    }
}

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Files(PathBuf);
impl Files {
    fn new(init: &str, next: &str, safe: &str, extra: &str, config_extra: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "dive-sim-init-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).expect("unique test directory");
        std::fs::write(dir.join("Boundary.tla"),format!("---- MODULE Boundary ----\nEXTENDS Integers,TLC\n{extra}\nVARIABLE x\nInit=={init}\nNext=={next}\nSafe=={safe}\n====\n")).expect("source");
        std::fs::write(
            dir.join("Boundary.cfg"),
            format!("INIT Init\nNEXT Next\nINVARIANT Safe\n{config_extra}"),
        )
        .expect("config");
        Self(dir)
    }
    fn request(&self) -> SimulateRequest {
        SimulateRequest {
            file: self.0.join("Boundary.tla"),
            seed: Some(0),
            depth: Some(1),
            traces: 1,
            compat_tlc: true,
            ..Default::default()
        }
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn nested_correlated_and_captured_randomness_has_independent_sample_proof() {
    for (init, next, safe, extra, config) in [
        (
            "x=RandomElement({RandomElement({7})})",
            "UNCHANGED x",
            "TRUE",
            "",
            "",
        ),
        (
            "x=RandomElement({7}) /\\ x=RandomElement({7})",
            "UNCHANGED x",
            "TRUE",
            "",
            "",
        ),
        ("x=RandomElement({7})+0", "UNCHANGED x", "TRUE", "", ""),
        (
            "x=Apply(RandomElement,{7})",
            "UNCHANGED x",
            "TRUE",
            "Apply(Draw(_),S)==Draw(S)",
            "",
        ),
        (
            "x=Draw({7})",
            "UNCHANGED x",
            "TRUE",
            "CONSTANT Draw(_)",
            "CONSTANT Draw <- RandomElement\n",
        ),
        (
            "x=RandomElement({7})",
            "UNCHANGED x",
            "RandomElement({TRUE})",
            "",
            "",
        ),
        (
            "x=RandomElement(IF FALSE THEN RandomElement({{7}}) ELSE {7})",
            "IF FALSE THEN x'=RandomElement({9}) ELSE UNCHANGED x",
            "TRUE",
            "",
            "",
        ),
        (
            "x=RandomElement(IF PrintT(\"before-inner\") THEN {RandomElement({7})} ELSE {})",
            "UNCHANGED x",
            "TRUE",
            "",
            "",
        ),
    ] {
        assert_refused(&Files::new(init, next, safe, extra, config).request());
    }
}

#[test]
fn nested_domain_errors_precede_the_unreached_outer_native_invocation() {
    let files = Files::new(
        "x=RandomElement(IF PrintT(\"before-inner-error\") THEN {RandomElement(42)} ELSE {})",
        "UNCHANGED x",
        "TRUE",
        "",
        "",
    );
    assert_refused(&files.request());
}
