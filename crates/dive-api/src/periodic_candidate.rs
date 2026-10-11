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
pub struct Report<R = periodic_snapshot::Run> {
    run: R,
    witness: Option<Witness>,
}
impl<R> Report<R> {
    /// Actual run, preserving ordered failures, partial passes, options and counters.
    #[must_use]
    pub fn run(&self) -> &R {
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
pub struct Failure<R = periodic_snapshot::Run> {
    /// Typed deciding admission/replay failure.
    pub error: Error,
    /// Actual source-owned run; absent before execution or if a thread loses it.
    pub run: Option<Box<R>>,
}
impl<R> std::fmt::Debug for Failure<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Failure")
            .field("error", &self.error)
            .field("execution_retained", &self.run.is_some())
            .finish()
    }
}
impl<R> std::fmt::Display for Failure<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl<R> std::error::Error for Failure<R> {}

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
fn before<R>(error: Error) -> Failure<R> {
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
    check_owned::<periodic_snapshot::Snapshot>(ir, cfg, options, limits, policy, requests, false)
}
/// Optional configuration-owned execution and witness admission. Absence derives
/// from the original configuration; it never inserts TRUE or a supplied callback.
/// The same private reconstruction and two fresh independent replay paths are used.
/// No witness absence, partial pass or safety EOF certifies temporal Holds, ordinary
/// scheduling equivalence, stopping counts or model completion.
///
/// # Errors
/// The same original binding, source/context, fixed resource and replay failures
/// as [`check`]. Existing mandatory selection remains unchanged.
pub fn check_optional(
    ir: Arc<Ir>,
    cfg: ModelConfig,
    options: CheckOptions,
    limits: phase_graph::Limits,
    policy: periodic_lifecycle::Options,
    requests: dive_check::progress::Options,
) -> Result<Report<periodic_snapshot::OptionalRun>, Box<Failure<periodic_snapshot::OptionalRun>>> {
    std::thread::Builder::new()
        .stack_size(dive_eval::RECOMMENDED_STACK)
        .spawn(move || {
            check_owned::<periodic_snapshot::OptionalSnapshot>(
                &ir, &cfg, &options, limits, policy, requests, false,
            )
        })
        .map_err(|e| before(Error::Thread(e.to_string())))?
        .join()
        .map_err(|_| before(Error::Thread("candidate admission thread failed".into())))?
}
/// Source-owned terminal service before timer/pool joins, followed by the same
/// fresh reconstruction and independent memory/JSON replay as [`check_optional`].
/// A genuinely finished queue skips its configured hook; earlier actual requests
/// still preserve hook/candidate/error precedence. Absence never means Holds.
/// Ordinary checker/CLI scheduling, checkpoint and stopping-count parity refuse.
///
/// # Errors
/// Existing callback-free admission, actual execution/resource failures and
/// independent replay failures. No caller EOF/graph/suspension flag is accepted.
pub fn check_optional_terminal(
    ir: Arc<Ir>,
    cfg: ModelConfig,
    options: CheckOptions,
    limits: phase_graph::Limits,
    policy: periodic_lifecycle::Options,
    requests: dive_check::progress::Options,
) -> Result<Report<periodic_snapshot::OptionalRun>, Box<Failure<periodic_snapshot::OptionalRun>>> {
    std::thread::Builder::new()
        .stack_size(dive_eval::RECOMMENDED_STACK)
        .spawn(move || {
            check_owned::<periodic_snapshot::OptionalSnapshot>(
                &ir, &cfg, &options, limits, policy, requests, true,
            )
        })
        .map_err(|e| before(Error::Thread(e.to_string())))?
        .join()
        .map_err(|_| before(Error::Thread("terminal candidate thread failed".into())))?
}
/// Source-owned optional hook with an additive eligible worker-error transaction.
/// The original worker winner remains a failure; diagnostic faults are retained
/// separately. Successful EOF and deciding temporal candidates use the shared
/// reconstruction and fresh independent memory/JSON replay paths.
///
/// # Errors
/// Original binding/admission/source failure or independent candidate refusal.
/// This diagnostic endpoint never certifies ordinary scheduling or Holds.
pub fn check_optional_stopped(
    ir: Arc<Ir>,
    cfg: ModelConfig,
    options: CheckOptions,
    limits: phase_graph::Limits,
    policy: periodic_lifecycle::Options,
    requests: dive_check::progress::Options,
) -> Result<Report<periodic_snapshot::OptionalRun>, Box<Failure<periodic_snapshot::OptionalRun>>> {
    std::thread::Builder::new()
        .stack_size(dive_eval::RECOMMENDED_STACK)
        .spawn(move || {
            check_owned_mode::<periodic_snapshot::OptionalSnapshot>(
                &ir,
                &cfg,
                &options,
                limits,
                policy,
                requests,
                ExecutionRoute::Stopped,
            )
        })
        .map_err(|e| before(Error::Thread(e.to_string())))?
        .join()
        .map_err(|_| before(Error::Thread("stopped candidate thread failed".into())))?
}
#[derive(Clone, Copy)]
enum ExecutionRoute {
    Requests,
    Terminal,
    Stopped,
}
trait Owned {
    type Run;
    fn bind(ir: Arc<Ir>, cfg: &ModelConfig) -> Result<Self, Box<dive_model::Failure>>
    where
        Self: Sized;
    fn model(&self) -> &dive_model::Model;
    fn execute(
        &self,
        options: &CheckOptions,
        limits: phase_graph::Limits,
        policy: periodic_lifecycle::Options,
        requests: dive_check::progress::Options,
        route: ExecutionRoute,
    ) -> Result<Self::Run, dive_eval::EvalError>;
    fn lifecycle(run: &Self::Run) -> &periodic_lifecycle::Run;
}
macro_rules! owned {
    ($snapshot:ty, $run:ty, $bind:path, $terminal:expr, $stopped:expr) => {
        impl Owned for $snapshot {
            type Run = $run;
            fn bind(ir: Arc<Ir>, cfg: &ModelConfig) -> Result<Self, Box<dive_model::Failure>> {
                $bind(ir, cfg)
            }
            fn model(&self) -> &dive_model::Model {
                self.model()
            }
            fn execute(
                &self,
                options: &CheckOptions,
                limits: phase_graph::Limits,
                policy: periodic_lifecycle::Options,
                requests: dive_check::progress::Options,
                route: ExecutionRoute,
            ) -> Result<Self::Run, dive_eval::EvalError> {
                match route {
                    ExecutionRoute::Terminal => {
                        ($terminal)(self, options, limits, policy, requests)
                    }
                    ExecutionRoute::Stopped => ($stopped)(self, options, limits, policy, requests),
                    ExecutionRoute::Requests => {
                        self.execute_candidate_source(options, limits, policy, requests)
                    }
                }
            }
            fn lifecycle(run: &Self::Run) -> &periodic_lifecycle::Run {
                run.lifecycle()
            }
        }
    };
}
owned!(
    periodic_snapshot::Snapshot,
    periodic_snapshot::Run,
    periodic_snapshot::bind,
    |_: &periodic_snapshot::Snapshot, _, _, _, _| Err(dive_eval::EvalError::unsupported(
        "mandatory terminal endpoint not admitted"
    )),
    |_: &periodic_snapshot::Snapshot, _, _, _, _| Err(dive_eval::EvalError::unsupported(
        "mandatory stopped endpoint not admitted"
    ))
);
owned!(
    periodic_snapshot::OptionalSnapshot,
    periodic_snapshot::OptionalRun,
    periodic_snapshot::bind_optional,
    periodic_snapshot::OptionalSnapshot::execute_terminal_candidate_source,
    periodic_snapshot::OptionalSnapshot::execute_stopped_candidate_source
);
#[allow(
    clippy::too_many_arguments,
    reason = "one shared original rebind/reconstruction/replay pipeline across additive modes"
)]
fn check_owned<S: Owned>(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    options: &CheckOptions,
    limits: phase_graph::Limits,
    policy: periodic_lifecycle::Options,
    requests: dive_check::progress::Options,
    terminal: bool,
) -> Result<Report<S::Run>, Box<Failure<S::Run>>> {
    check_owned_mode::<S>(
        ir,
        cfg,
        options,
        limits,
        policy,
        requests,
        if terminal {
            ExecutionRoute::Terminal
        } else {
            ExecutionRoute::Requests
        },
    )
}
#[allow(
    clippy::too_many_arguments,
    reason = "shared actual candidate reconstruction and independent replay"
)]
fn check_owned_mode<S: Owned>(
    ir: &Arc<Ir>,
    cfg: &ModelConfig,
    options: &CheckOptions,
    limits: phase_graph::Limits,
    policy: periodic_lifecycle::Options,
    requests: dive_check::progress::Options,
    route: ExecutionRoute,
) -> Result<Report<S::Run>, Box<Failure<S::Run>>> {
    let snapshot = S::bind(ir.clone(), cfg).map_err(|e| before(Error::Binding(e)))?;
    let run = snapshot
        .execute(options, limits, policy, requests, route)
        .map_err(|e| before(Error::Input(e)))?;
    let result = admit(cfg, snapshot.model(), S::lifecycle(&run), limits, || {
        S::bind(ir.clone(), cfg).map(|fresh| fresh.model().clone())
    });
    match result {
        Ok(witness) => Ok(Report { run, witness }),
        Err(error) => Err(Box::new(Failure {
            error,
            run: Some(Box::new(run)),
        })),
    }
}

fn admit(
    cfg: &ModelConfig,
    model: &dive_model::Model,
    run: &periodic_lifecycle::Run,
    limits: phase_graph::Limits,
    rebind: impl Fn() -> Result<dive_model::Model, Box<dive_model::Failure>>,
) -> Result<Option<Witness>, Error> {
    let Some(selected) = word::selected(run)? else {
        return Ok(None);
    };
    let trace = word::reconstruct(model, run, &selected, limits)?;
    let doc = trace.to_doc(model).map_err(Error::Document)?;
    // Original atom IDs belong to the execution interner. Decode lossless data
    // into a newly bound interner before independent in-memory replay.
    let fresh_a = rebind().map_err(Error::Binding)?;
    let memory_trace =
        dive_trace::Trace::decode(&fresh_a, &doc).map_err(|e| Error::Document(e.detail))?;
    let memory = dive_trace::validate(&fresh_a, &memory_trace, None);
    if memory.validation != Validation::Valid {
        return Err(Error::Validation(memory));
    }
    let bytes =
        serde_json::to_string(&doc.to_json()).map_err(|e| Error::Document(e.to_string()))?;
    let reparsed = TraceDoc::parse(&bytes).map_err(Error::Document)?;
    let fresh_b = rebind().map_err(Error::Binding)?;
    let json = dive_trace::validate_doc(&fresh_b, &reparsed, &cfg.model_values, None).0;
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
        let mut selected = word::selected(run.lifecycle())
            .expect("selection")
            .expect("actual candidate");
        assert!(word::reconstruct(snapshot.model(), run.lifecycle(), &selected, limits).is_ok());
        let anchor = selected.candidate.anchor;
        selected.written[anchor] = false;
        assert!(matches!(
            word::reconstruct(snapshot.model(), run.lifecycle(), &selected, limits),
            Err(Error::Reconstruction(_))
        ));
        selected.written[anchor] = true;
        let actual = selected.identities[anchor].fingerprint;
        selected.identities[anchor].fingerprint ^= 1;
        assert!(matches!(
            word::reconstruct(snapshot.model(), run.lifecycle(), &selected, limits),
            Err(Error::Reconstruction(_))
        ));
        selected.identities[anchor].fingerprint = actual;
        let mut limited = limits;
        limited.graph.steps = run.lifecycle().backend().graph_inspections();
        assert!(matches!(
            word::reconstruct(snapshot.model(), run.lifecycle(), &selected, limited),
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
    #[test]
    fn genuine_optional_absence_preserves_actual_request_prefix_origin() {
        let source = "---- MODULE Candidate ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == x'=IF x=0 THEN 1 ELSE 1\nLive == <>(x=2)\nHook == FALSE\n====\n";
        for width in [1, 3, 4, 8] {
            let analysis =
                dive_sema::analyze_source("Candidate.tla", source, &dive_sema::Options::default());
            let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
            let cfg =
                dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n")
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
                    check_owned::<periodic_snapshot::OptionalSnapshot>(
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
                        false,
                    )
                });
            assert!(observed, "actual returned prefix rendezvous");
            let report = report.expect("request candidate");
            assert_eq!(report.run().configured_name(), None);
            assert_eq!(report.run().definition(), None);
            assert_eq!(report.run().expression(), None);
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
    #[test]
    fn actual_request_candidate_stops_before_new_terminal_transaction() {
        for width in [1, 3, 4, 8] {
            let source = "---- MODULE TerminalRequest ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == x'=1\nLive == <>(x=2)\nHook == FALSE\n====\n";
            let analysis = dive_sema::analyze_source(
                "TerminalRequest.tla",
                source,
                &dive_sema::Options::default(),
            );
            let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
            let cfg = dive_config::parse(
                "INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n_PERIODIC Hook\n",
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
                    check_owned::<periodic_snapshot::OptionalSnapshot>(
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
                        true,
                    )
                });
            assert!(
                observed,
                "test-only dispatch observes genuine pending request"
            );
            let report = report.expect("actual candidate");
            let run = report.run().lifecycle();
            assert!(run.terminal_service().is_none());
            assert!(run.safety_eof().is_none());
            assert!(run.final_checks().is_empty());
            assert!(run.service_evaluations().is_empty());
            let witness = report.witness().expect("independently admitted word");
            let Origin::Request(service) = witness.origin() else {
                panic!("request origin required");
            };
            assert!(!service.committed().is_empty());
            assert!(!service.resume_requested());
            assert_eq!(witness.memory_validation().validation, Validation::Valid);
            assert_eq!(witness.json_validation().validation, Validation::Valid);
        }
    }
    #[test]
    fn stopped_route_keeps_actual_earlier_request_candidate_before_false_hook() {
        let source = "---- MODULE Candidate ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == x'=IF x=0 THEN 1 ELSE 1\nLive == <>(x=2)\nHook == FALSE\n====\n";
        for width in [1, 3, 4, 8] {
            let a =
                dive_sema::analyze_source("Candidate.tla", source, &dive_sema::Options::default());
            let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
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
                    check_owned_mode::<periodic_snapshot::OptionalSnapshot>(
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
                        ExecutionRoute::Stopped,
                    )
                });
            assert!(observed, "actual returned-prefix request");
            let report = report.expect("actual candidate");
            let run = report.run().lifecycle();
            assert!(run.stopped_service().is_none());
            assert!(run.terminal_service().is_none());
            assert!(run.safety_eof().is_none());
            assert!(run.final_checks().is_empty());
            assert!(run.service_evaluations().is_empty());
            let witness = report.witness().expect("independently replayed candidate");
            let Origin::Request(service) = witness.origin() else {
                panic!("actual request");
            };
            assert!(!service.committed().is_empty());
            assert!(!service.resume_requested());
            assert_eq!(witness.memory_validation().validation, Validation::Valid);
            assert_eq!(witness.json_validation().validation, Validation::Valid);
            for mode in 0..4 {
                let fresh =
                    periodic_snapshot::bind_optional(ir.clone(), &cfg).expect("fresh memory");
                let mut doc = witness.document().clone();
                match mode {
                    0 => {
                        doc.states[0].insert("x".into(), serde_json::json!(99));
                    }
                    1 => doc.properties = vec!["Unknown".into()],
                    2 => {
                        let mut wrong = doc.states[0].clone();
                        wrong.insert("x".into(), serde_json::json!(99));
                        doc.states.push(wrong);
                        doc.labels.push(None);
                        doc.length = Some(doc.states.len());
                        doc.ending = Some(dive_trace::Ending::Lasso { back_to: 0 });
                    }
                    _ => {
                        doc.ending = Some(dive_trace::Ending::Lasso {
                            back_to: doc.states.len(),
                        });
                    }
                }
                let trace = dive_trace::Trace::decode(fresh.model(), &doc).expect("transport");
                assert!(matches!(
                    dive_trace::validate(fresh.model(), &trace, None).validation,
                    Validation::Invalid(_)
                ));
                let bytes = serde_json::to_string(&doc.to_json()).expect("JSON");
                let parsed = TraceDoc::parse(&bytes).expect("parse");
                let fresh = periodic_snapshot::bind_optional(ir.clone(), &cfg).expect("fresh JSON");
                assert!(matches!(
                    dive_trace::validate_doc(fresh.model(), &parsed, &cfg.model_values, None)
                        .0
                        .validation,
                    Validation::Invalid(_)
                ));
            }
        }
    }
}
