//! Differential tests against transcripts of the pinned TLC (`tlc-2026.10.06.014338`).
//!
//! Each spec under `tests/fixtures/specs/` prints values with `PrintT` or triggers one
//! error; its TLC transcript, captured with
//! `dive-conformance dev capture --oracle tlc-2026.10.06.014338`, is under
//! `tests/fixtures/tlc/`. These tests rebuild every printed value with `dive-value`
//! under `Ctx::tlc()`, with names interned in SANY's lexing order, and require the
//! printed text, or the error message, to match TLC's byte for byte. No Java is
//! needed.

#![allow(
    clippy::panic,
    clippy::expect_used,
    reason = "test code: a failed expectation is a test failure"
)]

mod common;

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use common::{Evaluator, constants, seed_interner};
use dive_value::json::to_tlc_json;
use dive_value::print::{format_state, print_value};
use dive_value::{Ctx, ErrorKind, Value, ValueError};

const ORACLE: &str = "tlc-2026.10.06.014338";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn module(name: &str) -> String {
    fs::read_to_string(fixtures().join(format!("specs/{name}/{name}.tla")))
        .unwrap_or_else(|e| panic!("{name}.tla: {e}"))
}

fn transcript(name: &str, ext: &str) -> String {
    fs::read_to_string(fixtures().join(format!("tlc/{ORACLE}/{name}.{ext}")))
        .unwrap_or_else(|e| panic!("{name}.{ext}: {e}"))
}

fn spec_names(prefixes: &[&str]) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(fixtures().join("specs"))
        .expect("fixtures/specs")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| prefixes.iter().any(|p| n.starts_with(p)))
        .collect();
    names.sort();
    names
}

/// The `PrintT` output after `Starting...`, split at the `"#case"` markers.
fn printed_cases(stdout: &str) -> Vec<(String, String)> {
    let mut lines = stdout
        .lines()
        .skip_while(|l| *l != "@!@!@ENDMSG 2185 @!@!@");
    lines.next();
    let mut cases: Vec<(String, String)> = Vec::new();
    for line in lines.take_while(|l| !l.starts_with("@!@!@STARTMSG")) {
        if let Some(name) = line.strip_prefix("\"#").and_then(|l| l.strip_suffix('"')) {
            cases.push((name.to_owned(), String::new()));
        } else if let Some((_, text)) = cases.last_mut() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(line);
        }
    }
    cases
}

/// `ASSUME PrintT("#case") /\ PrintT(expr)` lines of a spec.
fn spec_cases(module: &str) -> Vec<(String, String)> {
    module
        .lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("ASSUME PrintT(\"#")?;
            let (name, rest) = rest.split_once("\") /\\ PrintT(")?;
            Some((name.to_owned(), rest.strip_suffix(')')?.to_owned()))
        })
        .collect()
}

/// The message of TLC's first error report, without the assumption header and the
/// Java-override wrapper.
fn tlc_error_message(stdout: &str) -> String {
    let lines: Vec<&str> = stdout.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.starts_with("@!@!@STARTMSG") && l.contains(":1 @!@!@"))
        .expect("an error report");
    let code = lines[start]
        .trim_start_matches("@!@!@STARTMSG ")
        .split(':')
        .next()
        .unwrap_or_default()
        .to_owned();
    let end_marker = format!("@!@!@ENDMSG {code} @!@!@");
    let mut body: Vec<&str> = lines[start + 1..]
        .iter()
        .take_while(|l| **l != end_marker)
        .copied()
        .filter(|l| !l.starts_with("@!@!@"))
        .filter(|l| !l.starts_with("Evaluating assumption"))
        .collect();
    if body
        .first()
        .is_some_and(|l| l.starts_with("Attempted to apply the operator overridden"))
    {
        body.drain(..3);
    }
    while body.last().is_some_and(|l| l.is_empty()) {
        body.pop();
    }
    body.join("\n")
}

fn evaluator<'a>(names: &'a dive_value::Interner, module: &str, ctx: Ctx) -> Evaluator<'a> {
    Evaluator {
        names,
        ctx,
        constants: constants(module),
    }
}

/// Evaluates and prints as `PrintT` does.
fn print_t(ev: &Evaluator<'_>, expr: &str) -> Result<String, ValueError> {
    let v = ev.eval(expr)?;
    print_value(&v, ev.ctx)
}

#[test]
fn printed_values_match_tlc() {
    let mut failures = String::new();
    let mut checked = 0;
    for name in spec_names(&["Print", "ChooseIntern"]) {
        let module = module(&name);
        let expected = printed_cases(&transcript(&name, "stdout"));
        let cases = spec_cases(&module);
        assert_eq!(
            cases.iter().map(|c| &c.0).collect::<Vec<_>>(),
            expected.iter().map(|c| &c.0).collect::<Vec<_>>(),
            "{name}: cases in the spec and the transcript differ"
        );
        let names = seed_interner(&module);
        let ev = evaluator(&names, &module, Ctx::tlc());
        for ((case, expr), (_, want)) in cases.iter().zip(&expected) {
            checked += 1;
            match print_t(&ev, expr) {
                Ok(got) if &got == want => {}
                Ok(got) => {
                    let _ = writeln!(
                        failures,
                        "{name}#{case}: {expr}\n  TLC:  {want}\n  dive: {got}"
                    );
                }
                Err(e) => {
                    let _ = writeln!(
                        failures,
                        "{name}#{case}: {expr}\n  TLC:  {want}\n  dive error: {e}"
                    );
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures}");
    assert!(checked > 100, "only {checked} cases checked");
}

#[test]
fn errors_match_tlc() {
    let mut failures = String::new();
    let mut checked = 0;
    for name in spec_names(&["Err", "Ov", "Ar"]) {
        let module = module(&name);
        let expr = module
            .lines()
            .find_map(|l| l.strip_prefix("ASSUME PrintT(")?.strip_suffix(')'))
            .expect("one ASSUME PrintT");
        let want = tlc_error_message(&transcript(&name, "stdout"));
        let names = seed_interner(&module);
        let ev = evaluator(&names, &module, Ctx::tlc());
        checked += 1;
        match print_t(&ev, expr) {
            Err(e) if e.message() == want => {
                if name.starts_with("OvLit") {
                    assert_eq!(e.kind(), ErrorKind::IntegerTooBig);
                    assert_eq!(e.kind().tlc_code(), 2265);
                }
            }
            Err(e) => {
                let _ = writeln!(
                    failures,
                    "{name}: {expr}\n  TLC:  {want:?}\n  dive: {:?}",
                    e.message()
                );
            }
            Ok(v) => {
                let _ = writeln!(
                    failures,
                    "{name}: {expr}\n  TLC:  {want:?}\n  dive printed {v}"
                );
            }
        }
    }
    assert!(failures.is_empty(), "{failures}");
    assert!(checked > 40, "only {checked} cases checked");
}

/// The counterexample of `TraceValues`: TLC's state printing and `-dumpTrace json`.
#[test]
fn trace_states_match_tlc() {
    let name = "TraceValues";
    let module = module(name);
    let names = seed_interner(&module);
    let ctx = Ctx::tlc();
    let ev = evaluator(&names, &module, ctx);
    let states = [
        [
            "[qfa |-> {1, 2}, qfb |-> <<\"qsz\", mvq1>>]",
            "1..3",
            "\"q<>&='\\t\\\"\\\\\"",
        ],
        [
            "[qfa |-> SUBSET {1, 2, 3}, qfb |-> <<\"qsz\", mvq1>>]",
            "<<1..3, {}, (7 :> mvq2)>>",
            "<<{\"qsz\", \"qsy\"}, -5, TRUE>>",
        ],
    ];
    let stdout = transcript(name, "stdout");
    let blocks: Vec<String> = stdout
        .split("@!@!@STARTMSG 2217:4 @!@!@\n")
        .skip(1)
        .map(|b| {
            let body = b.split("@!@!@ENDMSG 2217 @!@!@").next().unwrap_or_default();
            body.split_once('\n')
                .map(|x| x.1)
                .unwrap_or_default()
                .to_owned()
        })
        .collect();
    assert_eq!(blocks.len(), 2);
    let trace_json = transcript(name, "trace.json");
    for (i, exprs) in states.iter().enumerate() {
        let vals: Vec<Value> = exprs.iter().map(|e| ev.eval(e).expect(e)).collect();
        let vars: Vec<(&str, &Value)> = ["qva", "qvb", "qvc"].into_iter().zip(&vals).collect();
        // TLC frames the state as "%1%: %2%\n%3%" and ends the message with a newline.
        let printed = format_state(&vars, ctx).expect("printable") + "\n";
        assert_eq!(printed, blocks[i], "state {}", i + 1);
        let rec = Value::record(
            ["qva", "qvb", "qvc"]
                .iter()
                .map(|n| names.intern(n))
                .zip(vals.iter().cloned())
                .collect(),
            ctx,
        )
        .expect("record");
        let json = format!("[{},{}]", i + 1, to_tlc_json(&rec, ctx).expect("json"));
        assert!(
            trace_json.contains(&json),
            "state {}: {json} not in {trace_json}",
            i + 1
        );
    }
}

/// The lead's repro: an unused definition changes `CHOOSE` in TLC's order, and does
/// not in dive's canonical order.
#[test]
fn choose_depends_on_interning_only_in_tlc_order() {
    let pick = |name: &str, ctx: Ctx| {
        let m = module(name);
        let names = seed_interner(&m);
        let ev = evaluator(&names, &m, ctx);
        print_t(&ev, "CHOOSE e \\in {\"zeta\", \"alpha\", \"mid\"} : TRUE").expect("choose")
    };
    assert_eq!(pick("ChooseIntern", Ctx::tlc()), "\"zeta\"");
    assert_eq!(pick("ChooseInternUnused", Ctx::tlc()), "\"alpha\"");
    assert_eq!(pick("ChooseIntern", Ctx::dive()), "\"alpha\"");
    assert_eq!(pick("ChooseInternUnused", Ctx::dive()), "\"alpha\"");
}

/// Guards the differential test itself: with names in dive's canonical order some
/// printed values differ from TLC's, so the comparison above does look at order.
#[test]
fn canonical_order_differs_from_tlc_somewhere() {
    let mut differ = 0;
    for name in spec_names(&["Print"]) {
        let module = module(&name);
        let names = seed_interner(&module);
        let tlc = evaluator(&names, &module, Ctx::tlc());
        let canonical = Ctx {
            order: dive_value::ValueOrder::Canonical,
            ..Ctx::tlc()
        };
        let dive = evaluator(&names, &module, canonical);
        for (_, expr) in spec_cases(&module) {
            if print_t(&tlc, &expr).ok() != print_t(&dive, &expr).ok() {
                differ += 1;
            }
        }
    }
    assert!(differ >= 3, "only {differ} cases depend on the order");
}
