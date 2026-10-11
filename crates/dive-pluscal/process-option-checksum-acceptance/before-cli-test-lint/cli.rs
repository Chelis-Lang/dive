//! Smoke tests for the `dive` binary's command surface.

use std::process::Command;

fn dive(args: &[&str]) -> std::process::Output {
    match Command::new(env!("CARGO_BIN_EXE_dive")).args(args).output() {
        Ok(output) => output,
        Err(e) => unreachable!("cannot run dive binary: {e}"),
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
fn unimplemented_commands_refuse_explicitly() {
    for cmd in ["simulate", "lsp"] {
        let out = dive(&[cmd]);
        assert_eq!(out.status.code(), Some(3), "{cmd}");
        assert!(
            String::from_utf8_lossy(&out.stderr).starts_with("unsupported:"),
            "{cmd}"
        );
    }
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
    assert_eq!(v["schema_version"], 1);
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
    assert_eq!(v["schema_version"], 1);
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
    // TLC's exit status under --compat=tlc, dive's own otherwise.
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
    assert_eq!(v["schema_version"], 1);
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
        assert_eq!(own.status.code(), Some(10), "{store}");
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
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stdout).contains("unsupported: temporal"));
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
    // Ordinary checking keeps its explicit refusal until full temporal
    // compatibility is implemented.
    let ordinary = dive(&["check", &path, "--compat=tlc", "--output", "json"]);
    assert_eq!(ordinary.status.code(), Some(3));
    for (profile, code) in [("tlc", 13), ("dive", 10)] {
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
            assert_eq!(a["schema_version"], 2);
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
            assert_eq!(b["options"]["workers"]["used"], 1);
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
    assert_eq!(human.status.code(), Some(10));
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
    assert_eq!(base.status.code(), Some(10));
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
    // A higher-order formula whose scope is still unsupported is refused
    // before it can produce a temporal verdict.
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
    let rejected = dive(&[
        "check",
        &opaque.to_string_lossy(),
        "--lncheck=final",
        "--lasso=short",
        "--output=json",
    ]);
    assert_eq!(rejected.status.code(), Some(3));
    let rejected: serde_json::Value = serde_json::from_slice(&rejected.stdout).unwrap_or_default();
    assert_eq!(rejected["verdict"], "unsupported");
    assert!(rejected["counterexample"].is_null());
    assert!(
        rejected["diagnostics"]
            .as_array()
            .is_some_and(|d| !d.is_empty())
    );
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
    assert_eq!(document["model_proved"], false);
    assert_eq!(document["exhaustive"], false);
    assert_eq!(document["analysis"]["accepted"], true);
    assert!(
        document["module_source"]
            .as_str()
            .is_some_and(|text| text.contains("Next =="))
    );
    assert_eq!(std::fs::read_to_string(&file)?, source);
    let out = dive(&["translate", &path, "--write"]);
    assert_eq!(out.status.code(), Some(3));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("unsupported PlusCal translation option")
    );
    assert_eq!(std::fs::read_to_string(&file)?, source);
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
        assert_eq!(doc["model_proved"], false);
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
    assert_eq!(doc["model_proved"], false);
    assert_eq!(std::fs::read_to_string(file)?, source);
    Ok(())
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
        assert_eq!(doc["exhaustive"], false);
        assert_eq!(doc["model_proved"], false);
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
        assert!(output.stdout.is_empty());
    }
}
