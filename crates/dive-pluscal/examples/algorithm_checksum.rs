//! Development-only native AST checksum adapter; never claims artifact admission.
use std::io::{Read as _, Write as _};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = String::new();
    std::io::stdin()
        .take(8_388_609)
        .read_to_string(&mut source)?;
    let request: serde_json::Value = serde_json::from_str(&source)?;
    let source = request["source"].as_str().ok_or("missing source")?;
    let options: dive_pluscal::TranslationOptions = request
        .get("options")
        .cloned()
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default();
    let editing = dive_pluscal::rewrite::RewriteOptions {
        checksums: dive_pluscal::rewrite::ChecksumPolicy::BestEffort,
        ..Default::default()
    };
    let selected = dive_pluscal::rewrite::select_rewrite(source, &editing)?;
    let result = dive_pluscal::algorithm_checksum_at(
        source,
        selected.algorithm.start..selected.algorithm.end,
        &options,
    );
    let document = match result {
        Ok(checksum) => {
            serde_json::json!({"schema_version":1,"result":"checksum","accepted":true,"model_proved":false,"artifact_admitted":false,"checksum":checksum})
        }
        Err(error) => {
            serde_json::json!({"schema_version":1,"result":"refused","accepted":false,"model_proved":false,"artifact_admitted":false,"error":error})
        }
    };
    writeln!(std::io::stdout(), "{}", serde_json::to_string(&document)?)?;
    Ok(())
}
