//! Shared source option context matching pinned `trans`/`ParseAlgorithm`.
use super::{FairnessOption, TranslationError, TranslationOptions, TranslationStage, error};
use crate::{Code, ast::Span};
use serde::{Deserialize, Serialize};

/// One original source option word, including its exact UTF-8 extent.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceOptionArgument {
    /// Pinned unquoted option spelling.
    pub text: String,
    /// Original source bytes; tabs/newlines are not replaced in this span.
    pub span: Span,
}
/// First deciding reference-recognized declaration; later declarations are ignored.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceOptionDeclaration {
    /// Original `PlusCal` through closing `)` extent.
    pub span: Span,
    /// Ordered original argument words (commas are separators).
    pub arguments: Vec<SourceOptionArgument>,
}
/// Requested settings and the resolved source-option context.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TranslationContext {
    /// Explicit CLI/API settings before source processing.
    pub requested: TranslationOptions,
    /// Settings actually used for normalization/generation/CRC.
    pub effective: TranslationOptions,
    /// First deciding source declaration, if present.
    pub source_declaration: Option<SourceOptionDeclaration>,
}
impl TranslationContext {
    pub(super) fn explicit(options: &TranslationOptions) -> Self {
        Self {
            requested: options.clone(),
            effective: options.clone(),
            source_declaration: None,
        }
    }
}
fn whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}
fn skip_space(bytes: &[u8], mut cursor: usize) -> usize {
    while bytes.get(cursor).is_some_and(|b| whitespace(*b)) {
        cursor += 1;
    }
    cursor
}
fn word_end(bytes: &[u8], mut cursor: usize) -> usize {
    while bytes.get(cursor).is_some_and(|b| !whitespace(*b)) {
        cursor += 1;
    }
    cursor
}
/// Locates the first deciding source declaration using the reference whole-file scan.
/// This performs no comment/string removal. It is a context location, not parsing
/// or artifact admission; malformed declarations are returned too.
#[must_use]
pub fn source_options_span(source: &str) -> Option<Span> {
    let bytes = source.as_bytes();
    let mut cursor = skip_space(bytes, 0);
    while cursor < bytes.len() {
        let end = word_end(bytes, cursor);
        if source[cursor..].starts_with("PlusCal") {
            let suffix = source[cursor + 7..].chars().next();
            if !suffix.is_some_and(|c| c.is_alphabetic() || c == '_') {
                let next = skip_space(bytes, end);
                if source[next..].starts_with("options")
                    && !source[next + 7..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_alphanumeric() || c == '_')
                {
                    return Some(Span {
                        start: cursor,
                        end: next + 7,
                    });
                }
            }
        }
        cursor = skip_space(bytes, end);
    }
    None
}
fn refused(span: Span, message: &str) -> TranslationError {
    error(
        TranslationStage::Normalization,
        Code::Unsupported,
        span,
        format!("unsupported PlusCal source options: {message}"),
    )
}
/// Resolves supported source translator options before normalization.
/// First source fairness overrides explicit fairness. No malformed, unknown,
/// conflicting or partially applied declaration produces effective settings.
///
/// # Errors
/// Named unsupported contexts or a bounded source/argument resource diagnostic.
#[allow(
    clippy::too_many_lines,
    reason = "reference ordering and fail-closed partial-option admission remain explicit"
)]
pub fn resolve_translation_options(
    source: &str,
    requested: &TranslationOptions,
) -> Result<TranslationContext, TranslationError> {
    if source.len() > 4_194_304 {
        return Err(error(
            TranslationStage::Normalization,
            Code::Resource,
            Span::default(),
            "PlusCal source-option scan exceeds 4 MiB",
        ));
    }
    // Java tests UTF-16 Character.isLetter/Character.isDigit; Rust's Unicode
    // Alphabetic property is broader. Until that boundary is ported, reject
    // adjacent non-ASCII keyword suffixes instead of silently missing a context.
    for (start, word) in source.char_indices().filter(|(start, _)| {
        *start == 0
            || source
                .as_bytes()
                .get(start - 1)
                .is_some_and(|b| whitespace(*b))
    }) {
        let _ = word;
        for keyword in ["PlusCal", "options"] {
            if let Some(tail) = source[start..].strip_prefix(keyword)
                && tail
                    .chars()
                    .next()
                    .is_some_and(|character| !character.is_ascii())
            {
                return Err(refused(
                    Span {
                        start,
                        end: start + keyword.len(),
                    },
                    "unproved Unicode keyword boundary",
                ));
            }
        }
    }
    let mut context = TranslationContext::explicit(requested);
    finish_options(&mut context.effective)?;
    let Some(header) = source_options_span(source) else {
        return Ok(context);
    };
    let bytes = source.as_bytes();
    let mut cursor = skip_space(bytes, header.end);
    if bytes.get(cursor) != Some(&b'(') {
        return Err(refused(header, "declaration missing opening parenthesis"));
    }
    cursor += 1;
    let mut arguments = Vec::new();
    loop {
        cursor = skip_space(bytes, cursor);
        match bytes.get(cursor) {
            Some(b')') => {
                cursor += 1;
                break;
            }
            Some(b',') => {
                cursor += 1;
                continue;
            }
            None => return Err(refused(header, "declaration missing closing parenthesis")),
            Some(_) => {}
        }
        let start = cursor;
        while bytes
            .get(cursor)
            .is_some_and(|b| !whitespace(*b) && !matches!(*b, b',' | b')'))
        {
            cursor += 1;
        }
        let span = Span { start, end: cursor };
        if arguments.len() >= 128 || span.end - span.start > 256 {
            return Err(error(
                TranslationStage::Normalization,
                Code::Resource,
                span,
                "PlusCal source-option argument budget reached",
            ));
        }
        arguments.push(SourceOptionArgument {
            text: source[start..cursor].to_owned(),
            span,
        });
    }
    let mut fairness = None;
    let mut index = 0;
    while let Some(argument) = arguments.get(index) {
        let setting = match argument.text.as_str() {
            "-nof" | "nof" => Some(FairnessOption::None),
            "-wf" | "wf" => Some(FairnessOption::Weak),
            "-sf" | "sf" => Some(FairnessOption::Strong),
            "-wfNext" | "wfNext" => Some(FairnessOption::Next),
            "-label" | "label" => {
                context.effective.insert_missing_labels = true;
                None
            }
            "-termination" | "termination" => {
                context.effective.check_termination = true;
                None
            }
            "-noDoneDisjunct" | "noDoneDisjunct" => {
                context.effective.no_done_disjunct = true;
                None
            }
            "-noold" => {
                context.effective.no_old = true;
                None
            }
            "-nocfg" => {
                context.effective.no_configuration = true;
                None
            }
            "-lineWidth" => {
                index += 1;
                let width = arguments
                    .get(index)
                    .ok_or_else(|| refused(argument.span, "lineWidth argument missing"))?;
                context.effective.line_width = width
                    .text
                    .parse::<i32>()
                    .ok()
                    .filter(|n| *n >= 60)
                    .ok_or_else(|| {
                        refused(width.span, "lineWidth expects an i32 value at least 60")
                    })?;
                None
            }
            "-version" | "version" => {
                index += 1;
                let version = arguments
                    .get(index)
                    .ok_or_else(|| refused(argument.span, "version argument missing"))?;
                version_weight(Some(&version.text)).map_err(|mut e| {
                    e.diagnostic.span = version.span;
                    e
                })?;
                context.effective.version = Some(version.text.clone());
                None
            }
            "-spec" | "spec" | "-myspec" | "myspec" => {
                return Err(refused(
                    argument.span,
                    &format!("{} specification-backed translator backend", argument.text),
                ));
            }
            "-labelRoot" | "labelRoot" => {
                index += 1;
                let root = arguments
                    .get(index)
                    .ok_or_else(|| refused(argument.span, "label root argument missing"))?;
                if !root
                    .text
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphabetic)
                    || !root
                        .text
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_')
                {
                    return Err(refused(root.span, "unproved or invalid label root"));
                }
                context.effective.label_root.clone_from(&root.text);
                None
            }
            _ => {
                return Err(refused(
                    argument.span,
                    &format!("unknown or unproved option argument {}", argument.text),
                ));
            }
        };
        if let Some(setting) = setting {
            if fairness.replace(setting).is_some() {
                return Err(refused(
                    argument.span,
                    "repeated/conflicting fairness has partial reference application",
                ));
            }
            context.effective.fairness = setting;
        }
        index += 1;
    }
    finish_options(&mut context.effective)?;
    context.source_declaration = Some(SourceOptionDeclaration {
        span: Span {
            start: header.start,
            end: cursor,
        },
        arguments,
    });
    Ok(context)
}

/// Pinned `PcalParams.VersionToNumber`: decimal thousandths, not semver/f64.
/// Java i32 arithmetic wraps; multiple dots merely set the decimal flag.
pub(super) fn version_weight(version: Option<&str>) -> Result<i32, TranslationError> {
    let Some(version) = version else {
        return Ok(1903);
    };
    if version.len() > 256 {
        return Err(error(
            TranslationStage::Normalization,
            Code::Resource,
            Span::default(),
            "PlusCal version argument exceeds 256 bytes",
        ));
    }
    let mut value = 0_i32;
    let mut decimal = false;
    let mut digits = 0;
    for byte in version.bytes() {
        match byte {
            b'0'..=b'9' => {
                value = value.wrapping_mul(10).wrapping_add(i32::from(byte - b'0'));
                if decimal {
                    digits += 1;
                }
                if digits > 3 {
                    return Err(refused(Span::default(), "illegal version number"));
                }
            }
            b'.' => decimal = true,
            _ => return Err(refused(Span::default(), "illegal version number")),
        }
    }
    for _ in digits..3 {
        value = value.wrapping_mul(10);
    }
    if !(0..=1903).contains(&value) {
        return Err(refused(
            Span::default(),
            "illegal or later-than-current version number",
        ));
    }
    Ok(value)
}

fn finish_options(options: &mut TranslationOptions) -> Result<(), TranslationError> {
    if options.line_width < 60 {
        return Err(refused(
            Span::default(),
            "lineWidth expects an i32 value at least 60",
        ));
    }
    options.parser.input_version = version_weight(options.version.as_deref())?;
    if let Some((mode, _)) = &options.specification_backend {
        return Err(refused(
            Span::default(),
            &format!("{mode} specification-backed translator backend"),
        ));
    }
    if options.write_ast {
        return Err(refused(
            Span::default(),
            "writeAST filesystem operation; use --ast or original_ast_at for in-memory original bytes",
        ));
    }
    if options.report_labels {
        options.insert_missing_labels = true;
    }
    if options.check_termination && options.fairness == FairnessOption::Declared {
        options.fairness = FairnessOption::Weak;
    }
    Ok(())
}

/// Older process/procedure AST fields have not been measured by this delta.
pub(super) fn admit_version_shape(
    parsed: &crate::Parsed,
    weight: i32,
) -> Result<(), TranslationError> {
    if weight < 1500
        && (!parsed.algorithm.procedures.is_empty()
            || matches!(parsed.algorithm.body, crate::ast::Body::Processes(_)))
    {
        return Err(refused(
            parsed.algorithm.span,
            "pre-1.5 process/procedure AST and translation",
        ));
    }
    Ok(())
}
