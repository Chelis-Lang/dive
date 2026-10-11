//! `dive-trace`: the counterexample format and the independent trace
//! validator (ADR 0016).
//!
//! The validator checks a counterexample *as data* against a bound
//! [`dive_model::Model`]: that its first state satisfies the initial
//! predicate, that every step satisfies the next-state relation, evaluated as
//! a predicate over the two given states, that the states the search must
//! have explored are inside the model and violate nothing earlier, and that
//! the trace ends as it claims. It shares only the core evaluator, the value
//! library and the binding stage with the checker: it is built without
//! `dive-eval`'s successor enumerator (the `enumerate` feature), so it cannot call the checker
//! generator, and a bug in successor generation cannot make it
//! accept a trace the specification does not allow. `cargo xtask
//! check-layering` and the gate's standalone `cargo build -p dive-trace`
//! enforce that.
//!
//! Named pure periodic words (`TraceDoc::with_properties`) also check all
//! behavior temporal conjuncts and the complete violated-property set.
//! Fairness with recurring actions or finite independently decided enabledness
//! is supported. Missing finite covers, VIEW/symmetry closing evidence and
//! unnamed legacy periodic claims remain explicit refusals.
//!
//! - [`mod@format`]: the versioned trace format (also TLC's `-dumpTrace json`).
//! - [`value_json`]: dive's tagged JSON values, with documented representation
//!   and mixed-set round-trip limitations.
//! - [`validate()`]: the checks, giving [`Validation::Valid`],
//!   [`Validation::Invalid`] (the first failing state and check) or
//!   [`Validation::Unsupported`] (naming what cannot be checked). Given the
//!   run's seed and fingerprint polynomial ([`Replay`]), it replays the
//!   run's random draws in the initial predicate and the steps.
//! - [`tlc`]: reading TLC's lossy JSON by interpretation.
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
//! let (report, _) = validate_doc(&model, &doc, &cfg.model_values, None);
//! assert_eq!(report.validation, Validation::Valid);
//!
//! // Skipping a state is not a step of the specification.
//! let doc = TraceDoc::behavior(vec![
//!     serde_json::json!({"x": 0}),
//!     serde_json::json!({"x": 2}),
//! ]).unwrap();
//! let (report, _) = validate_doc(&model, &doc, &cfg.model_values, None);
//! let Validation::Invalid(failure) = report.validation else { panic!() };
//! assert_eq!((failure.state, failure.check), (1, dive_trace::Check::Next));
//! ```
#![forbid(unsafe_code)]

pub mod exists;
pub mod format;
pub mod mutate;
mod random_position;
mod runtime_order;
pub mod temporal;
pub mod tlc;
mod trace;
mod validate;
pub mod value_json;
pub mod value_repr;

use dive_model::Model;

pub use format::{Ending, Label, Source, TraceDoc, TraceEncoding};
pub use trace::{ShapeError, State, Trace, variables};
pub use validate::{
    Check, DEADLOCK_UNSUPPORTED, Failure, RANDOM_UNPLACED, RUNTIME_NAMES_UNSUPPORTED, Replay,
    Report, Validation, validate,
};

/// Reads a document in either format against `model` and validates it.
/// `model_values` are the configuration's model value names
/// (`dive_config::ModelConfig::model_values`), which reading TLC's format
/// needs. Returns the report and the trace as read (for a TLC document, the
/// interpretation the verdict is about; see [`tlc`]). `replay` is the
/// checking run's random streams, if known (see [`validate()`]).
#[must_use]
pub fn validate_doc(
    model: &Model,
    doc: &TraceDoc,
    model_values: &[String],
    replay: Option<&Replay>,
) -> (Report, Option<Trace>) {
    match doc.source {
        Source::Tlc => {
            let r = tlc::validate_tlc(model, doc, model_values, replay);
            (r.report, r.trace)
        }
        Source::Dive | Source::DiveRepresentation => match Trace::decode(model, doc) {
            Ok(t) => (validate(model, &t, replay), Some(t)),
            Err(e) => (
                Report {
                    validation: Validation::Invalid(Failure {
                        state: e.state.unwrap_or(0),
                        check: Check::Shape,
                        detail: e.detail,
                    }),
                    notes: Vec::new(),
                    length: doc.states.len(),
                },
                None,
            ),
        },
    }
}
