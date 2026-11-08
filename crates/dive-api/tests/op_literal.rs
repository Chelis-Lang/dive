//! An operator argument whose body is a numeral or a string literal
//! (issue #557). TLC evaluates such a definition once at start-up and
//! passes its value where an operator is expected, so a Java method that
//! takes an operator fails: `BagOfAll` with 2266, `SelectSeq` and `SortSeq`
//! with 2169,
//! and the CommunityModules overrides with 2154 ("Cannot cast
//! tlc2.value.impl.IntValue to tlc2.value.impl.OpValue"). A body that is an
//! operator application (`TRUE`, `-1`, `1 + 1`, a constant) is passed as an
//! operator, and a TLA+ operator that applies the argument gets the value.
//!
//! For each fixture of `tests/fixtures/op-literal`, the reference engine
//! gives the pinned TLC's exit status (under `--compat=tlc`) and the text of its first error, in
//! both profiles. The new core gives the same, or refuses the call.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]

use std::path::{Path, PathBuf};

use dive_api::{CheckRequest, Engine, check_file_engine};
use serde_json::Value;

const ORACLE: &str = "tlc-2026.10.06.014338";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/op-literal")
}

fn run(spec: &Path, engine: Engine, compat_tlc: bool) -> Value {
    let community = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-eval/tests/fixtures/community-modules");
    let report = check_file_engine(
        &CheckRequest {
            config: Some(spec.with_extension("cfg")),
            file: spec.to_path_buf(),
            library_paths: vec![community],
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            compat_tlc,
            ..CheckRequest::default()
        },
        engine,
    )
    .expect("check runs");
    let mut j = report.to_json();
    j["exit"] = Value::from(report.exit_code());
    j
}

/// The body of TLC's first error message (class 1), nested messages kept.
fn first_error(stdout: &str) -> Option<String> {
    let mut body: Option<String> = None;
    let mut depth = 0usize;
    for line in stdout.lines() {
        if let Some(b) = body.as_mut() {
            if line.starts_with("@!@!@STARTMSG ") {
                depth += 1;
            } else if line.starts_with("@!@!@ENDMSG ") {
                if depth == 0 {
                    return Some(b.trim_end().to_owned());
                }
                depth -= 1;
            }
            b.push_str(line);
            b.push('\n');
        } else if let Some(head) = line.strip_prefix("@!@!@STARTMSG ")
            && head.trim_end_matches(" @!@!@").ends_with(":1")
        {
            body = Some(String::new());
        }
    }
    None
}

#[test]
fn literal_bodies_reach_java_methods_as_values() {
    let mut names: Vec<String> = std::fs::read_dir(root().join("specs"))
        .expect("fixtures")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert!(names.len() >= 15, "{names:?}");
    for name in names {
        let spec = root().join("specs").join(&name).join(format!("{name}.tla"));
        let tlc = root().join(ORACLE);
        let exit: i64 = std::fs::read_to_string(tlc.join(format!("{name}.exit")))
            .expect("exit")
            .trim()
            .parse()
            .expect("exit code");
        let stdout = std::fs::read_to_string(tlc.join(format!("{name}.stdout"))).expect("stdout");
        // The state an error in an initial state names is reported apart
        // from the message (`counterexample`).
        let error = first_error(&stdout).map(|t| {
            match t.split_once("\nWhile working on the initial state:") {
                Some((m, _)) => m.to_owned(),
                None => t,
            }
        });
        let mut compat_verdict = Value::Null;
        for compat in [true, false] {
            let core = run(&spec, Engine::Core, compat);
            let at = format!("{name} compat={compat}");
            if core["verdict"] == "unsupported" {
                // Only the retired reference engine applied these (#791,
                // #910); Core refuses by name.
                assert!(
                    core["message"]
                        .as_str()
                        .is_some_and(|m| m.contains("body is constant")),
                    "{at}: {core:#}"
                );
                assert!(core["counterexample"].is_null(), "{at}: {core:#}");
                continue;
            }
            if compat {
                assert_eq!(core["exit"].as_i64(), Some(exit), "{at}: {core:#}");
                compat_verdict = core["verdict"].clone();
            } else if !compat_verdict.is_null() {
                // The default profile has its own exit codes.
                assert_eq!(compat_verdict, core["verdict"], "{at}: {core:#}");
            }
            match &error {
                Some(text) => assert_eq!(Some(text.as_str()), core["message"].as_str(), "{at}"),
                None => assert_eq!(core["verdict"], "success", "{at}: {core:#}"),
            }
        }
    }
}
