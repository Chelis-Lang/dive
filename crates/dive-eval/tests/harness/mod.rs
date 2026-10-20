//! Test harness: loads a specification through the front end, binds its
//! configuration's constants the way TLC does for constant-only models, runs
//! the assumptions, and renders the result as TLC's `-tool` transcript.
//!
//! This is a test-only stand-in for `dive-model` (WP-B1), which owns the real
//! binding stage. It binds plain constant assignments, model values and
//! `<-` substitutions; anything else is reported as unsupported.

#![allow(dead_code, reason = "shared by several test binaries")]
#![allow(clippy::expect_used, clippy::panic, reason = "test code")]

pub mod model;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dive_config::{ConfigValue, ConstantEntry, ModelConfig};
use dive_eval::{AssumptionOutcome, ConstBinding, Evaluator, Model, Options};
use dive_ir::{DefId, Ir, Symbol};
use dive_value::{Ctx, Interner, Value};

/// A lowered specification and its root module text.
pub struct Loaded {
    pub ir: Arc<Ir>,
    pub root_text: String,
}

/// Loads `path` with SANY's search path plus `after_standard` directories.
pub fn load(path: &Path, after_standard: &[PathBuf]) -> Result<Loaded, String> {
    let mut opts = dive_sema::Options::default();
    opts.search.after_standard = after_standard.to_vec();
    let analysis = dive_sema::analyze_file(path, &opts).map_err(|e| format!("read: {e}"))?;
    if !analysis.accepted() {
        return Err("rejected by the front end".to_owned());
    }
    let ir = dive_ir::lower(&analysis).map_err(|e| format!("lowering: {e:?}"))?;
    let root_text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    Ok(Loaded {
        ir: Arc::new(ir),
        root_text,
    })
}

/// Interns the names of a module in SANY/TLC's order, approximately:
/// identifiers as the lexer meets them, then string literal contents.
pub fn seed_interner(module: &str) -> Interner {
    let names = Interner::new();
    let mut strings = Vec::new();
    let b = module.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'"' {
            let (s, end) = read_string(module, i);
            strings.push(s);
            i = end;
        } else if c == b'\\' && i + 1 < b.len() && b[i + 1] == b'*' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c == b'(' && i + 1 < b.len() && b[i + 1] == b'*' {
            let mut depth = 0;
            while i + 1 < b.len() {
                if b[i] == b'(' && b[i + 1] == b'*' {
                    depth += 1;
                    i += 2;
                } else if b[i] == b'*' && b[i + 1] == b')' {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if c == b'\\' {
            i += 1;
            while i < b.len() && b[i].is_ascii_alphanumeric() {
                i += 1;
            }
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            names.intern(&module[start..i]);
        } else if c.is_ascii_digit() {
            while i < b.len() && b[i].is_ascii_alphanumeric() {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    for s in strings {
        names.intern(&s);
    }
    names
}

fn read_string(src: &str, start: usize) -> (String, usize) {
    let mut out = String::new();
    let mut chars = src[start + 1..].char_indices();
    while let Some((k, c)) = chars.next() {
        match c {
            '"' => return (out, start + 1 + k + 1),
            '\\' => {
                if let Some((_, e)) = chars.next() {
                    out.push(match e {
                        't' => '\t',
                        'n' => '\n',
                        'r' => '\r',
                        'f' => '\u{000C}',
                        other => other,
                    });
                }
            }
            c => out.push(c),
        }
    }
    (out, src.len())
}

fn cfg_value(v: &ConfigValue, names: &Interner, ctx: Ctx) -> Result<Value, String> {
    Ok(match v {
        ConfigValue::Int(i) => Value::Int(i64::from(*i)),
        ConfigValue::String(s) => Value::string(names, s),
        ConfigValue::Bool(b) => Value::Bool(*b),
        ConfigValue::ModelValue(m) => Value::model(names, m),
        ConfigValue::Set(items) => {
            let mut out = Vec::new();
            for i in items {
                out.push(cfg_value(&i.value, names, ctx)?);
            }
            Value::set(out, ctx).map_err(|e| e.to_string())?
        }
    })
}

/// Binds a configuration's constants. Errors name what is not supported.
pub fn bind(loaded: &Loaded, cfg: Option<&ModelConfig>, opts: Options) -> Result<Model, String> {
    let ir = &loaded.ir;
    let names = seed_interner(&loaded.root_text);
    if let Some(cfg) = cfg {
        for m in &cfg.model_values {
            names.intern(m);
        }
    }
    let mut model = Model::new(ir.clone());
    let ctx = opts.ctx;
    if let Some(cfg) = cfg {
        for entry in &cfg.constants {
            match entry {
                ConstantEntry::Assignment {
                    name,
                    args,
                    module: None,
                    value,
                } => {
                    if !args.is_empty() {
                        return Err(
                            "unsupported: a constant operator assigned values in the cfg"
                                .to_owned(),
                        );
                    }
                    let v = cfg_value(&value.value, &names, ctx)?;
                    match ir.lookup(ir.root(), &name.text) {
                        Some(Symbol::Decl(d)) => match ir.decl(d).constant {
                            Some(c) => model.constants[c.index()] = Some(ConstBinding::Value(v)),
                            None => return Err(format!("cfg assigns non-constant {}", name.text)),
                        },
                        Some(Symbol::Def(d)) => {
                            model.def_values.insert(d, v);
                        }
                        // TLC ignores assignments to names the specification does not declare.
                        _ => {}
                    }
                }
                ConstantEntry::Substitution {
                    name,
                    module: None,
                    target,
                } => {
                    let Some(Symbol::Def(t)) = ir.lookup(ir.root(), &target.text) else {
                        return Err(format!("cfg substitutes unknown {}", target.text));
                    };
                    match ir.lookup(ir.root(), &name.text) {
                        Some(Symbol::Decl(d)) => match ir.decl(d).constant {
                            Some(c) => model.constants[c.index()] = Some(ConstBinding::Def(t)),
                            None => {
                                return Err(format!("cfg substitutes non-constant {}", name.text));
                            }
                        },
                        Some(Symbol::Def(d)) => {
                            model.redirects.insert(d, t);
                        }
                        _ => return Err(format!("cfg names unknown {}", name.text)),
                    }
                }
                _ => return Err("unsupported: module-scoped cfg entries".to_owned()),
            }
        }
    }
    model.names = Arc::new(names);
    Ok(model)
}

/// Whether an assumption is written `AXIOM` (TLC skips those).
pub fn is_axiom(loaded: &Loaded, d: DefId) -> bool {
    let ir = &loaded.ir;
    let def = ir.def(d);
    let Some(file) = def.origin.file else {
        return false;
    };
    if ir.files()[file.index()].module != ir.module(ir.root()).name {
        return false;
    }
    let start = def.origin.range.0 as usize;
    let before = loaded.root_text.get(..start).unwrap_or("");
    let tail: String = before
        .chars()
        .rev()
        .take(64)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let word = tail
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>();
    let s = loaded.root_text.get(start..).unwrap_or("");
    s.starts_with("AXIOM") || word.iter().rev().take(3).any(|w| *w == "AXIOM")
}

/// The part of TLC's transcript that checking assumptions produces: printed
/// output and the assumption error, if any, up to the start of state
/// exploration.
pub fn tlc_assumption_part(stdout: &str) -> String {
    let lines: Vec<&str> = stdout.lines().collect();
    let Some(start) = lines.iter().position(|l| *l == "@!@!@ENDMSG 2185 @!@!@") else {
        // An error at start-up, before `Starting...`: the first error message
        // after SANY finished.
        let after = lines
            .iter()
            .position(|l| *l == "@!@!@ENDMSG 2219 @!@!@")
            .unwrap_or(0);
        let Some(b) = lines[after..]
            .iter()
            .position(|l| l.starts_with("@!@!@STARTMSG") && l.ends_with(":1 @!@!@"))
        else {
            return String::new();
        };
        let b = after + b;
        let code = lines[b]
            .trim_start_matches("@!@!@STARTMSG ")
            .split(':')
            .next()
            .unwrap_or("");
        let end_marker = format!("@!@!@ENDMSG {code} @!@!@");
        let e = lines[b..]
            .iter()
            .position(|l| *l == end_marker)
            .map_or(lines.len(), |k| b + k + 1);
        return lines[b..e].join("\n");
    };
    let mut out = Vec::new();
    let mut i = start + 1;
    while i < lines.len() {
        let l = lines[i];
        if l.starts_with("@!@!@STARTMSG 2189:") || l.starts_with("@!@!@STARTMSG 2186:") {
            break;
        }
        out.push(l);
        i += 1;
    }
    out.join("\n")
}

/// dive's rendering of the same part.
pub fn dive_assumption_part(printed: &[String], outcome: &AssumptionOutcome) -> String {
    let mut out: Vec<String> = printed.to_vec();
    match outcome {
        AssumptionOutcome::AllTrue => {}
        AssumptionOutcome::False { message, .. } => {
            out.push(format!(
                "@!@!@STARTMSG 2104:1 @!@!@\n{message}\n@!@!@ENDMSG 2104 @!@!@"
            ));
        }
        AssumptionOutcome::Error { def, error, .. } if def.0 == u32::MAX => {
            let code = error.ec().code();
            out.push(format!(
                "@!@!@STARTMSG {code}:1 @!@!@\n{}\n@!@!@ENDMSG {code} @!@!@",
                error.message()
            ));
        }
        AssumptionOutcome::Error { message, .. } => {
            out.push(format!(
                "@!@!@STARTMSG 2105:1 @!@!@\n{message}\n@!@!@ENDMSG 2105 @!@!@"
            ));
        }
    }
    out.join("\n")
}

/// Runs a spec directory `dir/name/name.tla` (+ `.cfg`) under `opts`.
pub fn run_spec(
    dir: &Path,
    name: &str,
    opts: Options,
) -> Result<(Evaluator, Vec<String>, AssumptionOutcome), String> {
    let tla = dir.join(name).join(format!("{name}.tla"));
    // The CommunityModules' `.tla` files of the pinned jar, which TLC finds
    // on its classpath after the standard modules.
    let community = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/community-modules");
    let loaded = load(&tla, &[community])?;
    let cfg_path = dir.join(name).join(format!("{name}.cfg"));
    let cfg = match std::fs::read_to_string(&cfg_path) {
        Ok(text) => {
            let parsed = dive_config::parse(&text);
            Some(parsed.into_config().ok_or("cfg rejected")?)
        }
        Err(_) => None,
    };
    let model = bind(&loaded, cfg.as_ref(), opts)?;
    let ev = Evaluator::new(model, opts).expect("valid evaluator context");
    if let Some(e) = ev.startup_error() {
        // TLC fails while processing the specification: render it as the
        // top-level error it is (see `dive_assumption_part`).
        return Ok((
            ev,
            Vec::new(),
            AssumptionOutcome::Error {
                def: DefId(u32::MAX),
                message: String::new(),
                error: e,
            },
        ));
    }
    let ev2 = ev.clone();
    let outcome = std::thread::Builder::new()
        .stack_size(dive_eval::RECOMMENDED_STACK)
        .spawn(move || ev2.check_assumptions(|d| is_axiom(&loaded, d)))
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "evaluation thread panicked".to_owned())?;
    let printed = ev.take_output();
    Ok((ev, printed, outcome))
}

/// Statistics helper.
pub fn bump(map: &mut HashMap<String, usize>, key: impl Into<String>) {
    *map.entry(key.into()).or_insert(0) += 1;
}
