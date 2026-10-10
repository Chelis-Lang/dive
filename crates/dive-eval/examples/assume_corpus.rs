//! Evaluates every `ASSUME` of a corpus with dive and classifies the results.
//!
//! `cargo run --release -p dive-eval --example assume_corpus -- [--lib DIR]... [--profile tlc|dive] ROOT...`
//!
//! Each `ROOT` is a `.tla` file, or a directory searched recursively for
//! `.cfg` files whose module (`X.cfg` next to `X.tla`) is the root. Every
//! assumption of the root module (extended ones included, `AXIOM`s
//! excluded) is evaluated separately under the configuration's constants.
//! TLC accepts these specifications, so each assumption must be `TRUE`, or
//! refused explicitly. Anything else is reported as `wrong`.
//!
//! Prints one line per assumption that is not `TRUE`, then a summary.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::expect_used,
    reason = "a command-line tool"
)]

#[path = "../tests/harness/mod.rs"]
mod harness;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use dive_eval::{AssumptionOutcome, ErrorClass, Evaluator, Options};

fn collect(root: &Path, out: &mut Vec<PathBuf>) {
    if root.is_file() {
        out.push(root.to_path_buf());
        return;
    }
    let Ok(rd) = std::fs::read_dir(root) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|e| e == "cfg") {
            let tla = p.with_extension("tla");
            if tla.exists() {
                out.push(tla);
            }
        }
    }
}

#[allow(clippy::too_many_lines, reason = "a linear command-line driver")]
fn main() {
    let mut libs = Vec::new();
    let mut roots = Vec::new();
    let mut opts = Options::tlc();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--lib" => libs.push(PathBuf::from(args.next().expect("--lib DIR"))),
            "--profile" => {
                opts = match args.next().as_deref() {
                    Some("dive") => Options::dive(),
                    _ => Options::tlc(),
                }
            }
            _ => roots.push(PathBuf::from(a)),
        }
    }
    let mut files = Vec::new();
    for r in &roots {
        collect(r, &mut files);
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut features: HashMap<String, usize> = HashMap::new();
    let mut specs: BTreeMap<&str, usize> = BTreeMap::new();
    for tla in &files {
        let loaded = match harness::load(tla, &libs) {
            Ok(l) => l,
            Err(e) => {
                *specs.entry("not loaded").or_default() += 1;
                println!("{}: not loaded: {e}", tla.display());
                continue;
            }
        };
        let cfg_path = tla.with_extension("cfg");
        let cfg = std::fs::read_to_string(&cfg_path)
            .ok()
            .and_then(|t| dive_config::parse(&t).into_config());
        let model = match harness::bind(&loaded, cfg.as_ref(), opts) {
            Ok(m) => m,
            Err(e) => {
                *specs.entry("not bound").or_default() += 1;
                println!("{}: not bound: {e}", tla.display());
                continue;
            }
        };
        *specs.entry("evaluated").or_default() += 1;
        let ev = Evaluator::new(model, opts).expect("valid evaluator context");
        let ids: Vec<_> = ev
            .assumptions()
            .into_iter()
            .filter(|&d| !harness::is_axiom(&loaded, d))
            .collect();
        let ev2 = ev.clone();
        let results = std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn(move || {
                if let Some(e) = ev2.startup_error() {
                    return vec![(
                        None,
                        AssumptionOutcome::Error {
                            def: dive_ir::DefId(u32::MAX),
                            message: String::new(),
                            error: e,
                        },
                    )];
                }
                ids.iter()
                    .map(|&d| (Some(d), ev2.check_assumption(d)))
                    .collect::<Vec<_>>()
            })
            .expect("thread")
            .join();
        let Ok(results) = results else {
            println!("{}: evaluation panicked", tla.display());
            *counts.entry("panic").or_default() += 1;
            continue;
        };
        let _ = ev.take_output();
        for (d, outcome) in results {
            let loc = d.map_or_else(String::new, |d| ev.location(ev.ir().def(d).body));
            match outcome {
                AssumptionOutcome::AllTrue => *counts.entry("true").or_default() += 1,
                AssumptionOutcome::False { message, .. } => {
                    *counts.entry("wrong").or_default() += 1;
                    println!("WRONG {}: {message}", tla.display());
                }
                AssumptionOutcome::Error { error, message, .. } => match error.class() {
                    ErrorClass::Unsupported => {
                        *counts.entry("unsupported").or_default() += 1;
                        *features.entry(error.message().to_owned()).or_default() += 1;
                        println!("UNSUPPORTED {} {loc}: {}", tla.display(), error.message());
                    }
                    ErrorClass::ResourceLimit => {
                        *counts.entry("resource-limit").or_default() += 1;
                        println!("RESOURCE {} {loc}: {}", tla.display(), error.message());
                    }
                    _ => {
                        *counts.entry("wrong").or_default() += 1;
                        println!(
                            "WRONG {} {loc}: {} {}",
                            tla.display(),
                            error,
                            message.replace('\n', " | ")
                        );
                    }
                },
            }
        }
    }
    eprintln!("specifications: {specs:?}");
    eprintln!("assumptions: {counts:?}");
    let mut f: Vec<_> = features.into_iter().collect();
    f.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (feat, n) in f {
        eprintln!("  unsupported {n:>5}  {feat}");
    }
}
