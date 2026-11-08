//! Graph-label policy: Core's unlimited graph cells.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "finite policy assertions"
)]

#[path = "support/core_final.rs"]
mod core_final;

use dive_runtime::CheckOptions;

const CFG: &str = "INIT Init\nNEXT Next\nPROPERTY P\n";

fn source(property: &str) -> String {
    format!(
        "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == {property}\n====\n"
    )
}

/// Core runs with no graph-label cell limit; the caller-set graph and word
/// cell ceilings belonged to the retired reference final route (#791).
#[test]
fn core_graph_labels_have_no_cell_limit_and_keep_validated_words() {
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        for workers in [1, 4] {
            let options = CheckOptions {
                workers,
                ..options.clone()
            };
            let run = core_final::check(source("[]<>(x \\in {FALSE,TRUE})"), CFG, &options)
                .expect("holds");
            let core_final::Outcome::Holds(capture) = run.outcome() else {
                panic!("holding property: {:?}", run.outcome())
            };
            assert_eq!(
                (
                    capture.stats.distinct,
                    capture.stats.generated,
                    capture.stats.depth
                ),
                (2, 3, 2)
            );
            let run = core_final::check(source("[]<>x"), CFG, &options).expect("violation");
            let core_final::Outcome::Violation(word) = run.outcome() else {
                panic!("stuttering falsifies GF x: {:?}", run.outcome())
            };
            assert_eq!(word.validation().validation, dive_trace::Validation::Valid);
            assert_eq!(word.document().properties, ["P"]);
        }
    }
}
