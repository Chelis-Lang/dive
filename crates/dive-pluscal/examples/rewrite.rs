//! Development-only source-editor/checksum adapter; not artifact/proof admission.
use serde::Deserialize;
use serde_json::json;
use std::{io::Read as _, process::ExitCode};
#[derive(Deserialize)]
struct Input {
    source: String,
    body: String,
    #[serde(default)]
    normalize: bool,
    #[serde(default)]
    replace_modified: bool,
    #[serde(default)]
    full: bool,
    #[serde(default)]
    max_bytes: Option<usize>,
}
fn main() -> ExitCode {
    let mut text = String::new();
    if std::io::stdin()
        .take(8_388_609)
        .read_to_string(&mut text)
        .is_err()
        || text.len() > 8_388_608
    {
        return ExitCode::from(2);
    }
    let Ok(input) = serde_json::from_str::<Input>(&text) else {
        return ExitCode::from(2);
    };
    let options = dive_pluscal::rewrite::RewriteOptions {
        checksums: if input.full {
            dive_pluscal::rewrite::ChecksumPolicy::Full
        } else {
            dive_pluscal::rewrite::ChecksumPolicy::TranslationOnly
        },
        normalize_newlines: input.normalize,
        replace_modified: input.replace_modified,
        max_bytes: input.max_bytes.unwrap_or(4_194_304),
    };
    match dive_pluscal::rewrite::rewrite_document(&input.source, &input.body, &options) {
        Ok(plan) => {
            println!(
                "{}",
                json!({"schema_version":1,"operation":"source-editor","result":"replacement","accepted":true,"exit_code":0,"model_proved":false,"exhaustive":false,"artifact_admitted":false,"plan":plan})
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            let exit = if error.code == dive_pluscal::Code::Syntax
                || error.code == dive_pluscal::Code::Lexical
            {
                1
            } else {
                3
            };
            println!(
                "{}",
                json!({"schema_version":1,"operation":"source-editor","result":"refused","accepted":false,"exit_code":exit,"model_proved":false,"exhaustive":false,"artifact_admitted":false,"diagnostic":error})
            );
            ExitCode::from(exit)
        }
    }
}
