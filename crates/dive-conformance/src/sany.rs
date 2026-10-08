//! SANY parse goldens: accept or reject, with the exit code and error text.
//!
//! How SANY reports failure (verified against both pinned oracles; see the
//! fixtures under `tests/fixtures/sany/`):
//! - Syntax errors and missing modules exit 255 (`-1`) in both versions.
//! - Semantic and level-checking errors exit **0** unless `-error-codes` is
//!   given. With `-error-codes`, `94d0c50` exits 4. v1.7.4 does not know
//!   `-error-codes`: it prints `Illegal switch: -error-codes` and exits 255
//!   without parsing anything.
//!
//! So the harness passes `-error-codes` only to oracles whose probe accepted
//! it, and decides acceptance from the exit code *and* SANY's error markers:
//! a module is accepted only if SANY exits 0 and prints no error marker. An
//! exit of 0 with error markers is a rejection with
//! `exit_code_reflects_errors = false`, which is the documented behaviour
//! above, not a guess. v1.7.4 prints a `Semantic errors:` header even when
//! the list under it holds only warnings; that is not a rejection.

use crate::process::ProcessOutcome;
use crate::tlc::result::{PARSE_MARKERS, SEMANTIC_MARKERS, unresolved_modules};
use serde::{Deserialize, Serialize};

/// SANY's verdict on one module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SanyVerdict {
    /// Parsed, analysed and level-checked without error.
    Accept,
    /// Rejected at the syntax level (including missing modules).
    ParseError,
    /// Rejected in semantic analysis or level checking.
    SemanticError,
    /// Rejected, but the harness cannot tell how; see the note.
    Unclassified,
    /// The harness timeout fired.
    Timeout,
}

/// The result of running SANY on one module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SanyResult {
    /// The verdict.
    pub verdict: SanyVerdict,
    /// Whether SANY accepted the module.
    pub accepted: bool,
    /// Process exit code.
    pub exit_code: Option<i32>,
    /// Whether the exit code alone tells acceptance correctly (false when
    /// SANY exits 0 despite printing errors).
    pub exit_code_reflects_errors: bool,
    /// SANY's error text from the first error marker on, with machine paths
    /// replaced by placeholders. `None` when accepted.
    pub error_text: Option<String>,
    /// Modules SANY could not find.
    pub unresolved_modules: Vec<String>,
    /// Caveats.
    pub note: Option<String>,
}

/// Every marker that means SANY reported an error. `Semantic errors:` is
/// deliberately not one: v1.7.4 prints that header above a list of
/// *warnings* only (`*** Warnings: 1`, e.g. "Multiple declarations or
/// definitions for symbol"), and accepts the module. A semantic rejection
/// always comes with `*** Errors:`.
fn error_markers() -> impl Iterator<Item = &'static str> {
    PARSE_MARKERS
        .iter()
        .chain(&["*** Errors:", "*** Abort messages:", "Illegal switch"])
        .copied()
}

/// Classifies one SANY run.
#[must_use]
pub fn classify(outcome: &ProcessOutcome) -> SanyResult {
    let text = format!("{}{}", outcome.stdout, outcome.stderr);
    if outcome.timed_out {
        return SanyResult {
            verdict: SanyVerdict::Timeout,
            accepted: false,
            exit_code: None,
            exit_code_reflects_errors: false,
            error_text: None,
            unresolved_modules: Vec::new(),
            note: Some("SANY hit the harness timeout".to_owned()),
        };
    }
    let first_marker = error_markers().filter_map(|m| text.find(m)).min();
    let has_errors = first_marker.is_some();
    let exit_ok = outcome.exit_code == Some(0);
    let accepted = exit_ok && !has_errors;
    let verdict = if accepted {
        SanyVerdict::Accept
    } else if text.contains("Illegal switch") {
        SanyVerdict::Unclassified
    } else if PARSE_MARKERS.iter().any(|m| text.contains(m)) {
        SanyVerdict::ParseError
    } else if SEMANTIC_MARKERS.iter().any(|m| text.contains(m)) {
        SanyVerdict::SemanticError
    } else {
        SanyVerdict::Unclassified
    };
    let note = match (verdict, exit_ok, has_errors) {
        (SanyVerdict::Unclassified, ..) => Some(format!(
            "SANY rejected the module (exit {:?}) without a recognised parse or semantic error marker",
            outcome.exit_code
        )),
        (_, true, true) => Some("SANY exited 0 but reported errors".to_owned()),
        _ => None,
    };
    SanyResult {
        verdict,
        accepted,
        exit_code: outcome.exit_code,
        exit_code_reflects_errors: exit_ok != has_errors,
        error_text: first_marker.map(|i| {
            // Start at the "Semantic errors:" header when it precedes the marker.
            let start = SEMANTIC_MARKERS
                .iter()
                .filter_map(|m| text.find(m))
                .filter(|h| *h < i)
                .min()
                .unwrap_or(i);
            text[start..].trim_end().to_owned()
        }),
        unresolved_modules: unresolved_modules(&text),
        note,
    }
}

/// Which SANY verdict a module run compares (`run --sany-phase`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SanyPhase {
    /// Full SANY: parsing, semantic analysis and level checking
    /// (`dive analyze`).
    #[default]
    Full,
    /// SANY's parse phase for the module's own file only (`dive parse`;
    /// SANY `-s`, with a failure attributed only to the file that printed
    /// it).
    Parse,
}

/// SANY's parse-phase verdict on one file, attributed as the syntax
/// acceptance procedure does (`crates/dive-syntax/acceptance/`): a failure
/// counts only for the file whose parse SANY reported failing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParseVerdict {
    /// The file parses (later failures, in dependencies or in semantic
    /// analysis, are not its parse failures).
    Accept,
    /// The file parses, but its module's name differs from the file name
    /// (SANY `MODULE_NAME_DIFFERENT_FROM_FILE_NAME`, dive `DIVE-S0006`).
    NameMismatch,
    /// SANY printed `Could not parse module <M> from file <F>` for this
    /// file: a syntax or lexical error.
    Reject,
}

/// The file stem of a corpus-relative module path (`a/B.tla` -> `B`).
#[must_use]
pub fn module_stem(module: &str) -> &str {
    let file = module.rsplit('/').next().unwrap_or(module);
    file.strip_suffix(".tla").unwrap_or(file)
}

/// Derives the parse-phase verdict of `module`'s own file from a full SANY
/// result, as recorded in a golden.
///
/// This is sound because SANY parses the root file first and aborts at the
/// first parse-phase failure, and because the golden's `error_text` keeps
/// everything from the first error marker on, which includes SANY's
/// attribution line:
/// - `accept` and `semantic-error` mean every file parsed;
/// - a `parse-error` is the root's own if SANY printed
///   `Could not parse module <stem> from file` (a dependency's failure
///   names the dependency), or a name mismatch if it printed
///   `File name '<stem>' does not match`;
/// - a `parse-error` attributed to a dependency (another module that could
///   not be parsed, could not be found, or has a mismatched name), or a
///   circular dependency (which SANY detects only once every file of the
///   cycle has parsed), means the root parsed.
///
/// # Errors
/// Returns why when the result does not determine the verdict (an
/// unclassified or timed-out run, or a parse error without a recognised
/// attribution). The harness reports that as `oracle-error`, never a guess.
pub fn parse_phase(result: &SanyResult, module: &str) -> std::result::Result<ParseVerdict, String> {
    let stem = module_stem(module);
    match result.verdict {
        SanyVerdict::Accept | SanyVerdict::SemanticError => Ok(ParseVerdict::Accept),
        SanyVerdict::Unclassified | SanyVerdict::Timeout => Err(format!(
            "SANY verdict {:?} does not determine the parse-phase verdict",
            result.verdict
        )),
        SanyVerdict::ParseError => {
            let text = result.error_text.as_deref().unwrap_or_default();
            if text.contains(&format!("Could not parse module {stem} from file ")) {
                Ok(ParseVerdict::Reject)
            } else if text.contains(&format!("File name '{stem}' does not match the name")) {
                Ok(ParseVerdict::NameMismatch)
            } else if text.contains("Could not parse module ")
                || text.contains("Cannot find source file for module ")
                || text.contains("File name '")
                || text.contains("Circular dependency among .tla files")
            {
                Ok(ParseVerdict::Accept)
            } else {
                Err(format!(
                    "SANY's parse error is not attributed to `{stem}` or to a dependency in the recorded text"
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn parse_phase_attribution() {
        let rej = |text: &str| SanyResult {
            verdict: SanyVerdict::ParseError,
            accepted: false,
            exit_code: Some(255),
            exit_code_reflects_errors: true,
            error_text: Some(text.to_owned()),
            unresolved_modules: Vec::new(),
            note: None,
        };
        let m = "dir/ParseError.tla";
        assert_eq!(module_stem(m), "ParseError");
        assert_eq!(
            parse_phase(
                &rej(
                    "***Parse Error***\nCould not parse module ParseError from file ParseError.tla"
                ),
                m
            ),
            Ok(ParseVerdict::Reject)
        );
        // A dependency's failure: the root parsed.
        assert_eq!(
            parse_phase(
                &rej(
                    "***Parse Error***\nCould not parse module ParseErrorDep from file ParseErrorDep.tla"
                ),
                m
            ),
            Ok(ParseVerdict::Accept)
        );
        assert_eq!(
            parse_phase(
                &rej("Cannot find source file for module SVG imported in module ParseError."),
                m
            ),
            Ok(ParseVerdict::Accept)
        );
        assert_eq!(
            parse_phase(
                &rej(
                    "File name 'ParseError' does not match the name 'Other' of the top level module it contains."
                ),
                m
            ),
            Ok(ParseVerdict::NameMismatch)
        );
        assert_eq!(
            parse_phase(
                &rej(
                    "Fatal errors while parsing TLA+ spec in file ParseError.tla\n\nUnknown location\n\nCircular dependency among .tla files; dependency cycle is:"
                ),
                m
            ),
            Ok(ParseVerdict::Accept)
        );
        assert!(parse_phase(&rej("Fatal errors while parsing TLA+ spec"), m).is_err());
        let mut sem = rej("Semantic errors:");
        sem.verdict = SanyVerdict::SemanticError;
        assert_eq!(parse_phase(&sem, m), Ok(ParseVerdict::Accept));
        sem.verdict = SanyVerdict::Unclassified;
        assert!(parse_phase(&sem, m).is_err());
    }

    fn outcome(exit: i32, stdout: &str) -> ProcessOutcome {
        ProcessOutcome {
            exit_code: Some(exit),
            signal: None,
            timed_out: false,
            stdout: stdout.to_owned(),
            stderr: String::new(),
            elapsed: Duration::ZERO,
        }
    }

    #[test]
    fn semantic_error_with_exit_zero_is_rejected() {
        let r = classify(&outcome(
            0,
            "Semantic processing of module M\nSemantic errors:\n\n*** Errors: 1\n\nUnknown operator: `y'.\n",
        ));
        assert_eq!(r.verdict, SanyVerdict::SemanticError);
        assert!(!r.accepted);
        assert!(!r.exit_code_reflects_errors);
        assert!(
            r.error_text
                .is_some_and(|t| t.starts_with("Semantic errors:"))
        );
    }

    #[test]
    fn warnings_under_semantic_errors_header_are_accepted() {
        let r = classify(&outcome(
            0,
            "Semantic errors:\n\n*** Warnings: 1\n\nline 5, col 1 to line 5, col 24 of module M\n\nMultiple declarations or definitions for symbol Foo.\n",
        ));
        assert_eq!(r.verdict, SanyVerdict::Accept);
        assert!(r.accepted);
    }

    #[test]
    fn clean_exit_is_accepted() {
        let r = classify(&outcome(
            0,
            "Semantic processing of module M\nLinting of module M\n",
        ));
        assert_eq!(r.verdict, SanyVerdict::Accept);
        assert!(r.exit_code_reflects_errors);
        assert_eq!(r.error_text, None);
    }

    #[test]
    fn parse_error_and_unknown_rejection() {
        let r = classify(&outcome(255, "***Parse Error***\nEncountered \"0\"\n"));
        assert_eq!(r.verdict, SanyVerdict::ParseError);
        let r = classify(&outcome(255, "something else\n"));
        assert_eq!(r.verdict, SanyVerdict::Unclassified);
        assert!(!r.exit_code_reflects_errors);
        let r = classify(&outcome(255, "Illegal switch: -error-codes\n"));
        assert_eq!(r.verdict, SanyVerdict::Unclassified);
    }
}
