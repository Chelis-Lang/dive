//! Compare public Core results against pinned TLC's community-module overrides.
#![allow(clippy::expect_used, reason = "test fixtures")]
#[path = "support/core_final.rs"]
mod core_final;
use dive_runtime::CheckOptions;
use serde_json::json;

const SOURCE: &str = include_str!("fixtures/collection-search/Collection.tla");

#[test]
fn subsequence_search_and_bag_folds_respect_profile_and_callback_order() {
    for (options, tlc) in [
        (CheckOptions::dive(0), false),
        (CheckOptions::tlc(0, 0), true),
    ] {
        for (name, expected) in [
            ("Positions", "success"),
            ("Bad", "invariant-violation"),
            ("Asc", if tlc { "unsupported" } else { "success" }),
            (
                "TlcOrder",
                if tlc {
                    "unsupported"
                } else {
                    "invariant-violation"
                },
            ),
            ("Low", "evaluation-error"),
            ("High", "evaluation-error"),
        ] {
            let cfg = format!("INIT Init\nNEXT Next\nINVARIANT {name}\n");
            let row = core_final::run(SOURCE, &cfg, &options).to_json();
            assert_eq!(row["verdict"], expected, "{name}: {row}");
            if expected == "invariant-violation" {
                assert_eq!(row["property"], name);
                let files = core_final::Files::new(SOURCE, &cfg);
                let model = dive_api::trace::SpecModel::load(
                    &files.file,
                    &files.file.with_extension("cfg"),
                    &dive_api::trace::TraceOptions {
                        compat_tlc: tlc,
                        run: Some((0, 0)),
                        ..Default::default()
                    },
                )
                .expect("validation model");
                let trace =
                    dive_api::trace::TraceDoc::parse(&row["counterexample"]["trace"].to_string())
                        .expect("trace");
                assert_eq!(
                    model.validate(&trace).validation,
                    dive_api::trace::Validation::Valid
                );
            }
            if name == "Positions" {
                assert_eq!(row["statistics"]["generated"], json!(3));
                assert_eq!(row["statistics"]["distinct"], json!(2));
            }
            if matches!(name, "Low" | "High") {
                assert!(
                    row["message"].as_str().is_some_and(|m| m.contains("2183")),
                    "{row}"
                );
            }
        }
    }
}
