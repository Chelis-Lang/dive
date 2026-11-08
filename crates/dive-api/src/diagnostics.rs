//! Explanations for stable syntax, selected semantic, temporal and command codes.
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
    "DIVE-D0001" => ("A native debugger request, frame, handle or supported-session boundary is invalid or unavailable.", "Read the named protocol argument or boundary. Initialize and attach/launch the authorized local check, use handles from the current stop, and continue or terminate explicitly. A failed debugger request is not a model verdict.");
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
    "DIVE-L0031" => ("Temporal product construction or acceptance search failed.", "For a named product/acceptance bound, use the pinned TLC and report the capacity limit; changing run budgets does not remove a count bound. For an invalid product shape or witness/search invariant, retain the inputs and report and file an internal bug; do not accept its candidate.");
    "DIVE-L0032" => ("Safety exploration did not supply a completed behavior graph.", "Read the retained exploration verdict and progress. For a stopped run, follow its resource-specific hint; resolve an earlier model error before retrying liveness.");
    "DIVE-L0033" => ("A temporal predicate failed evaluation or returned a non-Boolean value.", "Inspect the named property and source expression, including the closing action, and correct the reported value/type error. Retain a minimal reproducer if pinned TLC behaves differently.");
    "DIVE-L0034" => ("A liveness analysis stage reached an explicit resource bound.", "Include the named bound and progress in a capacity issue and use the pinned TLC for the unchanged model. A resource refusal proves no property.");
    "DIVE-L0035" => ("The requested temporal analysis mode is not implemented exactly.", "Inspect the named mode. If parallel temporal capture is refused, retry --workers 1; otherwise use the pinned TLC for the unchanged request.");
    "DIVE-L0036" => ("Temporal acceptance and direct property attribution disagree internally.", "Retain the source, configuration, seed and report and file a dive issue; do not use the candidate as a property verdict.");
    "DIVE-L0037" => ("The behavior-graph capture thread failed and lost execution evidence.", "Free host thread/memory resources and retry --workers 1; retain the report and file an issue if the failure repeats.");
    "DIVE-L0038" => ("The run budget stopped temporal analysis before a verdict.", "Follow the stopped block's resource and hint: raise --time-limit or --max-memory only when the host can support it; --time-limit=none disables the time budget. Retain the original progress for comparison.");
    "DIVE-R0001" => ("The run-budget watchdog could not start, so the requested limits cannot be enforced.", "Free host thread resources and retry. If deliberately running without both guards, pass --time-limit=none --max-memory=none explicitly; no model checking ran before this startup failure.");
    "DIVE-R0002" => ("A required process-memory observation or physical-RAM lookup was unavailable, so the requested memory guard could not be established.", "Retry on a supported host; if deliberately accepting unlimited memory, pass --max-memory=none explicitly. For an unavailable auto ceiling, an explicit --max-memory value avoids physical-RAM lookup but still requires working process-memory observations. No unavailable reading is zero memory or a model verdict.");
    "DIVE-X0001" => ("Command arguments are invalid or cannot be decoded as UTF-8.", "Run dive --help (or dive tlc -help), correct the named argument, and provide UTF-8 command arguments.");
    "DIVE-X0002" => ("The TLC command adapter refused an option or could not transport or independently certify a generated counterexample.", "Read the particular refusal. Use the pinned TLC for an unsupported option; for trace transport, missing run metadata or validation refusal, retain the source/configuration, seed, report and trace and file an issue. An uncertified candidate is not a violation.");
    "DIVE-X0003" => ("Independent validation rejected a generated TLC-adapter counterexample; this is an internal failure.", "Retain the source, configuration, seed, report and candidate trace and file a dive issue. Do not treat the independently rejected candidate as a property violation.");
    "DIVE-X0004" => ("A requested specification or configuration file could not be read.", "Check the reported path, working directory and read permissions, then retry with the intended file.");
}

// The typed match is deliberately exhaustive: a newly assigned syntax code
// must receive a concrete explanation before this crate can compile.
macro_rules! syntax_explanations {
    ($($variant:ident => ($code:literal, $cause:literal, $help:literal);)*) => {
        const SYNTAX_CODES: &[dive_syntax::Code] = &[$(dive_syntax::Code::$variant,)*];

        pub(crate) fn syntax_explanation(code: dive_syntax::Code) -> &'static Explanation {
            match code {
                $(dive_syntax::Code::$variant => &Explanation {
                    code: $code,
                    cause: $cause,
                    remediation: $help,
                },)*
            }
        }
    };
}

syntax_explanations! {
    UnexpectedCharacter => ("DIVE-S0001", "The source contains a character that cannot begin a TLA+ token.", "Inspect the highlighted character; replace it with the intended TLA+ symbol or put literal text inside a quoted string or comment.");
    BadString => ("DIVE-S0002", "A string literal has an invalid escape or no closing quote.", "Close the string with a double quote and use valid TLA+ string escapes; escape embedded quotes and backslashes.");
    UnterminatedComment => ("DIVE-S0003", "A block comment remains open at the end of the source.", "Close each open (* block comment with *), including any nested comment.");
    NoModule => ("DIVE-S0004", "The source has no TLA+ module header.", "Start the module with ---- MODULE Name ---- and finish it with ====; use the file's base name for Name.");
    MissingModuleEnd => ("DIVE-S0005", "The module ends before its closing delimiter.", "Add ==== after the module's final definition or proof; close any preceding nested module first.");
    ModuleNameMismatch => ("DIVE-S0006", "The top-level module name differs from its file name.", "Rename the file to match the declared module name, or change the MODULE name to match the file's base name.");
    InvalidUtf8 => ("DIVE-S0007", "The file contains invalid UTF-8 bytes, which were decoded as replacement characters.", "Save the file as UTF-8 and restore any characters replaced during decoding, then parse it again.");
    UnexpectedToken => ("DIVE-S0010", "The grammar does not allow this token at the reported position.", "Inspect the highlighted token and its preceding construct; complete missing delimiters or keywords before changing the token.");
    MissingExpression => ("DIVE-S0011", "A construct requires an expression at this position.", "Supply the missing expression, or remove the operator or separator that requires it; check the preceding definition and delimiters.");
    PrecedenceConflict => ("DIVE-S0012", "Adjacent operators have overlapping precedence without an explicit grouping.", "Add parentheses around the intended subexpressions so their grouping is explicit.");
    MissingOperator => ("DIVE-S0013", "Two expressions occur together without an operator between them.", "Insert the intended operator between the expressions, or add the missing delimiter that separates them.");
    MisplacedOperator => ("DIVE-S0014", "An operator appears where its prefix, infix or postfix form cannot be used.", "Move the operator to its declared position and supply its operands; parenthesize the surrounding expression if needed.");
    JunctionIndentation => ("DIVE-S0015", "A conjunction or disjunction list item extends left of its bullet's allowed indentation.", "Indent each list item's continuation to the right of its /\\ or \\/ bullet, and align sibling bullets.");
    StepLevel => ("DIVE-S0020", "A proof step number has the wrong nesting level.", "Make the step's level consistent with its containing proof and sibling steps; use a deeper level only inside a nested proof.");
    StepNumberMisuse => ("DIVE-S0021", "A proof step number occurs outside a proof or in an expression that cannot contain it.", "Move the numbered step into its proof, or replace the step-number token with the intended expression or reference.");
    UnexpectedProof => ("DIVE-S0022", "A proof is attached to a construct that cannot take one.", "Attach the proof to a theorem or a proof step that admits a subproof; remove an accidental PROOF keyword.");
    ProofTooDeep => ("DIVE-S0023", "Proof nesting exceeds SANY's 100-level limit.", "Restructure the proof into shallower subproofs or separate lemmas; increasing a run budget does not change this syntax limit.");
    BadLabel => ("DIVE-S0030", "The label separator :: does not follow a valid label.", "Place a valid label before ::, or remove the separator if no expression label was intended.");
    LabelChangesParse => ("DIVE-S0031", "Removing an expression label would change how its expression parses.", "Parenthesize the labeled expression so it has the same grouping with and without the label.");
    TimesAsArgument => ("DIVE-S0032", "Cartesian product syntax is being used as an operator argument.", "Define a named operator that takes the required arguments and returns their Cartesian product, then pass that operator by name.");
    SetMapWithIn => ("DIVE-S0033", "A mapped set's result is an unparenthesized membership expression.", "Parenthesize the result expression, for example {(a \\in b) : c \\in d}; keep the bound variables after the colon.");
    AtInExcept => ("DIVE-S0034", "An EXCEPT selector uses @ as a field name.", "Use the intended field name after !.; use @ only in the replacement expression to refer to the previous value.");
    BadFairness => ("DIVE-S0035", "A WF_ or SF_ fairness expression has an invalid structure.", "Use WF_vars(Action) or SF_vars(Action), with the state-variable expression after the underscore and the action in parentheses.");
    NewWithArguments => ("DIVE-S0036", "A NEW declaration gives arguments to a symbol before a set-membership constraint.", "Declare a set-constrained NEW symbol without arguments; put any required operator constraint in a separate assumption.");
    BadStructuralSelector => ("DIVE-S0037", "A structural selector uses a decimal number as a single selector.", "Separate structural selectors with !, for example Op!1!2; do not write Op!1.2.");
    NestingTooDeep => ("DIVE-S0040", "Expression nesting exceeds the parser's supported depth.", "Split deeply nested expressions into named operator definitions, then parse the restructured module again.");
    InputTooLarge => ("DIVE-S0041", "The input exceeds the parser's 4 GiB source-size capacity.", "Split the source into smaller modules and import their definitions; time and memory budgets do not change the source-offset capacity.");
}

// Semantic coverage is deliberately partial. Only these typed name-resolution
// and argument-shape families are registered; another code must not borrow help.
macro_rules! semantic_explanations {
    ($($variant:ident => ($code:literal, $cause:literal, $help:literal);)*) => {
        const SEMANTIC_CODES: &[dive_sema::Code] = &[$(dive_sema::Code::$variant,)*];

        fn semantic_explanation(code: dive_sema::Code) -> Option<&'static Explanation> {
            match code {
                $(dive_sema::Code::$variant => Some(&Explanation {
                    code: $code,
                    cause: $cause,
                    remediation: $help,
                }),)*
                _ => None,
            }
        }
    };
}

semantic_explanations! {
    UndefinedSymbol => ("DIVE-E0100", "The referenced name or qualified operator cannot be resolved in the current scope.", "Check the spelling and scope of the intended name. Define the intended operator, declare the intended CONSTANT or VARIABLE, or EXTENDS the module that defines it. For a named INSTANCE, check the Instance!Name qualification and the definitions exported by that instance.");
    UndefinedOperatorSymbol => ("DIVE-E0101", "The operator symbol used here has no definition in the current scope.", "Extend the module that defines the intended operator, or supply its definition in scope. Arithmetic operators such as + need Naturals, Integers or Reals; unary minus needs Integers or Reals, and sequence concatenation needs Sequences. An operator argument or INSTANCE substitution must also have the required arity.");
    WrongArgumentCount => ("DIVE-E0107", "An operator application supplies a different number of arguments than its declared arity.", "Match the application to the declared parameter list and the required/provided counts in the message. Reference a zero-argument operator without an argument list; apply function values using brackets.");
    FunctionArgumentCount => ("DIVE-E0110", "A recognized function definition rules out the supplied number of comma-separated bracket arguments.", "Check the function definition and its bracketed indices. Match comma-separated index parameters; a tuple-pattern parameter counts as one index. A complete tuple index can be supplied as one expression when it matches the function's domain.");
    OperatorArgumentExpected => ("DIVE-E0112", "A higher-order parameter requires an operator, but its argument is an expression.", "Pass an operator name of the required arity or a LAMBDA with that many parameters; inspect the declared higher-order parameter in the message.");
    OperatorArgumentArity => ("DIVE-E0113", "An operator supplied to a higher-order parameter has the wrong number of formal parameters.", "Pass an operator with the arity required by the higher-order parameter. If an adapter is intended, define a named operator or LAMBDA with that parameter count and the intended body.");
    LambdaArgumentArity => ("DIVE-E0114", "A LAMBDA supplied as an operator argument declares the wrong parameter count.", "Match the LAMBDA parameter list to the required operator arity in the message, and make its body use the intended arguments.");
}

pub(crate) fn semantic_help(code: dive_sema::Code, existing: Option<&str>) -> Option<String> {
    existing
        .map(str::to_owned)
        .or_else(|| semantic_explanation(code).map(|entry| entry.remediation.to_owned()))
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
    CATALOGUE
        .iter()
        .find(|entry| entry.code == code)
        .or_else(|| {
            SYNTAX_CODES
                .iter()
                .copied()
                .find(|syntax| syntax.as_str() == code)
                .map(syntax_explanation)
        })
        .or_else(|| {
            SEMANTIC_CODES
                .iter()
                .copied()
                .find(|semantic| semantic.as_str() == code)
                .and_then(semantic_explanation)
        })
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
    fn semantic_explanations_cover_only_the_selected_typed_codes() {
        for &code in dive_sema::Code::ALL {
            let expected = SEMANTIC_CODES.contains(&code);
            let entry = semantic_explanation(code);
            assert_eq!(entry.is_some(), expected, "{}", code.as_str());
            assert_eq!(explain(code.as_str()), entry);
            if let Some(entry) = entry {
                assert_eq!(entry.code, code.as_str());
                assert!(!entry.cause.is_empty() && !entry.remediation.is_empty());
                assert_eq!(
                    semantic_help(code, None).as_deref(),
                    Some(entry.remediation)
                );
            } else {
                assert_eq!(semantic_help(code, None), None);
            }
            for existing in ["", "specific producer suggestion"] {
                assert_eq!(
                    semantic_help(code, Some(existing)).as_deref(),
                    Some(existing)
                );
            }
        }
        for code in ["DIVE-E9999", "DIVE-E0107 extra", "dive-e0107", "DIVE-E0102"] {
            assert!(explain(code).is_none());
            assert_eq!(explanation_json(code)["known"], false);
        }
    }

    #[test]
    fn syntax_codes_have_exact_explanations_without_prefix_guessing() {
        for (index, &code) in SYNTAX_CODES.iter().enumerate() {
            let entry = syntax_explanation(code);
            assert_eq!(entry.code, code.as_str());
            assert!(!entry.cause.is_empty() && !entry.remediation.is_empty());
            assert!(!SYNTAX_CODES[..index].contains(&code));
            assert_eq!(explain(code.as_str()), Some(entry));
            let report = explanation_json(code.as_str());
            assert_eq!(report["known"], true);
            assert_eq!(report["schema_version"], crate::REPORT_SCHEMA_VERSION);
            assert!(report.get("verdict").is_none());
        }
        for code in [
            "DIVE-S0000",
            "DIVE-S0008",
            "DIVE-S0038",
            "dive-s0011",
            "DIVE-S0011 extra",
        ] {
            assert!(explain(code).is_none());
        }
    }

    #[test]
    fn parse_help_preserves_messages_spans_acceptance_and_specific_remediation() {
        let mut specific = 0;
        let mut fallback = 0;
        for source in [
            "---- MODULE Help ----\nA == \"λ\" /\\\n====\n",
            "---- MODULE Help ----\nA == 1\n",
            "---- MODULE Help ----\nA == {1 \\in {1} : c \\in {1}}\n====\n",
        ] {
            let raw = dive_syntax::parse_module(source);
            assert_ne!(raw.diagnostics(), []);
            let report = crate::parse_source("Help.tla", source);
            assert_eq!(
                report.accepted(),
                !raw.diagnostics()
                    .iter()
                    .any(dive_syntax::Diagnostic::is_error)
            );
            assert_eq!(report.diagnostics().len(), raw.diagnostics().len());
            let json = report.to_json(false);
            for (index, (original, enriched)) in raw
                .diagnostics()
                .iter()
                .zip(report.diagnostics())
                .enumerate()
            {
                assert_eq!(original.code, enriched.code);
                assert_eq!(original.message, enriched.message);
                assert_eq!(original.severity, enriched.severity);
                assert_eq!(original.range, enriched.range);
                if original.help.is_some() {
                    specific += 1;
                } else {
                    fallback += 1;
                }
                let expected = original
                    .help
                    .as_deref()
                    .unwrap_or_else(|| syntax_explanation(original.code).remediation);
                assert_eq!(enriched.help.as_deref(), Some(expected));
                assert_eq!(json["diagnostics"][index]["help"], expected);
                assert_eq!(
                    json["diagnostics"][index]["span"]["start"],
                    u32::from(original.range.start())
                );
                assert_eq!(
                    json["diagnostics"][index]["span"]["end"],
                    u32::from(original.range.end())
                );
                let (line, column) =
                    dive_syntax::diagnostics::line_col(source, original.range.start());
                assert_eq!(json["diagnostics"][index]["span"]["line"], line);
                assert_eq!(json["diagnostics"][index]["span"]["column"], column);
                assert!(report.render_human().contains(expected));
            }
        }
        assert!(specific > 0 && fallback > 0);
    }

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
            "DIVE-R9999",
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
        use crate::temporal::{Error, Failure, Stage};
        use dive_liveness::analyze::{self, AnalysisError, Limits};
        use dive_liveness::plan::PlanningError;
        let source = "---- MODULE Help ----\nEXTENDS TLC\nVARIABLE x\nInit == x=0\nNext == x'=x\nP == IF x'=x THEN [](x=0) ELSE [](x=0)\n====\n";
        let analysis =
            dive_sema::analyze_source("Help.tla", source, &dive_sema::Options::default());
        assert!(analysis.accepted());
        let ir = std::sync::Arc::new(dive_ir::lower(&analysis)?);
        let config = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
            .into_config()
            .ok_or("valid configuration")?;
        let model = dive_model::bind(ir, &config, &dive_model::BindOptions::tlc())
            .map_err(|e| format!("{e:?}"))?;
        // The planner's own refusal of an action-level temporal guard, as the
        // checker reports it before any search.
        let Err(planning) = dive_liveness::plan::lower_violation(
            &model,
            &model.property_claims()[0].pred,
            Limits::default().formula,
        ) else {
            return Err("an action-level temporal guard must refuse".into());
        };
        let failure = Failure::before(
            Error::Analysis(AnalysisError::Planning(planning)),
            Stage::Analysis(analyze::Stage::Planning),
        );
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

        let failure = Failure::before(
            Error::Analysis(AnalysisError::Limit("properties")),
            Stage::Analysis(analyze::Stage::Planning),
        );
        let value = temporal_failure(&failure);
        assert_eq!(value["code"], "DIVE-L0034");
        assert!(value["span"].is_null() && value["location"].is_null());
        assert!(value["help"].is_string());
        Ok(())
    }
}
