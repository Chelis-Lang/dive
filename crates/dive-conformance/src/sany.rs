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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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
