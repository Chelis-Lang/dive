//! Explanations for stable temporal and command diagnostic codes.
//!
//! Explanations describe a diagnostic family. A concrete report's message and
//! source remain authoritative for the particular failure. An explanation is
//! neither a model-checking result nor permission to discard a refusal.

use serde_json::{Value, json};

/// A diagnostic's cause and a concrete next action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Explanation {
    /// Stable diagnostic code, never reused for another meaning.
    pub code: &'static str,
    /// What this diagnostic family means.
    pub cause: &'static str,
    /// An action that does not presume the refused model was checked.
    pub remediation: &'static str,
}

macro_rules! explanations {
    ($($code:literal => ($cause:literal, $help:literal);)*) => {
        const CATALOGUE: &[Explanation] = &[$(Explanation {
            code: $code,
            cause: $cause,
            remediation: $help,
        },)*];
    };
}

explanations! {
    "DIVE-A0030" => ("The temporal model could not be bound.", "Fix the accompanying model/configuration diagnostic, then rerun the same check.");
    "DIVE-A0031" => ("Independent validation rejected a generated temporal counterexample; this is an internal failure.", "Retain the source, configuration, seed, report and candidate trace and file a dive issue; do not treat the candidate as a violation.");
    "DIVE-A0032" => ("Independent validation could not certify a generated temporal counterexample.", "Read the accompanying validation refusal; use the pinned TLC for the model until that feature is supported. An unvalidated candidate is not a violation.");
    "DIVE-A0033" => ("A temporal counterexample could not be represented in the trace format.", "Retain the report, source and configuration and file a trace-format issue; do not publish the unrepresented candidate as a certified counterexample.");
    "DIVE-A0034" => ("A temporal evaluation thread failed, so execution evidence is incomplete.", "Free host thread/memory resources and retry with --workers 1. If it repeats, retain the source, configuration, seed and report and file an issue.");
    "DIVE-L0020" => ("Temporal planning encountered a feature whose semantics it cannot establish exactly.", "Inspect the named feature and its cause location. Use the pinned TLC for this model and include the smallest reproducer in a dive issue; no budget increase implements a missing feature.");
    "DIVE-L0021" => ("Temporal formula construction reached an explicit bound.", "Use the pinned TLC for the unchanged model and report the named bound with the source/configuration. --max-memory does not raise formula-count bounds.");
    "DIVE-L0022" => ("Temporal planning contains an invalid internal formula reference.", "Retain the source, configuration and report and file a dive issue; this is not a property verdict.");
    "DIVE-L0023" => ("A constant expression failed during temporal source conversion.", "Inspect the expression at the reported source and the accompanying TLC error class; correct its arguments or configuration and rerun.");
    "DIVE-L0030" => ("Temporal tableau construction failed.", "Inspect the named formula bound or reference failure. Use the pinned TLC for a bounded refusal; report an invalid internal reference as a bug.");
    "DIVE-L0031" => ("Temporal product construction or acceptance search failed.", "Inspect the named product/acceptance bound. Use the pinned TLC for the unchanged model; changing run budgets does not remove an algorithmic count bound.");
    "DIVE-L0032" => ("Safety exploration did not supply a completed behavior graph.", "Read the retained exploration verdict and progress. For a stopped run, follow its resource-specific hint; resolve an earlier model error before retrying liveness.");
    "DIVE-L0033" => ("A temporal predicate failed evaluation or returned a non-Boolean value.", "Inspect the named property and source expression, including the closing action, and correct the reported value/type error. Retain a minimal reproducer if pinned TLC behaves differently.");
    "DIVE-L0034" => ("A liveness analysis stage reached an explicit resource bound.", "Include the named bound and progress in a capacity issue and use the pinned TLC for the unchanged model. A resource refusal proves no property.");
    "DIVE-L0035" => ("The requested temporal analysis mode is not implemented exactly.", "Inspect the named mode. If parallel temporal capture is refused, retry --workers 1; otherwise use the pinned TLC for the unchanged request.");
    "DIVE-L0036" => ("Temporal acceptance and direct property attribution disagree internally.", "Retain the source, configuration, seed and report and file a dive issue; do not use the candidate as a property verdict.");
    "DIVE-L0037" => ("The behavior-graph capture thread failed and lost execution evidence.", "Free host thread/memory resources and retry --workers 1; retain the report and file an issue if the failure repeats.");
    "DIVE-L0038" => ("The run budget stopped temporal analysis before a verdict.", "Follow the stopped block's resource and hint: raise --time-limit or --max-memory only when the host can support it; --time-limit=none disables the time budget. Retain the original progress for comparison.");
    "DIVE-R0001" => ("The run-budget watchdog could not start, so the requested limits cannot be enforced.", "Free host thread resources and retry. If deliberately running without both guards, pass --time-limit=none --max-memory=none explicitly; no model checking ran before this startup failure.");
    "DIVE-X0001" => ("Command arguments are invalid or cannot be decoded as UTF-8.", "Run dive --help (or dive tlc -help), correct the named argument, and provide UTF-8 command arguments.");
    "DIVE-X0002" => ("The TLC command adapter recognizes an option that dive does not implement.", "Use the pinned TLC with the original option; remove it only if you intend the resulting different request.");
    "DIVE-X0003" => ("The TLC command adapter could not construct a valid check request.", "Inspect the argument/configuration error and correct it before retrying dive tlc.");
    "DIVE-X0004" => ("The requested specification file could not be read.", "Check the reported path, working directory and read permissions, then retry with the intended file.");
}

/// Looks up an explicitly documented code. Unknown codes return `None`;
/// guessing an explanation from a code prefix would hide new failure classes.
///
/// ```
/// let entry = dive_api::diagnostics::explain("DIVE-L0020").ok_or("known code")?;
/// assert!(entry.remediation.contains("pinned TLC"));
/// assert!(dive_api::diagnostics::explain("DIVE-L9999").is_none());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn explain(code: &str) -> Option<&'static Explanation> {
    CATALOGUE.iter().find(|entry| entry.code == code)
}

/// A versioned informational report. `known` establishes catalogue coverage
/// only; this operation does not parse, execute or prove a model.
#[must_use]
pub fn explanation_json(code: &str) -> Value {
    let entry = explain(code);
    json!({
        "schema_version": crate::REPORT_SCHEMA_VERSION,
        "command": "explain",
        "code": code,
        "known": entry.is_some(),
        "explanation": entry.map(|entry| json!({
            "cause": entry.cause,
            "remediation": entry.remediation,
        })),
    })
}

/// Retains the actual planning/atom cause location. Stage-wide failures have
/// no expression location; inventing a root expression would misdirect a fix.
pub(crate) fn temporal_failure(failure: &crate::temporal::Failure) -> Value {
    use crate::temporal::Error;
    use dive_liveness::analyze::AnalysisError;
    use dive_liveness::plan::PlanningError;
    let error = failure.error();
    let source = match error {
        Error::Analysis(AnalysisError::Planning(PlanningError::Unsupported(refusal))) => {
            Some(&refusal.cause)
        }
        Error::Analysis(
            AnalysisError::Planning(
                PlanningError::Evaluation { source, .. } | PlanningError::Formula { source, .. },
            )
            | AnalysisError::Atom { source, .. },
        ) => Some(source.as_ref()),
        _ => None,
    };
    let span = source.and_then(|source| {
        source.origin.file.map(|file| {
            json!({
                "file": file.index(),
                "start": source.origin.range.0,
                "end": source.origin.range.1,
            })
        })
    });
    json!({
        "code": error.code(),
        "message": failure.to_string(),
        "cause": error.to_string(),
        "span": span,
        "location": source.map(|source| source.location.as_str()),
        "help": explain(error.code()).map(|entry| entry.remediation),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique_and_unknown_codes_never_borrow_an_explanation() {
        for (index, entry) in CATALOGUE.iter().enumerate() {
            assert!(!entry.cause.is_empty() && !entry.remediation.is_empty());
            assert!(
                CATALOGUE[..index]
                    .iter()
                    .all(|prior| prior.code != entry.code)
            );
            assert_eq!(explain(entry.code), Some(entry));
        }
        for unknown in [
            "",
            "DIVE-L9999",
            "DIVE-L0020 extra",
            "dive-l0020",
            "DIVE-R0002",
        ] {
            let report = explanation_json(unknown);
            assert_eq!(report["known"], false);
            assert_eq!(report["code"], unknown);
            assert!(report["explanation"].is_null());
            assert!(report.get("verdict").is_none());
        }
    }

    #[test]
    fn temporal_diagnostics_retain_the_actual_cause_and_never_invent_a_span()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::temporal::Error;
        use dive_liveness::analyze::{AnalysisError, Limits};
        use dive_liveness::plan::PlanningError;
        let source = "---- MODULE Help ----\nEXTENDS TLC\nVARIABLE x\nInit == x=0\nNext == x'=x\nP == IF x'=x THEN [](x=0) ELSE [](x=0)\n====\n";
        let analysis =
            dive_sema::analyze_source("Help.tla", source, &dive_sema::Options::default());
        assert!(analysis.accepted());
        let ir = std::sync::Arc::new(dive_ir::lower(&analysis)?);
        let config = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
            .into_config()
            .ok_or("valid configuration")?;
        let options = dive_check::CheckOptions::tlc(0, 0);
        let Err(failure) =
            crate::temporal::check_final_with_evidence(&ir, &config, &options, Limits::default())
        else {
            return Err("an action-level temporal guard must refuse".into());
        };
        let Error::Analysis(AnalysisError::Planning(PlanningError::Unsupported(refusal))) =
            failure.error()
        else {
            return Err(format!("expected source-aware refusal: {failure}").into());
        };
        let value = temporal_failure(&failure);
        assert_eq!(value["code"], "DIVE-L0020");
        assert_eq!(value["cause"], failure.error().to_string());
        assert_eq!(value["location"], refusal.cause.location);
        assert_eq!(value["span"]["start"], refusal.cause.origin.range.0);
        assert_eq!(value["span"]["end"], refusal.cause.origin.range.1);
        let (start, end) = refusal.cause.origin.range;
        assert_eq!(&source[start as usize..end as usize], "x'=x");
        assert!(value["span"]["file"].is_number());
        assert!(
            value["help"]
                .as_str()
                .is_some_and(|help| help.contains("pinned TLC"))
        );
        assert!(value.get("verdict").is_none());

        let Err(failure) = crate::temporal::check_final_with_evidence(
            &ir,
            &config,
            &options,
            Limits {
                properties: 0,
                ..Default::default()
            },
        ) else {
            return Err("a zero property bound must refuse".into());
        };
        let value = temporal_failure(&failure);
        assert_eq!(value["code"], "DIVE-L0034");
        assert!(value["span"].is_null() && value["location"].is_null());
        assert!(value["help"].is_string());
        Ok(())
    }
}
