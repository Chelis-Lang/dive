//! Every role of the repository's models against TLC's own binding.
//!
//! `tests/fixtures/models/<oracle>/repo.jsonl` is `acceptance/dump_models.sh`
//! output for the models `acceptance/repo_models.py` lists: this crate's
//! fixtures, the action-split fixtures, the TLC probes and the harness
//! fixtures. `ModelDump.java` built TLC's `FastTool` for each and recorded
//! what TLC made of it: the error, or the initial predicates, actions (with
//! their labels), invariants, implied initial predicates, implied actions,
//! temporal formulas and properties, constraints, view, alias, post
//! conditions, deadlock flag and the whole intern table. `repo-dumptrace.jsonl`
//! is the same with TLC set up as `-dumpTrace json` sets it up.
//!
//! The test binds each model under `--compat=tlc` and requires every role to
//! agree, the same error code where TLC fails, the model values' intern
//! tokens to be TLC's, and no name TLC's table orders differently. No Java is
//! needed.

#![allow(clippy::panic, clippy::expect_used, reason = "test code")]

mod common;

use std::path::{Path, PathBuf};

use common::oracle::{Outcome, Setup, TlcDump, compare, intern_stats};

const ORACLE: &str = "tlc-2026.10.06.014338";

/// Models whose TLC binding depends on something other than the
/// specification, with the reason.
const KNOWN: &[(&str, &str)] = &[(
    "crates/dive-conformance/tests/fixtures/specs/Oom/Oom.cfg",
    "TLC runs out of memory evaluating Inv at start-up and swallows the error \
     (SpecProcessor.processConstantDefns catches Throwable), so Inv stays an \
     invariant; dive evaluates it to TRUE",
)];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(file: &str, dump_trace: bool) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/models/{ORACLE}/{file}"));
    let text = std::fs::read_to_string(&path).expect("dump");
    let setup = Setup {
        after: Vec::new(),
        dump_trace,
        opts: dive_model::BindOptions::tlc(),
    };
    let (mut bound, mut errors, mut refused, mut skipped) = (0, 0, 0, 0);
    let (mut mvs, mut mvs_exact, mut common, mut out_of_order) = (0, 0, 0, Vec::new());
    let mut wrong = Vec::new();
    for line in text.lines() {
        let Some(tlc) = TlcDump::parse(line) else {
            continue;
        };
        if KNOWN.iter().any(|(id, _)| *id == tlc.id) {
            skipped += 1;
            continue;
        }
        let cfg = repo().join(&tlc.id);
        let dir = cfg.parent().expect("dir").to_path_buf();
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .expect("name")
            .to_owned();
        let cfg_name = cfg
            .file_name()
            .and_then(|n| n.to_str())
            .expect("cfg")
            .to_owned();
        let (outcome, model) = compare(&dir, &format!("{name}.tla"), &cfg_name, &tlc, &setup);
        match outcome {
            Outcome::Bound => bound += 1,
            Outcome::BoundUnsupported(_) | Outcome::Unsupported(_) => refused += 1,
            Outcome::SameError(_) | Outcome::FrontEnd => errors += 1,
            Outcome::Wrong(d) => wrong.push(format!("{}: {d}", tlc.id)),
        }
        if let Some(m) = model {
            let text = std::fs::read(&cfg).unwrap_or_default();
            let (_, parsed) = dive_config::parse_bytes(&text);
            if let Some(c) = parsed.config() {
                let s = intern_stats(&m, c, &tlc);
                mvs += s.model_values;
                mvs_exact += s.model_values_exact;
                common += s.common;
                for n in s.out_of_order {
                    out_of_order.push(format!("{}: {n}", tlc.id));
                }
            }
        }
    }
    eprintln!(
        "{file}: {bound} bound, {errors} same error, {refused} refused, {skipped} known, {} wrong; \
         model values with TLC's token {mvs_exact}/{mvs}, names out of order {}/{common}",
        wrong.len(),
        out_of_order.len()
    );
    for w in &wrong {
        eprintln!("{w}");
    }
    assert!(
        wrong.is_empty(),
        "{} models bind differently from TLC",
        wrong.len()
    );
    assert_eq!(mvs, mvs_exact, "model values with tokens other than TLC's");
    assert!(
        out_of_order.is_empty(),
        "names out of TLC's order: {out_of_order:?}"
    );
}

#[test]
fn roles_match_tlc() {
    run("repo.jsonl", false);
}

#[test]
fn roles_match_tlc_with_dump_trace() {
    run("repo-dumptrace.jsonl", true);
}
