//! Retained original reference spacing selection, now checking the public header alias.
use dive_pluscal::{source_option_declaration_span, source_options_span};
use std::path::Path;
#[test]
fn retained_reference_header_alias() -> Result<(), Box<dyn std::error::Error>> {
    let base =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("source-option-acceptance/source-spacing");
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(base.join("report.json"))?)?;
    let mut checked = 0;
    for row in report["rows"].as_array().ok_or("missing rows")? {
        let id = row["id"].as_str().ok_or("missing identifier")?;
        let source = std::fs::read_to_string(base.join(format!("{id}.tla")))?;
        let span = source_option_declaration_span(&source);
        assert_eq!(span, source_options_span(&source));
        assert_eq!(
            span.is_some(),
            row["native_guarded"]
                .as_bool()
                .ok_or("missing disposition")?
        );
        if let Some(span) = span {
            assert!(source.is_char_boundary(span.start));
            assert!(source.is_char_boundary(span.end));
        }
        checked += 1;
    }
    assert_eq!(checked, 20);
    Ok(())
}
