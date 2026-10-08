//! Lowers every listed module that `dive-sema` accepts, and reports.
//!
//! `cargo run --release -p dive-ir --example lower_corpus -- [--lib DIR]... [--after DIR]... [--twice] --list FILE`
//!
//! Prints one JSON object per line:
//! `{"path", "verdict", "defs", "exprs", "unbound", "unevaluable", "refusals": {feature: count}, "invariants": [...], "message"}`.
//! `verdict` is `lowered`, `rejected` (SANY rejects the specification, so it
//! is not lowered), `error` (lowering failed), `panic`, or `unreadable`.
//! `--pretty` prints each IR instead of the report. `--twice` lowers each specification twice and checks the results are
//! identical (determinism), and that a JSON round trip gives the same IR.
//! A summary goes to stderr.

use dive_ir::{Expr, Ir};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;

fn esc(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o
}

struct Report {
    verdict: &'static str,
    defs: usize,
    exprs: usize,
    unbound: usize,
    unevaluable: usize,
    refusals: BTreeMap<String, usize>,
    invariants: Vec<String>,
    message: String,
}

/// Every expression's level is SANY's level of the node it came from.
fn levels_match(a: &dive_sema::Analysis, ir: &Ir) -> Result<(), String> {
    let m = a.model();
    let mut ids: Vec<Option<dive_sema::NodeId>> = vec![None; m.nodes().len()];
    let mut work: Vec<dive_sema::NodeId> = m.modules().to_vec();
    while let Some(n) = work.pop() {
        let Some(slot) = ids.get_mut(n.index()) else {
            continue;
        };
        if slot.is_some() {
            continue;
        }
        *slot = Some(n);
        work.extend(m.children(n));
    }
    for (i, node) in ir.nodes().iter().enumerate() {
        let e = dive_ir::ExprId(u32::try_from(i).unwrap_or(0));
        let Some(Some(sema)) = ir.sema_node(e).and_then(|s| ids.get(s).copied()) else {
            continue;
        };
        let want = dive_ir::Level::from(m.level(sema));
        if node.level != want {
            return Err(format!("#{i}: level {:?}, SANY {want:?}", node.level));
        }
    }
    Ok(())
}

fn report(ir: &Ir) -> Report {
    // Refusals in definitions reachable from the root module's own
    // definitions and assumptions: what the specification can use.
    let roots: Vec<dive_ir::DefId> = ir
        .defs()
        .iter()
        .enumerate()
        .filter(|(_, d)| d.module == ir.root() && d.scope == dive_ir::DefScope::TopLevel)
        .map(|(i, _)| dive_ir::DefId(u32::try_from(i).unwrap_or(u32::MAX)))
        .collect();
    let reachable = ir.reachable_defs(&roots);
    let mut refusals = BTreeMap::new();
    for r in ir
        .refusals()
        .iter()
        .filter(|r| reachable.get(r.def.index()).copied().unwrap_or(false))
    {
        *refusals.entry(r.feature.name()).or_insert(0) += 1;
    }
    let unbound = ir
        .nodes()
        .iter()
        .filter(|n| matches!(n.expr, Expr::Unbound(_)))
        .count();
    let unevaluable = ir
        .nodes()
        .iter()
        .filter(|n| matches!(n.expr, Expr::Unevaluable(_)))
        .count();
    Report {
        verdict: "lowered",
        defs: ir.defs().len(),
        exprs: ir.nodes().len(),
        unbound,
        unevaluable,
        refusals,
        invariants: dive_ir::validate::check(ir),
        message: String::new(),
    }
}

fn lower_one(path: &std::path::Path, opts: &dive_sema::Options, twice: bool) -> Report {
    let empty = |verdict, message: String| Report {
        verdict,
        defs: 0,
        exprs: 0,
        unbound: 0,
        unevaluable: 0,
        refusals: BTreeMap::new(),
        invariants: Vec::new(),
        message,
    };
    let analysis = match dive_sema::analyze_file(path, opts) {
        Ok(a) => a,
        Err(e) => return empty("unreadable", e.to_string()),
    };
    if !analysis.accepted() {
        return empty("rejected", String::new());
    }
    match dive_ir::lower(&analysis) {
        Ok(ir) => {
            let mut r = report(&ir);
            if let Err(e) = levels_match(&analysis, &ir) {
                r.invariants.push(e);
            }
            if twice {
                match dive_ir::lower(&analysis) {
                    Ok(again) if again == ir => {}
                    Ok(_) => r
                        .invariants
                        .push("lowering is not deterministic".to_owned()),
                    Err(e) => r.invariants.push(format!("second lowering failed: {e}")),
                }
                match serde_json::to_string(&ir).map(|s| serde_json::from_str::<Ir>(&s)) {
                    Ok(Ok(back)) if back == ir => {}
                    Ok(Ok(_)) => r
                        .invariants
                        .push("JSON round trip changed the IR".to_owned()),
                    Ok(Err(e)) => r.invariants.push(format!("JSON read-back failed: {e}")),
                    Err(e) => r.invariants.push(format!("JSON write failed: {e}")),
                }
                if ir.pretty() != ir.pretty() {
                    r.invariants
                        .push("pretty-print is not deterministic".to_owned());
                }
            }
            r
        }
        Err(e) => empty("error", e.to_string()),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut opts = dive_sema::Options::default();
    let mut list = None;
    let mut twice = false;
    let mut pretty = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--lib" => opts.search.library_paths.extend(it.next().map(Into::into)),
            "--after" => opts.search.after_standard.extend(it.next().map(Into::into)),
            "--list" => list = it.next().cloned(),
            "--twice" => twice = true,
            "--pretty" => pretty = true,
            _ => {}
        }
    }
    let Some(list) = list else {
        eprintln!("usage: lower_corpus [--lib DIR]... [--after DIR]... [--twice] --list FILE");
        std::process::exit(2);
    };
    let Ok(text) = std::fs::read_to_string(&list) else {
        eprintln!("cannot read {list}");
        std::process::exit(2);
    };
    // Report panics as a verdict instead of printing them.
    std::panic::set_hook(Box::new(|_| {}));
    let start = std::time::Instant::now();
    let mut out = std::io::stdout().lock();
    let mut verdicts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut refusals: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut unbound = 0;
    let mut violations = 0;
    for path in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let p = std::path::Path::new(path);
        if pretty
            && let Ok(a) = dive_sema::analyze_file(p, &opts)
            && let Ok(ir) = dive_ir::lower(&a)
        {
            let _ = writeln!(out, "{}", ir.pretty());
            continue;
        }
        let r = std::panic::catch_unwind(|| lower_one(p, &opts, twice)).unwrap_or_else(|e| {
            let message = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                .unwrap_or_default();
            Report {
                verdict: "panic",
                defs: 0,
                exprs: 0,
                unbound: 0,
                unevaluable: 0,
                refusals: BTreeMap::new(),
                invariants: Vec::new(),
                message,
            }
        });
        *verdicts.entry(r.verdict).or_insert(0) += 1;
        for (f, n) in &r.refusals {
            let e = refusals.entry(f.clone()).or_insert((0, 0));
            e.0 += n;
            e.1 += 1;
        }
        unbound += r.unbound;
        if !r.invariants.is_empty() {
            violations += 1;
        }
        let refs: Vec<String> = r
            .refusals
            .iter()
            .map(|(f, n)| format!("\"{}\":{n}", esc(f)))
            .collect();
        let invs: Vec<String> = r
            .invariants
            .iter()
            .take(20)
            .map(|v| format!("\"{}\"", esc(v)))
            .collect();
        let _ = writeln!(
            out,
            "{{\"path\":\"{}\",\"verdict\":\"{}\",\"defs\":{},\"exprs\":{},\"unbound\":{},\"unevaluable\":{},\"refusals\":{{{}}},\"invariants\":[{}],\"message\":\"{}\"}}",
            esc(path),
            r.verdict,
            r.defs,
            r.exprs,
            r.unbound,
            r.unevaluable,
            refs.join(","),
            invs.join(","),
            esc(&r.message)
        );
    }
    eprintln!("{:.2}s", start.elapsed().as_secs_f64());
    eprintln!("verdicts: {verdicts:?}");
    eprintln!("unbound references: {unbound}");
    eprintln!("specifications with invariant violations: {violations}");
    eprintln!("refusals reachable from the root module (uses, specifications):");
    for (f, (n, s)) in &refusals {
        eprintln!("  {f}: {n} uses in {s} specifications");
    }
}
