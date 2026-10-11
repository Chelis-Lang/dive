//! dive's split of the next-state relation against TLC's own action list
//! (`Tool.getActions`, recorded by `acceptance/ActionDump.java`) for every
//! model in the repository: the split-rule fixtures in
//! `tests/fixtures/actions/specs/`, the TLC probes and the harness fixtures.
//! Every action's name, parameters and label, their order and their count
//! must be reproduced. The Examples models are compared by
//! `examples/action_oracle.rs` (see `acceptance/README.md`).

mod common;

use common::action_oracle::{Outcome, compare, parse};
use std::path::Path;

#[test]
fn actions_match_tlc() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = crate_dir.join("../..");
    let dump = crate_dir.join("tests/fixtures/actions/tlc-2026.10.06.014338/repo.jsonl");
    let text = std::fs::read_to_string(&dump).unwrap_or_default();
    let mut compared = 0;
    let mut tlc_errors = 0;
    let mut failures = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let Some(model) = parse(line) else {
            failures.push(format!("unparsable line {line}"));
            continue;
        };
        if model.error.is_some() {
            // TLC rejects these harness fixtures (parse errors, a missing
            // constant): there is no action list to compare.
            tlc_errors += 1;
            continue;
        }
        let dir = Path::new(&model.id).parent().unwrap_or(Path::new(""));
        let path = workspace.join(dir).join(&model.module);
        let analysis = dive_sema::analyze_file(&path, &dive_sema::Options::default());
        let outcome = match analysis {
            Ok(a) if a.accepted() => match dive_ir::lower(&a) {
                Ok(ir) => compare(&ir, &model),
                Err(e) => Outcome::Mismatch(format!("lowering failed: {e}")),
            },
            Ok(_) => Outcome::Mismatch("rejected by dive-sema".to_owned()),
            Err(e) => Outcome::Mismatch(format!("unreadable: {e}")),
        };
        compared += 1;
        if let Outcome::Mismatch(d) = outcome {
            failures.push(format!("{}: {d}", model.id));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    assert_eq!(
        tlc_errors, 4,
        "TLC rejected an unexpected number of fixtures"
    );
    assert_eq!(compared, 44, "compared {compared} models");
}
