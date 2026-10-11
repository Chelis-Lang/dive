//! Runnable source-option context guide and native-only evidence adapter.
use dive_api::{RewriteRequest, TranslationRequest};
use dive_pluscal::{FairnessOption, TranslationOptions};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let path = args.first().ok_or("usage: pluscal_source_context SOURCE [raw|translate|rewrite] [declared|none|weak|strong|next] [tlc]")?;
    let source = std::fs::read_to_string(path)?;
    let fairness = match args.get(2).map_or("declared", String::as_str) {
        "declared" => FairnessOption::Declared,
        "none" => FairnessOption::None,
        "weak" => FairnessOption::Weak,
        "strong" => FairnessOption::Strong,
        "next" => FairnessOption::Next,
        _ => return Err("unknown fairness".into()),
    };
    let request = TranslationRequest {
        limits: TranslationOptions {
            fairness,
            ..TranslationOptions::default()
        },
        compat_tlc: args.get(3).is_some_and(|arg| arg == "tlc"),
        ..TranslationRequest::default()
    };
    let mode = args.get(1).map_or("rewrite", String::as_str);
    let document = match mode {
        "translate" => dive_api::translate_source(path, &source, &request).to_json(),
        "rewrite" => {
            let editing = dive_api::RewriteOptions {
                replace_modified: true,
                ..dive_api::RewriteOptions::default()
            };
            dive_api::rewrite_source(
                path,
                &source,
                &RewriteRequest {
                    translation: request,
                    editing,
                },
            )
            .to_json()
        }
        "raw" => {
            match dive_pluscal::rewrite::select_rewrite(
                &source,
                &dive_api::RewriteOptions::default(),
            ) {
                Err(error) => {
                    json!({"result":"selection-error","diagnostic":error,"model_proved":false})
                }
                Ok(selected) => {
                    let region = selected.algorithm.start..selected.algorithm.end;
                    let translation =
                        dive_pluscal::translate_at(&source, region.clone(), &request.limits);
                    let checksum =
                        dive_pluscal::algorithm_checksum_at(&source, region, &request.limits);
                    json!({"translation":translation,"checksum":checksum,"model_proved":false,"artifact_admitted":false})
                }
            }
        }
        _ => return Err("unknown mode".into()),
    };
    println!("{}", serde_json::to_string(&document)?);
    Ok(())
}
