//! Private-source temporal witness admission (ADR 0070).
//!
//! This creates its own configuration snapshot and request service. No caller
//! graph, trace, validation flag or clock establishes authority. An admitted
//! witness certifies a concrete violating behavior, never ordinary TLC schedule
//! equivalence, stopping-count equality, temporal absence or model completion.
use std::sync::Arc;

use dive_check::CheckOptions;
use dive_config::ModelConfig;
use dive_ir::Ir;
use dive_liveness::{periodic_lifecycle, periodic_snapshot, phase_graph};
use dive_trace::{Report as ValidationReport, TraceDoc, Validation};

#[path = "periodic_candidate_word.rs"]
mod word;

/// Actual deciding origin, retaining request service separately from safety EOF.
#[derive(Clone, Debug)]
pub enum Origin {
    /// Real internally produced request, after actual prefix/frontier publication.
    Request(dive_liveness::periodic_service::Invocation),
    /// Mandatory final selection after genuine successful safety EOF.
    Eof(dive_check::Stats),
}
/// Privately admitted actual witness; callers cannot supply its validation flags.
#[derive(Clone, Debug)]
pub struct Witness {
    document: TraceDoc,
    memory: ValidationReport,
    json: ValidationReport,
    origin: Origin,
    order: (usize, usize),
    selection: dive_liveness::tlc_witness::Candidate,
}
impl Witness {
    /// Lossless word admitted against a freshly bound independent model.
    #[must_use]
    pub fn document(&self) -> &TraceDoc {
        &self.document
    }
    /// Fresh in-memory replay after rehoming original material atoms.
    #[must_use]
    pub fn memory_validation(&self) -> &ValidationReport {
        &self.memory
    }
    /// Separately rebound and serialized/parsed JSON replay.
    #[must_use]
    pub fn json_validation(&self) -> &ValidationReport {
        &self.json
    }
    /// Actual request stop or genuinely reached EOF, never inferred from emptiness.
    #[must_use]
    pub fn origin(&self) -> &Origin {
        &self.origin
    }
    /// Actual original OOS and PEM order.
    #[must_use]
    pub fn order(&self) -> (usize, usize) {
        self.order
    }
    /// Actual product-node selection before contiguous fingerprint packing.
    #[must_use]
    pub fn selection(&self) -> &dive_liveness::tlc_witness::Candidate {
        &self.selection
    }
}
/// Producer-owned diagnostic execution and optional independently admitted word.
pub struct Report {
    run: periodic_snapshot::Run,
    witness: Option<Witness>,
}
impl Report {
    /// Actual run, preserving ordered failures, partial passes, options and counters.
    #[must_use]
    pub fn run(&self) -> &periodic_snapshot::Run {
        &self.run
    }
    /// A selected witness admitted twice; absence never means temporal holds.
    #[must_use]
    pub fn witness(&self) -> Option<&Witness> {
        self.witness.as_ref()
    }
    /// Neither an admitted word nor successful EOF certifies ordinary scheduling.
    #[must_use]
    pub const fn model_proof(&self) -> bool {
        false
    }
}
/// Named admission, reconstruction or independent replay failure.
#[derive(Debug)]
pub enum Error {
    /// Original callback-free snapshot binding error.
    Binding(Box<dive_model::Failure>),
    /// Actual profile/context/policy admission before execution.
    Input(dive_eval::EvalError),
    /// Original source/selection authority was missing or inconsistent.
    Reconstruction(String),
    /// Existing fixed entry/inspection ceiling was exhausted.
    Limit(&'static str),
    /// Original word attribution could not be established.
    Attribution(String),
    /// A fresh independent replay refused or rejected the actual word.
    Validation(ValidationReport),
    /// Lossless document encoding, decoding or parsing failed.
    Document(String),
    /// A joining thread lost execution evidence.
    Thread(String),
}
impl Error {
    /// Stable diagnostic; not an ordinary TLC deciding-class claim.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Binding(_) => "DIVE-API-P001",
            Self::Input(_) => "DIVE-API-P002",
            Self::Reconstruction(_) => "DIVE-API-P003",
            Self::Limit(_) => "DIVE-API-P004",
            Self::Attribution(_) => "DIVE-API-P005",
            Self::Validation(_) => "DIVE-API-P006",
            Self::Document(_) => "DIVE-API-P007",
            Self::Thread(_) => "DIVE-API-P008",
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {self:?}", self.code())
    }
}
impl std::error::Error for Error {}
/// Noncertifying failure retaining its actual run when execution took place.
pub struct Failure {
    /// Typed deciding admission/replay failure.
    pub error: Error,
    /// Actual source-owned run; absent before execution or if a thread loses it.
    pub run: Option<Box<periodic_snapshot::Run>>,
}
impl std::fmt::Debug for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Failure")
            .field("error", &self.error)
            .field("execution_retained", &self.run.is_some())
            .finish()
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for Failure {}

/// Execute an internally owned configuration snapshot and independently admit
/// only its actual selected concrete word. An ordinary checking route is never
/// enabled: diagnostic stops/partial passes remain in [`Report::run`], and an
/// absent witness does not mean holds. Existing source/graph/validator bounds,
/// purity, TLC profile and configured hook restrictions remain unchanged.
///
/// # Errors
/// Callback-free input/binding refusal; missing concrete/product authority,
/// collisions, fixed bounds, document failure or independent replay refusal.
/// Both replays bind fresh evaluators and name tables; neither accepts backend
/// truth values as validation. A separate stack-sized thread is used.
pub fn check(
    ir: Arc<Ir>,
    cfg: ModelConfig,
    options: CheckOptions,
    limits: phase_graph::Limits,
    policy: periodic_lifecycle::Options,
    requests: dive_check::progress::Options,
) -> Result<Report, Box<Failure>> {
    std::thread::Builder::new()
        .stack_size(dive_eval::RECOMMENDED_STACK)
        .spawn(move || check_inner(&ir, &cfg, &options, limits, policy, requests))
        .map_err(|e| before(Error::Thread(e.to_string())))?
        .join()
        .map_err(|_| before(Error::Thread("candidate admission thread failed".into())))?
}
fn before(error: Error) -> Failure {
    Failure { error, run: None }
}
fn check_inner(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    options: &CheckOptions,
    limits: phase_graph::Limits,
    policy: periodic_lifecycle::Options,
    requests: dive_check::progress::Options,
) -> Result<Report, Box<Failure>> {
    let snapshot =
        periodic_snapshot::bind(ir.clone(), cfg).map_err(|e| before(Error::Binding(e)))?;
    let run = snapshot
        .execute_candidate_source(options, limits, policy, requests)
        .map_err(|e| before(Error::Input(e)))?;
    let result = admit(ir, cfg, snapshot.model(), &run, limits);
    match result {
        Ok(witness) => Ok(Report { run, witness }),
        Err(error) => Err(Box::new(Failure {
            error,
            run: Some(Box::new(run)),
        })),
    }
}
fn admit(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    model: &dive_model::Model,
    run: &periodic_snapshot::Run,
    limits: phase_graph::Limits,
) -> Result<Option<Witness>, Error> {
    let Some(selected) = word::selected(run)? else {
        return Ok(None);
    };
    let trace = word::reconstruct(model, run, &selected, limits)?;
    let doc = trace.to_doc(model).map_err(Error::Document)?;
    // Original atom IDs belong to the execution interner. Decode lossless data
    // into a newly bound interner before independent in-memory replay.
    let fresh_a = periodic_snapshot::bind(ir.clone(), cfg).map_err(Error::Binding)?;
    let memory_trace =
        dive_trace::Trace::decode(fresh_a.model(), &doc).map_err(|e| Error::Document(e.detail))?;
    let memory = dive_trace::validate(fresh_a.model(), &memory_trace, None);
    if memory.validation != Validation::Valid {
        return Err(Error::Validation(memory));
    }
    let bytes =
        serde_json::to_string(&doc.to_json()).map_err(|e| Error::Document(e.to_string()))?;
    let reparsed = TraceDoc::parse(&bytes).map_err(Error::Document)?;
    let fresh_b = periodic_snapshot::bind(ir.clone(), cfg).map_err(Error::Binding)?;
    let json = dive_trace::validate_doc(fresh_b.model(), &reparsed, &cfg.model_values, None).0;
    if json.validation != Validation::Valid {
        return Err(Error::Validation(json));
    }
    Ok(Some(Witness {
        document: reparsed,
        memory,
        json,
        origin: selected.origin,
        order: selected.order,
        selection: selected.candidate,
    }))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::panic,
        reason = "controlled actual request provenance and strict negative replay"
    )]
    use super::*;
    #[test]
    fn private_actual_selection_rejects_unwritten_missing_and_exhausted_authority() {
        let source = "---- MODULE Private ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == x'=1\nLive == <>(x=2)\nHook == TRUE\n====\n";
        let a = dive_sema::analyze_source("Private.tla", source, &dive_sema::Options::default());
        let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
        let cfg = dive_config::parse(
            "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
        )
        .into_config()
        .expect("cfg");
        let snapshot = periodic_snapshot::bind(ir, &cfg).expect("snapshot");
        let limits = phase_graph::Limits::default();
        let run = snapshot
            .execute_candidate_source(
                &CheckOptions::tlc(0, 0),
                limits,
                periodic_lifecycle::Options::default(),
                dive_check::progress::Options {
                    interval: std::time::Duration::from_secs(3600),
                    force: false,
                },
            )
            .expect("actual EOF");
        let mut selected = word::selected(&run)
            .expect("selection")
            .expect("actual candidate");
        assert!(word::reconstruct(snapshot.model(), &run, &selected, limits).is_ok());
        let anchor = selected.candidate.anchor;
        selected.written[anchor] = false;
        assert!(matches!(
            word::reconstruct(snapshot.model(), &run, &selected, limits),
            Err(Error::Reconstruction(_))
        ));
        selected.written[anchor] = true;
        let actual = selected.identities[anchor].fingerprint;
        selected.identities[anchor].fingerprint ^= 1;
        assert!(matches!(
            word::reconstruct(snapshot.model(), &run, &selected, limits),
            Err(Error::Reconstruction(_))
        ));
        selected.identities[anchor].fingerprint = actual;
        let mut limited = limits;
        limited.graph.steps = run.lifecycle().backend().graph_inspections();
        assert!(matches!(
            word::reconstruct(snapshot.model(), &run, &selected, limited),
            Err(Error::Limit(_))
        ));
    }
    #[test]
    fn actual_request_candidate_is_admitted_before_configured_false_hook() {
        let source = "---- MODULE Candidate ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == x'=IF x=0 THEN 1 ELSE 1\nLive == <>(x=2)\nHook == FALSE\n====\n";
        for width in [1, 3, 4, 8] {
            let analysis =
                dive_sema::analyze_source("Candidate.tla", source, &dive_sema::Options::default());
            let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
            let cfg = dive_config::parse(
                "INIT Init\nNEXT Next\nPROPERTY Live\n_PERIODIC Hook\nCHECK_DEADLOCK FALSE\n",
            )
            .into_config()
            .expect("cfg");
            let options = CheckOptions {
                workers: width,
                chaos: Some(7),
                ..CheckOptions::tlc(0, 0)
            };
            let (report, observed) =
                dive_check::progress::testing::with_returned_prefix_request(|| {
                    // Only this unit control bypasses the outer stack thread so the
                    // existing explicit test scheduler is installed on the actual
                    // execution caller. No caller supplies a Request or candidate.
                    check_inner(
                        &ir,
                        &cfg,
                        &options,
                        phase_graph::Limits::default(),
                        periodic_lifecycle::Options {
                            ratio_limit: f64::INFINITY,
                            ..periodic_lifecycle::Options::default()
                        },
                        dive_check::progress::Options {
                            interval: std::time::Duration::from_millis(1),
                            force: false,
                        },
                    )
                });
            assert!(observed, "actual returned prefix rendezvous");
            let report = report.expect("request candidate");
            let witness = report.witness().expect("validated candidate");
            let Origin::Request(service) = witness.origin() else {
                panic!("request origin");
            };
            assert!(!service.committed().is_empty());
            assert!(!service.resume_requested());
            assert!(report.run().lifecycle().service_evaluations().is_empty());
            assert!(report.run().lifecycle().safety_eof().is_none());
            assert_eq!(witness.memory_validation().validation, Validation::Valid);
            assert_eq!(witness.json_validation().validation, Validation::Valid);
        }
    }
}
