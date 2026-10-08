//! Comparing dive's state-variable order with `acceptance/VarOrderDump.java`'s.

use std::path::Path;

/// One line of a `VarOrderDump` dump.
pub struct Dumped {
    /// The id (the module path, relative to the corpus or the repository).
    pub id: String,
    /// TLC's order, or `None` when SANY or TLC rejected the module.
    pub vars: Option<Vec<String>>,
}

/// Parses one line of a dump.
#[must_use]
pub fn parse(line: &str) -> Option<Dumped> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let id = v.get("id")?.as_str()?.to_owned();
    if v.get("error").is_some() {
        return Some(Dumped { id, vars: None });
    }
    let vars = v
        .get("vars")?
        .as_array()?
        .iter()
        .map(|n| n.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()?;
    Some(Dumped {
        id,
        vars: Some(vars),
    })
}

/// dive's order for a root module file, from `Model::variable_decls` and, when
/// the module lowers, from `Ir::tlc_var_order`. `None` when dive-sema rejects
/// the module.
#[must_use]
pub fn dive_order(
    path: &Path,
    opts: &dive_sema::Options,
) -> Option<(Vec<String>, Option<Vec<String>>)> {
    let a = dive_sema::analyze_file(path, opts).ok()?;
    if !a.accepted() {
        return None;
    }
    let m = a.model();
    let root = m.root()?;
    let sema: Vec<String> = m
        .variable_decls(root)
        .into_iter()
        .map(|v| m.symbol_name(v).unwrap_or("?").to_owned())
        .collect();
    let ir = dive_ir::lower(&a).ok().map(|ir| {
        ir.tlc_var_order()
            .iter()
            .filter_map(|&v| ir.vars().get(v.index()))
            .map(|&d| ir.decl(d).name.clone())
            .collect()
    });
    Some((sema, ir))
}

/// Compares one dumped module with dive's order; `Err` describes a mismatch.
///
/// # Errors
/// When dive rejects a module SANY accepts, or an order differs.
pub fn compare(path: &Path, want: &[String], opts: &dive_sema::Options) -> Result<(), String> {
    let Some((sema, ir)) = dive_order(path, opts) else {
        return Err("SANY accepts it, dive-sema does not".to_owned());
    };
    if sema != want {
        return Err(format!("dive-sema {sema:?}, TLC {want:?}"));
    }
    match ir {
        Some(ir) if ir != want => Err(format!("IR {ir:?}, TLC {want:?}")),
        _ => Ok(()),
    }
}
