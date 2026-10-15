//! Bounded translation-block editing and pinned line-concatenation CRC32.
use crate::{Code, Diagnostic, ast::Span};
use serde::{Deserialize, Serialize};

/// Rewriting always computes the body CRC and uses an optional exact algorithm CRC.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChecksumPolicy {
    /// Keep an exact algorithm CRC when available; otherwise disable only that
    /// value using the pinned `\\in STRING` spelling and retain the body CRC.
    #[default]
    #[serde(alias = "Full", alias = "TranslationOnly")]
    BestEffort,
}
impl ChecksumPolicy {
    /// Compatibility spelling for the unified best-effort policy.
    #[allow(
        non_upper_case_globals,
        reason = "preserve the former public API spelling"
    )]
    pub const Full: Self = Self::BestEffort;
    /// Compatibility spelling; no longer a separate forced body-only policy.
    #[allow(
        non_upper_case_globals,
        reason = "preserve the former public API spelling"
    )]
    pub const TranslationOnly: Self = Self::BestEffort;
}
/// In-memory replacement limits and explicit overwrite policy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RewriteOptions {
    /// Unified best-effort checksum policy; old input spellings remain accepted.
    pub checksums: ChecksumPolicy,
    /// Maximum source and rewritten document bytes, checked before construction.
    pub max_bytes: usize,
    /// Permit replacing an unverified or modified existing body.
    pub replace_modified: bool,
    /// Normalize all document line endings to LF, matching pinned Unix output.
    /// Otherwise unchanged source bytes and the source's first newline are kept.
    pub normalize_newlines: bool,
}
impl Default for RewriteOptions {
    fn default() -> Self {
        Self {
            checksums: ChecksumPolicy::BestEffort,
            max_bytes: 4_194_304,
            replace_modified: false,
            normalize_newlines: false,
        }
    }
}
/// Recorded body/algorithm CRC evidence, not source identity or a proof.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreviousChecksum {
    /// Recorded hexadecimal CRC, if the recognized header supplies one.
    pub recorded: Option<String>,
    /// Recorded algorithm CRC; rewriting does not use it as an edit gate.
    pub algorithm_recorded: Option<String>,
    /// Whether a recognized old header deliberately disables algorithm CRC.
    pub algorithm_disabled: bool,
    /// CRC of the original body lines, without newline bytes.
    pub computed: String,
    /// Exact case-sensitive match, as in pinned Java; absence is unverified.
    pub matched: bool,
}
/// Original source spans selected by bounded document editing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RewriteSelection {
    /// Complete algorithm block-comment span.
    pub comment: Span,
    /// Algorithm parse region, excluding the outer comment terminator.
    pub algorithm: Span,
    /// Old translation section including both marker lines, if present.
    pub previous: Option<Span>,
    /// Body between old marker lines, if present.
    pub previous_body: Option<Span>,
    /// Old body checksum evidence; missing/disabled checksums are not verified.
    pub checksum: Option<PreviousChecksum>,
    /// Insertion point immediately after the algorithm comment's complete line.
    pub insertion: usize,
}
/// A source replacement, independently of body generation or semantic admission.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RewritePlan {
    /// Exact original byte interval to replace.
    pub span: Span,
    /// Replacement text using the selected newline policy.
    pub replacement: String,
    /// Complete resulting document; this does not itself mean TLA acceptance.
    pub module_source: String,
    /// CRC of generated body lines, excluding line-ending bytes.
    pub translation_checksum: String,
    /// Exact algorithm CRC when its serializer supports the source.
    pub algorithm_checksum: Option<String>,
    /// Nonfatal checksum unavailability; generation and body integrity remain independent.
    pub warnings: Vec<Diagnostic>,
    /// Selection and previous body verification evidence.
    pub selection: RewriteSelection,
    /// Whether unchanged source newline bytes were normalized.
    pub newlines_normalized: bool,
}
fn failure(code: Code, span: Span, message: &str) -> Diagnostic {
    Diagnostic {
        code,
        span,
        message: message.to_owned(),
    }
}
#[derive(Clone, Copy)]
struct Line<'a> {
    start: usize,
    end: usize,
    text: &'a str,
}
fn lines(source: &str) -> Vec<Line<'_>> {
    let mut result = Vec::new();
    let mut start = 0;
    let bytes = source.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] == b'\r' || bytes[cursor] == b'\n' {
            let end_text = cursor;
            cursor += 1;
            if bytes[end_text] == b'\r' && bytes.get(cursor) == Some(&b'\n') {
                cursor += 1;
            }
            result.push(Line {
                start,
                end: cursor,
                text: &source[start..end_text],
            });
            start = cursor;
        } else {
            cursor += 1;
        }
    }
    if start < source.len() {
        result.push(Line {
            start,
            end: source.len(),
            text: &source[start..],
        });
    }
    result
}
/// CRC32 matching pinned `Validator.checksum(Vector<String>)`: UTF-8 lines are
/// concatenated without any LF/CR separator; the hexadecimal result is unpadded.
/// This integrity checksum is not a cryptographic hash or semantic validation.
#[must_use]
pub fn translation_checksum(body: &str) -> String {
    crc32_iter(lines(body).iter().flat_map(|line| line.text.bytes()))
}
pub(crate) fn crc32(bytes: &[u8]) -> String {
    crc32_iter(bytes.iter().copied())
}
fn crc32_iter(bytes: impl Iterator<Item = u8>) -> String {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320_u32 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    format!("{:x}", !crc)
}
fn marker(line: &str, which: &str) -> bool {
    let mut words = line.trim_start_matches([' ', '\t']).split_whitespace();
    words.next() == Some("\\*")
        && words.next() == Some(which)
        && words.next() == Some("TRANSLATION")
}
// The recognized partial contract is deliberately narrower than arbitrary Java
// regex matches. Algorithm evidence is compared only by a full AST request.
struct Header<'a> {
    algorithm: Option<&'a str>,
    translation: Option<&'a str>,
    algorithm_value: Span,
    translation_value: Span,
    translation_hex: Option<Span>,
}
fn whitespace(text: &str) -> &str {
    text.trim_start_matches(|c: char| c.is_ascii_whitespace())
}
fn checksum_value(text: &str) -> Option<(Option<&str>, &str)> {
    let text = whitespace(text);
    if let Some(text) = text.strip_prefix("\\in") {
        return Some((None, whitespace(text).strip_prefix("STRING")?));
    }
    let text = whitespace(text.strip_prefix('=')?).strip_prefix('"')?;
    let end = text.find('"')?;
    let value = &text[..end];
    if !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some((Some(value), &text[end + 1..]))
}
fn field<'a>(text: &'a str, names: &[&str]) -> Option<&'a str> {
    for name in names {
        if text
            .get(..name.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(name))
        {
            return Some(&text[name.len()..]);
        }
    }
    None
}
fn checksum_header(header: &str) -> Option<Header<'_>> {
    let start = header.find("\\* BEGIN TRANSLATION")? + "\\* BEGIN TRANSLATION".len();
    let text = &header[start..];
    if text.len() == whitespace(text).len() {
        return None;
    }
    let text = whitespace(whitespace(text).strip_prefix('(')?);
    let text = field(
        text,
        &[
            "chksum(pcal)",
            "checksum(pcal)",
            "chksum(pluscal)",
            "checksum(pluscal)",
        ],
    )?;
    let algorithm_value_start = header.len() - whitespace(text).len();
    let (algorithm, text) = checksum_value(text)?;
    let algorithm_value = Span {
        start: algorithm_value_start,
        end: header.len() - text.len(),
    };
    let text = whitespace(whitespace(text).strip_prefix("/\\")?);
    let text = field(
        text,
        &[
            "chksum(tla)",
            "checksum(tla)",
            "chksum(tla+)",
            "checksum(tla+)",
        ],
    )?;
    let translation_value_start = header.len() - whitespace(text).len();
    let (translation, text) = checksum_value(text)?;
    let translation_value = Span {
        start: translation_value_start,
        end: header.len() - text.len(),
    };
    let translation_hex = translation.map(|value| Span {
        start: translation_value.end - value.len() - 1,
        end: translation_value.end - 1,
    });
    whitespace(text).strip_prefix(')')?;
    Some(Header {
        algorithm,
        translation,
        algorithm_value,
        translation_value,
        translation_hex,
    })
}
/// Select the algorithm region inside an already identified block comment.
/// Pinned translation ignores trailing ASCII-star comment decoration. The
/// enclosing comment and original source remain untouched; arbitrary trailing
/// prose is not removed. This selects bytes, without grammar or artifact admission.
/// Standalone parsing still requires every algorithm token to be consumed.
#[must_use]
pub fn algorithm_comment_region(source: &str, comment: Span) -> Option<Span> {
    let text = source.get(comment.start..comment.end)?;
    if !text.starts_with("(*") || !text.ends_with("*)") {
        return None;
    }
    let marker = algorithm_marker(text)?;
    let start = comment.start + marker;
    let mut end = comment.end.checked_sub(2)?;
    let content = source.get(start..end)?;
    let trimmed = content.trim_end_matches(['*', ' ', '\t', '\r', '\n']);
    if content.get(trimmed.len()..)?.contains('*') {
        end = start + trimmed.len();
    }
    Some(Span { start, end })
}

fn algorithm_marker(text: &str) -> Option<usize> {
    // trans.java scans lines in order and prefers --algorithm over --fair
    // only when both occur on that same line. Parsing validates the selected
    // marker; in particular, whitespace may separate --fair and algorithm.
    let mut offset = 0;
    text.split_inclusive(['\r', '\n']).find_map(|line| {
        let marker = line.find("--algorithm").or_else(|| line.find("--fair"));
        let start = offset;
        offset += line.len();
        marker.map(|column| start + column)
    })
}

/// Select only a completely parsed algorithm inside an identified comment.
/// The exact consumed C/P grammar end determines ownership; ordinary trailing
/// prose stays in the original comment but is not algorithm input. Unterminated
/// quoted/comment lexical state and resource limits remain deciding errors.
/// Standalone `parse_at` retains its strict EOF contract.
///
/// # Errors
/// Invalid comment selection or bounded source/lexical/grammar diagnostics.
pub fn owned_algorithm_comment_region(
    source: &str,
    comment: Span,
    limits: &crate::Options,
) -> Result<Span, Diagnostic> {
    let region = algorithm_comment_region(source, comment).ok_or_else(|| {
        // An invalid caller range must still yield a valid UTF-8 diagnostic.
        let boundary = |offset: usize| {
            let mut offset = offset.min(source.len());
            while !source.is_char_boundary(offset) {
                offset -= 1;
            }
            offset
        };
        let start = boundary(comment.start);
        failure(
            Code::Syntax,
            Span {
                start,
                end: boundary(comment.end).max(start),
            },
            "invalid algorithm comment region",
        )
    })?;
    let parsed = crate::parser::parse_prefix_at(source, region.start..region.end, limits)?;
    Ok(parsed.algorithm.span)
}

/// Selects one outer algorithm comment and at most one following marker pair.
/// Unknown/ambiguous marker layouts yield named diagnostics, not guessed edits.
///
/// # Errors
/// Syntax/unsupported/resource diagnostics with exact source byte spans.
pub fn select_rewrite(
    source: &str,
    options: &RewriteOptions,
) -> Result<RewriteSelection, Diagnostic> {
    select_rewrite_with_parser(
        source,
        options,
        &crate::Options {
            max_bytes: options.max_bytes,
            ..crate::Options::default()
        },
    )
}

/// Select a source replacement using the resolved input grammar and parser bounds.
/// Callers resolve options from the original whole file before masking a previous
/// translation. Syntax failures retain the raw candidate for downstream admission;
/// a successfully parsed first algorithm owns only its consumed prefix.
///
/// # Errors
/// Syntax/unsupported/resource diagnostics with exact source byte spans.
#[allow(
    clippy::too_many_lines,
    reason = "ordered line/lexical selection retains deciding source spans"
)]
pub fn select_rewrite_with_parser(
    source: &str,
    options: &RewriteOptions,
    parser: &crate::Options,
) -> Result<RewriteSelection, Diagnostic> {
    if source.len() > options.max_bytes {
        return Err(failure(
            Code::Resource,
            Span::default(),
            "PlusCal rewrite source byte limit reached",
        ));
    }
    let source_lines = lines(source);
    for line in &source_lines {
        for keyword in ["BEGIN", "END"] {
            if let Some(index) = line.text.find(keyword)
                && line.text[index + keyword.len()..]
                    .trim_start_matches([' ', '\t'])
                    .starts_with("TRANSLATION")
                && !marker(line.text, keyword)
            {
                return Err(failure(
                    Code::Unsupported,
                    Span {
                        start: line.start,
                        end: line.end,
                    },
                    "translation-marker text outside a proved line-comment marker",
                ));
            }
        }
    }

    let begins: Vec<_> = source_lines
        .iter()
        .enumerate()
        .filter(|(_, l)| marker(l.text, "BEGIN"))
        .map(|(i, _)| i)
        .collect();
    let ends: Vec<_> = source_lines
        .iter()
        .enumerate()
        .filter(|(_, l)| marker(l.text, "END"))
        .map(|(i, _)| i)
        .collect();
    if begins.len() > 1 || ends.len() > 1 {
        return Err(failure(
            Code::Unsupported,
            Span::default(),
            "ambiguous multiple translation markers",
        ));
    }
    let (previous, previous_body, checksum) = match (begins.first(), ends.first()) {
        (None, None) => (None, None, None),
        (Some(&begin), Some(&end)) if begin < end => {
            let body = Span {
                start: source_lines[begin].end,
                end: source_lines[end].start,
            };
            let computed = translation_checksum(&source[body.start..body.end]);
            let old_header = checksum_header(source_lines[begin].text);
            let recorded = old_header
                .as_ref()
                .and_then(|h| h.translation)
                .map(str::to_owned);
            let algorithm_recorded = old_header
                .as_ref()
                .and_then(|h| h.algorithm)
                .map(str::to_owned);
            let algorithm_disabled = old_header.as_ref().is_some_and(|h| h.algorithm.is_none());
            let matched = recorded.as_ref() == Some(&computed);
            (
                Some(Span {
                    start: source_lines[begin].start,
                    end: source_lines[end].end,
                }),
                Some(body),
                Some(PreviousChecksum {
                    recorded,
                    algorithm_recorded,
                    algorithm_disabled,
                    computed,
                    matched,
                }),
            )
        }
        _ => {
            return Err(failure(
                Code::Syntax,
                Span::default(),
                "missing or reversed translation marker pair",
            ));
        }
    };
    let masked = mask_previous(source, previous);
    let mut selected = None;
    let mut module_end = None;
    for token in dive_syntax::tokens::tokenize_spec(&masked) {
        let start = usize::from(token.range.start());
        let end = usize::from(token.range.end());
        if token.kind == dive_syntax::SyntaxKind::END_MODULE {
            module_end = Some(start);
        }
        if token.kind != dive_syntax::SyntaxKind::BLOCK_COMMENT {
            continue;
        }
        let text = &source[start..end];
        let offset = algorithm_marker(text);
        if offset.is_some() {
            if token.error.is_some() {
                return Err(failure(
                    Code::Lexical,
                    Span { start, end },
                    "unterminated algorithm comment",
                ));
            }
            let comment = Span { start, end };
            let candidate = algorithm_comment_region(source, comment).ok_or_else(|| {
                failure(Code::Syntax, comment, "invalid algorithm comment region")
            })?;
            // Low-level source selection is not grammar/artifact admission.
            // Refine only a proved complete prefix; malformed grammar keeps
            // its raw region and the strict downstream deciding diagnostic.
            let region = match owned_algorithm_comment_region(source, comment, parser) {
                Ok(owned) => owned,
                Err(error) if matches!(error.code, Code::Resource | Code::Internal) => {
                    return Err(error);
                }
                Err(_) => candidate,
            };
            if selected.replace((comment, region)).is_some() {
                return Err(failure(
                    Code::Unsupported,
                    Span { start, end },
                    "multiple algorithm comments",
                ));
            }
        }
    }
    let (comment, algorithm) = selected.ok_or_else(|| {
        failure(
            Code::Unsupported,
            Span::default(),
            "missing algorithm block comment",
        )
    })?;
    let module_end = module_end.ok_or_else(|| {
        failure(
            Code::Syntax,
            Span::default(),
            "missing enclosing module terminator",
        )
    })?;
    if previous.is_some_and(|span| span.start < comment.end || span.end > module_end) {
        return Err(failure(
            Code::Unsupported,
            previous.unwrap_or_default(),
            "translation markers outside the proved post-algorithm module region",
        ));
    }
    let line = source_lines
        .iter()
        .find(|line| line.start < comment.end && comment.end <= line.end)
        .ok_or_else(|| failure(Code::Internal, comment, "missing algorithm closing line"))?;
    // trans.java uses String.trim(), whose boundary is ASCII space (0x20).
    let tail = source[comment.end..line.start + line.text.len()]
        .trim_matches(|character: char| character <= ' ');
    if !tail.is_empty() && !tail.starts_with("\\*") {
        return Err(failure(
            Code::Syntax,
            comment,
            "text follows algorithm comment on the same line",
        ));
    }
    Ok(RewriteSelection {
        comment,
        algorithm,
        previous,
        previous_body,
        checksum,
        insertion: line.end,
    })
}
/// Produces a byte-position-preserving masked view for native source generation.
/// Existing translation lines are blanked, with UTF-8 byte offsets and CR/LF kept.
#[must_use]
pub fn mask_previous(source: &str, previous: Option<Span>) -> String {
    let Some(span) = previous.filter(|s| {
        s.start <= s.end
            && source.is_char_boundary(s.start)
            && source.is_char_boundary(s.end)
            && s.end <= source.len()
    }) else {
        return source.to_owned();
    };
    let mut masked = String::with_capacity(source.len());
    masked.push_str(&source[..span.start]);
    for byte in source.as_bytes()[span.start..span.end].iter().copied() {
        masked.push(if byte == b'\r' || byte == b'\n' {
            char::from(byte)
        } else {
            ' '
        });
    }
    masked.push_str(&source[span.end..]);
    masked
}
/// Builds a bounded body-checksummed replacement. Body generation and enclosing
/// module semantic admission belong to the caller and are never implied here.
///
/// # Errors
/// Changed/unverified body checksum, invalid editing boundaries, or resource limit.
pub fn rewrite_document(
    source: &str,
    body: &str,
    options: &RewriteOptions,
) -> Result<RewritePlan, Diagnostic> {
    rewrite_document_with_translation_options(
        source,
        body,
        options,
        &crate::TranslationOptions::default(),
    )
}
/// Build a replacement with the exact label/options context used by generation.
/// Algorithm CRC availability never gates replacement; the body CRC is always written.
/// A supplied body is never an admitted artifact or model proof by itself.
///
/// # Errors
/// Modified/unverified body integrity, invalid source or byte limits.
#[allow(
    clippy::too_many_lines,
    reason = "source replacement and both integrity contracts remain ordered and bounded"
)]
pub fn rewrite_document_with_translation_options(
    source: &str,
    body: &str,
    options: &RewriteOptions,
    translation: &crate::TranslationOptions,
) -> Result<RewritePlan, Diagnostic> {
    if body.len() > options.max_bytes {
        return Err(failure(
            Code::Resource,
            Span::default(),
            "PlusCal rewrite body byte limit reached",
        ));
    }
    // Validate marker/layout boundaries before resolving original source options.
    select_rewrite(source, options)?;
    let context = crate::resolve_translation_options(source, translation)
        .map_err(|error| error.diagnostic)?;
    let selection = select_rewrite_with_parser(source, options, &context.effective.parser)?;
    let masked = mask_previous(source, selection.previous);
    let mut warnings = Vec::new();
    let algorithm_checksum = match crate::algorithm_checksum_at(
        &masked,
        selection.algorithm.start..selection.algorithm.end,
        translation,
    ) {
        Ok(result) => Some(result.checksum),
        Err(error) if matches!(error.diagnostic.code, Code::Unsupported | Code::Resource) => {
            warnings.push(error.diagnostic);
            None
        }
        Err(error) => return Err(error.diagnostic),
    };
    if selection.checksum.as_ref().is_some_and(|c| !c.matched) && !options.replace_modified {
        return Err(failure(
            Code::Unsupported,
            selection.previous.unwrap_or_default(),
            "existing body checksum is modified, missing or disabled; explicit replacement is required",
        ));
    }
    let newline = if options.normalize_newlines {
        "\n"
    } else {
        source.find(['\r', '\n']).map_or("\n", |i| {
            if source.as_bytes()[i] == b'\r' {
                if source.as_bytes().get(i + 1) == Some(&b'\n') {
                    "\r\n"
                } else {
                    "\r"
                }
            } else {
                "\n"
            }
        })
    };
    let checksum = translation_checksum(body);
    let algorithm_value = algorithm_checksum.as_ref().map_or_else(
        || "\\in STRING".to_owned(),
        |checksum| format!("= \"{checksum}\""),
    );
    let checksum_header = format!(
        "\\* BEGIN TRANSLATION (chksum(pcal) {algorithm_value} /\\ chksum(tla) = \"{checksum}\")"
    );
    let (header, end_marker) = if let Some(previous) = selection.previous {
        let old_lines = lines(&source[previous.start..previous.end]);
        let first = old_lines
            .first()
            .ok_or_else(|| failure(Code::Internal, previous, "missing previous header"))?
            .text;
        let last = old_lines
            .last()
            .ok_or_else(|| failure(Code::Internal, previous, "missing previous end marker"))?
            .text;
        let prefix_end = first
            .find("TRANSLATION")
            .map(|i| i + "TRANSLATION".len())
            .ok_or_else(|| failure(Code::Internal, previous, "missing previous marker token"))?;
        let header = if let Some(old) = self::checksum_header(first) {
            let mut text = first.to_owned();
            if let Some(span) = old.translation_hex {
                text.replace_range(span.start..span.end, &checksum);
            } else {
                text.replace_range(
                    old.translation_value.start..old.translation_value.end,
                    &format!("= \"{checksum}\""),
                );
            }
            if old.algorithm.is_some() || algorithm_checksum.is_some() {
                text.replace_range(
                    old.algorithm_value.start..old.algorithm_value.end,
                    &algorithm_value,
                );
            }
            text
        } else if first[prefix_end..].trim().is_empty() {
            checksum_header.clone()
        } else {
            return Err(failure(
                Code::Unsupported,
                previous,
                "unrecognized previous checksum header text",
            ));
        };
        (header, last.to_owned())
    } else {
        (checksum_header, "\\* END TRANSLATION ".to_owned())
    };
    let span = selection.previous.unwrap_or(Span {
        start: selection.insertion,
        end: selection.insertion,
    });
    let prefix_newline = span.start > 0 && !source[..span.start].ends_with(['\r', '\n']);
    let body_lines = lines(body);
    // PcalTLAGen ends its final Spec/Termination block with an empty line.
    // Preserve an existing empty line, or add that real separator before END.
    let final_separator = body_lines.last().is_none_or(|line| !line.text.is_empty());
    let replacement_bytes = header
        .len()
        .checked_add(body_lines.iter().map(|l| l.text.len()).sum::<usize>())
        .and_then(|s| {
            s.checked_add(
                (body_lines.len() + 2 + usize::from(prefix_newline) + usize::from(final_separator))
                    * newline.len(),
            )
        })
        .and_then(|s| s.checked_add(end_marker.len()))
        .ok_or_else(|| failure(Code::Resource, span, "PlusCal rewrite output size overflow"))?;
    let total = source
        .len()
        .saturating_sub(span.end - span.start)
        .checked_add(replacement_bytes)
        .ok_or_else(|| failure(Code::Resource, span, "PlusCal rewrite output size overflow"))?;
    if total > options.max_bytes {
        return Err(failure(
            Code::Resource,
            span,
            "PlusCal rewrite output byte limit reached",
        ));
    }
    let mut replacement = String::with_capacity(replacement_bytes);
    if prefix_newline {
        replacement.push_str(newline);
    }
    replacement.push_str(&header);
    replacement.push_str(newline);
    for line in body_lines {
        replacement.push_str(line.text);
        replacement.push_str(newline);
    }
    if final_separator {
        replacement.push_str(newline);
    }
    replacement.push_str(&end_marker);
    replacement.push_str(newline);
    let mut module_source = String::with_capacity(total);
    module_source.push_str(&source[..span.start]);
    module_source.push_str(&replacement);
    module_source.push_str(&source[span.end..]);
    if options.normalize_newlines {
        let normalized = lines(&module_source);
        let length = normalized.iter().map(|l| l.text.len() + 1).sum::<usize>();
        if length > options.max_bytes {
            return Err(failure(
                Code::Resource,
                span,
                "normalized PlusCal rewrite output byte limit reached",
            ));
        }
        let mut text = String::with_capacity(length);
        for line in normalized {
            text.push_str(line.text);
            text.push('\n');
        }
        module_source = text;
    }
    Ok(RewritePlan {
        span,
        replacement,
        module_source,
        translation_checksum: checksum,
        algorithm_checksum,
        warnings,
        selection,
        newlines_normalized: options.normalize_newlines,
    })
}
