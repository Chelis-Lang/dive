//! Original-stage PlusCal AST artifacts, separate from translation admission.
use crate::{ApiError, TranslationRequest};
use dive_pluscal::{Code, Diagnostic};
use serde_json::{Value, json};
use std::{io::Read as _, path::Path};

/// An in-memory original AST result; it never admits a generated specification.
#[derive(Clone, Debug)]
pub struct OriginalAstReport {
    document: Value,
    status: u8,
}
impl OriginalAstReport {
    /// Whether exact original AST bytes are available in the report.
    #[must_use]
    pub const fn artifact_available(&self) -> bool {
        self.status == 0
    }
    /// Zero artifact, one source rejection, three unsupported/resource, four bug.
    /// This is the native in-memory operation's exit, not `pcal.trans -writeAST`.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        self.status
    }
    /// Stage, artifact, option context and explicit non-admission qualifications.
    #[must_use]
    pub fn to_json(&self) -> Value {
        self.document.clone()
    }
}
fn original_artifact_json(artifact: &dive_pluscal::OriginalAstArtifact) -> Value {
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
    details: Value,
) -> OriginalAstReport {
    let mut document = json!({"schema_version":2,"command":"original-ast","file":file,
        "profile":if request.compat_tlc {"tlc"} else {"dive"},"exit_code":status,
        "stage":"original-ast","result":match status { 0=>"ast-artifact", 1=>"rejected", 3=>"unsupported", _=>"internal-error" },
        "artifact_available":status==0,"accepted":false,
        "generated_module_accepted":false,
        "writes_performed":false,"requested_translation_options":request.limits});
    if let (Some(object), Value::Object(extra)) = (document.as_object_mut(), details) {
        object.extend(extra);
    }
    OriginalAstReport { document, status }
}
fn failure(file: &str, request: &TranslationRequest, diagnostic: &Diagnostic) -> OriginalAstReport {
    let status = match diagnostic.code {
        Code::Unsupported | Code::Resource => 3,
        Code::Internal => 4,
        _ => 1,
    };
    report(
        file,
        request,
        status,
        json!({"code":diagnostic.code.as_str(),"diagnostic":diagnostic}),
    )
}

/// Returns normalized original AST bytes for one algorithm in a module.
/// This explicit in-memory operation neither translates nor writes `AST.tla`.
/// It therefore performs no generated-module analysis or model checking.
/// Modern procedure/macro ownership and compound source locations use genuine
/// pre-generation controls; historical fields and full original-stage coverage
/// remain unfinished. No original-stage header checksum is advertised.
///
/// ```
/// let source = "---- MODULE A ----\n(* --algorithm A begin L:skip; end algorithm *)\n====\n";
/// let report = dive_api::original_ast_source("A.tla", source, &dive_api::TranslationRequest::default());
/// assert!(report.artifact_available());
/// assert_eq!(report.to_json()["accepted"], false);
/// ```
#[must_use]
pub fn original_ast_source(
    file: &str,
    source: &str,
    request: &TranslationRequest,
) -> OriginalAstReport {
    if source.len() > 4_194_304 {
        return failure(
            file,
            request,
            &Diagnostic {
                code: Code::Resource,
                span: dive_pluscal::ast::Span::default(),
                message: "PlusCal module source exceeds 4 MiB".into(),
            },
        );
    }
    let context = match dive_pluscal::resolve_translation_options(source, &request.limits) {
        Ok(context) => context,
        Err(error) => return failure(file, request, &error.diagnostic),
    };
    let selection = match dive_pluscal::rewrite::select_rewrite_with_parser(
        source,
        &dive_pluscal::rewrite::RewriteOptions::default(),
        &context.effective.parser,
    ) {
        Ok(selection) => selection,
        Err(diagnostic) => return failure(file, request, &diagnostic),
    };
    if let (Some(previous), Some(declaration)) = (selection.previous, &context.source_declaration)
        && declaration.span.start < previous.end
        && declaration.span.end > previous.start
    {
        return failure(file, request, &Diagnostic { code:Code::Unsupported, span:declaration.span,
            message:"unsupported PlusCal source option declaration inside previous translation; masking would change the deciding context".into() });
    }
    // Shared ownership uses the resolved grammar and limits. A malformed first
    // algorithm retains its raw candidate and the deciding strict parser error.
    let region = selection.algorithm;
    match dive_pluscal::original_ast_at(source, region.start..region.end, &request.limits) {
        Ok(artifact) => report(
            file,
            request,
            0,
            json!({"translation_options":artifact.option_context.effective,
            "source_options":artifact.option_context.source_declaration,"ast_artifact":original_artifact_json(&artifact),
            "artifact_stage":"normalized-original-before-name-fixing"}),
        ),
        Err(error) => failure(file, request, &error.diagnostic),
    }
}

/// Reads bounded module source and returns its original AST entirely in memory.
///
/// # Errors
/// An I/O API error if input cannot be read as UTF-8; stage failures are reports.
pub fn original_ast_file(
    path: &Path,
    request: &TranslationRequest,
) -> Result<OriginalAstReport, ApiError> {
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
        return Ok(failure(
            &file,
            request,
            &Diagnostic {
                code: Code::Resource,
                span: dive_pluscal::ast::Span::default(),
                message: "PlusCal module source exceeds 4 MiB".into(),
            },
        ));
    }
    let source = String::from_utf8(bytes).map_err(|error| ApiError::Io {
        path: file.clone(),
        error: std::io::Error::new(std::io::ErrorKind::InvalidData, error),
    })?;
    Ok(original_ast_source(&file, &source, request))
}
