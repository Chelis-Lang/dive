//! TLC's state-variable order (`Ir::tlc_var_order`, `Model::variable_decls`)
//! against the pinned TLC.
//!
//! The reference is `acceptance/VarOrderDump.java`, which reads the order out of
//! TLC's own classes (see `acceptance/README.md`):
//! - `specs.jsonl`: the specifications in `tests/fixtures/var-order/specs/`,
//!   through TLC's `FastTool` (each variable's `varLoc` checked against its
//!   index). They cover 1 to 50 variables across the `Hashtable`'s resize
//!   thresholds, with and without standard modules, names with equal Java
//!   hashes, variables inserted in different resize epochs, `EXTENDS` diamonds,
//!   `INSTANCE`s and an inner module;
//! - `repo.jsonl`: every `.tla` fixture of the repository, through SANY
//!   (`ModuleNode.getVariableDecls` of the root module).
//!
//! `examples/var_order_oracle.rs` compares the Examples (`examples.jsonl`).
//! `dive-store`'s `tests/tlc_state_fingerprints.rs` checks the resulting state
//! fingerprints against TLC's.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed expectation is a test failure"
)]

#[path = "common/var_order.rs"]
mod var_order;

use std::path::{Path, PathBuf};

use dive_sema::Options;
use var_order::{Dumped, compare, parse};

const ORACLE: &str = "tlc-2026.10.06.014338";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/var-order")
}

fn dump(name: &str) -> Vec<Dumped> {
    let p = fixtures().join(ORACLE).join(name);
    let text = std::fs::read_to_string(&p).expect("the dump");
    text.lines()
        .map(|l| parse(l).expect("a dump line"))
        .collect()
}

#[test]
fn generated_specs_match_tlc() {
    let lines = dump("specs.jsonl");
    assert_eq!(lines.len(), 29);
    let mut counts = Vec::new();
    let mut failures = Vec::new();
    for l in &lines {
        let want = l
            .vars
            .as_ref()
            .unwrap_or_else(|| panic!("{}: TLC failed", l.id));
        let path = fixtures()
            .join("specs")
            .join(&l.id)
            .join(format!("{}.tla", l.id));
        if let Err(e) = compare(&path, want, &Options::default()) {
            failures.push(format!("{}: {e}", l.id));
        }
        counts.push(want.len());
    }
    assert!(failures.is_empty(), "{failures:#?}");
    for n in [1, 2, 5, 11, 12, 13, 25, 50] {
        assert!(counts.contains(&n), "no specification with {n} variables");
    }
}

#[test]
fn repository_fixtures_match_sany() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut compared = 0;
    let mut failures = Vec::new();
    for l in dump("repo.jsonl") {
        let Some(want) = &l.vars else {
            continue;
        };
        if let Err(e) = compare(&repo.join(&l.id), want, &Options::default()) {
            failures.push(format!("{}: {e}", l.id));
        }
        compared += 1;
    }
    assert!(failures.is_empty(), "{failures:#?}");
    assert!(compared > 200, "compared only {compared}");
}
