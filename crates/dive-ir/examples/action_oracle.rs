//! Compares dive's split of each model's next-state relation with TLC's.
//!
//! `cargo run --release -p dive-ir --example action_oracle -- --corpus DIR --dump FILE [--after DIR]...`
//!
//! `FILE` is `acceptance/dump_actions.sh` output: one line per model, with
//! the model's cfg path relative to `DIR` as its id. Prints one line per
//! model (`match N`, `substituted-spec N`, `MISMATCH ...`) and a summary on
//! stderr. Exits non-zero on any mismatch.

#[path = "../tests/common/action_oracle.rs"]
mod action_oracle;

use action_oracle::{Outcome, compare, parse};
use std::io::Write as _;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut opts = dive_sema::Options::default();
    let mut corpus = None;
    let mut dump = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--corpus" => corpus = it.next().map(PathBuf::from),
            "--dump" => dump = it.next().cloned(),
            "--after" => opts.search.after_standard.extend(it.next().map(Into::into)),
            _ => {}
        }
    }
    let (Some(corpus), Some(dump)) = (corpus, dump) else {
        eprintln!("usage: action_oracle --corpus DIR --dump FILE [--after DIR]...");
        std::process::exit(2);
    };
    let Ok(text) = std::fs::read_to_string(&dump) else {
        eprintln!("cannot read {dump}");
        std::process::exit(2);
    };
    let mut out = std::io::stdout().lock();
    let (mut models, mut matched, mut substituted, mut actions, mut mismatches) = (0, 0, 0, 0, 0);
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let Some(model) = parse(line) else {
            let _ = writeln!(out, "unparsable line");
            mismatches += 1;
            continue;
        };
        models += 1;
        let dir = std::path::Path::new(&model.id)
            .parent()
            .map_or_else(|| corpus.clone(), |p| corpus.join(p));
        let path = dir.join(&model.module);
        let outcome = match dive_sema::analyze_file(&path, &opts) {
            Ok(a) if a.accepted() => match dive_ir::lower(&a) {
                Ok(ir) => compare(&ir, &model),
                Err(e) => Outcome::Mismatch(format!("lowering failed: {e}")),
            },
            Ok(_) => Outcome::Mismatch("dive-sema rejects the module".to_owned()),
            Err(e) => Outcome::Mismatch(format!("cannot read {}: {e}", path.display())),
        };
        match &outcome {
            Outcome::Match(n) => {
                matched += 1;
                actions += n;
                let _ = writeln!(out, "{}: match {n}", model.id);
            }
            Outcome::SubstitutedSpec(n) => {
                substituted += 1;
                actions += n;
                let _ = writeln!(out, "{}: substituted-spec {n}", model.id);
            }
            Outcome::Mismatch(d) => {
                mismatches += 1;
                let _ = writeln!(
                    out,
                    "{}: MISMATCH ({} TLC actions) {d}",
                    model.id,
                    model.actions.len()
                );
            }
        }
    }
    eprintln!(
        "models {models}: {matched} match, {substituted} substituted SPECIFICATION, {mismatches} mismatch; {actions} actions"
    );
    if mismatches > 0 {
        std::process::exit(1);
    }
}
