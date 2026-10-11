//! What TLC prints at start-up, before it computes the initial states,
//! against transcripts of the pinned TLC (`tlc-2026.10.06.014338`).
//!
//! Each specification under `tests/fixtures/specs/` was run with
//! `dive-conformance dev capture`:
//! - `tests/fixtures/tlc/`: as users run TLC, without `-dumpTrace`, which is
//!   what `--compat=tlc` reproduces;
//! - `tests/fixtures/tlc-dumptrace/` (the `Er`, `Su`, `As` and `Ov`
//!   specifications): with `-dumpTrace json`, as the harness ran TLC before
//!   issue #35, which loads `_JsonTrace` and with it `Integers`
//!   ([`dive_model::DUMP_TRACE_JSON_MODULES`]). Each `.command` file records
//!   the command line.
//!
//! The test binds each model under `--compat=tlc` (with the extra modules for
//! the second set), checks its assumptions, and requires the start-up part of
//! TLC's transcript byte for byte: `Print` output of constant definitions,
//! warnings, the configuration error or the assumption failure. No Java is
//! needed.

#![allow(clippy::panic, clippy::expect_used, reason = "test code")]

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::oracle::{Setup, analyze, bind};
use dive_eval::AssumptionOutcome;
use dive_model::{ErrorKind, Message};

const ORACLE: &str = "tlc-2026.10.06.014338";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// The start-up part of a `-tool` transcript: everything after `Starting...`
/// up to the initial states (or the end).
fn tlc_part(stdout: &str) -> String {
    let lines: Vec<&str> = stdout.lines().collect();
    let Some(start) = lines.iter().position(|l| *l == "@!@!@ENDMSG 2185 @!@!@") else {
        return String::new();
    };
    let stops = ["2189", "2186", "2199", "2200", "2190", "2193"];
    let mut out = Vec::new();
    for l in &lines[start + 1..] {
        if let Some(rest) = l.strip_prefix("@!@!@STARTMSG ")
            && stops.iter().any(|s| rest.starts_with(&format!("{s}:")))
        {
            break;
        }
        out.push(*l);
    }
    out.join("\n")
}

fn message(m: &Message) -> String {
    match m {
        Message::Print(s) => s.clone(),
        Message::Warning { code, message } => {
            let c = code.code();
            format!("@!@!@STARTMSG {c}:3 @!@!@\n{message}\n@!@!@ENDMSG {c} @!@!@")
        }
    }
}

/// dive's rendering of the same part, or `Err` with a refusal.
fn dive_part(dir: &Path, name: &str, dump_trace: bool) -> Result<String, String> {
    let setup = Setup {
        after: Vec::new(),
        dump_trace,
        opts: dive_model::BindOptions::tlc(),
    };
    let ir = analyze(&dir.join(format!("{name}.tla")), &setup)?;
    let text = fs::read(dir.join(format!("{name}.cfg"))).unwrap_or_default();
    let (_, parsed) = dive_config::parse_bytes(&text);
    let cfg = parsed.config().cloned().ok_or("cfg rejected")?;
    let mut out: Vec<String> = Vec::new();
    match bind(ir, cfg, setup.opts) {
        Err(f) => {
            out.extend(f.output.iter().map(message));
            match &f.error.kind {
                ErrorKind::Unsupported { feature } => return Err(feature.clone()),
                ErrorKind::Bug => return Err(format!("bug: {}", f.error.message)),
                ErrorKind::Eval { code: 1000 } => out.push(format!(
                    "@!@!@STARTMSG 1000:1 @!@!@\nTLC threw an unexpected exception.\nThis was probably caused by an error in the spec or model.\nSee the User Output or TLC Console for clues to what happened.\nThe exception was a java.lang.RuntimeException\n: {}\n@!@!@ENDMSG 1000 @!@!@",
                    f.error.message
                )),
                _ => out.push(f.error.tool_text()),
            }
        }
        Ok(model) => {
            out.extend(model.output.iter().map(message));
            if let Some(u) = model.unsupported.first() {
                return Err(u.feature.clone());
            }
            let m2 = model.clone();
            let outcome = std::thread::Builder::new()
                .stack_size(dive_eval::RECOMMENDED_STACK)
                .spawn(move || m2.check_assumptions())
                .expect("spawn")
                .join()
                .map_err(|_| "panic".to_owned())?;
            out.extend(model.take_output());
            match outcome {
                AssumptionOutcome::AllTrue => {}
                AssumptionOutcome::False { message, .. } => out.push(format!(
                    "@!@!@STARTMSG 2104:1 @!@!@\n{message}\n@!@!@ENDMSG 2104 @!@!@"
                )),
                AssumptionOutcome::Error { message, error, .. } => {
                    if error.class() != dive_eval::ErrorClass::Eval {
                        return Err(error.to_string());
                    }
                    out.push(format!(
                        "@!@!@STARTMSG 2105:1 @!@!@\n{message}\n@!@!@ENDMSG 2105 @!@!@"
                    ));
                }
            }
        }
    }
    Ok(out.join("\n"))
}

/// Specifications TLC rejects in its front end, which `dive-sema` covers.
const FRONT_END: &[&str] = &[];

/// Models dive refuses, with the feature named in the refusal. TLC sets up
/// its liveness checker at start-up, which prints its own messages.
const REFUSED: &[(&str, &str)] = &[("RoProps", "temporal properties")];

fn run_set(set: &str, dump_trace: bool) {
    let specs = fixtures().join("specs");
    let mut names: Vec<String> = fs::read_dir(&specs)
        .expect("specs")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .collect();
    names.sort();
    let mut failures = Vec::new();
    let mut agree = 0;
    for name in &names {
        if FRONT_END.contains(&name.as_str()) {
            continue;
        }
        let path = fixtures().join(format!("{set}/{ORACLE}/{name}.stdout"));
        let Ok(stdout) = fs::read_to_string(&path) else {
            // The `-dumpTrace` set is frozen: `dev capture` no longer passes
            // the flag (issue #35), so later fixtures exist only in the
            // plain set.
            if !dump_trace {
                failures.push(format!("{name}: no transcript at {}", path.display()));
            }
            continue;
        };
        let expected = tlc_part(&stdout);
        match dive_part(&specs.join(name), name, dump_trace) {
            Ok(got) if got == expected => agree += 1,
            Ok(got) => failures.push(format!("{name}:\n--- TLC\n{expected}\n--- dive\n{got}\n")),
            Err(e) if REFUSED.iter().any(|(n, f)| n == name && e.starts_with(f)) => {}
            Err(e) => failures.push(format!("{name}: dive refuses: {e}")),
        }
    }
    eprintln!(
        "{set}: {} specs, {agree} agree, {} disagree",
        names.len(),
        failures.len()
    );
    for f in &failures {
        eprintln!("{f}");
    }
    assert!(
        failures.is_empty(),
        "{} start-up transcripts disagree",
        failures.len()
    );
}

#[test]
fn startup_matches_tlc() {
    run_set("tlc", false);
}

#[test]
fn startup_matches_tlc_with_dump_trace() {
    run_set("tlc-dumptrace", true);
}
