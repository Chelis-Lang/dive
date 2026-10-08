//! Lowers every `.tla` file in the repository's test fixtures that
//! `dive-sema` accepts (the SANY cases, SANY's error corpus, the TLC probes,
//! and the harness and configuration fixtures), and checks on each:
//! - lowering succeeds and `dive_ir::validate::check` finds nothing;
//! - every expression's level is SANY's;
//! - no reference is left unresolved;
//! - lowering is deterministic, and a JSON round trip gives the same IR.
//!
//! The external corpora (Examples, tlaplus tests, CommunityModules) are
//! lowered by `examples/lower_corpus.rs`; see `crates/dive-ir/acceptance/README.md`.

use dive_ir::{Expr, Ir, Level};
use dive_sema::{NodeId, Options};
use std::path::{Path, PathBuf};

fn tla_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            tla_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "tla") {
            out.push(p);
        }
    }
}

fn levels_match(a: &dive_sema::Analysis, ir: &Ir) -> Result<(), String> {
    let m = a.model();
    let mut ids: Vec<Option<NodeId>> = vec![None; m.nodes().len()];
    let mut work: Vec<NodeId> = m.modules().to_vec();
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
        let want = Level::from(m.level(sema));
        if node.level != want {
            return Err(format!("#{i}: level {:?}, SANY {want:?}", node.level));
        }
    }
    Ok(())
}

#[test]
fn every_accepted_fixture_lowers() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for dir in [
        "dive-sema/tests",
        "dive-check/tests/fixtures",
        "dive-conformance/tests/fixtures",
        "dive-config/tests/fixtures",
    ] {
        tla_files(&crates.join(dir), &mut files);
    }
    assert!(files.len() > 200, "found only {} fixtures", files.len());
    let mut lowered = 0;
    let mut failures = Vec::new();
    for f in &files {
        let Ok(a) = dive_sema::analyze_file(f, &Options::default()) else {
            continue;
        };
        if !a.accepted() {
            continue;
        }
        let ir = match dive_ir::lower(&a) {
            Ok(ir) => ir,
            Err(e) => {
                failures.push(format!("{}: {e}", f.display()));
                continue;
            }
        };
        lowered += 1;
        let mut problems = dive_ir::validate::check(&ir);
        if let Err(e) = levels_match(&a, &ir) {
            problems.push(e);
        }
        if ir
            .nodes()
            .iter()
            .any(|n| matches!(n.expr, Expr::Unbound(_)))
        {
            problems.push("unresolved reference".to_owned());
        }
        if dive_ir::lower(&a).ok().as_ref() != Some(&ir) {
            problems.push("lowering is not deterministic".to_owned());
        }
        let back = serde_json::to_string(&ir)
            .ok()
            .and_then(|s| serde_json::from_str::<Ir>(&s).ok());
        if back.as_ref() != Some(&ir) {
            problems.push("JSON round trip differs".to_owned());
        }
        if !problems.is_empty() {
            failures.push(format!("{}: {problems:?}", f.display()));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    assert!(lowered > 80, "lowered only {lowered}");
}
