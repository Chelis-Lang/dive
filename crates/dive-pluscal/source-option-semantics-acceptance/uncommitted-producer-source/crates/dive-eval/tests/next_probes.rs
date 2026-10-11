//! The successor generator against the TLC probes of
//! `crates/dive-check/tests/fixtures/tlc-probes/` (milestone CM0's probes).
//!
//! Each probe runs through the test-only binding and single-worker search of
//! `harness::model` under `--compat=tlc`, and its verdict, violated property,
//! states generated, distinct states, states left on the queue, depth and
//! counterexample length must equal those in TLC's transcript. Those counts
//! follow from the exact sequence of successors TLC generates, duplicates
//! included, so they test the enumerator's order and multiplicity.

#![allow(clippy::panic, clippy::expect_used, reason = "test code")]

mod harness;

use std::path::{Path, PathBuf};

use harness::model::{Checked, Verdict, bfs, bind_model, on_big_stack};

const ORACLE: &str = "tlc-2026.10.06.014338";

fn probes() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../dive-check/tests/fixtures/tlc-probes")
}

/// What TLC's `-tool` transcript reports.
#[derive(Debug, PartialEq, Eq)]
struct Tlc {
    verdict: Verdict,
    stats: Option<(u64, u64, u64)>,
    depth: Option<u64>,
    trace_len: Option<u64>,
    /// The `TLC_UNCHANGED_VARIABLE_CHANGED` warnings, as printed.
    warnings: Vec<String>,
}

fn after<'a>(text: &'a str, start: &str) -> Option<&'a str> {
    text.find(start).map(|i| &text[i + start.len()..])
}

fn parse(stdout: &str) -> Tlc {
    let mut verdict = Verdict::Success;
    let lines: Vec<&str> = stdout.lines().collect();
    for (i, l) in lines.iter().enumerate() {
        let Some(rest) = l.strip_prefix("@!@!@STARTMSG ") else {
            continue;
        };
        let Some((code, level)) = rest.trim_end_matches(" @!@!@").split_once(':') else {
            continue;
        };
        if level != "1" {
            continue;
        }
        let code: u32 = code.parse().unwrap_or(0);
        let msg = lines.get(i + 1).copied().unwrap_or("");
        verdict = match code {
            2110 => Verdict::Invariant {
                name: msg
                    .trim_start_matches("Invariant ")
                    .trim_end_matches(" is violated.")
                    .to_owned(),
                initial: false,
            },
            2107 => Verdict::Invariant {
                name: msg
                    .trim_start_matches("Invariant ")
                    .trim_end_matches(" is violated by the initial state:")
                    .to_owned(),
                initial: true,
            },
            2112 => Verdict::ActionProperty {
                name: msg
                    .trim_start_matches("Action property ")
                    .trim_end_matches(" is violated.")
                    .to_owned(),
            },
            2114 => Verdict::Deadlock,
            2109 => Verdict::NotCompletelySpecified { initial: false },
            2106 => Verdict::NotCompletelySpecified { initial: true },
            2121 | 2103 => continue,
            code => {
                let end = format!("@!@!@ENDMSG {code} @!@!@");
                let body: Vec<&str> = lines[i + 1..]
                    .iter()
                    .take_while(|l| **l != end)
                    .copied()
                    .collect();
                Verdict::EvalError {
                    code,
                    message: body.join("\n"),
                }
            }
        };
        break;
    }
    let stats = stdout
        .lines()
        .filter_map(|l| {
            let (g, rest) = l.split_once(" states generated, ")?;
            let (d, rest) = rest.split_once(" distinct states found, ")?;
            let q = rest.strip_suffix(" states left on queue.")?;
            Some((g.parse().ok()?, d.parse().ok()?, q.parse().ok()?))
        })
        .next_back();
    let depth = after(stdout, "The depth of the complete state graph search is ")
        .and_then(|r| r.split('.').next())
        .and_then(|d| d.parse().ok());
    // An initial state that violates an invariant is printed inside the
    // error message, not as a trace.
    let n = match verdict {
        Verdict::Invariant { initial: true, .. } => 1,
        _ => stdout.matches("@!@!@STARTMSG 2217:4 @!@!@").count() as u64,
    };
    Tlc {
        verdict,
        stats,
        depth,
        trace_len: (n > 0).then_some(n),
        warnings: warnings(stdout),
    }
}

/// The `2143` warning blocks of a transcript or of dive's output.
fn warnings(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("@!@!@STARTMSG 2143:3 @!@!@") {
        let end = "@!@!@ENDMSG 2143 @!@!@";
        let Some(j) = rest[i..].find(end) else { break };
        out.push(rest[i..i + j + end.len()].to_owned());
        rest = &rest[i + j + end.len()..];
    }
    out
}

fn run(specs: &Path, name: &str) -> Result<Checked, String> {
    run_with(specs, name, dive_eval::Options::tlc())
}

fn run_with(specs: &Path, name: &str, opts: dive_eval::Options) -> Result<Checked, String> {
    let dir = specs.join(name);
    let loaded = harness::load(&dir.join(format!("{name}.tla")), &[])?;
    let cfg_text =
        std::fs::read_to_string(dir.join(format!("{name}.cfg"))).map_err(|e| e.to_string())?;
    let cfg = dive_config::parse(&cfg_text)
        .into_config()
        .ok_or("cfg rejected")?;
    on_big_stack(move || {
        let b = bind_model(&loaded, &cfg, opts)?;
        Ok(bfs(&b, 1_000_000))
    })?
}

fn compare(tlc: &Tlc, dive: &Checked) -> Result<(), String> {
    let mine = warnings(&dive.output.join("\n"));
    if tlc.warnings != mine {
        return Err(format!("warnings: TLC {:?}, dive {mine:?}", tlc.warnings));
    }
    if tlc.verdict != dive.verdict {
        return Err(format!(
            "verdict: TLC {:?}, dive {:?}",
            tlc.verdict, dive.verdict
        ));
    }
    if let Some((g, d, q)) = tlc.stats {
        let mine = (dive.generated, dive.distinct, dive.queue);
        if (g, d, q) != mine {
            return Err(format!(
                "generated/distinct/queue: TLC {:?}, dive {mine:?}",
                (g, d, q)
            ));
        }
    }
    if let Some(depth) = tlc.depth
        && depth != dive.depth
    {
        return Err(format!("depth: TLC {depth}, dive {}", dive.depth));
    }
    if tlc.trace_len != dive.trace_len {
        return Err(format!(
            "trace length: TLC {:?}, dive {:?}",
            tlc.trace_len, dive.trace_len
        ));
    }
    Ok(())
}

/// Runs every spec of `root/specs` against its transcript in
/// `root/<oracle>/`. `known` are reported differences, `refused` specs dive
/// must refuse with a message containing the given text. Returns the number
/// that agree.
fn run_set(root: &Path, known: &[(&str, &str)], refused: &[(&str, &str)]) -> usize {
    let specs = root.join("specs");
    let mut names: Vec<String> = std::fs::read_dir(&specs)
        .expect("specs")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .collect();
    names.sort();
    let mut failures = Vec::new();
    let mut agree = 0;
    for name in &names {
        let stdout = std::fs::read_to_string(root.join(ORACLE).join(format!("{name}.stdout")))
            .expect("transcript");
        let tlc = parse(&stdout);
        let dive = run(&specs, name);
        if let Some((_, feature)) = refused.iter().find(|(n, _)| n == name) {
            match &dive {
                Ok(Checked {
                    verdict: Verdict::Unsupported(m),
                    ..
                }) if m.contains(feature) => {}
                other => failures.push(format!(
                    "{name}: expected a refusal of {feature}, got {other:?}"
                )),
            }
            continue;
        }
        let outcome = dive.and_then(|d| compare(&tlc, &d));
        match (outcome, known.iter().find(|(n, _)| n == name)) {
            (Ok(()), None) => agree += 1,
            (Ok(()), Some(_)) => failures.push(format!("{name}: agrees but is listed as known")),
            (Err(_), Some(_)) => {}
            (Err(e), None) => failures.push(format!("{name}: {e}")),
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    assert_eq!(agree, names.len() - known.len() - refused.len());
    agree
}

#[test]
fn probes_match_tlc() {
    assert_eq!(run_set(&probes(), &[], &[]), 19);
}

/// The enumerator's own rule fixtures (`tests/fixtures/next/`), each pinning
/// one rule of `Tool.getNextStates`/`getInitStates`/`enabled` against the
/// pinned TLC.
#[test]
fn rule_fixtures_match_tlc() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/next");
    let refused = [("NxInLazyCup", "TLC's unnormalized enumeration of it")];
    assert_eq!(run_set(&root, &[], &refused), 17);
}

/// With warnings off (`-nowarning`, `Options::warnings = false`) TLC skips
/// the all-assigned shortcut for successors, but not for initial states, and
/// prints no warning. The transcripts `*.nowarning.stdout` were captured by
/// running the pinned jar with the harness's flags plus `-nowarning` (see
/// the `.nowarning.command` files).
#[test]
fn no_warnings_matches_tlc() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/next");
    let opts = dive_eval::Options {
        warnings: false,
        ..dive_eval::Options::tlc()
    };
    for name in ["NxFastPath", "NxUnchangedChanged", "NxInitFastPath"] {
        let stdout =
            std::fs::read_to_string(root.join(ORACLE).join(format!("{name}.nowarning.stdout")))
                .expect("transcript");
        let tlc = parse(&stdout);
        let dive = run_with(&root.join("specs"), name, opts).expect("runs");
        if let Err(e) = compare(&tlc, &dive) {
            panic!("{name} with -nowarning: {e}");
        }
    }
}
