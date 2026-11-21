//! `dive-trace`: the counterexample format and the independent trace
//! validator.
//!
//! The validator checks a counterexample *as data* against a bound
//! [`dive_model::Model`]: that its first state satisfies the initial
//! predicate, that every step satisfies the next-state relation, evaluated as
//! a predicate over the two given states, that the states the search must
//! have explored are inside the model and violate nothing earlier, and that
//! the trace ends as it claims. It shares only the dive-eval evaluator, the
//! value library and the binding stage with the checker, never the search,
//! store or liveness graph. This is deliberate: code such as the scoped
//! temporal walk in [`mod@temporal`] duplicates what `dive-liveness` does
//! rather than sharing it, so that a checker bug cannot also hide itself from
//! validation. Whether a state has a successor (`ENABLED`, deadlock and
//! fairness) is the dive-eval evaluator's `ENABLED`; the states and
//! steps of the counterexample are still replayed and checked here.
//!
//! Named pure periodic words (`TraceDoc::with_properties`) also check all
//! behavior temporal conjuncts and the complete violated-property set.
//! Fairness is decided with the dive-eval evaluator's `ENABLED`. VIEW/symmetry
//! closing evidence and unnamed periodic claims from older documents remain explicit
//! refusals.
//!
//! - [`mod@format`]: the versioned trace format (also TLC's `-dumpTrace json`).
//! - [`value_json`]: dive's tagged JSON values, with documented representation
//!   and mixed-set round-trip limitations.
//! - [`validate()`]: the checks, giving [`Validation::Valid`],
//!   [`Validation::Invalid`] (the first failing state and check) or
//!   [`Validation::Unsupported`] (naming what cannot be checked). A random
//!   draw is refused by name ([`RANDOM_UNSUPPORTED`]), and so is a string
//!   created at run time when strings follow encounter order
//!   ([`RUNTIME_NAMES_UNSUPPORTED`]).
//!   TLC register reads and writes are refused ([`REGISTERS_UNSUPPORTED`])
//!   because their contents can depend on evaluations outside the trace.
//!   [`Session`] reconstructs immutable source/config inputs privately and
//!   supports startup registers through a single-candidate Init operation;
//!   every later register access retains that refusal.
//! - [`tlc`]: reading TLC's lossy JSON by interpretation.
//! - [`stream`]: documents whose states are produced one at a time, written
//!   and validated without holding the whole document.
//! - [`mutate`]: by-construction invalid mutants, the negative controls.
//!
//! Evaluation recurses on the caller's stack: run validation on a thread
//! with [`dive_eval::RECOMMENDED_STACK`], as `dive-api` does.
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//! use dive_trace::{TraceDoc, Validation, validate_doc};
//!
//! let src = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\n\
//!            Init == x = 0\nNext == x' = x + 1\nInv == x < 2\n====\n";
//! let analysis = dive_sema::analyze_source("M.tla", src, &dive_sema::Options::default());
//! let ir = Arc::new(dive_ir::lower(&analysis).unwrap());
//! let cfg = dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\n").into_config().unwrap();
//! let model = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).unwrap();
//!
//! let doc = TraceDoc::parse(r#"{"format": "dive-trace", "version": 1, "variables": ["x"],
//!     "states": [{"action": null, "values": {"x": 0}},
//!                {"action": {"name": "Next"}, "values": {"x": 1}},
//!                {"action": {"name": "Next"}, "values": {"x": 2}}],
//!     "ending": {"kind": "invariant", "name": "Inv"}}"#).unwrap();
//! let (report, _) = validate_doc(&model, &doc, &cfg.model_values);
//! assert_eq!(report.validation, Validation::Valid);
//!
//! // Skipping a state is not a step of the specification.
//! let doc = TraceDoc::behavior(vec![
//!     serde_json::json!({"x": 0}),
//!     serde_json::json!({"x": 2}),
//! ]).unwrap();
//! let (report, _) = validate_doc(&model, &doc, &cfg.model_values);
//! let Validation::Invalid(failure) = report.validation else { panic!() };
//! assert_eq!((failure.state, failure.check), (1, dive_trace::Check::Next));
//! ```
#![forbid(unsafe_code)]

mod candidate_error;
mod choices;
mod diagnostic;
pub mod enabled;
mod endpoint;
pub mod format;
mod initial;
mod invariant_error;
pub mod mutate;
mod next_error;
mod next_temporal;
mod ordered;
mod ordered_prefix;
mod partial;
mod row_demand;
mod session;
mod source_row;
pub mod stream;
pub mod temporal;
pub mod tlc;
mod trace;
mod validate;
pub mod value_json;

use dive_model::Model;

pub use endpoint::{
    CandidateErrorSite, CandidateStateEndpoint, NextStateEndpoint, PartialNextEndpoint,
    PartialStateEndpoint, TemporalSourceEndpoint, TemporalStateEndpoint, ViewSourceEndpoint,
};
pub use format::{Ending, Label, Source, TraceDoc};
pub use invariant_error::{InitialErrorStage, InitialInvariantClass};
pub use ordered_prefix::InvariantErrorClass;
pub use session::Session;
pub use trace::{ShapeError, State, Trace, variables};
pub use validate::{
    COMPOSITION_UNSUPPORTED, Check, DEADLOCK_UNSUPPORTED, Failure, RANDOM_UNSUPPORTED,
    REGISTERS_UNSUPPORTED, RUNTIME_NAMES_UNSUPPORTED, Report, Validation, ValidationProgress,
    ValidationRunError, validate, validate_controlled,
};

/// Reads a document in either format against `model` and validates it.
/// `model_values` are the configuration's model value names
/// (`dive_config::ModelConfig::model_values`), which reading TLC's format
/// needs. Returns the report and the trace as read (for a TLC document, the
/// interpretation the verdict is about; see [`tlc`]). Random draws are refused
/// by name ([`RANDOM_UNSUPPORTED`]).
#[must_use]
pub fn validate_doc(
    model: &Model,
    doc: &TraceDoc,
    model_values: &[String],
) -> (Report, Option<Trace>) {
    // Decoding uses public variable metadata too, so validate structural
    // ownership before reading either document format, not only at replay.
    if let Err(error) = model.validate_context() {
        return (
            Report {
                validation: Validation::Unsupported(error.to_string()),
                notes: Vec::new(),
                length: doc.states.len(),
            },
            None,
        );
    }
    match doc.source {
        Source::Tlc => {
            let r = tlc::validate_tlc(model, doc, model_values);
            (r.report, r.trace)
        }
        Source::Dive => match Trace::decode(model, doc) {
            Ok(t) => (validate(model, &t), Some(t)),
            Err(e) => (
                validate::guard_runtime_names(
                    model,
                    Report {
                        validation: Validation::Invalid(Failure {
                            state: e.state.unwrap_or(0),
                            check: Check::Shape,
                            detail: e.detail,
                        }),
                        notes: Vec::new(),
                        length: doc.states.len(),
                    },
                ),
                None,
            ),
        },
    }
}
