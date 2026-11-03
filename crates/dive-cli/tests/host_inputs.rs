//! CLI capture and explicit offline replay of external input assumptions.
#![allow(
    clippy::expect_used,
    reason = "fixture setup and behavioral assertions"
)]

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "dive-cli-inputs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).expect("fixture");
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../dive-api/tests/fixtures/host-inputs");
        for model in [
            "Captured",
            "Demand",
            "EnvironmentOrder",
            "TwoFilesSafeNames",
            "TextBeforeEnvOrder",
            "EnvBeforeTextOrder",
        ] {
            for suffix in ["tla", "cfg"] {
                let name = format!("{model}.{suffix}");
                std::fs::copy(source.join(&name), path.join(name)).expect("spec fixture");
            }
        }
        std::fs::write(path.join("input.txt"), "hello\n").expect("input");
        Self(path)
    }
    fn run(&self, command: &str, tlc: bool, extra: &[&str]) -> Output {
        self.run_model("Captured", command, tlc, extra)
    }
    fn run_model(&self, model: &str, command: &str, tlc: bool, extra: &[&str]) -> Output {
        self.run_model_environment(
            model,
            command,
            tlc,
            extra,
            &[
                ("DIVE_COUNT", if command == "check" { "2" } else { "99" }),
                (
                    "DIVE_LABEL",
                    if command == "check" {
                        "outside"
                    } else {
                        "changed"
                    },
                ),
            ],
        )
    }
    fn run_model_environment(
        &self,
        model: &str,
        command: &str,
        tlc: bool,
        extra: &[&str],
        environment: &[(&str, &str)],
    ) -> Output {
        let modules = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../dive-eval/tests/fixtures/community-modules");
        let mut run = Command::new(env!("CARGO_BIN_EXE_dive"));
        run.env_clear()
            .envs(environment.iter().copied())
            .current_dir(&self.0)
            .arg(command)
            .arg(format!("{model}.tla"))
            .args([
                "--output",
                "json",
                "--seed",
                "7",
                "--fp",
                "0",
                "--library-path",
            ])
            .arg(modules)
            .args(extra);
        if tlc {
            run.arg("--compat=tlc");
        }
        if command == "check" {
            run.args([
                "--workers",
                "1",
                "--time-limit",
                "none",
                "--max-memory",
                "none",
            ]);
        }
        run.output().expect("CLI")
    }
    fn save(&self, path: &str, value: &serde_json::Value) {
        std::fs::write(self.0.join(path), serde_json::to_vec(value).expect("JSON"))
            .expect("report");
    }
}

#[test]
fn primary_success_paths_keep_counts_demand_order_and_the_actual_tlc_name_refusal() {
    let oracle: serde_json::Value = serde_json::from_str(include_str!(
        "../../dive-api/tests/fixtures/host-inputs/oracle.json"
    ))
    .expect("primary controls");
    for tlc in [false, true] {
        let fixture = Fixture::new();
        for row in oracle["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .filter(|row| row["model"] != "EnvironmentOrder")
        {
            let model = row["model"].as_str().expect("model");
            let output = fixture.run_model(model, "check", tlc, &["--allow-io=env,files"]);
            status(
                &output,
                i32::try_from(row["physical_exit"].as_i64().expect("exit")).expect("bounded exit"),
            );
            let report = json(&output);
            for field in ["generated", "distinct", "depth"] {
                assert_eq!(report["statistics"][field], row[field], "{model}: {field}");
            }
            assert_eq!(report["output"], row["printed"], "{model}");
            fixture.save("primary-run.json", &report);
            status(
                &fixture.run_model(
                    model,
                    "validate-trace",
                    tlc,
                    &["--inputs", "primary-run.json"],
                ),
                0,
            );
        }
        let boundary = fixture.run_model("EnvironmentOrder", "check", tlc, &["--allow-io=env"]);
        if tlc {
            status(&boundary, 3);
            let report = json(&boundary);
            assert_eq!(report["verdict"], "unsupported");
            assert!(
                report["message"]
                    .as_str()
                    .expect("refusal")
                    .contains("TLC construction order")
            );
            assert_eq!(oracle["rows"][2]["first_state"]["x"], "DIVE_LABEL");
        } else {
            status(&boundary, 0);
            assert_eq!(json(&boundary)["verdict"], "success");
        }
    }
}

#[test]
fn primary_native_history_controls_refuse_second_text_and_tlc_text_before_env() {
    let oracle: serde_json::Value = serde_json::from_str(include_str!(
        "../../dive-api/tests/fixtures/host-inputs/oracle.json"
    ))
    .expect("primary controls");
    for tlc in [false, true] {
        let fixture = Fixture::new();
        let row = &oracle["native_history_controls"][0];
        std::fs::write(
            fixture.0.join("input.txt"),
            row["input_text"].as_str().expect("primary first text"),
        )
        .expect("first input");
        std::fs::write(
            fixture.0.join("second.txt"),
            row["second_text"].as_str().expect("primary second text"),
        )
        .expect("second input");
        let second = fixture.run_model("TwoFilesSafeNames", "check", tlc, &["--allow-io=files"]);
        status(&second, 3);
        let report = json(&second);
        assert_eq!(report["verdict"], "unsupported");
        assert!(
            report["message"]
                .as_str()
                .expect("refusal")
                .contains("subsequent TXT")
        );
        assert_eq!(
            report["external_inputs"]["records"]
                .as_array()
                .expect("only first input recorded")
                .len(),
            1
        );
        assert_eq!(row["first_state"]["x"][1], 0);

        std::fs::write(fixture.0.join("input.txt"), "aUnknown").expect("ordering input");
        for model in ["TextBeforeEnvOrder", "EnvBeforeTextOrder"] {
            let output = fixture.run_model_environment(
                model,
                "check",
                tlc,
                &["--allow-io=env,files"],
                &[("DIVE_LABEL", "zUnknown")],
            );
            if tlc && model == "TextBeforeEnvOrder" {
                status(&output, 3);
                assert!(
                    json(&output)["message"]
                        .as_str()
                        .expect("refusal")
                        .contains("after a TXT request")
                );
            } else if tlc {
                status(&output, 12);
                let report = json(&output);
                assert_eq!(report["verdict"], "invariant-violation");
                fixture.save("ordered-inputs.json", &report);
                std::fs::remove_file(fixture.0.join("input.txt")).expect("remove input");
                status(
                    &fixture.run_model(
                        model,
                        "validate-trace",
                        tlc,
                        &["--inputs", "ordered-inputs.json"],
                    ),
                    0,
                );
            } else {
                status(&output, 0);
                assert_eq!(json(&output)["verdict"], "success");
            }
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).expect("JSON output")
}
fn status(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn real_env_and_text_capture_has_usable_offline_cli_replay_and_strict_negatives() {
    for tlc in [false, true] {
        let fixture = Fixture::new();
        let checked = fixture.run("check", tlc, &["--allow-io=env,files"]);
        status(&checked, 12);
        let report = json(&checked);
        assert_eq!(report["verdict"], "invariant-violation");
        assert_eq!(report["statistics"]["generated"], 3);
        assert_eq!(report["statistics"]["distinct"], 3);
        assert_eq!(report["statistics"]["depth"], 3);
        assert_eq!(
            report["external_inputs"]["records"]
                .as_array()
                .expect("records")
                .len(),
            2
        );
        fixture.save("run.json", &report);
        std::fs::remove_file(fixture.0.join("input.txt")).expect("remove ambient input");
        let valid = fixture.run("validate-trace", tlc, &["--inputs", "run.json"]);
        status(&valid, 0);
        assert_eq!(json(&valid)["result"], "valid");
        assert!(
            json(&valid)["notes"]
                .as_array()
                .expect("notes")
                .iter()
                .any(|note| note
                    .as_str()
                    .is_some_and(|text| text.contains("explicit assumptions")))
        );
        for mode in 0..5 {
            let mut changed = report.clone();
            if mode == 0 {
                let mut word = dive_api::trace::TraceDoc::from_json(&report).expect("trace");
                word.states[0]["x"] = serde_json::json!(0);
                changed["counterexample"]["trace"] = word.to_json();
            } else {
                let records = changed["external_inputs"]["records"]
                    .as_array_mut()
                    .expect("records");
                match mode {
                    1 => {
                        records.pop();
                    }
                    2 => {
                        let file = records
                            .iter_mut()
                            .find(|r| r["operation"] == "IOUtils!Deserialize")
                            .expect("file record");
                        file["path"] = serde_json::json!("not-input.txt");
                    }
                    3 => records.push(records[0].clone()),
                    _ => records.swap(0, 1),
                }
            }
            fixture.save("changed.json", &changed);
            status(
                &fixture.run("validate-trace", tlc, &["--inputs", "changed.json"]),
                if mode == 0 { 1 } else { 3 },
            );
        }
    }
}

#[test]
fn source_coverage_and_grants_share_the_actual_reference_binding() {
    for tlc in [false, true] {
        let fixture = Fixture::new();
        let plain = fixture.run("check", tlc, &["--allow-io=env,files"]);
        let covered = fixture.run(
            "check",
            tlc,
            &[
                "--allow-io=env,files",
                "--source-coverage",
                "--engine",
                "auto",
            ],
        );
        status(&plain, 12);
        status(&covered, 12);
        let plain = json(&plain);
        let covered = json(&covered);
        for key in ["verdict", "statistics", "external_inputs"] {
            assert_eq!(covered[key], plain[key], "{key}");
        }
        assert_eq!(
            covered["counterexample"]["trace"],
            plain["counterexample"]["trace"]
        );
        assert_eq!(covered["source_coverage"]["backend"], "reference");
        assert_eq!(covered["source_coverage"]["active"], false);
        assert_eq!(covered["source_coverage"]["tlc_cost_equivalent"], false);
        assert!(
            covered["source_coverage"]["rows"]
                .as_array()
                .expect("coverage rows")
                .iter()
                .any(|row| row["counts"]["evaluations"].as_u64() > Some(0))
        );
        fixture.save("covered.json", &covered);
        status(
            &fixture.run("validate-trace", tlc, &["--inputs", "covered.json"]),
            0,
        );
    }
}

#[test]
fn missing_categories_and_invalid_flags_never_run_a_spec_host_operation() {
    for tlc in [false, true] {
        let fixture = Fixture::new();
        for categories in [None, Some("env"), Some("files"), Some("exec")] {
            let flags = categories.map(|categories| format!("--allow-io={categories}"));
            let args = flags.as_deref().map(|s| vec![s]).unwrap_or_default();
            let output = fixture.run("check", tlc, &args);
            status(&output, 3);
            let report = json(&output);
            assert_eq!(report["verdict"], "unsupported");
            assert!(
                report["message"]
                    .as_str()
                    .expect("refusal")
                    .contains("--allow-io=")
            );
            assert_eq!(
                std::fs::read_to_string(fixture.0.join("input.txt")).expect("original input"),
                "hello\n"
            );
        }
        for categories in ["", "network", "env,env"] {
            let output = fixture.run("check", tlc, &[&format!("--allow-io={categories}")]);
            status(&output, 2);
        }
    }
}
