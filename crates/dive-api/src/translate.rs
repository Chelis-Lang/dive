//! Bounded native sequential/finite-process PlusCal artifacts, validated as enclosing modules.
use crate::{AnalyzeOptions, ApiError, analyze_source};
use dive_pluscal::{Code, TranslationOptions, ast::Span};
use serde_json::{Value, json};
use std::io::Read as _;
use std::path::Path;

/// Version of native translation report documents.
pub const TRANSLATE_SCHEMA_VERSION: u32 = 1;
/// Translation mode and explicit resource/module resolution settings.
#[derive(Clone, Debug, Default)]
pub struct TranslationRequest {
    /// Native bounded parser/generator limits.
    pub limits: TranslationOptions,
    /// Module resolution for validating the resulting module.
    pub analyze: AnalyzeOptions,
    /// Global compatibility selection. Current supported generation is shared;
    /// historical flags, formatting and rewrite/checksum parity remain pending.
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
        "exit_code":status,"model_proved":false,"exhaustive":false,"limits":request.limits,
        "pending":["unproved-overlapping-process-domains","unproved-sequential-procedure-modifiers","name-renaming","unproved-scoped-process-procedure-renaming","unproved-macro-substitution","labels-inside-branches","historical-options","pretty-width","file-rewriting","checksums","mapping-format"]});
    if let (Some(object), Value::Object(extra)) = (document.as_object_mut(), details) {
        object.extend(extra);
    }
    TranslationReport { document, status }
}
fn refused(
    file: &str,
    request: &TranslationRequest,
    span: Span,
    feature: &str,
) -> TranslationReport {
    report(
        file,
        request,
        3,
        "selection",
        json!({"diagnostic":{"code":"DIVE-P0090","span":span,"message":format!("unsupported PlusCal translation: {feature}")}}),
    )
}
/// Translates a file in memory without rewriting it or invoking external tools.
/// The file must contain one algorithm in a block comment and no old translation.
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
#[allow(
    clippy::too_many_lines,
    reason = "ordered source selection, bounded generation and enclosing-module validation retain deciding-stage evidence"
)]
pub fn translate_source(
    file: &str,
    source: &str,
    request: &TranslationRequest,
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
    let mut selected = None;
    let mut module_end = None;
    for token in dive_syntax::tokens::tokenize_spec(source) {
        let start = usize::from(token.range.start());
        let end = usize::from(token.range.end());
        let text = &source[start..end];
        if token.kind == dive_syntax::SyntaxKind::END_MODULE {
            module_end = Some(start);
        }
        if token.kind == dive_syntax::SyntaxKind::BLOCK_COMMENT {
            if token.error.is_some() {
                return report(
                    file,
                    request,
                    1,
                    "selection",
                    json!({"diagnostic":{"code":"DIVE-P0001","span":{"start":start,"end":end},"message":"unterminated module block comment"}}),
                );
            }
            let candidate =
                dive_pluscal::rewrite::algorithm_comment_region(source, Span { start, end });
            if candidate.is_none() {
                continue;
            }
            // Pinned trans.java admits only whitespace or a line comment after
            // the closing delimiter, and inserts after that complete line.
            let closing_line = source[end..]
                .find('\n')
                .map_or(source.len(), |offset| end + offset);
            let tail = source[end..closing_line].trim_matches(|character: char| character <= ' ');
            if !tail.is_empty() && !tail.starts_with("\\*") {
                return report(
                    file,
                    request,
                    1,
                    "selection",
                    json!({"diagnostic":{"code":"DIVE-P0002","span":{"start":end,"end":closing_line},"message":"text after algorithm closing comment must be whitespace or a line comment"}}),
                );
            }
            let insertion = closing_line + usize::from(closing_line < source.len());
            let region = match dive_pluscal::rewrite::owned_algorithm_comment_region(
                source,
                Span { start, end },
                &request.limits.parser,
            ) {
                Ok(region) => region,
                Err(error) => {
                    let status = match error.code {
                        Code::Unsupported | Code::Resource => 3,
                        Code::Internal => 4,
                        _ => 1,
                    };
                    return report(
                        file,
                        request,
                        status,
                        "selection",
                        json!({"code":error.code.as_str(),"diagnostic":error}),
                    );
                }
            };
            if selected
                .replace((region.start..region.end, insertion))
                .is_some()
            {
                return refused(
                    file,
                    request,
                    Span { start, end },
                    "multiple algorithm selection",
                );
            }
        }
        if token.kind.is_trivia()
            && (text.contains("BEGIN TRANSLATION") || text.contains("END TRANSLATION"))
        {
            return refused(
                file,
                request,
                Span { start, end },
                "rewriting an existing translation/checksum",
            );
        }
    }
    let Some((region, insertion)) = selected else {
        return refused(
            file,
            request,
            Span::default(),
            "missing algorithm block comment",
        );
    };
    let Some(_) = module_end else {
        return report(
            file,
            request,
            1,
            "selection",
            json!({"diagnostic":{"code":"DIVE-P0002","span":{"start":0,"end":source.len()},"message":"missing enclosing TLA module terminator"}}),
        );
    };
    let artifact = match dive_pluscal::translate_at(source, region, &request.limits) {
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
    let generated = format!(
        "{}\n{}\n{}",
        &source[..insertion],
        artifact.tla,
        &source[insertion..]
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
        json!({"translation_options":artifact.option_context.effective,"requested_translation_options":request.limits,"option_context":artifact.option_context,"artifact":artifact,"module_source":generated,"analysis":analysis.to_json()}),
    )
}
