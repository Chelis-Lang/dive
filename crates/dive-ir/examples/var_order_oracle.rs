//! Compares dive's state-variable order with TLC's on a corpus.
//!
//! `cargo run --release -p dive-ir --example var_order_oracle -- --corpus DIR --dump FILE [--after DIR]...`
//!
//! `FILE` is `acceptance/VarOrderDump.java` output (see
//! `acceptance/README.md`): one line per module, with the module's path
//! relative to `DIR` as its id. Prints one line per module SANY accepts
//! (`match N` or `MISMATCH ...`) and a summary on stderr. Exits non-zero on any
//! mismatch.

#[path = "../tests/common/var_order.rs"]
mod var_order;

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
        eprintln!("usage: var_order_oracle --corpus DIR --dump FILE [--after DIR]...");
        std::process::exit(2);
    };
    let Ok(text) = std::fs::read_to_string(&dump) else {
        eprintln!("cannot read {dump}");
        std::process::exit(2);
    };
    let mut out = std::io::stdout().lock();
    let (mut modules, mut matched, mut vars, mut multi, mut mismatches) = (0, 0, 0, 0, 0);
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let Some(m) = var_order::parse(line) else {
            let _ = writeln!(out, "unparsable line");
            mismatches += 1;
            continue;
        };
        let Some(want) = m.vars else {
            continue;
        };
        modules += 1;
        match var_order::compare(&corpus.join(&m.id), &want, &opts) {
            Ok(()) => {
                matched += 1;
                vars += want.len();
                if want.len() > 1 {
                    multi += 1;
                }
                let _ = writeln!(out, "{}: match {}", m.id, want.len());
            }
            Err(e) => {
                mismatches += 1;
                let _ = writeln!(out, "{}: MISMATCH {e}", m.id);
            }
        }
    }
    eprintln!(
        "{modules} modules: {matched} match ({multi} with two or more variables, {vars} variables), {mismatches} mismatches"
    );
    if mismatches > 0 {
        std::process::exit(1);
    }
}
