//! Reference-backed multiline admission is distinct from AST/checksum evidence.
use dive_api::{ChecksumPolicy, RewriteRequest, rewrite_source};
use std::path::Path;
// These historical named-refusal rows now have pinned original-after-generation
// AST/CRC and SANY-valid module controls. Keep the report's original dispositions.
const PORTED_ADMISSIONS: [&str; 34] = [
    "C_junction",
    "C_junction-value",
    "C_unicode-junction",
    "C_binder",
    "C_function-binder",
    "C_fair_junction",
    "C_fair_junction-value",
    "C_fair_unicode-junction",
    "C_fair_binder",
    "C_fair_function-binder",
    "P_junction",
    "P_junction-value",
    "P_unicode-junction",
    "P_binder",
    "P_function-binder",
    "P_empty-then",
    "P_macro",
    "P_fair_junction",
    "P_fair_junction-value",
    "P_fair_unicode-junction",
    "P_fair_binder",
    "P_fair_function-binder",
    "P_fair_empty-then",
    "P_fair_macro",
    "layout-controls__C_junction",
    "layout-controls__C_fair_junction",
    "layout-controls__P_junction",
    "layout-controls__P_fair_junction",
    "supplemental__C_macro-empty-parameters",
    "supplemental__C_fair_macro-empty-parameters",
    "supplemental__P_macro-empty-parameters",
    "supplemental__P_process-lines",
    "supplemental__P_fair_macro-empty-parameters",
    "supplemental__P_fair_process-lines",
];

#[test]
fn complete_selected_multiline_artifacts_match_primary_trees()
-> Result<(), Box<dyn std::error::Error>> {
    let base =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../dive-pluscal/tests/fixtures/multiline");
    let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        base.join("full-controls/report.json"),
    )?)?;
    let mut matched = 0;
    let mut ported = 0;
    let rows = report["rows"]
        .as_array()
        .ok_or("missing complete selection")?;
    assert_eq!(rows.len(), 208);
    for row in rows {
        let id = row["id"].as_str().ok_or("missing id")?;
        let (folder, name) = id.split_once("__").unwrap_or(("primary", id));
        let directory = base.join(folder);
        let source = std::fs::read_to_string(directory.join(format!("{name}.tla")))?;
        let request = RewriteRequest::default();
        let module_name = name.replace('-', "_");
        let actual = rewrite_source(&format!("{module_name}.tla"), &source, &request);
        let document = actual.to_json();
        assert!(document.get("model_proved").is_none());
        assert!(document.get("exhaustive").is_none());
        let outcome = row["dive"]["outcome"]
            .as_str()
            .ok_or("missing disposition")?;
        let newly_supported = PORTED_ADMISSIONS.contains(&id);
        if newly_supported {
            assert_eq!(outcome, "named-refusal", "{id}");
            assert_eq!(row["primary_translated"], true, "{id}");
            ported += 1;
        }
        if outcome == "full-derivation-and-admission" || newly_supported {
            assert!(actual.accepted(), "{id}: {document}");
            assert!(document["replacement"]["algorithm_checksum"].is_string());
            let module = document["replacement"]["module_source"]
                .as_str()
                .ok_or("missing module")?;
            let primary =
                std::fs::read_to_string(directory.join(format!("{name}.translated.tla")))?;
            let primary_crc = primary
                .split_once("chksum(pcal) = \"")
                .and_then(|(_, tail)| tail.split_once('"').map(|(crc, _)| crc))
                .ok_or("missing primary algorithm checksum")?;
            assert_eq!(
                document["replacement"]["algorithm_checksum"], primary_crc,
                "{id}"
            );
            let native = dive_syntax::parse_module(module);
            let oracle = dive_syntax::parse_module(&primary);
            assert!(native.is_ok() && oracle.is_ok(), "{id}");
            assert_eq!(
                structure(&native.syntax()),
                structure(&oracle.syntax()),
                "{id}"
            );
            let expected =
                std::fs::read_to_string(directory.join(format!("{name}.translated.ast")))?;
            assert_eq!(exact_algorithm_ast(&source)?, expected, "{id}");
            matched += 1;
        } else {
            assert!(!actual.accepted(), "{id}: {document}");
            assert_eq!(
                actual.exit_code(),
                if outcome == "named-refusal" { 3 } else { 1 },
                "{id}"
            );
        }
    }
    assert_eq!(matched, 164);
    assert_eq!(ported, PORTED_ADMISSIONS.len());
    Ok(())
}
#[test]
fn multiline_scope_and_lowered_budgets_never_admit_partial_output()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "---- MODULE B ----\nEXTENDS Integers\n(* --algorithm A variable x=0; begin L:x:=1+\n 2; E:skip; end algorithm; *)\n====\n";
    for partial in [false, true] {
        let mut request = RewriteRequest::default();
        if partial {
            request.editing.checksums = ChecksumPolicy::TranslationOnly;
        }
        assert!(rewrite_source("B.tla", source, &request).accepted());
        for limit in [0, 1, 8] {
            let mut bounded = request.clone();
            bounded.translation.limits.max_output_bytes = limit;
            let report = rewrite_source("B.tla", source, &bounded);
            assert_eq!(report.exit_code(), 3);
            assert!(!report.accepted());
            assert!(report.to_json().get("model_proved").is_none());
            let mut bounded = request.clone();
            bounded.editing.max_bytes = limit;
            assert_eq!(rewrite_source("B.tla", source, &bounded).exit_code(), 3);
        }
    }
    let cr = source.replace("+\n", "+\r");
    let selection =
        dive_pluscal::rewrite::select_rewrite(&cr, &dive_api::RewriteOptions::default())?;
    assert!(
        dive_pluscal::algorithm_checksum_at(
            &cr,
            selection.algorithm.start..selection.algorithm.end,
            &dive_pluscal::TranslationOptions::default()
        )
        .is_err_and(
            |error| error.diagnostic.code == dive_pluscal::Code::Unsupported
                && error.diagnostic.message.contains("bare carriage-return")
        )
    );
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-pluscal/tests/fixtures/bare-cr-checksum");
    assert_eq!(
        std::fs::read_to_string(fixture.join("B.lf-source.tla"))?,
        source
    );
    assert_eq!(
        std::fs::read_to_string(fixture.join("B.bare-cr-source.tla"))?,
        cr
    );
    let primary = std::fs::read_to_string(fixture.join("B.primary.tla"))?;
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.join("manifest.json"))?)?;
    assert_eq!(manifest["sany_accepted"], true);
    let primary_body_crc = primary
        .split_once("chksum(tla) = \"")
        .and_then(|(_, tail)| tail.split_once('"').map(|(crc, _)| crc))
        .ok_or("primary body CRC")?;
    assert_eq!(primary_body_crc, manifest["actual_primary_body_crc"]);
    for compat_tlc in [false, true] {
        for checksums in [ChecksumPolicy::Full, ChecksumPolicy::TranslationOnly] {
            let mut request = RewriteRequest::default();
            request.translation.compat_tlc = compat_tlc;
            request.editing.checksums = checksums;
            let translated = dive_api::translate_source("B.tla", &cr, &request.translation);
            assert!(translated.accepted(), "{}", translated.to_json());
            let report = rewrite_source("B.tla", &cr, &request);
            assert!(report.accepted(), "{}", report.to_json());
            let doc = report.to_json();
            assert_eq!(
                doc["artifact"]["tla"],
                translated.to_json()["artifact"]["tla"]
            );
            assert!(doc["replacement"]["algorithm_checksum"].is_null());
            assert_eq!(doc["replacement"]["translation_checksum"], primary_body_crc);
            let warnings: Vec<dive_pluscal::Diagnostic> =
                serde_json::from_value(doc["replacement"]["warnings"].clone())?;
            assert!(
                warnings
                    .iter()
                    .any(|warning| warning.code == dive_pluscal::Code::Unsupported)
            );
            assert_eq!(doc["analysis"]["accepted"], true);
            let module = doc["replacement"]["module_source"]
                .as_str()
                .ok_or("module")?;
            assert_ne!(
                module, primary,
                "unlike disabled/exact headers are not byteidentity"
            );
            assert_eq!(
                structure(&dive_syntax::parse_module(module).syntax()),
                structure(&dive_syntax::parse_module(&primary).syntax())
            );
        }
    }
    // Ordinary LF input retains exact primary bytes and CRC, without header stripping.
    let lf = rewrite_source("B.tla", source, &RewriteRequest::default());
    assert!(lf.accepted());
    assert_eq!(lf.to_json()["replacement"]["module_source"], primary);
    Ok(())
}

fn structure(node: &dive_syntax::SyntaxNode) -> serde_json::Value {
    let children: Vec<_> = node
        .children_with_tokens()
        .filter_map(|child| {
            if let Some(node) = child.clone().into_node() {
                Some(structure(&node))
            } else {
                child
                    .into_token()
                    .filter(|token| !token.kind().is_trivia())
                    .map(|token| serde_json::json!([format!("{:?}", token.kind()), token.text()]))
            }
        })
        .collect();
    serde_json::json!([format!("{:?}", node.kind()), children])
}

fn exact_algorithm_ast(source: &str) -> Result<String, Box<dyn std::error::Error>> {
    let options = dive_pluscal::TranslationOptions::default();
    let context = dive_pluscal::resolve_translation_options(source, &options)?;
    let selection = dive_pluscal::rewrite::select_rewrite_with_parser(
        source,
        &dive_pluscal::rewrite::RewriteOptions::default(),
        &context.effective.parser,
    )?;
    Ok(dive_pluscal::algorithm_checksum_at(
        source,
        selection.algorithm.start..selection.algorithm.end,
        &options,
    )?
    .serialized_ast)
}
