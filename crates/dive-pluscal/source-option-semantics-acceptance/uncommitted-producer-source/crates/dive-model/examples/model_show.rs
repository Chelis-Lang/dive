//! Binds one model and prints what dive makes of it.
//!
//! `cargo run -p dive-model --example model_show -- Module.tla Model.cfg [--after DIR]... [--dump-trace] [--compat=dive]`

#![allow(clippy::print_stdout, clippy::expect_used, reason = "tooling")]

#[path = "../tests/common/oracle.rs"]
mod oracle;

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut setup = oracle::Setup {
        after: Vec::new(),
        dump_trace: false,
        opts: dive_model::BindOptions::tlc(),
    };
    let mut files = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--after" => setup.after.extend(it.next().map(PathBuf::from)),
            "--dump-trace" => setup.dump_trace = true,
            "--compat=dive" => setup.opts = dive_model::BindOptions::dive(),
            other => files.push(PathBuf::from(other)),
        }
    }
    let [tla, cfg] = &files[..] else {
        eprintln!("usage: model_show Module.tla Model.cfg [--after DIR]...");
        std::process::exit(2);
    };
    let ir = match oracle::analyze(tla, &setup) {
        Ok(ir) => ir,
        Err(e) => {
            println!("front end: {e}");
            return;
        }
    };
    let text = std::fs::read(cfg).unwrap_or_default();
    let (_, parsed) = dive_config::parse_bytes(&text);
    let Some(c) = parsed.config().cloned() else {
        println!("cfg rejected: {:?}", parsed.error());
        return;
    };
    match oracle::bind(ir, c, setup.opts.clone()) {
        Ok(m) => {
            for o in &m.output {
                println!("output: {o:?}");
            }
            for i in &m.init {
                println!("init: {}", m.location(&i.pred));
            }
            for a in &m.actions {
                println!("action: {}", a.label());
            }
            for (what, group) in [
                ("invariant", &m.invariants),
                ("implied init", &m.implied_inits),
                ("implied action", &m.implied_actions),
                ("temporal", &m.temporals),
                ("implied temporal", &m.implied_temporals),
                ("constraint", &m.constraints),
                ("action constraint", &m.action_constraints),
            ] {
                for n in group {
                    println!("{what}: {} at {}", n.name, m.location(&n.pred));
                }
            }
            for u in &m.unsupported {
                println!("unsupported: {}", u.feature);
            }
            println!("assumptions: {}", m.assumptions.len());
            println!("assume outcome: {:?}", m.check_assumptions());
            for p in m.take_output() {
                println!("printed by assumptions: {p}");
            }
        }
        Err(f) => {
            for o in &f.output {
                println!("output: {o:?}");
            }
            println!("error: {}", f.error);
        }
    }
}
