//! Semantic acceptance runner for the matrix scripts in `acceptance/`.
//!
//! `cargo run --release -p dive-sema --example acceptance -- [--lib DIR]... [--after DIR]... [--no-semantic] [--no-level-check] --list FILE`
//!
//! Prints one JSON object per line: `{"path", "verdict", "codes", "sany", "message"}`.
//! `--lib` directories are searched before the standard modules, `--after`
//! directories after them (the harness's classpath libraries).

use std::fmt::Write as _;
use std::io::Write as _;

fn esc(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o
}

/// Checks the guarantees of `docs/sema-model.md` on an accepted analysis.
/// Returns the first violation found.
fn check_invariants(a: &dive_sema::Analysis) -> Option<String> {
    use dive_sema::model::{Kind, NodeKind};
    let m = a.model();
    let mut seen = std::collections::HashSet::new();
    let mut stack: Vec<dive_sema::NodeId> = m.modules().to_vec();
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        let node = m.node(id);
        let kind = m.kind(id);
        if kind == Kind::Error {
            return Some(format!("error node {id:?} reachable"));
        }
        let builtin = matches!(&node.kind, NodeKind::OpDef(d) if d.kind == dive_sema::model::DefKind::BuiltIn);
        if builtin {
            continue;
        }
        let unchecked_ok = builtin
            || matches!(
                kind,
                Kind::Module
                    | Kind::ModuleInstance
                    | Kind::FormalParam
                    | Kind::ConstantDecl
                    | Kind::VariableDecl
                    | Kind::NewConstant
                    | Kind::NewVariable
                    | Kind::NewState
                    | Kind::NewAction
                    | Kind::NewTemporal
                    | Kind::Theorem
                    | Kind::Assume
            );
        if node.level.checked == 0 && !unchecked_ok {
            return Some(format!(
                "{kind:?} node {id:?} at {:?} was not level-checked",
                node.span
            ));
        }
        if let NodeKind::OpAppl(d) = &node.kind {
            let op = m.node(d.operator);
            if let NodeKind::OpDef(od) = &op.kind
                && od.kind != dive_sema::model::DefKind::BuiltIn
                && od.arity >= 0
                && usize::try_from(od.arity).ok() != Some(d.operands.len())
            {
                return Some(format!("arity mismatch at {:?}", node.span));
            }
            if d.operands.iter().any(Option::is_none) && m.symbol_name(d.operator) != Some("$Pair")
            {
                return Some(format!("missing operand at {:?}", node.span));
            }
        }
        stack.extend(m.children(id));
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut opts = dive_sema::Options::default();
    let mut list = None;
    let mut invariants = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--lib" => opts.search.library_paths.extend(it.next().map(Into::into)),
            "--after" => opts.search.after_standard.extend(it.next().map(Into::into)),
            "--list" => list = it.next().cloned(),
            "--no-semantic" => opts.semantic = false,
            "--no-level-check" => opts.level_check = false,
            "--invariants" => invariants = true,
            _ => {}
        }
    }
    let Some(list) = list else {
        eprintln!("usage: acceptance [--lib DIR]... [--after DIR]... --list FILE");
        std::process::exit(2);
    };
    let Ok(text) = std::fs::read_to_string(&list) else {
        eprintln!("cannot read {list}");
        std::process::exit(2);
    };
    let start = std::time::Instant::now();
    let mut out = std::io::stdout().lock();
    for path in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let line = match dive_sema::analyze_file(std::path::Path::new(path), &opts) {
            Ok(a) => {
                let mut codes: Vec<String> = Vec::new();
                let mut sany: Vec<String> = Vec::new();
                let mut msg = String::new();
                for (_, d) in a.syntax_diagnostics() {
                    if d.is_error() {
                        codes.push(format!("\"{}\"", d.code.as_str()));
                        if msg.is_empty() {
                            msg.clone_from(&d.message);
                        }
                    }
                }
                if invariants
                    && a.accepted()
                    && opts.level_check
                    && let Some(v) = check_invariants(&a)
                {
                    eprintln!("INVARIANT {path}: {v}");
                }
                for d in a.diagnostics().iter().filter(|d| d.is_error()) {
                    codes.push(format!("\"{}\"", d.code.as_str()));
                    if let Some(s) = d.sany {
                        sany.push(s.to_string());
                    }
                    if msg.is_empty() {
                        msg.clone_from(&d.message);
                    }
                }
                format!(
                    "{{\"path\":\"{}\",\"verdict\":\"{}\",\"codes\":[{}],\"sany\":[{}],\"message\":\"{}\"}}",
                    esc(path),
                    if a.accepted() { "accept" } else { "reject" },
                    codes.join(","),
                    sany.join(","),
                    esc(&msg)
                )
            }
            Err(e) => format!(
                "{{\"path\":\"{}\",\"verdict\":\"error\",\"codes\":[],\"sany\":[],\"message\":\"{}\"}}",
                esc(path),
                esc(&e.to_string())
            ),
        };
        let _ = writeln!(out, "{line}");
    }
    eprintln!("{:.2}s", start.elapsed().as_secs_f64());
}
