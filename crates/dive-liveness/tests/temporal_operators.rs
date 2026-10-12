//! Pinned TLC planning outcomes for temporal operator models: the supported
//! models plan and pass TLC's liveness formula classifier, and the refused ones
//! stay refused. TLC's internal normalization fields and tableau numbering are
//! not compared (ADR 0082).
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "field-level fixture assertions"
)]
use dive_liveness::formula::Limits;
use serde_json::Value;

/// Plans every fairness conjunct and property, as TLC's `processLiveness`
/// does, then applies TLC's classifier.
fn project(model: &dive_model::Model, limits: Limits) -> Result<(), String> {
    let plans = model
        .temporals
        .iter()
        .chain(&model.implied_temporals)
        .map(|named| dive_liveness::plan::lower_tlc(model, &named.pred, limits))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let formulas: Vec<_> = plans.iter().map(|p| &p.formula).collect();
    let (fairness, properties) = formulas.split_at(model.temporals.len());
    dive_liveness::tlc_plan::classify(fairness, properties, limits).map_err(|e| e.to_string())
}

#[test]
fn exact_operator_fields() {
    complete_fields(
        "stored-successor-material",
        "capture-projection.json",
        14,
        0,
    );
    complete_fields("temporal-operators", "capture.json", 10, 2);
    complete_fields(
        "native-sequence-operators",
        "capture-projection.json",
        9,
        24,
    );
    complete_fields(
        "native-temporal-operators",
        "capture-projection.json",
        17,
        3,
    );
    complete_fields(
        "temporal-operators",
        "capture-instance-projection.json",
        4,
        0,
    );
}
fn complete_fields(
    family: &str,
    capture_file: &str,
    expected_supported: usize,
    expected_refused: usize,
) {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/probes")
        .join(family);
    let capture: Value =
        serde_json::from_str(&std::fs::read_to_string(base.join(capture_file)).expect("capture"))
            .expect("JSON");
    let mut supported = 0;
    let mut refused = 0;
    for row in capture["rows"].as_array().expect("complete selection") {
        let name = row["name"].as_str().expect("name");
        let dir = base.join("models").join(name);
        let analyzed = dive_sema::analyze_file(
            &dir.join(format!("{name}.tla")),
            &dive_sema::Options::default(),
        )
        .expect("file");
        if family == "native-sequence-operators" && !analyzed.accepted() {
            assert!(name.contains("WrongArity") || name == "SequenceTailActionActual");
            assert_ne!(row["exit"], 0, "retained primary rejection");
            refused += 1;
            continue;
        }
        if family == "native-temporal-operators" && !analyzed.accepted() {
            assert!(name == "NativeLenActionRefusal" || name == "NativeWrongArity");
            assert_ne!(row["exit"], 0, "retained primary semantic rejection");
            refused += 1;
            continue;
        }
        assert!(analyzed.accepted(), "{name} SANY-compatible input");
        let ir = std::sync::Arc::new(dive_ir::lower(&analyzed).expect("IR"));
        let config = std::fs::read_to_string(dir.join(format!("{name}.cfg"))).expect("config");
        let config = dive_config::parse(&config).into_config().expect("cfg");
        let model =
            dive_model::bind(ir, &config, &dive_model::BindOptions::tlc()).expect("binding");
        let projection = project(&model, Limits::default());
        if family == "native-sequence-operators" && !name.contains("Append") {
            refused += 1;
            assert!(
                projection.is_err(),
                "{name} runtime-name/native capture stays refused"
            );
            continue;
        }
        if family == "native-temporal-operators" && row["exit"] != 0 {
            refused += 1;
            if name == "NativePrintUnused" {
                assert!(
                    projection.is_ok(),
                    "unused native effect leaves no temporal obligation; facade pre-bind refusal is separate"
                );
            } else {
                assert!(
                    projection.is_err(),
                    "{name} explicitly unsupported or invalid reference source"
                );
            }
            continue;
        }
        if name.contains("Effect") {
            refused += 1;
            assert!(projection.is_err(), "{name} actual reference failure");
            continue;
        }
        if row["exit"] != 0 {
            // The diagnostic helper dereferences null for empty temporal
            // obligations. Preserve this raw failure without field credit;
            // the clean primary run and source-derived safety route are tested.
            assert!(name.contains("Constant"));
            assert!(projection.is_ok(), "{name} native empty planning");
            continue;
        }
        supported += 1;
        projection.unwrap_or_else(|e| panic!("{name}: supported planning: {e}"));
    }
    assert_eq!((supported, refused), (expected_supported, expected_refused));
}
