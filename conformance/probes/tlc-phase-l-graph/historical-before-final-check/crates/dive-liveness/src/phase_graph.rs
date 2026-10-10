//! Concrete phase-L commits into a bounded native null-tableau graph.
//!
//! This staged path captures through the actual safety pool, then commits source
//! receipts in discovery order. It certifies no strict temporal verdict or TLC
//! stop counters: capture and predicate work are separate observed stages.

use std::collections::BTreeMap;

use dive_check::capture::{self, Exploration};
use dive_check::{CheckOptions, Stats, Workers};
use dive_model::Model;

use crate::Graph;
use crate::phase_l::{self, Incoming, Receipt, Session};
use crate::search::Acceptance;
use crate::tlc_plan::PossibleErrorModel;

/// Fixed entry/work bounds for native graph retention. These are not byte bounds.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Existing safety-capture bounds, never raised by this stage.
    pub capture: capture::Limits,
    /// Existing bounded receipt evaluation limits.
    pub phase_l: phase_l::Limits,
    /// Existing product limits, used for nodes, edges, bin cells and inspections.
    pub graph: crate::product::Limits,
    /// Maximum requested safety pool width; a named structural resource bound.
    pub workers: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            capture: capture::Limits::default(),
            phase_l: phase_l::Limits::default(),
            graph: crate::product::Limits::default(),
            workers: 64,
        }
    }
}

/// Which observed stage failed; no temporal absence-of-error claim is implied.
#[derive(Clone, Debug)]
pub enum Error {
    /// Conservative scoped planning or unsupported graph execution shape.
    Phase(Box<phase_l::Failure>),
    /// Named structural bounds, checked before retaining/evaluating graph bins.
    Limit(&'static str),
    /// Source capture stopped; its safety result is retained separately.
    Capture(Box<dive_check::CheckResult>),
    /// A worker failure lost execution evidence.
    Execution(String),
    /// An internal receipt/graph inconsistency; never an empty language.
    Input(String),
}
impl Error {
    /// Stable diagnostic code; no model-checking verdict is implied.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Phase(e) => e.error.code(),
            Self::Limit(_) => "DIVE-L0050",
            Self::Input(_) => "DIVE-L0051",
            Self::Capture(_) => "DIVE-L0052",
            Self::Execution(_) => "DIVE-L0053",
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Phase(e) => e.error.fmt(f),
            Self::Limit(e) => write!(f, "phase-L graph resource limit: {e}"),
            Self::Capture(e) => write!(f, "phase-L safety capture stopped: {:?}", e.verdict),
            Self::Execution(e) => write!(f, "phase-L execution evidence unavailable: {e}"),
            Self::Input(e) => write!(f, "phase-L graph inconsistency: {e}"),
        }
    }
}
impl std::error::Error for Error {}

/// One actual committed transition, in TLC slot/insertion order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transition {
    /// Fingerprint identity used by the native liveness graph.
    pub target: u64,
    /// Null-tableau identity is exactly -1.
    pub tableau: i32,
    /// Original slot's action bins, in OOS order.
    pub action: Vec<bool>,
}

/// One persisted null-tableau graph node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    /// TLC fingerprint identity; concrete source states remain in capture evidence.
    pub fingerprint: u64,
    /// Source state bins in OOS order.
    pub state: Vec<bool>,
    /// Native outgoing transitions in their actual insertion order.
    pub transitions: Vec<Transition>,
}

/// A real graph commit, kept distinct from the diagnostic receipt it consumed.
#[derive(Clone, Debug)]
pub struct Commit {
    /// Source discovery id in the completed safety capture.
    pub source: usize,
    /// Internally derived source fingerprint.
    pub fingerprint: u64,
    /// Exact diagnostic bin/slot receipt consumed by this commit.
    /// Its own `graph_insertion` remains false; it alone proves no insertion.
    pub receipt: Receipt,
    /// The commit actually inserted/revised a native graph node.
    pub graph_insertion: bool,
}

/// A completed, concretely labelled native graph. This is not a TLC verdict.
#[derive(Clone, Debug)]
pub struct Complete {
    /// Complete safety exploration, including accepted original values and workers.
    pub capture: Box<capture::Complete>,
    /// Native nodes in first write order. Fingerprint identity may merge states.
    nodes: Vec<Node>,
    /// Initial graph fingerprints, preserving TLC registration order/duplicates.
    roots: Vec<u64>,
    /// Actual source commits in the one logical capacity history.
    commits: Vec<Commit>,
    /// Ordered original possible-error-model bin references.
    pems: Vec<PossibleErrorModel>,
    /// Actual predicate/collection work, distinct from capture counters/workers.
    pub phase_work: phase_l::Progress,
    /// Neither this graph nor its diagnostic receipt claims a model proof.
    model_proof: bool,
    graph_limits: crate::product::Limits,
    configured_properties: Vec<dive_model::Named>,
    role_names: Vec<String>,
}

/// Failure evidence from completed capture and any already written native nodes.
#[derive(Clone, Debug)]
pub struct Failure {
    /// Original phase/error/refusal, with scope and prefix information retained.
    pub error: Error,
    /// Observed safety capture counters; never TLC temporal stop counters.
    pub capture_stats: Option<Stats>,
    /// Actual safety pool width, absent if execution evidence was lost.
    pub capture_workers: Option<Workers>,
    /// Observed receipt evaluation work; this stage executes inline.
    pub phase_work: phase_l::Progress,
    /// Nodes that were actually written before the failure.
    pub nodes: Vec<Node>,
    /// Actual completed graph commits; no unfinished source is counted here.
    pub commits: Vec<Commit>,
}

fn failure(error: Error, session: &Session<'_>, requested: usize) -> Box<Failure> {
    Box::new(Failure {
        error,
        capture_stats: None,
        capture_workers: Some(Workers {
            requested,
            used: 0,
            sequential: false,
            reason: Some("capture not started".into()),
        }),
        phase_work: session.progress().clone(),
        nodes: Vec::new(),
        commits: Vec::new(),
    })
}

fn bounded(n: Option<usize>, maximum: usize, name: &'static str) -> Result<usize, Error> {
    n.filter(|n| *n <= maximum).ok_or(Error::Limit(name))
}

/// Capture and commit one pure null-tableau OOS using accepted concrete values.
/// This is an explicit staged API: it does not enable ordinary strict liveness.
/// The safety pool executes at its reported width; receipt evaluation/graph
/// insertion executes inline in source order and reports that work separately.
///
/// Capture is completed before temporal evaluation. A stopped capture is retained
/// as such, without claiming TLC's temporal-versus-safety first-error precedence.
/// Multiple OOS/tableau revisitation, VIEW/symmetry and stateful/effectful temporal
/// predicates remain explicit refusals. No default or resource limit is raised.
///
/// ```
/// use std::sync::Arc;
/// use dive_check::CheckOptions;
/// use dive_liveness::phase_graph;
/// let src = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\nLive == []<>~x\n====\n";
/// let a = dive_sema::analyze_source("M.tla", src, &Default::default());
/// let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
/// let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\n").into_config().expect("config");
/// let m = dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("model");
/// let c = phase_graph::compile(&m, &CheckOptions::tlc(0, 0), Default::default()).expect("bounded graph");
/// assert!(c.commits()[0].graph_insertion);
/// assert!(!c.commits()[0].receipt.graph_insertion);
/// assert!(!c.model_proof());
/// assert!(c.product_inputs(0).is_ok());
/// ```
///
/// # Errors
/// Planning/refusal, actual capture stop, named entry/work bound, bin error,
/// worker failure or an internal graph inconsistency, with observed prefix data.
#[allow(
    clippy::too_many_lines,
    reason = "staged commit preserves safety, bin and persisted-prefix evidence in explicit order"
)]
pub fn compile(
    model: &Model,
    options: &CheckOptions,
    limits: Limits,
) -> Result<Complete, Box<Failure>> {
    if options.workers == 0 || options.workers > limits.workers {
        return Err(Box::new(Failure {
            error: if options.workers == 0 {
                Error::Input("zero requested capture worker width".into())
            } else {
                Error::Limit("requested capture worker width")
            },
            capture_stats: None,
            capture_workers: Some(Workers {
                requested: options.workers,
                used: 0,
                sequential: false,
                reason: Some("parameter check before planning/capture".into()),
            }),
            phase_work: phase_l::Progress::default(),
            nodes: Vec::new(),
            commits: Vec::new(),
        }));
    }
    if model
        .evaluator
        .validate_model_context(&model.ir, &model.names)
        .is_err()
    {
        return Err(Box::new(Failure {
            error: Error::Input("foreign model IR or name table before graph planning".into()),
            capture_stats: None,
            capture_workers: Some(Workers {
                requested: options.workers,
                used: 0,
                sequential: false,
                reason: Some("model ownership check before planning/capture".into()),
            }),
            phase_work: phase_l::Progress::default(),
            nodes: Vec::new(),
            commits: Vec::new(),
        }));
    }
    let mut session = Session::new(model, options.fp_index, 1, limits.phase_l).map_err(|e| {
        Box::new(Failure {
            phase_work: e.progress.clone(),
            error: Error::Phase(e),
            capture_stats: None,
            capture_workers: Some(Workers {
                requested: options.workers,
                used: 0,
                sequential: false,
                reason: Some("planning before capture".into()),
            }),
            nodes: Vec::new(),
            commits: Vec::new(),
        })
    })?;
    if session.orders().len() != 1 || session.has_tableau() {
        return Err(failure(
            Error::Phase(Box::new(phase_l::Failure {
                error: phase_l::Error::Unsupported("graph commits require one null-tableau OOS"),
                progress: session.progress().clone(),
                partial: None,
            })),
            &session,
            options.workers,
        ));
    }
    let order = session.orders()[0].clone();
    let capture = capture::explore_concrete_parallel_with_evidence(model, options, limits.capture)
        .map_err(|e| {
            let mut f = failure(Error::Execution(e), &session, options.workers);
            f.capture_workers = None;
            f
        })?;
    let capture = match capture {
        Exploration::Complete(c) => c,
        Exploration::Stopped(stop) => {
            let mut f = failure(Error::Capture(stop.clone()), &session, options.workers);
            f.capture_stats = stop.stats;
            f.capture_workers = Some(stop.workers.clone());
            return Err(f);
        }
    };
    let mut evidence = failure(
        Error::Input("uninitialized graph failure".into()),
        &session,
        options.workers,
    );
    evidence.capture_stats = Some(capture.stats);
    evidence.capture_workers = Some(capture.workers.clone());
    let result = (|| {
        let states = capture.graph.states.len();
        bounded(Some(states), limits.graph.vertices, "graph vertices")?;
        if capture.graph.concrete.len() != states || capture.graph.edges.len() != states {
            return Err(Error::Input("capture source dimensions".into()));
        }
        let edge_bound = capture
            .graph
            .concrete
            .iter()
            .try_fold(0usize, |n, row| n.checked_add(row.len().checked_add(1)?));
        let edge_bound = bounded(edge_bound, limits.graph.edges, "graph transitions")?;
        bounded(
            states.checked_mul(order.check_state.len()).and_then(|n| {
                edge_bound
                    .checked_mul(order.check_action.len())?
                    .checked_add(n)
            }),
            limits.graph.acceptance_cells,
            "graph state/action bin cells",
        )?;
        bounded(
            states.checked_add(edge_bound),
            limits.graph.steps,
            "graph source/transition inspections",
        )?;
        // All graph retention upper bounds are charged before predicate calls.
        let mut fingerprints = Vec::with_capacity(states);
        for state in &capture.graph.states {
            fingerprints.push(session.concrete_fingerprint(state).map_err(Error::Phase)?);
        }
        let mut roots = Vec::with_capacity(capture.graph.roots.len());
        for root in &capture.graph.roots {
            let state = capture
                .graph
                .states
                .get(*root)
                .ok_or_else(|| Error::Input("capture initial reference".into()))?;
            session.initial(state).map_err(Error::Phase)?;
            roots.push(
                *fingerprints
                    .get(*root)
                    .ok_or_else(|| Error::Input("initial fingerprint reference".into()))?,
            );
        }
        let mut written = BTreeMap::<u64, usize>::new();
        for (source, state) in capture.graph.states.iter().enumerate() {
            let incoming: Vec<_> = capture.graph.concrete[source]
                .iter()
                .map(|edge| Incoming {
                    state: edge.state.clone(),
                    ordinal: edge.ordinal,
                    label: Some(edge.label.clone()),
                })
                .collect();
            let receipt = session.source(state, &incoming).map_err(Error::Phase)?;
            let fp = fingerprints[source];
            let inserted = insert_source(
                &mut evidence.nodes,
                &mut written,
                fp,
                &receipt,
                order.check_state.len(),
                order.check_action.len(),
            )?;
            evidence.commits.push(Commit {
                source,
                fingerprint: fp,
                receipt,
                graph_insertion: inserted,
            });
        }
        Ok(roots)
    })();
    evidence.phase_work = session.progress().clone();
    let roots = match result {
        Ok(roots) => roots,
        Err(error) => {
            evidence.error = error;
            return Err(evidence);
        }
    };
    Ok(Complete {
        capture,
        roots,
        nodes: evidence.nodes,
        commits: evidence.commits,
        pems: order.pems,
        phase_work: evidence.phase_work,
        model_proof: false,
        graph_limits: limits.graph,
        configured_properties: session.configured_properties().to_vec(),
        role_names: session.role_names().to_vec(),
    })
}

fn insert_source(
    nodes: &mut Vec<Node>,
    written: &mut BTreeMap<u64, usize>,
    fp: u64,
    receipt: &Receipt,
    state_bins: usize,
    action_bins: usize,
) -> Result<bool, Error> {
    let old = written.get(&fp).copied();
    let mut node = old.map_or_else(
        || Node {
            fingerprint: fp,
            state: vec![false; state_bins],
            transitions: Vec::new(),
        },
        |i| nodes[i].clone(),
    );
    for check in &receipt.checks {
        if check.kind == crate::formula::AtomKind::State {
            *node
                .state
                .get_mut(check.bin)
                .ok_or_else(|| Error::Input("state bin reference".into()))? = check.holds;
        }
    }
    let before = node.transitions.len();
    for (position, next) in receipt.successors.iter().enumerate() {
        // Match ptr == -1 OR !transExists. Unwritten target duplicates
        // remain distinct even if their fingerprints coincide.
        if !written.contains_key(&next.fingerprint)
            || !node
                .transitions
                .iter()
                .any(|t| t.target == next.fingerprint)
        {
            let mut action = vec![false; action_bins];
            for check in &receipt.checks {
                if check.successor == Some(position) {
                    *action
                        .get_mut(check.bin)
                        .ok_or_else(|| Error::Input("action bin reference".into()))? = check.holds;
                }
            }
            node.transitions.push(Transition {
                target: next.fingerprint,
                tableau: -1,
                action,
            });
        }
    }
    let inserted = (before == 0 && node.transitions.is_empty()) || before < node.transitions.len();
    if inserted {
        if let Some(i) = old {
            nodes[i] = node;
        } else {
            written.insert(fp, nodes.len());
            nodes.push(node);
        }
    }
    Ok(inserted)
}

impl Complete {
    /// Actual graph nodes in first write order, retaining all transition labels.
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
    /// Registered initial fingerprints, separately from derived property roles.
    #[must_use]
    pub fn roots(&self) -> &[u64] {
        &self.roots
    }
    /// Actual committed sources, with immutable receipt/evaluation evidence.
    #[must_use]
    pub fn commits(&self) -> &[Commit] {
        &self.commits
    }
    /// Original configured property identities; never derived role substitutions.
    #[must_use]
    pub fn configured_properties(&self) -> &[dive_model::Named] {
        &self.configured_properties
    }
    /// Derived PROPERTY-role identities, kept separate from configured identities.
    #[must_use]
    pub fn role_names(&self) -> &[String] {
        &self.role_names
    }
    /// This staged graph never claims a complete temporal model proof.
    #[must_use]
    pub fn model_proof(&self) -> bool {
        self.model_proof
    }

    /// Produce actual graph/PEM acceptance inputs from committed bin data.
    /// This performs no SCC search, witness selection or independent validation.
    ///
    /// # Errors
    /// Missing written target/root, malformed PEM reference or graph shape.
    #[allow(
        clippy::too_many_lines,
        reason = "project bounded PEM bin references without an implicit acceptance certificate"
    )]
    pub fn product_inputs(&self, pem: usize) -> Result<(Graph, Acceptance), Error> {
        let pem = self
            .pems
            .get(pem)
            .ok_or_else(|| Error::Input("PEM reference".into()))?;
        let edges_count = self
            .nodes
            .iter()
            .try_fold(0usize, |n, node| n.checked_add(node.transitions.len()));
        let edges_count = bounded(edges_count, self.graph_limits.edges, "product edges")?;
        bounded(
            Some(self.nodes.len()),
            self.graph_limits.vertices,
            "product vertices",
        )?;
        bounded(
            self.nodes
                .len()
                .checked_mul(pem.ae_state.len())
                .and_then(|n| {
                    edges_count
                        .checked_mul(pem.ae_action.len().checked_add(1)?)?
                        .checked_add(n)
                }),
            self.graph_limits.acceptance_cells,
            "product acceptance cells",
        )?;
        bounded(
            self.nodes.len().checked_add(edges_count),
            self.graph_limits.steps,
            "product inspections",
        )?;
        let ids: BTreeMap<_, _> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (node.fingerprint, i))
            .collect();
        let edges = self
            .nodes
            .iter()
            .map(|node| {
                node.transitions
                    .iter()
                    .map(|t| {
                        ids.get(&t.target)
                            .copied()
                            .ok_or_else(|| Error::Input("unwritten graph target".into()))
                    })
                    .collect()
            })
            .collect::<Result<Vec<Vec<_>>, _>>()?;
        let roots = self
            .roots
            .iter()
            .map(|fp| {
                ids.get(fp)
                    .copied()
                    .ok_or_else(|| Error::Input("unwritten graph root".into()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let graph = Graph::new(edges, roots).map_err(|e| Error::Input(e.to_string()))?;
        let action = |bins: &[usize]| {
            self.nodes
                .iter()
                .map(|node| {
                    node.transitions
                        .iter()
                        .map(|t| {
                            bins.iter().try_fold(true, |v, bin| {
                                t.action
                                    .get(*bin)
                                    .copied()
                                    .map(|b| v && b)
                                    .ok_or_else(|| Error::Input("PEM action bin".into()))
                            })
                        })
                        .collect()
                })
                .collect::<Result<Vec<Vec<bool>>, Error>>()
        };
        let recurrent = action(&pem.ea_action)?;
        let actions = pem
            .ae_action
            .iter()
            .map(|bin| action(&[*bin]))
            .collect::<Result<Vec<_>, _>>()?;
        let states = pem
            .ae_state
            .iter()
            .map(|bin| {
                self.nodes
                    .iter()
                    .map(|node| {
                        node.state
                            .get(*bin)
                            .copied()
                            .ok_or_else(|| Error::Input("PEM state bin".into()))
                    })
                    .collect()
            })
            .collect::<Result<Vec<Vec<bool>>, Error>>()?;
        Ok((
            graph,
            Acceptance {
                recurrent,
                states,
                actions,
            },
        ))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "controlled pinned collision projection")]
mod written_controls {
    use super::*;
    use crate::formula::AtomKind;
    use phase_l::{Check, Representative};
    use std::sync::Arc;
    #[test]
    fn actual_pinned_unwritten_duplicates_and_no_rewrite_are_retained() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../conformance/probes/tlc-phase-l-graph");
        let capture: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join("collision-capture.json")).expect("capture"),
        )
        .expect("JSON");
        let fields = &capture["rows"][0]["fields"];
        let mut nodes = vec![];
        let mut written = BTreeMap::new();
        for (step, fp, targets, holds) in [
            (0, 100u64, vec![100u64, 42, 42], vec![true, false, true]),
            (1, 42u64, vec![42u64, 42], vec![false, true]),
            (2, 100u64, vec![100u64, 42, 42], vec![true, false, true]),
        ] {
            let receipt = Receipt {
                multiplier: 1,
                capacity: 16,
                graph_insertion: false,
                successors: targets
                    .iter()
                    .enumerate()
                    .map(|(i, fp)| Representative {
                        slot: i,
                        fingerprint: *fp,
                        state: Arc::from([Some(dive_value::Value::Int(0))]),
                        ordinal: None,
                        label: None,
                    })
                    .collect(),
                checks: holds
                    .iter()
                    .enumerate()
                    .map(|(i, holds)| Check {
                        group: 0,
                        kind: AtomKind::Action,
                        bin: 0,
                        successor: Some(i),
                        holds: *holds,
                    })
                    .chain([Check {
                        group: 0,
                        kind: AtomKind::State,
                        bin: 0,
                        successor: None,
                        holds: step != 1,
                    }])
                    .collect(),
            };
            let inserted = insert_source(&mut nodes, &mut written, fp, &receipt, 1, 1)
                .expect("well-shaped receipt");
            assert_eq!(inserted, step != 2);
            let node = nodes
                .iter()
                .find(|n| n.fingerprint == fp)
                .expect("written node");
            let actual = node
                .transitions
                .iter()
                .map(|t| {
                    format!(
                        "{}:{}:{}",
                        t.target,
                        t.tableau,
                        if t.action[0] { "1" } else { "0" }
                    )
                })
                .collect::<Vec<_>>()
                .join(";");
            assert_eq!(actual, fields[format!("step.{step}.transitions")]);
        }
        assert_eq!(nodes.len(), 2);
    }
}
