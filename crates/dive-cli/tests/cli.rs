//! Smoke tests for the `dive` binary's command surface.

use std::process::Command;

fn dive(args: &[&str]) -> std::process::Output {
    match Command::new(env!("CARGO_BIN_EXE_dive")).args(args).output() {
        Ok(output) => output,
        Err(e) => unreachable!("cannot run dive binary: {e}"),
    }
}

#[test]
fn check_and_tlc_expose_search_caps_as_stopped_not_success() {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/user/examples/Counter.tla");
    for command in ["check", "tlc"] {
        for (flag, resource) in [("--max-states=2", "states"), ("--max-depth=1", "depth")] {
            let output = dive(&[
                command,
                &file.to_string_lossy(),
                flag,
                "--workers=4",
                "--seed=0",
                "--fp=0",
                "--output=json",
            ]);
            assert_eq!(output.status.code(), Some(5), "{output:?}");
            let report: serde_json::Value =
                serde_json::from_slice(&output.stdout).unwrap_or_default();
            assert_eq!(report["verdict"], "stopped");
            assert_eq!(report["stopped"]["resource"], resource);
            assert_eq!(report["statistics"]["exhaustive"], false);
            assert_eq!(report["options"]["workers"]["requested"], 4);
            assert!(report["counterexample"].is_null());
            assert_eq!(output.stderr, [] as [u8; 0]);
        }
    }
}

#[test]
fn diagnostic_explanations_are_informational_and_versioned() {
    for code in ["DIVE-L0020", "DIVE-S0011", "DIVE-S0033"] {
        let output = dive(&["explain", code, "--output", "json"]);
        assert!(output.status.success(), "{output:?}");
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_default();
        assert_eq!(report["schema_version"], dive_api::REPORT_SCHEMA_VERSION);
        assert_eq!(report["command"], "explain");
        assert_eq!(report["known"], true);
        assert!(report["explanation"]["remediation"].is_string());
        assert!(report.get("verdict").is_none());
    }
}

#[test]
fn symmetry_startup_general_errors_have_real_process_exit_parity() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-model/tests/fixtures/symmetry-group/specs");
    for name in ["SyDivZero", "SyMixedElements"] {
        let file = root.join(name).join(format!("{name}.tla"));
        for (profile, exit) in [("--compat=dive", 255), ("--compat=tlc", 255)] {
            let output = dive(&[
                "check",
                &file.to_string_lossy(),
                profile,
                "--seed=0",
                "--fp=0",
                "--workers=1",
                "--output=json",
            ]);
            assert_eq!(output.status.code(), Some(exit), "{name}: {output:?}");
            let report: serde_json::Value =
                serde_json::from_slice(&output.stdout).unwrap_or_default();
            assert_eq!(report["verdict"], "model-error");
            assert_eq!(report["tlc"]["code"], 1000);
            assert_eq!(report["tlc"]["name"], "GENERAL");
            assert_eq!(report["tlc"]["reported"], 1000);
            assert_eq!(report["tlc"]["exit_status"], 255);
            assert_eq!(report["tlc"]["verified"], true);
            assert!(report["statistics"].is_null());
            assert!(report["counterexample"].is_null());
            assert_eq!(report["output"], serde_json::json!([]));
        }
    }
}

/// Writes `contents` to a fresh file named `name` and returns its path.
fn temp_file(name: &str, contents: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "dive-cli-test-{}-{}",
        std::process::id(),
        name.replace('.', "-")
    ));
    if let Err(e) = std::fs::create_dir_all(&dir) {
        unreachable!("cannot create {}: {e}", dir.display());
    }
    let path = dir.join(name);
    if let Err(e) = std::fs::write(&path, contents) {
        unreachable!("cannot write {}: {e}", path.display());
    }
    path
}

#[test]
fn version_succeeds() {
    let out = dive(&["--version"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("dive "));
}

#[test]
fn lsp_refuses_explicitly() {
    let out = dive(&["lsp"]);
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stderr).starts_with("unsupported:"));
}

#[test]
fn validate_trace_checks_a_trace_against_the_spec() {
    let spec = temp_file(
        "Counter.tla",
        "---- MODULE Counter ----\nEXTENDS Naturals\nVARIABLE n\nInit == n = 0\nNext == n' = n + 1\nInv == n < 2\n====\n",
    );
    let cfg = spec.with_extension("cfg");
    if let Err(e) = std::fs::write(&cfg, "INIT Init\nNEXT Next\nINVARIANT Inv\n") {
        unreachable!("cannot write {}: {e}", cfg.display());
    }
    let spec = spec.to_string_lossy().into_owned();
    // TLC's -dumpTrace json form, which carries no claim.
    let tlc = temp_file(
        "tlc.json",
        r#"{"vars":["n"],"counterexample":{"state":[[1,{"n":0}],[2,{"n":1}],[3,{"n":2}]],"action":[]}}"#,
    );
    let tlc = tlc.to_string_lossy().into_owned();
    let run = |extra: &[&str]| {
        let mut args = vec![
            "validate-trace",
            spec.as_str(),
            "--trace",
            tlc.as_str(),
            "--compat=tlc",
        ];
        args.extend_from_slice(extra);
        dive(&args)
    };
    let out = run(&["--claim", "invariant:Inv"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("valid:"));
    // A claim that does not hold is invalid, with the failing state.
    // Claiming deadlock: the invariant fails in the last state first.
    let out = run(&["--claim", "deadlock", "--output", "json"]);
    assert_eq!(out.status.code(), Some(1));
    let out = run(&["--claim", "invariant:Missing", "--output", "json"]);
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
    assert_eq!(v["result"], "invalid");
    assert_eq!(v["failure"]["check"], "ending");
    // A TLC document needs a claim; a missing trace is a usage error.
    assert_eq!(run(&[]).status.code(), Some(1));
    assert_eq!(
        dive(&["validate-trace", spec.as_str()]).status.code(),
        Some(2)
    );
}

#[test]
fn unknown_command_is_usage_error() {
    assert_eq!(dive(&["frobnicate"]).status.code(), Some(2));
}

#[test]
fn parse_without_file_is_usage_error() {
    assert_eq!(dive(&["parse"]).status.code(), Some(2));
    assert_eq!(
        dive(&["parse", "--output", "xml", "x.tla"]).status.code(),
        Some(2)
    );
}

#[test]
fn parse_accepts_a_valid_module() {
    let path = temp_file("Good.tla", "---- MODULE Good ----\nx == 1\n====\n");
    let out = dive(&["parse", &path.to_string_lossy()]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("ok:"));
}

#[test]
fn parse_json_reports_diagnostics() {
    let path = temp_file("Bad.tla", "---- MODULE Bad ----\nx == 1 * -2\n====\n");
    let out = dive(&["parse", &path.to_string_lossy(), "--output", "json"]);
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => unreachable!("invalid JSON: {e}"),
    };
    assert_eq!(v["schema_version"], 3);
    assert_eq!(v["accepted"], false);
    assert_eq!(v["module"], "Bad");
    assert_eq!(v["diagnostics"][0]["code"], "DIVE-S0012");
    assert!(v.get("cst").is_none());
}

#[test]
fn parse_json_can_include_the_cst() {
    let path = temp_file("Cst.tla", "---- MODULE Cst ----\nx == 1\n====\n");
    let out = dive(&[
        "parse",
        &path.to_string_lossy(),
        "--output",
        "json",
        "--cst",
    ]);
    assert!(out.status.success());
    let v: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => unreachable!("invalid JSON: {e}"),
    };
    assert_eq!(v["cst"]["kind"], "SOURCE_FILE");
}

#[test]
fn parse_reads_a_config() {
    let tla = temp_file("Cfg.tla", "---- MODULE Cfg ----\nx == 1\n====\n");
    let cfg = temp_file("Cfg.cfg", "CONSTANT N = 3\nINIT Init\nINVARIANT Inv\n");
    let out = dive(&[
        "parse",
        &tla.to_string_lossy(),
        "--config",
        &cfg.to_string_lossy(),
        "--output",
        "json",
    ]);
    assert!(out.status.success());
    let v: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => unreachable!("invalid JSON: {e}"),
    };
    assert_eq!(v["accepted"], true);
    assert_eq!(v["config"]["accepted"], true);
    assert_eq!(v["config"]["model"]["invariants"][0]["text"], "Inv");
}

#[test]
fn parse_reports_config_errors() {
    let cfg = temp_file("Bad.cfg", "CONSTANT N = -1\nINIT Init\n");
    let out = dive(&["parse", "--config", &cfg.to_string_lossy()]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("error[DIVE-C0013]"), "{err}");
    assert!(
        err.contains("CFG_EXPECTED_SYMBOL (5006), exit 151, line 2"),
        "{err}"
    );
}

#[test]
fn parse_refuses_embedded_configs() {
    let tla = temp_file("Mono.tla", "---- MODULE Mono ----\n====\n");
    let out = dive(&["parse", "--config", &tla.to_string_lossy()]);
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stderr).starts_with("unsupported:"));
}

#[test]
fn parse_missing_file_is_usage_error() {
    assert_eq!(
        dive(&["parse", "/nonexistent/dive/Missing.tla"])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn analyze_accepts_and_reports_levels() {
    let path = temp_file(
        "Counter.tla",
        "---- MODULE Counter ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\n====\n",
    );
    let out = dive(&["analyze", &path.to_string_lossy(), "--output", "json"]);
    assert!(out.status.success());
    let v: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => unreachable!("invalid JSON: {e}"),
    };
    assert_eq!(v["schema_version"], 3);
    assert_eq!(v["command"], "analyze");
    assert_eq!(v["accepted"], true);
    assert_eq!(v["module"], "Counter");
    assert_eq!(v["modules"], serde_json::json!(["Naturals", "Counter"]));
    let levels: Vec<(String, String)> = v["definitions"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|d| (d["name"].to_string(), d["level"].to_string()))
                .collect()
        })
        .unwrap_or_default();
    assert!(levels.contains(&("\"Init\"".to_owned(), "\"state\"".to_owned())));
    assert!(levels.contains(&("\"Next\"".to_owned(), "\"action\"".to_owned())));
}

#[test]
fn analyze_rejects_with_codes_and_sany_cross_reference() {
    let path = temp_file("Undef.tla", "---- MODULE Undef ----\nInit == y = 0\n====\n");
    let out = dive(&["analyze", &path.to_string_lossy(), "--output", "json"]);
    assert_eq!(out.status.code(), Some(1));
    let v: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => unreachable!("invalid JSON: {e}"),
    };
    assert_eq!(v["accepted"], false);
    assert_eq!(v["diagnostics"][0]["code"], "DIVE-E0100");
    assert_eq!(v["diagnostics"][0]["sany_code"], 4200);
    assert_eq!(v["diagnostics"][0]["span"]["line"], 2);
    let human = dive(&["analyze", &path.to_string_lossy()]);
    assert!(String::from_utf8_lossy(&human.stderr).contains("error[DIVE-E0100] (SANY 4200)"));
}

#[test]
fn analyze_finds_modules_on_the_library_path() {
    let lib = temp_file("Lib.tla", "---- MODULE Lib ----\nk == 1\n====\n");
    let root = temp_file(
        "UsesLib.tla",
        "---- MODULE UsesLib ----\nEXTENDS Lib\nz == k\n====\n",
    );
    let dir = lib
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    assert_eq!(
        dive(&["analyze", &root.to_string_lossy()]).status.code(),
        Some(1)
    );
    for flag in ["--library-path", "--module-path"] {
        let out = dive(&["analyze", &root.to_string_lossy(), flag, &dir]);
        assert!(out.status.success(), "{flag}");
    }
    assert_eq!(dive(&["analyze"]).status.code(), Some(2));
    assert_eq!(
        dive(&["analyze", "x.tla", "--library-path"]).status.code(),
        Some(2)
    );
}

const CHECK_SPEC: &str = "---- MODULE CheckMe ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = (x + 1) % 4\nInv == x < 3\n====\n";

#[test]
fn check_reports_a_violation_with_each_profiles_exit_code() {
    let tla = temp_file("CheckMe.tla", CHECK_SPEC);
    let cfg = temp_file("CheckMe.cfg", "INIT Init\nNEXT Next\nINVARIANT Inv\n");
    let (tla, cfg) = (tla.to_string_lossy(), cfg.to_string_lossy());
    // Physical statuses follow TLC even with native evaluator semantics.
    let tlc = dive(&[
        "check",
        &tla,
        "--config",
        &cfg,
        "--compat=tlc",
        "--output",
        "json",
    ]);
    assert_eq!(tlc.status.code(), Some(12));
    let v: serde_json::Value = match serde_json::from_slice(&tlc.stdout) {
        Ok(v) => v,
        Err(e) => unreachable!("not JSON: {e}"),
    };
    assert_eq!(v["schema_version"], 3);
    assert_eq!(v["verdict"], "invariant-violation");
    assert_eq!(v["property"], "Inv");
    assert_eq!(v["tlc"]["code"], 2110);
    assert_eq!(v["statistics"]["generated"], 4);
    assert_eq!(v["counterexample"]["length"], 4);
    assert_eq!(v["counterexample"]["states"][3]["values"]["x"], 3);
    assert_eq!(v["options"]["profile"], "tlc");
    for store in ["fp", "exact"] {
        let own = dive(&[
            "check", &tla, "--config", &cfg, "--store", store, "--seed", "7",
        ]);
        assert_eq!(own.status.code(), Some(12), "{store}");
        let native = dive(&[
            "check",
            &tla,
            "--config",
            &cfg,
            "--store",
            store,
            "--seed",
            "7",
            "--exit-codes=native",
        ]);
        assert_eq!(native.status.code(), Some(10), "native {store}");
        let text = String::from_utf8_lossy(&own.stdout);
        assert!(text.contains("Error: Invariant Inv is violated."), "{text}");
        assert!(text.contains("seed 7"), "{text}");
    }
}

#[test]
fn check_refuses_and_rejects_with_distinct_codes() {
    assert_eq!(dive(&["check"]).status.code(), Some(2));
    let tla = temp_file(
        "CheckLive.tla",
        "---- MODULE CheckLive ----\nVARIABLE x\nInit == x = 0\nNext == x' = x\nSpec == Init /\\ [][Next]_x\nLive == <>(x = 1)\n====\n",
    );
    let cfg = temp_file("CheckLive.cfg", "SPECIFICATION Spec\nPROPERTY Live\n");
    let out = dive(&[
        "check",
        &tla.to_string_lossy(),
        "--config",
        &cfg.to_string_lossy(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(13),
        "eligible ordinary temporal check"
    );
    // --sequential is honored by the temporal route.
    let out = dive(&[
        "check",
        &tla.to_string_lossy(),
        "--config",
        &cfg.to_string_lossy(),
        "--sequential",
    ]);
    assert_eq!(out.status.code(), Some(13));
    // An option outside the final temporal route keeps the named refusal.
    let out = dive(&[
        "check",
        &tla.to_string_lossy(),
        "--config",
        &cfg.to_string_lossy(),
        "--chaos",
        "0",
    ]);
    assert_eq!(out.status.code(), Some(3));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("unsupported: temporal"));
    assert!(stdout.contains("temporal checking does not yet support --chaos"));
    let bad = temp_file("CheckBad.tla", "---- MODULE CheckBad ----\nx == y\n====\n");
    let out = dive(&["check", &bad.to_string_lossy(), "--compat=tlc"]);
    assert_eq!(out.status.code(), Some(150));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "end-to-end profile/store/replay matrix"
)]
fn bounded_final_temporal_check_reports_validated_lassos_and_mixed_profiles() {
    let spec = temp_file(
        "FinalTemporal.tla",
        "---- MODULE FinalTemporal ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nP == <>[]x\nQ == []<>x\n====\n",
    );
    let cfg = spec.with_extension("cfg");
    assert!(std::fs::write(&cfg, "SPECIFICATION Spec\nPROPERTY P Q\n").is_ok());
    let path = spec.to_string_lossy();
    // A nonzero polynomial selects the same ordinary TLC final route: temporal
    // capture keeps concrete states, so the polynomial cannot change the result.
    let ordinary = dive(&[
        "check",
        &path,
        "--compat=tlc",
        "--fp=1",
        "--seed=17",
        "--output",
        "json",
    ]);
    assert_eq!(ordinary.status.code(), Some(13));
    let ordinary: serde_json::Value = serde_json::from_slice(&ordinary.stdout).unwrap_or_default();
    assert_eq!(ordinary["verdict"], "liveness-violation");
    assert_eq!(ordinary["counterexample"]["validation"], "valid");
    for (profile, code) in [("tlc", 13), ("dive", 13)] {
        let run = |store: &str, workers: &str| {
            dive(&[
                "check",
                &path,
                "--lncheck=final",
                "--lasso=short",
                "--compat",
                profile,
                "--store",
                store,
                "--workers",
                workers,
                "--seed",
                "17",
                "--fp",
                "0",
                "--output=json",
            ])
        };
        for store in ["fp", "exact"] {
            let first = run(store, "1");
            let many = run(store, "8");
            assert_eq!(
                first.status.code(),
                Some(code),
                "{}",
                String::from_utf8_lossy(&first.stderr)
            );
            assert_eq!(many.status.code(), Some(code));
            let a: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap_or_default();
            let b: serde_json::Value = serde_json::from_slice(&many.stdout).unwrap_or_default();
            assert_eq!(a["schema_version"], 3);
            assert_eq!(a["verdict"], "liveness-violation");
            assert_eq!(a["properties"], serde_json::json!(["P"]));
            assert_eq!(a["counterexample"]["validation"], "valid");
            assert_eq!(a["counterexample"], b["counterexample"]);
            assert_eq!(a["statistics"], b["statistics"]);
            assert_eq!(
                a["options"]["profile"],
                if profile == "tlc" {
                    "tlc+overrides"
                } else {
                    "dive"
                }
            );
            assert_eq!(a["options"]["tlc_exact_temporal"], false);
            assert_eq!(a["options"]["lncheck"], "final");
            assert_eq!(a["options"]["lasso"], "short");
            assert_eq!(a["options"]["seed"], 17);
            assert_eq!(b["options"]["workers"]["used"], 8);
            assert_eq!(b["options"]["temporal_bounds"]["max_expansion_units"], 8);
            assert_eq!(
                b["options"]["temporal_bounds"]["successors_per_unit"],
                65_536
            );
            assert!(a["tlc"].is_null(), "unverified TLC class cannot be claimed");
            let trace_path = temp_file(
                &format!("temporal-{profile}-{store}.json"),
                &a["counterexample"]["trace"].to_string(),
            );
            let trace_path = trace_path.to_string_lossy();
            let validated = dive(&[
                "validate-trace",
                &path,
                "--trace",
                &trace_path,
                "--compat",
                profile,
                "--output",
                "json",
            ]);
            assert!(
                validated.status.success(),
                "{}",
                String::from_utf8_lossy(&validated.stdout)
            );
        }
    }
    let human = dive(&[
        "check",
        &path,
        "--lncheck",
        "final",
        "--lasso",
        "short",
        "--seed",
        "17",
        "--store",
        "exact",
    ]);
    assert_eq!(human.status.code(), Some(13));
    assert!(
        String::from_utf8_lossy(&human.stdout).contains("Independently validated counterexample")
    );
    // A true fair temporal property succeeds through the same backend.
    assert!(std::fs::write(&cfg, "SPECIFICATION Spec\nPROPERTY Q\n").is_ok());
    let held = dive(&[
        "check",
        &path,
        "--lncheck=final",
        "--lasso=short",
        "--output=json",
    ]);
    assert!(held.status.success());
    let held: serde_json::Value = serde_json::from_slice(&held.stdout).unwrap_or_default();
    assert_eq!(held["verdict"], "success");
    assert!(held["counterexample"].is_null());
    assert_eq!(held["statistics"]["distinct"], 2);
}

#[test]
fn final_binding_errors_preserve_primary_periodic_classes_and_exits() {
    let primary: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/periodic-binding.json")).unwrap_or_default();
    let controls = primary["controls"]
        .as_array()
        .unwrap_or_else(|| unreachable!("missing primary periodic controls"));
    assert_eq!(controls.len(), 2);
    for control in controls {
        let name = control["file"].as_str().unwrap_or_default();
        let spec = temp_file(name, control["source"].as_str().unwrap_or_default());
        assert!(
            std::fs::write(
                spec.with_extension("cfg"),
                control["config"].as_str().unwrap_or_default()
            )
            .is_ok()
        );
        for profile in ["--compat=dive", "--compat=tlc"] {
            for final_short in [false, true] {
                let path = spec.to_string_lossy();
                let mut args = vec!["check", &path, profile, "--seed=0", "--output=json"];
                if final_short {
                    args.extend(["--lncheck=final", "--lasso=short"]);
                }
                let output = dive(&args);
                let expected_exit =
                    i32::try_from(control["exit"].as_i64().unwrap_or_default()).unwrap_or_default();
                assert_eq!(output.status.code(), Some(expected_exit));
                let report: serde_json::Value =
                    serde_json::from_slice(&output.stdout).unwrap_or_default();
                assert_eq!(report["verdict"], "model-error");
                assert_eq!(report["tlc"]["code"], control["code"]);
                assert_eq!(report["tlc"]["name"], control["name"]);
                assert_eq!(report["tlc"]["exit_status"], control["exit"]);
                assert_eq!(report["tlc"]["reported"], control["code"]);
                assert_eq!(report["tlc"]["verified"], true);
                assert!(report["statistics"].is_null());
                assert!(report["counterexample"].is_null());
                assert_eq!(report["output"], serde_json::json!([]));
                if final_short {
                    assert_eq!(report["execution"]["stage"], "binding");
                    assert_eq!(report["execution"]["capture_completed"], false);
                }
            }
        }
    }
    // A valid hook remains a named refusal until hook execution is supported.
    let spec = temp_file(
        "ValidPeriodic.tla",
        "---- MODULE ValidPeriodic ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = ~x\nHook == TRUE\n====\n",
    );
    assert!(
        std::fs::write(
            spec.with_extension("cfg"),
            "INIT Init\nNEXT Next\n_PERIODIC Hook\n"
        )
        .is_ok()
    );
    for profile in ["--compat=dive", "--compat=tlc"] {
        let output = dive(&[
            "check",
            &spec.to_string_lossy(),
            profile,
            "--lncheck=final",
            "--lasso=short",
            "--output=json",
        ]);
        assert_eq!(output.status.code(), Some(3));
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_default();
        assert_eq!(report["verdict"], "unsupported");
        assert!(report["tlc"].is_null());
        assert!(
            report["message"]
                .as_str()
                .is_some_and(|m| m.contains("_PERIODIC"))
        );
    }
}

#[test]
fn numeric_temporal_operand_reports_pinned_evaluation_class_and_exit() {
    // Pinned TLC 2026.10.06.014338 reports 2215 / TLC_EXPECTED_VALUE and
    // exits 255 for this complete input, before generating initial states.
    let spec = temp_file(
        "BadTemporal.tla",
        "---- MODULE BadTemporal ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = ~x\nBad == <>1\n====\n",
    );
    assert!(
        std::fs::write(
            spec.with_extension("cfg"),
            "INIT Init\nNEXT Next\nPROPERTY Bad\n"
        )
        .is_ok()
    );
    for (profile, exit) in [("--compat=dive", 255), ("--compat=tlc", 255)] {
        let output = dive(&[
            "check",
            &spec.to_string_lossy(),
            "--lncheck=final",
            "--lasso=short",
            "--output=json",
            "--seed=0",
            profile,
        ]);
        assert_eq!(output.status.code(), Some(exit));
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_default();
        assert_eq!(report["verdict"], "evaluation-error");
        assert_eq!(report["tlc"]["code"], 2215);
        assert_eq!(report["tlc"]["name"], "TLC_EXPECTED_VALUE");
        assert_eq!(report["tlc"]["exit_status"], 255);
        assert_eq!(report["tlc"]["verified"], true);
        assert_eq!(report["execution"]["stage"], "planning");
        assert_eq!(report["execution"]["capture_completed"], false);
        assert_eq!(report["output"], serde_json::json!([]));
        assert!(report["statistics"].is_null());
        assert!(report["counterexample"].is_null());
        assert!(report["message"].as_str().is_some_and(|message| {
            message.contains("TLC expected a boolean value, but did not find one.")
        }));
    }
}

#[test]
fn final_temporal_mode_preserves_safety_stops_and_refuses_incomplete_options() {
    let spec = temp_file(
        "TemporalSafety.tla",
        "---- MODULE TemporalSafety ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nInv == ~x\nP == []<>x\n====\n",
    );
    assert!(
        std::fs::write(
            spec.with_extension("cfg"),
            "INIT Init\nNEXT Next\nINVARIANT Inv\nPROPERTY P\n"
        )
        .is_ok()
    );
    let path = spec.to_string_lossy();
    let base = dive(&[
        "check",
        &path,
        "--lncheck=final",
        "--lasso=short",
        "--output=json",
        "--seed=0",
    ]);
    assert_eq!(base.status.code(), Some(12));
    let base: serde_json::Value = serde_json::from_slice(&base.stdout).unwrap_or_default();
    assert_eq!(base["verdict"], "invariant-violation");
    assert_eq!(base["counterexample"]["ending"], "finite");
    assert_eq!(base["statistics"]["distinct"], 2);
    for flags in [
        ["--lncheck", "final"],
        ["--lasso", "short"],
        ["--lasso", "tlc"],
        ["--lncheck", "off"],
    ] {
        assert_eq!(
            dive(&["check", &path, flags[0], flags[1]]).status.code(),
            Some(2)
        );
    }
    // Preserve the former known-user forwarding fixture as a real supported
    // liveness violation: the allowed initial stutter falsifies []<>x.
    let opaque = temp_file(
        "OpaqueTemporal.tla",
        "---- MODULE OpaqueTemporal ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nT(p) == []<>p\nH(F(_),p) == F(p)\nP == H(T,x)\n====\n",
    );
    assert!(
        std::fs::write(
            opaque.with_extension("cfg"),
            "INIT Init\nNEXT Next\nPROPERTY P\n"
        )
        .is_ok()
    );
    let supported = dive(&[
        "check",
        &opaque.to_string_lossy(),
        "--seed=0",
        "--lncheck=final",
        "--lasso=short",
        "--output=json",
    ]);
    assert_eq!(supported.status.code(), Some(13));
    let supported: serde_json::Value =
        serde_json::from_slice(&supported.stdout).unwrap_or_default();
    assert_eq!(supported["verdict"], "liveness-violation");
    assert_eq!(supported["counterexample"]["validation"], "valid");
    assert_eq!(supported["counterexample"]["ending"], "lasso");
    assert_eq!(supported["counterexample"]["back_to"], 1);
    assert_eq!(supported["counterexample"]["length"], 1);
    assert_eq!(
        supported["counterexample"]["states"][0]["values"]["x"],
        false
    );
    // A genuinely temporal recursive operator actual stays a named planning
    // refusal. A state-only PROPERTY follows the ordinary safety engine below.
    let higher = temp_file(
        "HigherTemporal.tla",
        "---- MODULE HigherTemporal ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nRECURSIVE T(_)\nT(p) == IF p THEN p ELSE T(p)\nH(F(_),p) == F(p)\nP == []<>H(T,x)\n====\n",
    );
    assert!(
        std::fs::write(
            higher.with_extension("cfg"),
            "INIT Init\nNEXT Next\nPROPERTY P\n"
        )
        .is_ok()
    );
    let rejected = dive(&[
        "check",
        &higher.to_string_lossy(),
        "--seed=0",
        "--workers=1",
        "--lncheck=final",
        "--lasso=short",
        "--output=json",
    ]);
    assert_eq!(rejected.status.code(), Some(3));
    let rejected: serde_json::Value = serde_json::from_slice(&rejected.stdout).unwrap_or_default();
    assert_eq!(rejected["verdict"], "unsupported");
    assert!(rejected["counterexample"].is_null());
    assert!(rejected["statistics"].is_null());
    assert_eq!(rejected["diagnostics"][0]["code"], "DIVE-L0020");
    assert!(rejected["message"].as_str().is_some_and(|message| {
        message.contains("opaque/recursive/higher-order operator capture")
    }));
    assert_eq!(rejected["execution"]["stage"], "planning");
    assert_eq!(rejected["execution"]["capture_completed"], false);
    assert_eq!(rejected["options"]["workers"]["used"], 0);
}

#[test]
#[allow(
    clippy::unwrap_used,
    reason = "fixture and JSON failures must fail this command-contract test"
)]
fn simulate_is_a_seeded_finite_sample_and_refuses_unbuilt_modes() {
    let spec = temp_file(
        "SimCli.tla",
        "---- MODULE SimCli ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\n====\n",
    );
    std::fs::write(spec.with_extension("cfg"), "INIT Init\nNEXT Next\n").unwrap();
    let text = spec.to_str().unwrap();
    let out = dive(&[
        "simulate", text, "--seed", "7", "--traces", "2", "--depth", "3", "--output", "json",
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["command"], "simulate");
    assert_eq!(json["result"], "sampled");
    assert_eq!(json["model_proved"], false);
    assert_eq!(json["traces"].as_array().unwrap().len(), 2);
    assert_eq!(dive(&["simulate", text]).status.code(), Some(2));
    assert_eq!(
        dive(&["simulate", text, "--traces", "0"]).status.code(),
        Some(2)
    );
    for option in ["--continue", "--probabilistic", "--rl", "--distributed"] {
        assert_eq!(
            dive(&["simulate", text, "--traces", "2", option])
                .status
                .code(),
            Some(3)
        );
    }
    assert_constructive_simulation_sample();
}

#[allow(
    clippy::unwrap_used,
    reason = "fixture and JSON failures must fail this constructive command-contract test"
)]
fn assert_constructive_simulation_sample() {
    let spec = temp_file(
        "ConstructiveSimCli.tla",
        "---- MODULE ConstructiveSimCli ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x>=0 /\\ (\\E n \\in 1..16 : x'=n)\nInv == x>=0\n====\n",
    );
    std::fs::write(
        spec.with_extension("cfg"),
        "INIT Init\nNEXT Next\nINVARIANT Inv\n",
    )
    .unwrap();
    let text = spec.to_str().unwrap();
    let args = [
        "simulate",
        text,
        "--generate",
        "--seed",
        "7",
        "--traces",
        "2",
        "--depth",
        "3",
        "--output",
        "json",
    ];
    let out = dive(&args);
    assert!(out.status.success(), "{out:?}");
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["command"], "simulate");
    assert_eq!(report["result"], "sampled");
    assert_eq!(report["exhaustive"], false);
    assert_eq!(report["model_proved"], false);
    assert_eq!(report["options"]["algorithm"], "constructive");
    assert_eq!(report["options"]["probabilistic"], true);
    assert_eq!(report["options"]["seed"], 7);
    assert_eq!(report["options"]["seed_given"], true);
    assert_eq!(report["options"]["workers_used"], 1);
    assert_eq!(report["options"]["depth"], 3);
    assert_eq!(report["options"]["traces"], 2);
    assert_eq!(report["statistics"]["generated"], 7);
    let repeated = dive(&args);
    assert!(repeated.status.success(), "{repeated:?}");
    let repeated: serde_json::Value = serde_json::from_slice(&repeated.stdout).unwrap();
    assert_eq!(report["traces"], repeated["traces"]);
    let traces = report["traces"].as_array().unwrap();
    assert_eq!(traces.len(), 2);
    for trace in traces {
        let word = &trace["document"];
        assert_eq!(word["format"], "dive-trace");
        assert_eq!(word["ending"]["kind"], "behavior");
        let states = word["states"].as_array().unwrap();
        assert_eq!(states.len(), 4);
        assert_eq!(states[0]["values"]["x"], 0);
        for state in &states[1..] {
            assert!(
                state["values"]["x"]
                    .as_u64()
                    .is_some_and(|x| (1..=16).contains(&x))
            );
        }
        let path = temp_file("ConstructiveSimCliSample.json", &word.to_string());
        let checked = dive(&[
            "validate-trace",
            text,
            "--sample",
            "--trace",
            path.to_str().unwrap(),
            "--output",
            "json",
        ]);
        assert!(checked.status.success(), "{checked:?}");
        let checked: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
        assert_eq!(checked["command"], "validate-trace");
        assert_eq!(checked["validation_goal"], "sample");
        assert_eq!(checked["result"], "valid");
        assert_eq!(checked["length"], 4);
    }
}

#[test]
#[allow(clippy::expect_used, reason = "CLI JSON assertions")]
fn frontier_spill_flags_and_directory_dependency() {
    let spec = temp_file(
        "SpillFlags.tla",
        "---- MODULE SpillFlags ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\n====\n",
    );
    let cfg = spec.with_extension("cfg");
    std::fs::write(&cfg, "INIT Init\nNEXT Next\n").expect("cfg");
    let path = spec.to_string_lossy();
    let out = dive(&[
        "check",
        &path,
        "--compat=tlc",
        "--frontier-memory-bytes",
        "0",
        "--output",
        "json",
    ]);
    assert_eq!(out.status.code(), Some(0));
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("JSON");
    assert_eq!(json["verdict"], "success");
    assert_eq!(json["frontier_storage"]["memory_bytes_per_queue"], 0);
    assert_eq!(json["frontier_storage"]["retained_bytes"], 0);
    assert!(
        json["frontier_storage"]["committed_spill_bytes"]
            .as_u64()
            .is_some_and(|n| n > 0)
    );
    for args in [
        vec!["check", &path, "--frontier-directory", "."],
        vec!["check", &path, "--frontier-memory-bytes", "bad"],
    ] {
        assert_eq!(dive(&args).status.code(), Some(2));
    }
    for args in [
        vec![
            "simulate",
            &path,
            "--traces",
            "1",
            "--frontier-memory-bytes",
            "0",
        ],
        vec![
            "dfid",
            &path,
            "--max-level",
            "2",
            "--frontier-memory-bytes",
            "0",
        ],
    ] {
        let refused = dive(&args);
        assert_eq!(refused.status.code(), Some(3));
        assert_eq!(refused.stdout, [] as [u8; 0]);
    }
}

#[test]
fn translate_returns_an_admitted_module_without_rewriting_input()
-> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(dive(&["translate"]).status.code(), Some(2));
    let source = "---- MODULE TranslateNative ----\nEXTENDS Integers, TLC\n(* --algorithm A variable x=0; begin L: x:=1; E: skip; end algorithm; *)\n====\n";
    let file = temp_file("TranslateNative.tla", source);
    let path = file.to_string_lossy();
    let out = dive(&["translate", &path, "--compat=tlc", "--output", "json"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    assert_eq!(document["result"], "artifact");
    assert!(document.get("model_proved").is_none());
    assert!(document.get("exhaustive").is_none());
    assert_eq!(document["analysis"]["accepted"], true);
    assert!(
        document["module_source"]
            .as_str()
            .is_some_and(|text| text.contains("Next =="))
    );
    assert_eq!(std::fs::read_to_string(&file)?, source);
    let out = dive(&["translate", &path, "--write", "--output", "json"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let written: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    assert_eq!(written["filesystem_plan"]["writes_performed"], true);
    assert_eq!(
        std::fs::read_to_string(&file)?,
        written["replacement"]["module_source"]
            .as_str()
            .ok_or("module source")?
    );
    assert_eq!(std::fs::read_to_string(file.with_extension("old"))?, source);
    Ok(())
}

#[test]
fn translate_label_options_preserve_files_and_default_configuration()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "---- MODULE TranslateLabels ----\nEXTENDS Naturals,TLC\n(* --algorithm A variable x; begin x:=1; E: skip; end algorithm; *)\n====\n";
    let file = temp_file("TranslateLabels.tla", source);
    let path = file.to_string_lossy();
    let out = dive(&[
        "translate",
        &path,
        "--label-root",
        "Step",
        "--output",
        "json",
    ]);
    assert_eq!(out.status.code(), Some(1));
    for flags in [
        ["--label", "--label-root", "Step"],
        ["-reportLabels", "-labelRoot", "Step"],
    ] {
        let out = dive(&[
            "translate",
            &path,
            flags[0],
            flags[1],
            flags[2],
            "--output",
            "json",
        ]);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let doc: serde_json::Value = serde_json::from_slice(&out.stdout)?;
        assert_eq!(
            doc["artifact"]["actions"],
            serde_json::json!(["Step1", "E"])
        );
        assert_eq!(
            doc["artifact"]["configuration"],
            "CONSTANT defaultInitValue = defaultInitValue\n"
        );
        assert!(doc.get("model_proved").is_none());
    }
    assert_eq!(std::fs::read_to_string(file)?, source);
    Ok(())
}
#[test]
fn translate_process_storage_and_label_default_are_explicit()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "---- MODULE TranslateProcess ----\nEXTENDS Naturals,TLC\n(* --algorithm A process P \\in{1,2} variables x; begin x:=1; E: skip; end process; end algorithm; *)\n====\n";
    let file = temp_file("TranslateProcess.tla", source);
    let path = file.to_string_lossy();
    assert_eq!(
        dive(&["translate", &path, "--output", "json"])
            .status
            .code(),
        Some(1)
    );
    let out = dive(&[
        "translate",
        &path,
        "--compat=tlc",
        "--label",
        "--output",
        "json",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    assert_eq!(doc["artifact"]["processes"][0]["binding_kind"], "Member");
    assert_eq!(
        doc["artifact"]["configuration"],
        "CONSTANT defaultInitValue = defaultInitValue\n"
    );
    assert!(doc.get("model_proved").is_none());
    assert_eq!(std::fs::read_to_string(file)?, source);
    Ok(())
}

#[test]
fn native_checkpoint_option_contract_rejects_orphans_and_unsupported_modes() {
    for flags in [
        vec!["--checkpoint", "unused.cp"],
        vec!["--checkpoint-after-levels", "0"],
        vec!["--recover", "unused.cp", "--checkpoint-after-levels", "0"],
        vec![
            "--checkpoint",
            "unused.cp",
            "--checkpoint-after-levels",
            "0",
            "--recover",
            "other.cp",
        ],
        vec!["--checkpoint-counters", "retain"],
        vec!["--recover", "unused.cp", "--checkpoint-counters", "wrong"],
    ] {
        let mut args = vec!["check", "missing-native-spec.tla"];
        args.extend(flags);
        let output = dive(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("checkpoint"));
    }
    for flags in [
        vec!["--lncheck", "final", "--lasso", "short"],
        vec!["--frontier-directory", "unused"],
        vec!["--predecessor-directory", "unused"],
    ] {
        let mut args = vec![
            "check",
            "missing-native-spec.tla",
            "--checkpoint",
            "unused.cp",
            "--checkpoint-after-levels",
            "0",
        ];
        args.extend(flags);
        let output = dive(&args);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("checkpoint")
                || String::from_utf8_lossy(&output.stderr).contains("predecessor")
                || String::from_utf8_lossy(&output.stderr).contains("frontier")
        );
        assert!(!std::path::Path::new("unused.cp").exists());
    }
}

#[test]
fn native_checkpoint_spill_budgets_pass_option_admission_before_input_error() {
    for flags in [
        vec!["--frontier-memory-bytes", "0"],
        vec!["--predecessor-memory-bytes", "0"],
        vec![
            "--frontier-memory-bytes",
            "0",
            "--predecessor-memory-bytes",
            "0",
        ],
    ] {
        let archive = "admitted-spill-checkpoint-missing.cp";
        let mut args = vec![
            "check",
            "missing-native-spec.tla",
            "--checkpoint",
            archive,
            "--checkpoint-after-levels",
            "0",
        ];
        args.extend(flags);
        let output = dive(&args);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(output.stdout, [] as [u8; 0]);
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("cannot read missing-native-spec.tla")
        );
        assert!(!std::path::Path::new(archive).exists());
    }
}

#[test]
fn translate_process_fairness_is_explicit_and_never_a_proof()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "---- MODULE TranslateFairness ----\nEXTENDS Naturals,TLC\n(* --algorithm A variable g=0; process P \\in{1,2} variable x=0; begin L: + x:=1; E: - g:=x; end process; end algorithm; *)\n====\n";
    let file = temp_file("TranslateFairness.tla", source);
    let path = file.to_string_lossy();
    for (flag, option, present) in [
        ("-wf", "Weak", "WF_vars((pc[self] #"),
        ("-sf", "Strong", "SF_vars((pc[self] #"),
        ("-nof", "None", "Spec == Init"),
        ("-wfNext", "Next", "WF_vars(Next)"),
    ] {
        let out = dive(&["translate", &path, "--compat=tlc", flag, "--output", "json"]);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let doc: serde_json::Value = serde_json::from_slice(&out.stdout)?;
        assert_eq!(doc["limits"]["fairness"], option);
        assert_eq!(doc["accepted"], true);
        assert!(doc.get("exhaustive").is_none());
        assert!(doc.get("model_proved").is_none());
        assert!(
            doc["artifact"]["tla"]
                .as_str()
                .is_some_and(|text| text.contains(present))
        );
    }
    assert_eq!(std::fs::read_to_string(file)?, source);
    Ok(())
}

#[test]
fn translate_rejects_multiple_fairness_flags_before_loading() {
    for (first, second) in [("-wf", "-sf"), ("-nof", "-wfNext"), ("-wf", "-wf")] {
        let output = dive(&[
            "translate",
            "missing-option-control.tla",
            first,
            second,
            "--rewrite",
        ]);
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("select only one"));
        assert_eq!(output.stdout, [] as [u8; 0]);
    }
}

#[test]
fn composition_modes_and_jvm_alias_override_profile_in_either_order()
-> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-eval/tests/fixtures/cdot/specs/CdDeadlock/CdDeadlock.tla");
    let controls: &[(&[&str], &str, i32, u64)] = &[
        (&[], "tla", 11, 1),
        (&["--compat=tlc"], "off", 75, 1),
        (&["--compat=tlc", "--action-composition=tlc"], "tlc", 0, 2),
        (&["--action-composition=tlc", "--compat=tlc"], "tlc", 0, 2),
        (&["--compat=tlc", "--action-composition=tla"], "tla", 11, 1),
        (&["--action-composition=tla", "--compat=tlc"], "tla", 11, 1),
        (
            &["--compat=tlc", "-Dtlc2.tool.impl.Tool.cdot=true"],
            "tlc",
            0,
            2,
        ),
        (
            &["-Dtlc2.tool.impl.Tool.cdot=true", "--compat=tlc"],
            "tlc",
            0,
            2,
        ),
        (
            &[
                "--action-composition=tla",
                "-Dtlc2.tool.impl.Tool.cdot=true",
            ],
            "tlc",
            0,
            2,
        ),
        (
            &[
                "-Dtlc2.tool.impl.Tool.cdot=true",
                "--action-composition=off",
            ],
            "off",
            75,
            1,
        ),
        (
            &[
                "-Dtlc2.tool.impl.Tool.cdot=true",
                "--action-composition=off",
                "--exit-codes=native",
            ],
            "off",
            13,
            1,
        ),
    ];
    for (flags, mode, exit, generated) in controls {
        let path = path.to_string_lossy();
        let mut args = vec!["check", &path, "--seed=0", "--fp=0", "--output=json"];
        args.extend_from_slice(flags);
        let output = dive(&args);
        let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(output.status.code(), Some(*exit), "{flags:?}: {report}");
        assert_eq!(report["options"]["action_composition"], *mode);
        assert_eq!(report["statistics"]["generated"], *generated);
        assert_eq!(report["statistics"]["distinct"], 1);
        assert_eq!(report["tlc"]["verified"], true, "{report}");
        if *mode == "off" {
            assert_eq!(report["verdict"], "evaluation-error");
            assert_eq!(report["tlc"]["code"], 2103);
            assert_eq!(report["tlc"]["reported"], 1000);
            assert_eq!(report["tlc"]["exit_status"], 75);
        }
    }
    Ok(())
}

#[test]
fn simulation_basic_statistics_are_optional_and_replayable() {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-sim/tests/fixtures/SimChoice.tla");
    for profile in [false, true] {
        let mut args = vec![
            "simulate",
            file.to_str().unwrap_or_default(),
            "--traces",
            "3",
            "--depth",
            "5",
            "--seed",
            "7",
            "--output",
            "json",
        ];
        if profile {
            args.push("--compat=tlc");
        }
        let plain = dive(&args);
        assert!(plain.status.success(), "{plain:?}");
        args.extend(["--stats", "basic"]);
        let counted = dive(&args);
        assert!(counted.status.success(), "{counted:?}");
        let a: serde_json::Value = serde_json::from_slice(&plain.stdout).unwrap_or_default();
        let b: serde_json::Value = serde_json::from_slice(&counted.stdout).unwrap_or_default();
        assert_eq!(a["traces"], b["traces"]);
        assert_eq!(a["statistics"], b["statistics"]);
        assert_eq!(b["options"]["action_stats"], true);
        assert_eq!(b["action_flow"]["mode"], "basic");
        let replay = dive(&args);
        assert_eq!(counted.stdout, replay.stdout);
    }
    for (stats, code) in [("wrong", 2), ("full", 3)] {
        let output = dive(&["simulate", "missing.tla", "--traces", "1", "--stats", stats]);
        assert_eq!(output.status.code(), Some(code), "{output:?}");
        assert_eq!(output.stdout, [] as [u8; 0]);
    }
}

#[test]
fn recursive_state_property_final_mode_matches_ordinary_evaluation()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "---- MODULE RecursiveState ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nRECURSIVE T(_)\nT(p) == IF p THEN p ELSE T(p)\nH(F(_),p) == F(p)\nP == H(T,x)\n====\n";
    let spec = temp_file("RecursiveState.tla", source);
    std::fs::write(
        spec.with_extension("cfg"),
        "INIT Init\nNEXT Next\nPROPERTY P\n",
    )?;
    let path = spec.to_string_lossy();
    for initial in [false, true] {
        std::fs::write(
            &spec,
            source.replace(
                "Init == x = FALSE",
                if initial {
                    "Init == x = TRUE"
                } else {
                    "Init == x = FALSE"
                },
            ),
        )?;
        for profile in ["--compat=dive", "--compat=tlc"] {
            let base = [
                "check",
                &path,
                profile,
                "--seed=0",
                "--workers=1",
                "--output=json",
            ];
            let ordinary = dive(&base);
            let mut args = base.to_vec();
            args.extend(["--lncheck=final", "--lasso=short"]);
            let final_mode = dive(&args);
            let expected = if initial { 0 } else { 3 };
            assert_eq!(ordinary.status.code(), Some(expected));
            assert_eq!(final_mode.status.code(), Some(expected));
            let a: serde_json::Value = serde_json::from_slice(&ordinary.stdout)?;
            let b: serde_json::Value = serde_json::from_slice(&final_mode.stdout)?;
            for key in [
                "verdict",
                "message",
                "statistics",
                "property",
                "counterexample",
                "diagnostics",
                "output",
                "tlc",
                "warnings",
            ] {
                assert_eq!(a[key], b[key], "{initial}/{profile}/{key}");
            }
            assert_eq!(b["execution"]["stage"], "capture");
            assert_eq!(b["execution"]["capture_completed"], false);
            assert_eq!(b["options"]["workers"]["requested"], 1);
            assert_eq!(b["options"]["seed"], 0);
            assert_eq!(b["statistics"]["initial_generated"], 1);
            assert!(b["property"].is_null());
            assert_eq!(b["diagnostics"], serde_json::json!([]));
            if initial {
                assert_eq!(b["verdict"], "success");
                assert!(b["message"].is_null());
                assert!(b["counterexample"].is_null());
                assert_eq!(b["statistics"]["generated"], 3);
                assert_eq!(b["statistics"]["distinct"], 2);
                assert_eq!(b["statistics"]["depth"], 2);
                assert_eq!(b["statistics"]["left_on_queue"], 0);
            } else {
                assert_eq!(b["verdict"], "resource-limit");
                assert_eq!(
                    b["message"],
                    "evaluation nested more than 400 levels deep (recursion at line 6, col 28 to line 6, col 28 of module RecursiveState)"
                );
                assert_eq!(b["statistics"]["generated"], 1);
                assert_eq!(b["statistics"]["distinct"], 1);
                assert_eq!(b["statistics"]["depth"], 1);
                assert_eq!(b["statistics"]["left_on_queue"], 1);
                assert_eq!(b["counterexample"]["ending"], "finite");
                assert_eq!(b["counterexample"]["length"], 1);
                assert_eq!(b["counterexample"]["states"][0]["values"]["x"], false);
                assert!(b["tlc"].is_null());
            }
        }
    }
    Ok(())
}

#[test]
fn translate_remaining_flags_report_artifacts_without_writing()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "---- MODULE TranslateFlags ----\nEXTENDS Naturals,TLC\n(* --algorithm A variable x=0; begin L:x:=1; E:skip; end algorithm; *)\n====\n";
    let file = temp_file("TranslateFlags.tla", source);
    let path = file.to_string_lossy();
    let out = dive(&[
        "translate",
        &path,
        "-termination",
        "-noDoneDisjunct",
        "-lineWidth",
        "60",
        "-version",
        "1.903",
        "-noold",
        "-nocfg",
        "-unixEOL",
        "--rewrite",
        "--output",
        "json",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    assert_eq!(doc["translation_options"]["line_width"], 60);
    assert_eq!(doc["translation_options"]["version"], "1.903");
    assert_eq!(doc["artifact"]["configuration"], "PROPERTY Termination\n");
    assert_eq!(doc["filesystem_plan"]["writes_performed"], false);
    assert_eq!(doc["filesystem_plan"]["backup_requested"], false);
    assert_eq!(doc["filesystem_plan"]["configuration_requested"], false);
    assert_eq!(std::fs::read_to_string(&file)?, source);
    let debug = dive(&["translate", &path, "-debug", "--output", "json"]);
    assert_eq!(
        debug.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&debug.stderr)
    );
    let debug_doc: serde_json::Value = serde_json::from_slice(&debug.stdout)?;
    assert_eq!(debug_doc["translation_options"]["debug"], true);
    assert_eq!(std::fs::read_to_string(&file)?, source);
    for flag in ["-spec", "-myspec", "-spec2", "-myspec2"] {
        let out = dive(&["translate", &path, flag, "LocalSpec", "--output", "json"]);
        assert_eq!(out.status.code(), Some(3));
        assert_eq!(std::fs::read_to_string(&file)?, source);
    }
    let help = dive(&["translate", "-help"]);
    assert_eq!(help.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&help.stdout).contains("-lineWidth"));
    let bad = dive(&["translate", &path, "-lineWidth", "59"]);
    assert_eq!(bad.status.code(), Some(2));
    Ok(())
}

#[test]
fn translate_report_labels_is_distinct_from_insertion() -> Result<(), Box<dyn std::error::Error>> {
    let source = "---- MODULE TranslateReportLabels ----\nEXTENDS Naturals,TLC\n(* --algorithm A variable x=0; process P\\in {1,2} begin x:=1; end process; end algorithm; *)\n====\n";
    let file = temp_file("TranslateReportLabels.tla", source);
    let path = file.to_string_lossy();
    let report = dive(&["translate", &path, "-reportLabels", "-labelRoot", "Step"]);
    assert_eq!(report.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&report.stderr)
            .contains("The following label was added:\n  Step1 at line 3, column 57")
    );
    let inserted = dive(&["translate", &path, "-label", "-labelRoot", "Step"]);
    assert_eq!(inserted.status.code(), Some(0));
    assert_eq!(report.stdout, inserted.stdout);
    assert!(!String::from_utf8_lossy(&inserted.stderr).contains("labels were added"));
    assert_eq!(std::fs::read_to_string(file)?, source);
    Ok(())
}

#[test]
fn translate_original_ast_and_debug_consume_primary_controls()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../dive-pluscal/tests/fixtures/original-ast/primary.json"
    ))?;
    let controls = fixture["controls"].as_array().ok_or("controls")?;
    let source = controls[0]["source"].as_str().ok_or("source")?;
    let file = temp_file("Case.tla", source);
    let path = file.to_string_lossy();
    for compat_tlc in [false, true] {
        let profile: &[&str] = if compat_tlc { &["--compat=tlc"] } else { &[] };
        for (debug, index) in [(false, 0usize), (true, 1)] {
            let mut args = vec!["translate", path.as_ref(), "--ast", "--output", "json"];
            args.extend_from_slice(profile);
            if debug {
                args.push("-debug");
            }
            let output = dive(&args);
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let doc: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            assert_eq!(
                doc["ast_artifact"]["module_source"],
                controls[index]["original_module"]
            );
            assert_eq!(doc["artifact_available"], true);
            assert_eq!(doc["accepted"], false);
            assert_eq!(doc["writes_performed"], false);
        }
        let mut args = vec![
            "translate",
            path.as_ref(),
            "-debug",
            "--rewrite",
            "--output",
            "json",
        ];
        args.extend_from_slice(profile);
        let output = dive(&args);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let doc: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(
            doc["replacement"]["algorithm_checksum"],
            controls[2]["algorithm_crc"]
        );
    }
    let human = dive(&["translate", path.as_ref(), "--ast", "-debug"]);
    assert_eq!(human.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(human.stdout)?,
        controls[1]["original_module"].as_str().ok_or("module")?
    );
    assert_eq!(std::fs::read_to_string(&file)?, source);
    for name in ["AST.tla", "Case.old", "Case.cfg"] {
        assert!(!file.with_file_name(name).exists());
    }
    let conflict = dive(&["translate", path.as_ref(), "--ast", "--rewrite"]);
    assert_eq!(conflict.status.code(), Some(2));
    Ok(())
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "paired profiles and untouched sentinel assertions consume each stage-specific CLI control"
)]
fn translate_original_ownership_controls_keep_files_untouched()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/original-ast-ownership");
    for compat_tlc in [false, true] {
        for id in [
            "owners_debug",
            "macro_multiline_debug",
            "label_if_debug",
            "corrected_c_wrapper_debug",
            "if_translation_debug",
            "with_translation_debug",
            "corrected_macro_nested_translation_debug",
            "label_if_translation_debug",
            "label_either_translation_debug",
            "while_translation_debug",
            "owners_sequences_translation_debug",
        ] {
            let row: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(root.join(format!("{id}.json")))?)?;
            let source = row["source"].as_str().ok_or("source")?;
            let file = temp_file(&format!("Ownership-{id}-{compat_tlc}.tla"), source)
                .with_file_name("Case.tla");
            std::fs::write(&file, source)?;
            for name in ["AST.tla", "Case.old", "Case.cfg"] {
                std::fs::write(file.with_file_name(name), format!("\\* sentinel {name}\n"))?;
            }
            let path = file.to_string_lossy();
            let profile: &[&str] = if compat_tlc { &["--compat=tlc"] } else { &[] };
            let mut args = vec![
                "translate",
                path.as_ref(),
                "--ast",
                "-debug",
                "--output",
                "json",
            ];
            args.extend_from_slice(profile);
            if row["insert_labels"] == true {
                args.push("-label");
            }
            let output = dive(&args);
            assert_eq!(
                output.status.code(),
                Some(0),
                "{id}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let doc: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            assert_eq!(doc["artifact_available"], true);
            assert_eq!(doc["accepted"], false);
            assert_eq!(doc["generated_module_accepted"], false);
            assert_eq!(doc["writes_performed"], false);
            if row["stage"] == "pre-generation WriteAST" {
                assert_eq!(
                    doc["ast_artifact"]["module_source"], row["original_module"],
                    "{id}"
                );
            } else {
                let mut args = vec![
                    "translate",
                    path.as_ref(),
                    "-debug",
                    "--rewrite",
                    "--output",
                    "json",
                ];
                args.extend_from_slice(profile);
                if row["insert_labels"] == true {
                    args.push("-label");
                }
                let output = dive(&args);
                if row["native_pending"].is_string() {
                    assert_eq!(output.status.code(), Some(3));
                } else {
                    assert_eq!(
                        output.status.code(),
                        Some(0),
                        "{id}: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    let doc: serde_json::Value = serde_json::from_slice(&output.stdout)?;
                    if id == "owners_sequences_translation_debug" {
                        // This genuine owned-procedure fixture has no exact CRC after
                        // retiring header-transfer emulation; original AST/body remain real.
                        assert!(doc["replacement"]["algorithm_checksum"].is_null());
                        assert!(doc["replacement"]["warnings"].as_array().is_some_and(
                            |warnings| warnings.iter().any(|warning| warning["code"]
                                == "Unsupported"
                                && warning["message"].as_str().is_some_and(|message| {
                                    message.contains("procedure AST checksum is unavailable")
                                }))
                        ));
                    } else {
                        assert_eq!(
                            doc["replacement"]["algorithm_checksum"],
                            row["algorithm_crc"]
                        );
                    }
                    assert_eq!(doc["replacement"]["translation_checksum"], row["body_crc"]);
                }
            }
            let mut args = vec![
                "translate",
                path.as_ref(),
                "-writeAST",
                "-debug",
                "--output",
                "json",
            ];
            args.extend_from_slice(profile);
            if row["insert_labels"] == true {
                args.push("-label");
            }
            let ast_output = Command::new(env!("CARGO_BIN_EXE_dive"))
                .current_dir(file.parent().ok_or("parent")?)
                .args(&args)
                .output()?;
            assert_eq!(ast_output.status.code(), Some(255), "{ast_output:?}");
            let ast_doc: serde_json::Value = serde_json::from_slice(&ast_output.stdout)?;
            assert_eq!(ast_doc["writes_performed"], true);
            assert_eq!(ast_doc["requested_translation_options"]["write_ast"], true);
            assert_eq!(ast_doc["ast_artifact"], doc["ast_artifact"]);
            assert_eq!(
                std::fs::read_to_string(file.with_file_name("AST.tla"))?,
                ast_doc["ast_artifact"]["module_source"]
                    .as_str()
                    .ok_or("AST module")?
            );
            assert_eq!(std::fs::read_to_string(&file)?, source);
            for name in ["Case.old", "Case.cfg"] {
                assert_eq!(
                    std::fs::read_to_string(file.with_file_name(name))?,
                    format!("\\* sentinel {name}\n")
                );
            }
        }
    }
    Ok(())
}

#[test]
fn translate_best_effort_checksum_is_data_and_invalid_fair_flag_stays_a_cli_error()
-> Result<(), Box<dyn std::error::Error>> {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/procedure/cases/Procedure_P_simple.tla");
    let text = file.to_str().ok_or("path")?;
    let before = std::fs::read(&file)?;
    for compat_tlc in [false, true] {
        let profile: &[&str] = if compat_tlc { &["--compat=tlc"] } else { &[] };
        for policy in ["full", "tla"] {
            let mut args = vec![
                "translate",
                text,
                "--rewrite",
                "--checksums",
                policy,
                "--output",
                "json",
            ];
            args.extend_from_slice(profile);
            let output = dive(&args);
            assert_eq!(output.status.code(), Some(0));
            let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            assert_eq!(report["schema_version"], 3);
            assert_eq!(report["analysis"]["accepted"], true);
            assert!(report["replacement"]["algorithm_checksum"].is_null());
            assert!(report.get("model_proved").is_none());
            assert!(
                report["replacement"]["module_source"].as_str().is_some_and(
                    |module| module.contains("chksum(pcal) \\in STRING /\\ chksum(tla) = ")
                )
            );
        }
        let mut args = vec!["translate", text, "-fair"];
        args.extend_from_slice(profile);
        let invalid = dive(&args);
        assert_eq!(invalid.status.code(), Some(2));
        assert!(
            String::from_utf8_lossy(&invalid.stderr)
                .contains("unknown dive translate option -fair")
        );
    }
    let human = dive(&["translate", text, "--rewrite"]);
    assert_eq!(human.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&human.stderr).contains("algorithm checksum unavailable"));
    assert_eq!(std::fs::read(&file)?, before);
    Ok(())
}

#[test]
fn translate_compat_overwrites_like_primary_noop_and_default_requires_the_grant()
-> Result<(), Box<dyn std::error::Error>> {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/best-effort-checksum/Case.modified-body-input.tla");
    let before = std::fs::read(&file)?;
    let input = std::str::from_utf8(&before)?;
    let run_file = temp_file("Primary-modified-body", input).with_file_name("Case.tla");
    std::fs::write(&run_file, &before)?;
    let path = run_file.to_str().ok_or("path")?;
    for (flags, accepted) in [
        (vec![], false),
        (vec!["--replace-modified"], true),
        (vec!["--compat=tlc"], true),
    ] {
        let mut args = vec!["translate", path, "--rewrite", "--output", "json"];
        args.extend(flags);
        let output = dive(&args);
        assert_eq!(output.status.code(), Some(if accepted { 0 } else { 3 }));
        let doc: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(doc["accepted"], accepted);
        if accepted {
            assert_eq!(doc["editing"]["replace_modified"], true);
            assert_eq!(
                doc["replacement"]["selection"]["checksum"]["matched"],
                false
            );
            assert!(
                doc["replacement"]["module_source"]
                    .as_str()
                    .is_some_and(|source| source.contains("x' = 1") && !source.contains("x' = 2"))
            );
        } else {
            assert_eq!(doc["editing"]["replace_modified"], false);
            assert!(doc.get("replacement").is_none());
        }
        assert_eq!(std::fs::read(&file)?, before);
        assert_eq!(std::fs::read(&run_file)?, before);
    }
    Ok(())
}

#[test]
fn translate_write_ast_keeps_source_and_filesystem_errors_separate_from_admission()
-> Result<(), Box<dyn std::error::Error>> {
    let invalid =
        "---- MODULE AstRejected ----\n(* --algorithm A begin L: if end algorithm *)\n====\n";
    let file = temp_file("AstRejected.tla", invalid);
    let directory = file.parent().ok_or("parent")?;
    let ast = directory.join("AST.tla");
    std::fs::write(&ast, "AST sentinel")?;
    let output = Command::new(env!("CARGO_BIN_EXE_dive"))
        .current_dir(directory)
        .args([
            "translate",
            file.to_str().ok_or("path")?,
            "-writeAST",
            "--output",
            "json",
        ])
        .output()?;
    assert_eq!(output.status.code(), Some(255));
    let document: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(document["writes_performed"], false);
    assert_eq!(document["accepted"], false);
    assert_eq!(document["exit_code"], 255);
    assert_eq!(std::fs::read_to_string(&ast)?, "AST sentinel");
    assert_eq!(std::fs::read_to_string(&file)?, invalid);
    std::fs::remove_file(&ast)?;
    std::fs::create_dir(&ast)?;
    let valid = invalid.replace("if end", "skip; end");
    std::fs::write(&file, &valid)?;
    let failed_write = Command::new(env!("CARGO_BIN_EXE_dive"))
        .current_dir(directory)
        .args([
            "translate",
            file.to_str().ok_or("path")?,
            "-writeAST",
            "--output",
            "json",
        ])
        .output()?;
    assert_eq!(failed_write.status.code(), Some(255));
    assert!(ast.is_dir());
    assert_eq!(std::fs::read_to_string(&file)?, valid);
    std::fs::remove_dir(ast)?;
    Ok(())
}

#[test]
fn core_candidate_flags_run_real_spill_and_reject_missing_budget() {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/dive-api/tests/fixtures/core-m0/CompoundTrace.tla");
    for profile in ["dive", "tlc"] {
        let output = dive(&[
            "check",
            &file.to_string_lossy(),
            "--engine=core",
            &format!("--compat={profile}"),
            "--workers=4",
            "--seed=0",
            "--fp=0",
            "--candidate-memory-bytes=0",
            "--output=json",
        ]);
        assert_eq!(output.status.code(), Some(12), "{output:?}");
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_default();
        assert_eq!(report["verdict"], "invariant-violation");
        assert_eq!(
            report["core"]["metrics"]["candidate_storage"]["memory_bytes_per_worker"],
            0
        );
        assert_eq!(
            report["core"]["metrics"]["candidate_storage"]["retained_bytes"],
            0
        );
        assert_eq!(
            report["core"]["metrics"]["candidate_storage"]["failures"],
            serde_json::json!([])
        );
    }
    let output = dive(&[
        "check",
        "missing.tla",
        "--engine=core",
        "--candidate-directory=unused",
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("requires --candidate-memory-bytes"));
}
