//! Primary startup draws remain separate from selection and independent membership.
#![allow(clippy::expect_used, reason = "bounded fixture setup and assertions")]

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
fn texts(doc: &Value) -> Vec<&str> {
    doc["runtime_output"]
        .as_array()
        .expect("events")
        .iter()
        .map(|v| v["text"].as_str().expect("text"))
        .collect()
}
fn words(doc: &Value) -> Value {
    json!(
        doc["traces"]
            .as_array()
            .expect("traces")
            .iter()
            .map(|trace| {
                trace["tlc_states"]
                    .as_array()
                    .expect("states")
                    .iter()
                    .map(|s| s["x"].clone())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    )
}
#[test]
fn actual_primary_init_words_errors_and_output_keep_their_boundaries() {
    let oracle: Value = serde_json::from_str(
        &std::fs::read_to_string(fixtures().join("oracle.json")).expect("generated primary data"),
    )
    .expect("primary JSON");
    assert_eq!(oracle["oracle"], "tlc-2026.10.06.014338");
    assert_eq!(oracle["primary_before_production_implementation"], true);
    for row in oracle["primary_probes"].as_array().expect("primary cases") {
        for compat_tlc in [true, false] {
            if !compat_tlc && row["aril"] != 0 {
                continue;
            }
            let req = request(row, compat_tlc);
            let report = dive_api::simulate_file_engine(&req, dive_api::Engine::Reference)
                .expect("simulation");
            let doc = report.to_json();
            assert_eq!(doc["exhaustive"], false);
            assert_eq!(doc["model_proved"], false);
            assert!(
                doc["startup_output"]
                    .as_array()
                    .expect("startup events")
                    .iter()
                    .all(|event| event["kind"] == "warning"),
                "unexpected startup Print: {doc}"
            );
            if row["native_scope"] == "unsupported" {
                // Preserve the old primary capture's historical native scope.
                // Its reached Next draw now has an independent source relation.
                assert_eq!(row["model"], "InitRandomNextBoundary");
                assert_eq!(doc["result"], "sampled", "{doc}");
                assert_eq!(report.exit_code(), 0);
                assert_eq!(json!(texts(&doc)), row["printed"]);
                assert_eq!(doc["statistics"]["generated"], row["generated"]);
                assert!(
                    doc["traces"]
                        .as_array()
                        .expect("traces")
                        .iter()
                        .all(|trace| trace["validation"]["result"] == "valid")
                );
                continue;
            }
            if row["native_scope"] == "original-error" {
                assert_eq!(doc["result"], "evaluation-error", "{doc}");
                assert_eq!(doc["statistics"]["generated"], 0);
            } else {
                assert!(
                    doc["traces"]
                        .as_array()
                        .expect("traces")
                        .iter()
                        .all(|t| t["validation"]["result"] == "valid"),
                    "{doc}"
                );
                assert_eq!(json!(texts(&doc)), row["printed"]);
                if row["physical_exit"] == 0 {
                    assert_eq!(doc["result"], "sampled", "{doc}");
                    if compat_tlc {
                        assert_eq!(words(&doc), row["traces"], "{row}");
                    }
                } else {
                    assert_eq!(doc["result"], "violation", "{doc}");
                    assert_eq!(doc["violation_validated"], true);
                    assert_eq!(doc["observed_violation"]["name"], row["violated_property"]);
                    if compat_tlc {
                        assert_eq!(words(&doc), json!([row["printed_states"]]));
                    }
                }
            }
            if compat_tlc {
                assert_eq!(json!(report.exit_code()), row["physical_exit"]);
                assert_eq!(
                    doc["observed_tlc_code"],
                    row["codes"]
                        .as_array()
                        .expect("codes")
                        .first()
                        .cloned()
                        .unwrap_or(Value::Null)
                );
                if !row["generated"].is_null() {
                    assert_eq!(doc["statistics"]["generated"], row["generated"]);
                }
            }
        }
    }
}

#[test]
fn seed_prefix_statistics_and_true_word_mutations_remain_independent() {
    for compat_tlc in [false, true] {
        let mut req = SimulateRequest {
            file: fixtures().join("InitRandomInterval.tla"),
            seed: Some(0),
            depth: Some(3),
            traces: 2,
            compat_tlc,
            ..Default::default()
        };
        let plain = dive_api::simulate_file_engine(&req, dive_api::Engine::Reference)
            .expect("plain")
            .to_json();
        assert_eq!(
            plain,
            dive_api::simulate_file_engine(&req, dive_api::Engine::Reference)
                .expect("replay")
                .to_json()
        );
        req.action_stats = true;
        let counted = dive_api::simulate_file_engine(&req, dive_api::Engine::Reference)
            .expect("counted")
            .to_json();
        for field in ["traces", "statistics", "runtime_output", "observed_result"] {
            assert_eq!(plain[field], counted[field]);
        }
        req.traces = 4;
        let more = dive_api::simulate_file_engine(&req, dive_api::Engine::Reference)
            .expect("longer sample")
            .to_json();
        assert_eq!(
            plain["traces"].as_array().expect("short"),
            &more["traces"].as_array().expect("long")[..2]
        );
        let model = SpecModel::load(
            &req.file,
            &req.file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc,
                ..Default::default()
            },
        )
        .expect("fresh model");
        let doc = TraceDoc::from_json(&plain["traces"][0]["document"]).expect("actual word");
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
        let other =
            TraceDoc::behavior(vec![json!({"x":17})]).expect("different supported initial value");
        assert_eq!(model.validate(&other).validation, Validation::Valid);
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
    ] {
        let files = Files::new(init, next, safe, extra, config);
        let report = dive_api::simulate_file_engine(&files.request(), dive_api::Engine::Reference)
            .expect("finite random simulation")
            .to_json();
        assert_eq!(report["result"], "sampled", "{init}: {report}");
        assert!(
            report["traces"]
                .as_array()
                .expect("traces")
                .iter()
                .all(|trace| trace["validation"]["result"] == "valid")
        );
    }
    let quiet = Files::new(
        "x=RandomElement(IF FALSE THEN RandomElement({{7}}) ELSE {7})",
        "IF FALSE THEN x'=RandomElement({9}) ELSE UNCHANGED x",
        "TRUE",
        "",
        "",
    );
    assert_eq!(
        dive_api::simulate_file_engine(&quiet.request(), dive_api::Engine::Reference)
            .expect("untaken effects")
            .to_json()["result"],
        "sampled"
    );
    let reached = Files::new(
        "x=RandomElement(IF PrintT(\"before-inner\") THEN {RandomElement({7})} ELSE {})",
        "UNCHANGED x",
        "TRUE",
        "",
        "",
    );
    let report = dive_api::simulate_file_engine(&reached.request(), dive_api::Engine::Reference)
        .expect("actual nested invocation")
        .to_json();
    assert_eq!(report["result"], "sampled", "{report}");
    assert_eq!(report["traces"][0]["validation"]["result"], "valid");
    assert_eq!(texts(&report), ["\"before-inner\""]);
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
    let report = dive_api::simulate_file_engine(&files.request(), dive_api::Engine::Reference)
        .expect("normal domain evaluation error")
        .to_json();
    assert_eq!(report["result"], "evaluation-error", "{report}");
    assert_eq!(report["observed_tlc_code"], 1000);
    assert_eq!(report["statistics"]["generated"], 0);
    assert_eq!(texts(&report), ["\"before-inner-error\""]);
    let message = report["message"].as_str().expect("original error");
    assert!(message.contains("not a finite set"), "{message}");
    assert!(message.contains("42"), "{message}");
}
