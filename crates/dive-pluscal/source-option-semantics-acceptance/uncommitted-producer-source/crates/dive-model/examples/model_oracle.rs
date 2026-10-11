//! Compares dive's binding of each model with TLC's.
//!
//! `cargo run --release -p dive-model --example model_oracle -- --tsv FILE --dump FILE [--after DIR]... [--dump-trace] [--compat=tlc|dive] [--verbose]`
//!
//! `--tsv` lists the models (`acceptance/test_models.py`, or
//! `crates/dive-ir/acceptance/models.py`); `--dump` is
//! `acceptance/dump_models.sh` output for the same list. With
//! `--dump-trace`, dive loads the modules TLC's `-dumpTrace json` loads; use
//! it with a dump made with `dumptrace`. Prints one line per model and a
//! summary; exits non-zero if any model is `WRONG`.

#![allow(clippy::expect_used, clippy::print_stdout, reason = "tooling")]

#[path = "../tests/common/oracle.rs"]
mod oracle;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use oracle::{Outcome, Setup, TlcDump, compare, intern_stats};

#[allow(clippy::too_many_lines, reason = "a flat report loop")]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut tsv = None;
    let mut dump = None;
    let mut setup = Setup {
        after: Vec::new(),
        dump_trace: false,
        opts: dive_model::BindOptions::tlc(),
    };
    let mut verbose = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--tsv" => tsv = it.next().cloned(),
            "--dump" => dump = it.next().cloned(),
            "--after" => setup.after.extend(it.next().map(PathBuf::from)),
            "--dump-trace" => setup.dump_trace = true,
            "--compat=dive" => setup.opts = dive_model::BindOptions::dive(),
            "--compat=tlc" => setup.opts = dive_model::BindOptions::tlc(),
            "--verbose" => verbose = true,
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    let (Some(tsv), Some(dump)) = (tsv, dump) else {
        eprintln!("usage: model_oracle --tsv FILE --dump FILE [--after DIR]... [--dump-trace]");
        std::process::exit(2);
    };
    let dumps: HashMap<String, TlcDump> = std::fs::read_to_string(&dump)
        .expect("dump")
        .lines()
        .filter_map(TlcDump::parse)
        .map(|d| (d.id.clone(), d))
        .collect();
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut features: BTreeMap<String, usize> = BTreeMap::new();
    let (mut mvs, mut mvs_exact, mut common, mut ooo, mut intern_models) = (0, 0, 0, 0, 0);
    for line in std::fs::read_to_string(&tsv).expect("tsv").lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let [id, dir, module, cfg] = f[..] else {
            continue;
        };
        let Some(tlc) = dumps.get(id) else {
            println!("{id}: no TLC dump");
            *counts.entry("no dump").or_default() += 1;
            continue;
        };
        if tlc.error_code() == Some(-1) {
            println!("{id}: TLC did not finish (timeout or JVM failure)");
            *counts.entry("TLC did not finish").or_default() += 1;
            continue;
        }
        let dir = PathBuf::from(dir);
        let (outcome, model) = compare(&dir, module, cfg, tlc, &setup);
        let key = match &outcome {
            Outcome::Bound => "bound",
            Outcome::BoundUnsupported(fs) => {
                for f in fs {
                    *features.entry(feature_class(f)).or_default() += 1;
                }
                "bound, refused (unsupported features)"
            }
            Outcome::SameError(_) => "same TLC error",
            Outcome::Unsupported(f) => {
                *features.entry(feature_class(f)).or_default() += 1;
                "unsupported"
            }
            Outcome::FrontEnd => "rejected by both front ends",
            Outcome::Wrong(_) => "WRONG",
        };
        *counts.entry(key).or_default() += 1;
        if let Some(m) = &model
            && let Ok(text) = std::fs::read(dir.join(cfg))
            && let Some(c) = dive_config::parse_bytes(&text).1.config()
        {
            let s = intern_stats(m, c, tlc);
            mvs += s.model_values;
            mvs_exact += s.model_values_exact;
            common += s.common;
            ooo += s.out_of_order.len();
            intern_models += 1;
            if verbose && !s.out_of_order.is_empty() {
                println!("{id}: names out of TLC's order: {:?}", s.out_of_order);
            }
        }
        match &outcome {
            Outcome::Wrong(d) => println!("{id}: WRONG {d}"),
            other if verbose => println!("{id}: {other:?}"),
            _ => {}
        }
    }
    println!("--- summary");
    for (k, n) in &counts {
        println!("{k}: {n}");
    }
    println!("--- unsupported, by feature");
    for (f, n) in &features {
        println!("{f}: {n}");
    }
    println!(
        "--- interning ({intern_models} bound models): model values with TLC's token {mvs_exact}/{mvs}; names out of TLC's relative order {ooo}/{common}"
    );
    if counts.contains_key("WRONG") {
        std::process::exit(1);
    }
}

/// A feature name without model-specific detail, for counting.
fn feature_class(f: &str) -> String {
    match f.split_once(':') {
        Some((head, _)) if head.starts_with("temporal properties") => head.to_owned(),
        _ if f.starts_with("SYMMETRY") => "SYMMETRY".to_owned(),
        _ => f.to_owned(),
    }
}
