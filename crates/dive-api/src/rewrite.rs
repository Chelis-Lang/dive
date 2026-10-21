//! Native bounded in-memory translation-block replacement; no file writes.
use crate::{ApiError, TranslationRequest, analyze_source};
pub use dive_pluscal::rewrite::{ChecksumPolicy, RewriteOptions};
use dive_pluscal::{Code, Diagnostic};
use serde_json::{Value, json};
use std::{io::Read as _, path::Path};

/// Generation settings and in-memory editing policy.
#[derive(Clone, Debug, Default)]
pub struct RewriteRequest {
    /// Native PlusCal generation and enclosing-module resolution.
    pub translation: TranslationRequest,
    /// Source edit limits and modified-body protection; CRCs use best effort.
    pub editing: RewriteOptions,
}
/// Stage-separated replacement report; it never certifies a checked model.
#[derive(Clone, Debug)]
pub struct RewriteReport {
    document: Value,
    status: u8,
}
impl RewriteReport {
    /// Whether native generation and resulting module admission both succeeded.
    #[must_use]
    pub const fn accepted(&self) -> bool {
        self.status == 0
    }
    /// Zero replacement, one rejection, three unsupported/resource, four bug.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        self.status
    }
    /// Complete replacement/provenance/limits/qualification document.
    #[must_use]
    pub fn to_json(&self) -> Value {
        self.document.clone()
    }
}
// Describe emitted replacement bytes, rather than predicting the editor policy.
fn replacement_newlines(replacement: &str) -> &'static str {
    match replacement
        .find(['\r', '\n'])
        .map(|at| &replacement.as_bytes()[at..])
    {
        Some([b'\r', b'\n', ..]) => "crlf",
        Some([b'\r', ..]) => "cr",
        _ => "lf",
    }
}

fn effective_editing(request: &RewriteRequest) -> RewriteOptions {
    let mut editing = request.editing.clone();
    if request.translation.compat_tlc {
        // Ordinary pinned pcal.trans uses its Noop overwrite callback.
        editing.normalize_newlines = true;
        editing.replace_modified = true;
    }
    editing
}
fn report(
    file: &str,
    request: &RewriteRequest,
    status: u8,
    stage: &str,
    details: Value,
) -> RewriteReport {
    let mut document = json!({"schema_version":crate::REPORT_SCHEMA_VERSION,"command":"translate-rewrite","file":file,
        "result":match status {0=>"replacement",1=>"rejected",3=>"unsupported",_=>"internal-error"},
        "stage":stage,"accepted":status==0,"exit_code":status,
        "profile":if request.translation.compat_tlc {"tlc"} else {"dive"},
        "editing":effective_editing(request),"requested_editing":request.editing,"requested_translation_options":request.translation.limits});
    if let (Some(object), Value::Object(extra)) = (document.as_object_mut(), details) {
        object.extend(extra);
    }
    RewriteReport { document, status }
}
fn failure(
    file: &str,
    request: &RewriteRequest,
    stage: &str,
    diagnostic: &Diagnostic,
) -> RewriteReport {
    let status = match diagnostic.code {
        Code::Unsupported | Code::Resource => 3,
        Code::Internal => 4,
        _ => 1,
    };
    report(
        file,
        request,
        status,
        stage,
        json!({"code":diagnostic.code.as_str(),"diagnostic":diagnostic}),
    )
}
/// Generate, replace and validate a module in memory. Nothing is written to disk.
/// An unavailable algorithm CRC disables only that value; body CRCs remain real.
/// Existing unverified/modified bodies require an explicit replacement grant.
///
/// ```
/// use dive_api::{ChecksumPolicy, RewriteOptions, RewriteRequest, rewrite_source};
/// let source = "---- MODULE R ----\nEXTENDS Integers\n(* --algorithm A variable x=0; begin L:x:=1; E:skip; end algorithm; *)\n====\n";
/// let request = RewriteRequest {
///     editing: RewriteOptions { checksums: ChecksumPolicy::TranslationOnly, ..RewriteOptions::default() },
///     ..RewriteRequest::default()
/// };
/// let report = rewrite_source("R.tla", source, &request);
/// assert!(report.accepted());
/// assert!(report.to_json()["replacement"]["translation_checksum"].is_string());
/// ```
#[must_use]
pub fn rewrite_source(file: &str, source: &str, request: &RewriteRequest) -> RewriteReport {
    let selection = match dive_pluscal::rewrite::select_rewrite(source, &request.editing) {
        Ok(selection) => selection,
        Err(error) => return failure(file, request, "selection", &error),
    };
    let context =
        match dive_pluscal::resolve_translation_options(source, &request.translation.limits) {
            Ok(context) => context,
            Err(error) => return failure(file, request, "source-options", &error.diagnostic),
        };
    if let (Some(previous), Some(declaration)) = (selection.previous, &context.source_declaration)
        && declaration.span.start < previous.end
        && declaration.span.end > previous.start
    {
        return failure(file, request, "source-options", &Diagnostic {
                code: Code::Unsupported,
                span: dive_pluscal::source_option_declaration_span(source)
                    .unwrap_or(declaration.span),
                message: "unsupported PlusCal source option declaration inside previous translation; masking would change the deciding context".into(),
            });
    }
    let masked = dive_pluscal::rewrite::mask_previous(source, selection.previous);
    let generated = crate::translate::translate_source_at_insertion(
        file,
        &masked,
        &request.translation,
        selection.previous.map(|previous| previous.start),
    );
    if !generated.accepted() {
        return report(
            file,
            request,
            generated.exit_code(),
            "native-generation",
            json!({"generation":generated.to_json()}),
        );
    }
    let generated = generated.to_json();
    let Some(body) = generated["artifact"]["tla"].as_str() else {
        return report(
            file,
            request,
            4,
            "native-generation",
            json!({"diagnostic":{"code":"DIVE-P0050","message":"accepted generator omitted body"}}),
        );
    };
    let editing = effective_editing(request);
    let plan = match dive_pluscal::rewrite::rewrite_document_with_translation_options(
        source,
        body,
        &editing,
        &request.translation.limits,
    ) {
        Ok(plan) => plan,
        Err(error) => return failure(file, request, "replacement", &error),
    };
    let analysis = analyze_source(file, &plan.module_source, &request.translation.analyze);
    if !analysis.accepted() {
        return report(
            file,
            request,
            1,
            "replacement-analysis",
            json!({"analysis":analysis.to_json(),"replacement_returned":false}),
        );
    }
    let mut filesystem_plan = generated["filesystem_plan"].clone();
    filesystem_plan["source_newlines"] = json!(if editing.normalize_newlines {
        "normalized-lf"
    } else {
        "preserved"
    });
    filesystem_plan["body_newlines"] = json!(replacement_newlines(&plan.replacement));
    report(
        file,
        request,
        0,
        "replacement-analysis",
        json!({"translation_options":context.effective,"source_options":context.source_declaration,"replacement":plan,"artifact":generated["artifact"],"analysis":analysis.to_json(),"filesystem_plan":filesystem_plan}),
    )
}
/// Read a bounded UTF-8 module and return its in-memory replacement plan.
///
/// # Errors
/// An I/O API error on unreadable/non-UTF-8 source. Stage failures are reports.
pub fn rewrite_file(path: &Path, request: &RewriteRequest) -> Result<RewriteReport, ApiError> {
    let file = path.display().to_string();
    let reader = std::fs::File::open(path).map_err(|error| ApiError::Io {
        path: file.clone(),
        error,
    })?;
    let limit = request.editing.max_bytes.min(4_194_304);
    let mut bytes = Vec::new();
    reader
        .take(u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| ApiError::Io {
            path: file.clone(),
            error,
        })?;
    if bytes.len() > limit {
        return Ok(failure(
            &file,
            request,
            "selection",
            &Diagnostic {
                code: Code::Resource,
                span: dive_pluscal::ast::Span::default(),
                message: "PlusCal rewrite source byte limit reached".to_owned(),
            },
        ));
    }
    let source = String::from_utf8(bytes).map_err(|error| ApiError::Io {
        path: file.clone(),
        error: std::io::Error::new(std::io::ErrorKind::InvalidData, error),
    })?;
    Ok(rewrite_source(&file, &source, request))
}
