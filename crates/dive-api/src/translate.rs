//! Bounded native sequential/finite-process PlusCal artifacts, validated as enclosing modules.
use crate::{AnalyzeOptions, ApiError, analyze_source};
use dive_pluscal::{Code, TranslationOptions};
use serde_json::{Value, json};
use std::io::Read as _;
use std::path::Path;

/// Version of native translation report documents.
pub const TRANSLATE_SCHEMA_VERSION: u32 = 2;
/// Translation mode and explicit resource/module resolution settings.
#[derive(Clone, Debug, Default)]
pub struct TranslationRequest {
    /// Native bounded parser/generator limits.
    pub limits: TranslationOptions,
    /// Module resolution for validating the resulting module.
    pub analyze: AnalyzeOptions,
    /// Global compatibility selection. Current supported generation is shared;
    /// specification backends and broader
    /// checksum shapes remain pending.
    pub compat_tlc: bool,
}
/// Stage-separated translation report. Acceptance never means a model proof.
#[derive(Clone, Debug)]
pub struct TranslationReport {
    document: Value,
    status: u8,
}
impl TranslationReport {
    /// Whether a generated enclosing module passed native syntax and semantics.
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.status == 0
    }
    /// Native command exit: zero artifact, one rejected, three unsupported/bound,
    /// four internal error. No TLC model-checking exit is implied.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        self.status
    }
    /// Complete stage/disposition/limits/evidence document.
    #[must_use]
    pub fn to_json(&self) -> Value {
        self.document.clone()
    }
}
pub(crate) fn translation_artifact_json(artifact: &dive_pluscal::Translation) -> Value {
    let mut value = json!(artifact);
    if let Some(object) = value.as_object_mut() {
        object.remove("option_context");
    }
    value
}
fn report(
    file: &str,
    request: &TranslationRequest,
    status: u8,
    stage: &str,
    details: Value,
) -> TranslationReport {
    let result = match status {
        0 => "artifact",
        1 => "rejected",
        3 => "unsupported",
        _ => "internal-error",
    };
    let mut document = json!({"schema_version":TRANSLATE_SCHEMA_VERSION,"command":"translate","file":file,
        "profile":if request.compat_tlc {"tlc"} else {"dive"},"result":result,"stage":stage,"accepted":status==0,
        "exit_code":status,"limits":request.limits});
    if let (Some(object), Value::Object(extra)) = (document.as_object_mut(), details) {
        object.extend(extra);
    }
    TranslationReport { document, status }
}
/// Translates a file in memory without rewriting it or invoking external tools.
/// The file must contain one algorithm in a block comment. An existing translation
/// is replaced in its original location; nothing is written to disk.
/// Generated modules pass native syntax/name/level analysis before acceptance.
///
/// # Errors
/// An I/O API error if the input cannot be read; stage failures are reports.
pub fn translate_file(
    path: &Path,
    request: &TranslationRequest,
) -> Result<TranslationReport, ApiError> {
    let file = path.display().to_string();
    let reader = std::fs::File::open(path).map_err(|error| ApiError::Io {
        path: file.clone(),
        error,
    })?;
    let mut bytes = Vec::new();
    reader
        .take(4_194_305)
        .read_to_end(&mut bytes)
        .map_err(|error| ApiError::Io {
            path: file.clone(),
            error,
        })?;
    if bytes.len() > 4_194_304 {
        return Ok(report(
            &file,
            request,
            3,
            "selection",
            json!({"diagnostic":{"code":"DIVE-P0040","span":{"start":0,"end":0},"message":"PlusCal module source exceeds 4 MiB"}}),
        ));
    }
    let source = String::from_utf8(bytes).map_err(|error| ApiError::Io {
        path: file.clone(),
        error: std::io::Error::new(std::io::ErrorKind::InvalidData, error),
    })?;
    Ok(translate_source(&file, &source, request))
}
/// Translates module source in memory. Artifact spans refer to generated body;
/// original spans refer to this exact UTF-8 source. No file mapping/checksum
/// compatibility or complete PlusCal translation coverage is implied.
#[must_use]
pub fn translate_source(
    file: &str,
    source: &str,
    request: &TranslationRequest,
) -> TranslationReport {
    translate_source_at_insertion(file, source, request, None)
}

/// Generates at the final placement before module analysis, including rewrite's
/// masked view of a previous block located after intervening user definitions.
#[allow(
    clippy::too_many_lines,
    reason = "ordered source selection, bounded generation and enclosing-module validation retain deciding-stage evidence"
)]
pub(crate) fn translate_source_at_insertion(
    file: &str,
    source: &str,
    request: &TranslationRequest,
    insertion_override: Option<usize>,
) -> TranslationReport {
    if source.len() > 4_194_304 {
        return report(
            file,
            request,
            3,
            "selection",
            json!({"diagnostic":{"code":"DIVE-P0040","span":{"start":0,"end":0},"message":"PlusCal module source exceeds 4 MiB"}}),
        );
    }
    let selection = match dive_pluscal::rewrite::select_rewrite(
        source,
        &dive_pluscal::rewrite::RewriteOptions::default(),
    ) {
        Ok(selection) => selection,
        Err(mut diagnostic) => {
            // Plain translation has always refused incomplete marker layouts;
            // retain that public selection boundary while sharing rewrite's selector.
            if diagnostic.code == Code::Syntax
                && diagnostic.message == "missing or reversed translation marker pair"
            {
                diagnostic.code = Code::Unsupported;
                "unsupported incomplete translation marker pair"
                    .clone_into(&mut diagnostic.message);
            }
            let status = match diagnostic.code {
                Code::Unsupported | Code::Resource => 3,
                Code::Internal => 4,
                _ => 1,
            };
            return report(
                file,
                request,
                status,
                "selection",
                json!({"code": diagnostic.code.as_str(), "diagnostic": diagnostic}),
            );
        }
    };
    if let Some(previous) = selection.previous
        && let Some(declaration) = dive_pluscal::source_option_declaration_span(source)
        && declaration.start < previous.end
        && declaration.end > previous.start
    {
        return report(
            file,
            request,
            3,
            "source-options",
            json!({"diagnostic": {
                "code": "DIVE-P0090", "span": declaration,
                "message": "unsupported PlusCal source option declaration inside previous translation; masking would change the deciding context",
            }}),
        );
    }
    let context = match dive_pluscal::resolve_translation_options(source, &request.limits) {
        Ok(context) => context,
        Err(error) => {
            let status = match error.diagnostic.code {
                Code::Unsupported | Code::Resource => 3,
                Code::Internal => 4,
                _ => 1,
            };
            return report(
                file,
                request,
                status,
                "translation",
                json!({"translation_stage":error.stage,"diagnostic":error.diagnostic,"code":error.diagnostic.code.as_str()}),
            );
        }
    };
    let selection = match dive_pluscal::rewrite::select_rewrite_with_parser(
        source,
        &dive_pluscal::rewrite::RewriteOptions::default(),
        &context.effective.parser,
    ) {
        Ok(selection) => selection,
        Err(diagnostic) => {
            let status = match diagnostic.code {
                Code::Unsupported | Code::Resource => 3,
                Code::Internal => 4,
                _ => 1,
            };
            return report(
                file,
                request,
                status,
                "selection",
                json!({"code":diagnostic.code.as_str(),"diagnostic":diagnostic}),
            );
        }
    };
    let masked = dive_pluscal::rewrite::mask_previous(source, selection.previous);
    let region = selection.algorithm.start..selection.algorithm.end;
    let insertion = insertion_override.unwrap_or_else(|| {
        selection
            .previous
            .map_or(selection.insertion, |previous| previous.start)
    });
    let replacement_end = selection
        .previous
        .map_or(insertion, |previous| previous.end);
    let artifact = match dive_pluscal::translate_at(&masked, region, &request.limits) {
        Ok(artifact) => artifact,
        Err(error) => {
            let status = match error.diagnostic.code {
                Code::Unsupported | Code::Resource => 3,
                Code::Internal => 4,
                _ => 1,
            };
            return report(
                file,
                request,
                status,
                "translation",
                json!({"translation_stage":error.stage,"diagnostic":error.diagnostic,"code":error.diagnostic.code.as_str()}),
            );
        }
    };
    let body = if selection.previous.is_some() {
        format!(
            "\\* BEGIN TRANSLATION\n{}\n\\* END TRANSLATION\n",
            artifact.tla
        )
    } else {
        artifact.tla.clone()
    };
    let generated = format!(
        "{}\n{body}\n{}",
        &source[..insertion],
        &source[replacement_end..]
    );
    let analysis = analyze_source(file, &generated, &request.analyze);
    if !analysis.accepted() {
        return report(
            file,
            request,
            1,
            "artifact-analysis",
            json!({"analysis":analysis.to_json(),"artifact_returned":false}),
        );
    }
    report(
        file,
        request,
        0,
        "artifact-analysis",
        json!({"translation_options":artifact.option_context.effective,"requested_translation_options":request.limits,"source_options":artifact.option_context.source_declaration,"artifact":translation_artifact_json(&artifact),"module_source":generated,"analysis":analysis.to_json(),"filesystem_plan":{
            "writes_performed":false,
            "backup_requested":!artifact.option_context.effective.no_old,
            "configuration_requested":!artifact.option_context.effective.no_configuration,
            "configuration_requirements":artifact.configuration,
            "unix_eol_requested":artifact.option_context.effective.unix_eol,
            "body_newlines":"lf",
            "source_newlines":"preserved"
        }}),
    )
}
