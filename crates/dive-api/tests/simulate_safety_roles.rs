//! Simulation phase S1 (ADR 0162): SYMMETRY, VIEW, ALIAS and implied
//! properties for safety, against the pinned TLC receipts in
//! `crates/dive-sim/tests/fixtures/temporal/oracle.json` (cases V3, V4, Y4,
//! A1, U12 to U14). Every reported violation is independently validated.

use dive_api::{SimulateRequest, simulate_file};
use serde_json::{Value, json};
use std::path::PathBuf;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Removes a per-test scratch directory.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Writes `module` from the temporal fixtures with configuration `cfg`.
fn fixture(module: &str, cfg: &str) -> Result<(Scratch, PathBuf)> {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-sim/tests/fixtures/temporal")
        .join(format!("{module}.tla"));
    written(module, &std::fs::read_to_string(source)?, cfg)
}

fn written(module: &str, text: &str, cfg: &str) -> Result<(Scratch, PathBuf)> {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "dive-sim-s1-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory)?;
    let file = directory.join(format!("{module}.tla"));
    std::fs::write(&file, text)?;
    std::fs::write(file.with_extension("cfg"), cfg)?;
    Ok((Scratch(directory), file))
}

fn run(file: &std::path::Path, compat_tlc: bool, traces: u64, depth: usize) -> Result<Value> {
    Ok(simulate_file(&{
        let mut built = SimulateRequest::default();
        built.file = file.to_path_buf();
        built.compat_tlc = compat_tlc;
        built.seed = Some(1);
        built.traces = traces;
        built.depth = Some(depth);
        built
    })?
    .to_json())
}

/// Every retained trace validated, none claims a proof.
fn validated(report: &Value) {
    assert_eq!(report["model_proved"], false, "{report}");
    for trace in report["traces"].as_array().into_iter().flatten() {
        assert_eq!(trace["validation"]["result"], "valid", "{report}");
    }
}

fn last_values(report: &Value) -> Value {
    let trace = report["observed_violation"]["trace"].as_u64().unwrap_or(0);
    let index = usize::try_from(trace).unwrap_or(0);
    report["traces"][index]["document"]["states"]
        .as_array()
        .and_then(|states| states.last())
        .map(|state| state["values"].clone())
        .unwrap_or_default()
}

/// V3, V4 and Y4: VIEW and SYMMETRY do not affect simulation safety. The
/// verdicts are TLC's, the reports say why, and violations validate.
#[test]
fn view_and_symmetry_leave_safety_unchanged() -> Result {
    let cases = [
        (
            "ViewLoop",
            "SPECIFICATION FairSpec\nINVARIANT BadY\nVIEW View\n",
            Some(("BadY", json!({"x": 0, "y": 2}))),
            "VIEW View does not affect simulation, which never deduplicates states.",
        ),
        (
            "ViewLoop",
            "SPECIFICATION FairSpec\nINVARIANT SafeY\nVIEW View\n",
            None,
            "VIEW View does not affect simulation, which never deduplicates states.",
        ),
        (
            "SymLoop",
            "CONSTANTS a = a b = b\nSPECIFICATION FairSpec\nINVARIANT NotB\nSYMMETRY Perms\n",
            Some(("NotB", json!({"x": {"model": "b"}}))),
            "SYMMETRY Perms does not affect simulation, which never deduplicates states.",
        ),
    ];
    for (module, cfg, violation, note) in cases {
        let (_scratch, file) = fixture(module, cfg)?;
        for compat in [false, true] {
            let report = run(&file, compat, 20, 10)?;
            validated(&report);
            assert!(
                report["notes"]
                    .as_array()
                    .is_some_and(|notes| notes.contains(&json!(note))),
                "{report}"
            );
            if let Some((name, last)) = &violation {
                assert_eq!(report["result"], "violation", "{cfg}: {report}");
                assert_eq!(report["observed_violation"]["name"], *name);
                assert_eq!(report["tlc_code"], 2110);
                assert_eq!(report["violation_validated"], true, "{report}");
                assert_eq!(&last_values(&report), last, "{report}");
                if compat {
                    assert_eq!(report["exit_code"], 12, "{report}");
                }
            } else {
                assert_eq!(report["result"], "sampled", "{cfg}: {report}");
                assert_eq!(report["statistics"]["traces_started"], 20);
                assert_eq!(report["exit_code"], 0);
            }
        }
    }
    Ok(())
}

/// A1: each trace entry renders ALIAS on its validated states, as TLC prints
/// the violating trace; the states themselves are unchanged.
#[test]
fn alias_renders_beside_the_validated_states() -> Result {
    let (_scratch, file) = fixture(
        "Unbounded",
        "SPECIFICATION FairSpec\nINVARIANT BelowFour\nALIAS AliasDef\n",
    )?;
    for compat in [false, true] {
        let report = run(&file, compat, 20, 10)?;
        validated(&report);
        assert_eq!(report["result"], "violation", "{report}");
        assert_eq!(report["violation_validated"], true);
        let trace = &report["traces"][0];
        let expected: Vec<Value> = (0..=4)
            .map(|x| json!({"record": {"x": x, "double": 2 * x}}))
            .collect();
        assert_eq!(trace["alias"], json!(expected), "{report}");
        assert_eq!(last_values(&report), json!({"x": 4}));
        assert_eq!(report["notes"], json!([]), "{report}");
    }
    Ok(())
}

/// ALIAS sees `TLCGet("level")` as the state's 1-based position, and a
/// rendering error leaves the verdict alone and is named in the notes.
#[test]
fn alias_level_and_rendering_errors() -> Result {
    let text = "---- MODULE AliasLevel ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == x = 0\nNext == x' = x + 1\nInv == x < 2\nLevel == [x |-> x, level |-> TLCGet(\"level\")]\nBroken == [x |-> 1 \\div (1 - x)]\n====\n";
    let (_level, level) = written(
        "AliasLevel",
        text,
        "INIT Init\nNEXT Next\nINVARIANT Inv\nALIAS Level\n",
    )?;
    let (_broken, broken) = written(
        "AliasLevel",
        text,
        "INIT Init\nNEXT Next\nINVARIANT Inv\nALIAS Broken\n",
    )?;
    for compat in [false, true] {
        let report = run(&level, compat, 1, 5)?;
        validated(&report);
        assert_eq!(report["result"], "violation", "{report}");
        assert_eq!(
            report["traces"][0]["alias"],
            json!([
                {"record": {"x": 0, "level": 1}},
                {"record": {"x": 1, "level": 2}},
                {"record": {"x": 2, "level": 3}}
            ]),
            "{report}"
        );
        let report = run(&broken, compat, 1, 5)?;
        validated(&report);
        assert_eq!(report["result"], "violation", "{report}");
        assert_eq!(report["violation_validated"], true);
        assert!(report["traces"][0]["alias"].is_null(), "{report}");
        let notes = report["notes"].to_string();
        assert!(
            notes.contains("ALIAS Broken not rendered for trace 0"),
            "{report}"
        );
    }
    Ok(())
}

/// U12 (D-036): implied initial properties are checked on initial states and
/// reported as `dive check` reports them (2108), in both profiles; TLC's
/// simulator never checks them.
#[test]
fn implied_initial_properties_are_checked() -> Result {
    let (_scratch, file) = fixture("Unbounded", "SPECIFICATION FairSpec\nPROPERTY InitOne\n")?;
    let (_held, held) = fixture("Unbounded", "SPECIFICATION FairSpec\nPROPERTY InitZero\n")?;
    for compat in [false, true] {
        let report = run(&file, compat, 20, 5)?;
        validated(&report);
        assert_eq!(report["result"], "violation", "{report}");
        assert_eq!(report["tlc_code"], 2108);
        assert_eq!(report["observed_violation"]["kind"], "property");
        assert_eq!(report["observed_violation"]["name"], "InitOne");
        assert_eq!(report["violation_validated"], true, "{report}");
        assert_eq!(report["statistics"]["traces_started"], 0);
        let states = report["traces"][0]["document"]["states"]
            .as_array()
            .map_or(0, Vec::len);
        assert_eq!(states, 1, "{report}");
        assert_eq!(report["exit_code"], if compat { 13 } else { 10 });
        let report = run(&held, compat, 20, 5)?;
        validated(&report);
        assert_eq!(report["result"], "sampled", "{report}");
        assert_eq!(report["statistics"]["generated"], 101);
    }
    Ok(())
}

/// U13, U14 and `SimConstrainedAction`: an action property or invariant is
/// checked on a candidate before the constraint excludes it, as TLC does; the
/// validator accepts the excluded last state.
#[test]
fn properties_on_constrained_candidates_validate() -> Result {
    let unbounded = [
        (
            "SPECIFICATION FairSpec\nCONSTRAINT BelowThree\nPROPERTY StepBelowThree\n",
            "StepBelowThree",
            2112,
            13,
            3,
        ),
        (
            "SPECIFICATION FairSpec\nCONSTRAINT BelowFour\nPROPERTY AlwaysBelowFour\n",
            "AlwaysBelowFour",
            2110,
            12,
            4,
        ),
    ];
    for (cfg, name, code, exit, last) in unbounded {
        let (_scratch, file) = fixture("Unbounded", cfg)?;
        for compat in [false, true] {
            let report = run(&file, compat, 20, 10)?;
            validated(&report);
            assert_eq!(report["result"], "violation", "{cfg}: {report}");
            assert_eq!(report["observed_violation"]["name"], name);
            assert_eq!(report["tlc_code"], code);
            assert_eq!(report["violation_validated"], true, "{report}");
            assert_eq!(last_values(&report), json!({"x": last}), "{report}");
            if compat {
                assert_eq!(report["exit_code"], exit);
            }
        }
    }
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-sim/tests/fixtures/SimConstrainedAction.tla");
    for compat in [false, true] {
        let report = run(&file, compat, 3, 5)?;
        validated(&report);
        assert_eq!(report["result"], "violation", "{report}");
        assert_eq!(report["observed_violation"]["name"], "Prop");
        assert_eq!(report["tlc_code"], 2112);
        assert_eq!(report["violation_validated"], true, "{report}");
        assert_eq!(last_values(&report), json!({"x": 1}));
    }
    Ok(())
}

/// Each sampled step carries the chosen action's name and location, which
/// the validator checks on the step.
#[test]
fn sampled_steps_carry_validated_action_labels() -> Result {
    let text = "---- MODULE Labels ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nUp == x < 3 /\\ x' = x + 1\nReset == x = 3 /\\ x' = 0\nNext == Up \\/ Reset\n====\n";
    let (_scratch, file) = written("Labels", text, "INIT Init\nNEXT Next\n")?;
    for compat in [false, true] {
        let report = run(&file, compat, 2, 6)?;
        validated(&report);
        assert_eq!(report["result"], "sampled", "{report}");
        let states = report["traces"][0]["document"]["states"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert_eq!(states.len(), 7);
        assert!(states[0]["action"].is_null());
        let names: Vec<_> = states[1..]
            .iter()
            .map(|state| state["action"]["name"].clone())
            .collect();
        assert_eq!(
            names,
            ["Up", "Up", "Up", "Reset", "Up", "Up"].map(|name| json!(name)),
            "{report}"
        );
        assert!(
            states[1]["action"]["location"]
                .as_str()
                .is_some_and(|location| location.starts_with("<Up line 5")),
            "{report}"
        );
    }
    Ok(())
}

/// Randomness at startup under VIEW or SYMMETRY needs nothing extra in
/// simulation, which takes no fingerprint or canonical form; the sampled
/// traces still validate on a fresh binding of the same run.
#[test]
fn startup_randomness_under_view_samples_and_validates() -> Result {
    let text = "---- MODULE RandomView ----\nEXTENDS Naturals, TLC\nVARIABLE x\nStart == RandomElement(1..3)\nInit == x = Start\nNext == x' = (x + 1) % 5\nInv == x \\in 0..4\nView == x % 2\n====\n";
    let (_scratch, file) = written(
        "RandomView",
        text,
        "INIT Init\nNEXT Next\nINVARIANT Inv\nVIEW View\n",
    )?;
    for compat in [false, true] {
        let report = run(&file, compat, 3, 4)?;
        validated(&report);
        assert_eq!(report["result"], "sampled", "{report}");
        assert_eq!(report["traces"].as_array().map_or(0, Vec::len), 3);
    }
    Ok(())
}

/// The SYMMETRY declaration is prepared at startup as `dive check` prepares
/// it: its warnings are the checker's, though no reduction is used.
#[test]
fn symmetry_startup_warnings_match_checking() -> Result {
    let text = "---- MODULE SymWarn ----\nEXTENDS TLC\nCONSTANTS a, b\nVARIABLE x\nInit == x = a\nNext == x' = IF x = a THEN b ELSE a\nInv == x \\in {a, b}\nPerms == Permutations({a})\n====\n";
    let (_scratch, file) = written(
        "SymWarn",
        text,
        "CONSTANTS a = a b = b\nINIT Init\nNEXT Next\nINVARIANT Inv\nSYMMETRY Perms\n",
    )?;
    for compat in [false, true] {
        let checked = dive_api::check_file(&dive_api::CheckRequest {
            file: file.clone(),
            compat_tlc: compat,
            ..Default::default()
        })?
        .to_json();
        let expected: Vec<Value> = checked["warnings"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|w| json!({"kind": "warning", "code": w["code"], "text": w["message"]}))
            .collect();
        assert!(!expected.is_empty(), "{checked}");
        let report = run(&file, compat, 2, 3)?;
        validated(&report);
        assert_eq!(report["result"], "sampled", "{report}");
        assert_eq!(report["startup_output"], json!(expected), "{report}");
    }
    Ok(())
}

/// The S1 cases of the pinned TLC receipts, compared field by field under
/// `--compat=tlc`: exit status, deciding code, counterexample length and,
/// for a clean sample, the generated count. U12 is compared with its
/// model-checking control B-U12, because TLC's simulator skips the check
/// (D-036). The specifications are deterministic, so the traces are TLC's.
#[test]
fn s1_cases_match_the_pinned_tlc_receipts() -> Result {
    let oracle: Value = serde_json::from_str(include_str!(
        "../../dive-sim/tests/fixtures/temporal/oracle.json"
    ))?;
    let find = |group: &str, id: &str| {
        oracle[group]
            .as_array()
            .and_then(|cases| cases.iter().find(|case| case["id"] == id))
            .cloned()
            .ok_or_else(|| format!("no oracle case {id}"))
    };
    for id in [
        "V3", "V4", "Y4", "A1", "U11", "U12", "U13", "U14", "C5", "C6", "U4", "U6", "U7",
    ] {
        let case = find("cases", id)?;
        let flags: Vec<&str> = case["flags"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let after = |flag: &str| {
            flags
                .iter()
                .position(|f| *f == flag)
                .and_then(|i| flags.get(i + 1))
                .copied()
        };
        let depth: usize = after("-depth").ok_or("depth")?.parse()?;
        let traces: u64 = after("-simulate")
            .and_then(|num| num.strip_prefix("num="))
            .ok_or("num")?
            .parse()?;
        let module = case["module"].as_str().ok_or("module")?;
        let (_scratch, file) = fixture(module, case["cfg"].as_str().ok_or("cfg")?)?;
        let report = simulate_file(&{
            let mut built = SimulateRequest::default();
            built.file = file;
            built.compat_tlc = true;
            built.seed = Some(1);
            built.traces = traces;
            built.depth = Some(depth);
            built.no_deadlock = flags.contains(&"-deadlock");
            built
        })?
        .to_json();
        validated(&report);
        let expected = if id == "U12" {
            find("bfs_controls", "B-U12")?
        } else {
            case.clone()
        };
        assert_eq!(report["exit_code"], expected["exit"], "{id}: {report}");
        assert_eq!(
            report["tlc_code"], expected["deciding"][0]["code"],
            "{id}: {report}"
        );
        if report["result"] == "violation" {
            assert_eq!(report["violation_validated"], true, "{id}: {report}");
            let shown = expected["counterexample"].as_array().map_or(0, Vec::len);
            let trace = &report["traces"][report["observed_violation"]["trace"]
                .as_u64()
                .and_then(|t| usize::try_from(t).ok())
                .unwrap_or(0)];
            // TLC prints no state for an initial-state property (B-U12).
            let states = trace["tlc_states"].as_array().map_or(0, Vec::len);
            assert_eq!(states, shown.max(1), "{id}: {report}");
        } else {
            assert_eq!(report["result"], "sampled", "{id}: {report}");
            assert_eq!(
                report["statistics"]["generated"], case["generated"],
                "{id}: {report}"
            );
        }
    }
    Ok(())
}
