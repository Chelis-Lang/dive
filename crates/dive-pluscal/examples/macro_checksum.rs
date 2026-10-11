//! Runnable bounded checksum API example; no artifact or model proof is implied.
use dive_pluscal::{FairnessOption, TranslationOptions, algorithm_checksum_at, rewrite};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let file = arguments
        .first()
        .ok_or("usage: macro_checksum SOURCE [-label|-nof|-wf|-sf|-wfNext]")?;
    let source = std::fs::read_to_string(file)?;
    let mut options = TranslationOptions::default();
    for flag in arguments.iter().skip(1) {
        match flag.as_str() {
            "-label" => options.insert_missing_labels = true,
            "-nof" => options.fairness = FairnessOption::None,
            "-wf" => options.fairness = FairnessOption::Weak,
            "-sf" => options.fairness = FairnessOption::Strong,
            "-wfNext" => options.fairness = FairnessOption::Next,
            _ => return Err(format!("unsupported example option: {flag}").into()),
        }
    }
    let selection = rewrite::select_rewrite(&source, &rewrite::RewriteOptions::default());
    let document = match selection {
        Ok(selection) => {
            let checksum = algorithm_checksum_at(
                &source,
                selection.algorithm.start..selection.algorithm.end,
                &options,
            );
            json!({"checksum":checksum,"artifact_admitted":false,"model_proved":false,"family_certified":false})
        }
        Err(error) => {
            json!({"selection_error":error,"artifact_admitted":false,"model_proved":false,"family_certified":false})
        }
    };
    println!("{}", serde_json::to_string(&document)?);
    Ok(())
}
