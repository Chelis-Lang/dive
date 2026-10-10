//! Bounded concrete successor receipts and ordered phase-L predicate checks.
//!
//! This diagnostic stage does not insert graph nodes, revisit tableau products,
//! run SCC checks, select witnesses or certify a model. It consumes the concrete
//! accepted successors supplied by a caller, without replacing them by stored
//! representatives or enumerating Next again. Admission/constraint/safety-stop
//! provenance remains the caller's obligation. Receipts are never proof evidence.

use std::sync::Arc;

use dive_eval::St;
use dive_model::Model;
use dive_value::fp64::{Fingerprinter, TlcFingerprint};
use dive_value::{Ctx, ErrorKind, Value};

use crate::formula::{Atom, AtomKind, Node};
use crate::tlc_plan::{self, BoundProjection};
use crate::tlc_tableau::{self, Tableau, Term};

/// Concrete values in the model's IR variable order, before any representative reduction.
pub type State = Arc<[Option<Value>]>;

/// Fixed bounds for retained receipts, slots, values and predicate cells.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Existing planning/tableau bounds; defaults are unchanged.
    pub formula: crate::formula::Limits,
    /// Maximum incoming and implicit-stutter admissions per source.
    pub incoming: usize,
    /// Maximum slot capacity (not an aggregate byte budget).
    pub slots: usize,
    /// Maximum material-value visits before fingerprinting or comparison.
    pub values: usize,
    /// Maximum retained bin results, initial consistency results and diagnostic snapshots.
    pub cells: usize,
    /// Maximum hash probes and reinsertion work across a session.
    pub work: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            formula: crate::formula::Limits::default(),
            incoming: 8_000_000,
            slots: 8_000_000,
            values: 8_000_000,
            cells: 8_000_000,
            work: 32_000_000,
        }
    }
}

/// A named diagnostic refusal or deciding evaluation error; never a verdict.
#[derive(Clone, Debug)]
pub enum Error {
    /// Explicit resource ceiling, before the refused work or allocation.
    Limit(&'static str),
    /// A malformed concrete state, polynomial or diagnostic input.
    Input(String),
    /// A feature whose ordering/representation is not proven by this slice.
    Unsupported(&'static str),
    /// Original source-bearing bound planning refusal.
    Planning(tlc_plan::ModelPlanningError),
    /// Original bounded tableau construction failure.
    Tableau(tlc_tableau::Error),
    /// A material value fingerprint or equality failure.
    Value(dive_value::ValueError),
    /// Original source-bearing predicate evaluation error.
    Evaluation(dive_eval::EvalError),
}
impl Error {
    /// Stable phase-L diagnostic code, preserving original predicate codes.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Limit(_) => "DIVE-L0040",
            Self::Input(_) => "DIVE-L0041",
            Self::Unsupported(_) => "DIVE-L0042",
            Self::Planning(e) => e.code(),
            Self::Tableau(e) => e.code(),
            Self::Value(e) => e.kind().dive_code(),
            Self::Evaluation(_) => "DIVE-L0043",
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Limit(s) => write!(f, "phase-L diagnostic resource limit: {s}"),
            Self::Input(s) => write!(f, "invalid phase-L diagnostic input: {s}"),
            Self::Unsupported(s) => write!(f, "unsupported phase-L diagnostic: {s}"),
            Self::Planning(e) => e.fmt(f),
            Self::Tableau(e) => e.fmt(f),
            Self::Value(e) => e.fmt(f),
            Self::Evaluation(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {}

/// Actual diagnostic work, including the prefix before a refusal/error.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    /// This stage executes inline. Zero until collection/evaluation starts.
    pub actual_workers: usize,
    /// Completed source receipts, in caller-supplied commit order.
    pub sources: usize,
    /// Admissions attempted, including implicit stutters and duplicates.
    pub admissions: usize,
    /// New entries, including reinsertions during growth.
    pub insertions: usize,
    /// Growth operations that actually allocated a replacement table.
    pub grows: usize,
    /// Equality comparisons attempted, with incoming value on the left.
    pub comparisons: usize,
    /// Hash probes/reinsertions charged before executing them.
    pub work: usize,
    /// Material-value visits charged before fingerprinting/comparison.
    pub values: usize,
    /// Predicate calls begun, including the deciding failed call.
    pub predicates: usize,
    /// Completed bin or initial-consistency result cells.
    pub completed_cells: usize,
    /// Retained cell capacity charged before allocations.
    pub retained_cells: usize,
}

/// Ordered state/action-bin location and its completed result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    /// First-encounter order-of-solution index.
    pub group: usize,
    /// Predicate domain.
    pub kind: AtomKind,
    /// Index in the group's state/action bin.
    pub bin: usize,
    /// Slot-order successor index; absent for a source state check.
    pub successor: Option<usize>,
    /// The predicate's result. On error the failing call has no result cell.
    pub holds: bool,
}

/// An incoming concrete eligible successor. `seen` is deliberately not used:
/// TLC inserts both newly discovered and previously seen concrete successors.
#[derive(Clone, Debug)]
pub struct Incoming {
    /// Concrete IR-ordered values, not a stored representative.
    pub state: State,
    /// Original generator ordinal; distinct from the eventual slot index.
    pub ordinal: u64,
    /// Original action label, if retained by the caller.
    pub label: Option<String>,
}

/// A retained concrete first representative and its slot placement.
#[derive(Clone, Debug)]
pub struct Representative {
    /// Actual table slot, after all growth/reinsertion operations.
    pub slot: usize,
    /// Internally derived TLC fingerprint, before bucket masking.
    pub fingerprint: u64,
    /// Concrete values retained from the first equal admission.
    pub state: State,
    /// `None` only if the implicit stutter was the first admission of this state.
    pub ordinal: Option<u64>,
    /// Original incoming action label; absent for the implicit stutter.
    pub label: Option<String>,
}

/// Source-local ordered diagnostic receipt. No graph/proof status is inferred.
#[derive(Clone, Debug)]
pub struct Receipt {
    /// Logical worker multiplier before this source (one committed history).
    pub multiplier: usize,
    /// Allocated capacity after collection, even if subsequent evaluation failed.
    pub capacity: usize,
    /// Concrete first representatives in slot order, including implicit stutter.
    pub successors: Vec<Representative>,
    /// Completed checks in OOS/action-slot-bin/state-bin evaluation order.
    pub checks: Vec<Check>,
    /// Always false: graph insertion and product revisitation are not implemented here.
    pub graph_insertion: bool,
}

/// Refusal with actual partial work and, when available, the unfinished receipt.
#[derive(Clone, Debug)]
pub struct Failure {
    /// Original named diagnostic/error.
    pub error: Error,
    /// Actual prefix counters; no assumed worker width or fabricated completion.
    pub progress: Progress,
    /// Collection/bin prefix for the failed source, not a successful source receipt.
    pub partial: Option<Receipt>,
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for Failure {}

#[derive(Clone)]
struct Entry {
    fp: u64,
    state: State,
    ordinal: Option<u64>,
    label: Option<String>,
}
struct Slots {
    slots: Vec<Option<Entry>>,
    count: usize,
}
fn step(progress: &mut Progress, limits: Limits) -> Result<(), Error> {
    if progress.work >= limits.work {
        return Err(Error::Limit("hash probes and reinsertion work"));
    }
    progress.work += 1;
    Ok(())
}
fn capacity_room(capacity: usize, limits: Limits) -> Result<(), Error> {
    if capacity > limits.slots || capacity > i32::MAX as usize {
        return Err(Error::Limit("successor slot capacity"));
    }
    Ok(())
}
impl Slots {
    fn new(capacity: usize, limits: Limits) -> Result<Self, Error> {
        capacity_room(capacity, limits)?;
        Ok(Self { slots: vec![None; capacity], count: 0 })
    }
    fn grow(&mut self, limits: Limits, progress: &mut Progress) -> Result<(), Error> {
        let capacity = self.slots.len().saturating_mul(2).saturating_add(1).min(i32::MAX as usize);
        capacity_room(capacity, limits)?;
        step(progress, limits)?;
        let mut replacement = Self::new(capacity, limits)?;
        progress.grows += 1;
        // SetOfStates.grow revisits occupied OLD slots, retaining concrete objects.
        // Frozen material values make recomputed fingerprints equal to entry.fp.
        // On a diagnostic work failure preserve the original concrete prefix;
        // speculative rehash operations still remain in the actual counters.
        for entry in self.slots.iter().flatten() {
            replacement.put(entry.clone(), limits, progress)?;
        }
        *self = replacement;
        Ok(())
    }
    fn put(&mut self, entry: Entry, limits: Limits, progress: &mut Progress) -> Result<bool, Error> {
        if self.count >= self.slots.len() / 2 {
            self.grow(limits, progress)?;
        }
        let mut at = usize::try_from(entry.fp & 0x7fff_ffff)
            .map_err(|_| Error::Input("masked fingerprint".into()))? % self.slots.len();
        loop {
            step(progress, limits)?;
            let Some(existing) = &self.slots[at] else {
                self.slots[at] = Some(entry);
                self.count += 1;
                progress.insertions += 1;
                return Ok(false);
            };
            progress.comparisons += 1;
            let mut same = true;
            for (a, b) in entry.state.iter().zip(existing.state.iter()) {
                let (Some(a), Some(b)) = (a, b) else {
                    return Err(Error::Input("incomplete state in successor table".into()));
                };
                match dive_value::tlc::equals(a, b, Ctx::tlc()) {
                    Ok(true) => {}
                    Ok(false) => { same = false; break; }
                    // TLC deliberately continues probing incomparable states. Other
                    // errors are not swallowed by this conservative material slice.
                    Err(e) if e.kind() == ErrorKind::Incomparable => { same = false; break; }
                    Err(e) => return Err(Error::Value(e)),
                }
            }
            if same { return Ok(true); }
            at = (at + 1) % self.slots.len();
        }
    }
    fn representatives(&self) -> Vec<Representative> {
        self.slots.iter().enumerate().filter_map(|(slot, e)| e.as_ref().map(|e| Representative {
            slot, fingerprint: e.fp, state: e.state.clone(), ordinal: e.ordinal, label: e.label.clone(),
        })).collect()
    }
}

fn material(value: &Value, limits: Limits, progress: &mut Progress, depth: usize) -> Result<(), Error> {
    if depth > 256 { return Err(Error::Limit("material value depth")); }
    if progress.values >= limits.values { return Err(Error::Limit("material value visits")); }
    progress.values += 1;
    match value {
        Value::Set(set) => {
            let Some(elements) = set.raw_elements() else {
                return Err(Error::Unsupported("lazy/infinite/opaque successor set value"));
            };
            for v in elements { material(v, limits, progress, depth + 1)?; }
        }
        Value::Fcn(function) => {
            for (k, v) in function.iter() {
                material(&k, limits, progress, depth + 1)?;
                material(v, limits, progress, depth + 1)?;
            }
        }
        Value::Bool(_) | Value::Int(_) | Value::BigInt(_) | Value::Str(_) | Value::Model(_) => {}
    }
    Ok(())
}
fn cell_room(count: usize, limits: Limits, progress: &mut Progress) -> Result<(), Error> {
    let total = progress.retained_cells.checked_add(count)
        .filter(|n| *n <= limits.cells).ok_or(Error::Limit("retained result cells"))?;
    progress.retained_cells = total;
    Ok(())
}
fn failure(error: Error, progress: &Progress, partial: Option<Receipt>) -> Box<Failure> {
    Box::new(Failure { error, progress: progress.clone(), partial })
}

/// One bounded inline logical-worker diagnostic history. Fresh bound planning
/// and private projection ownership prevent callers from changing predicate bins
/// into opaque callbacks after the purity check.
pub struct Session<'a> {
    model: &'a Model,
    plan: BoundProjection,
    tableaux: Vec<Option<Tableau>>,
    fingerprint: TlcFingerprint,
    limits: Limits,
    multiplier: usize,
    progress: Progress,
}
impl<'a> Session<'a> {
    /// Plans a diagnostic history with the existing fixed bounds. The current
    /// receipt evaluator is inline; a wider request is refused rather than
    /// represented as an executed pool. TLC-profile material ordering is required.
    ///
    /// # Errors
    /// Pure scoped planning/tableau refusals, non-TLC value mode, VIEW/symmetry,
    /// invalid polynomial, wider requested diagnostic execution or named bounds.
    pub fn new(model: &'a Model, polynomial: usize, workers: usize, limits: Limits) -> Result<Self, Box<Failure>> {
        let progress = Progress::default();
        let build = || {
            if workers != 1 { return Err(Error::Unsupported("diagnostic worker width other than one")); }
            if model.evaluator.options().ctx != Ctx::tlc() {
                return Err(Error::Unsupported("phase-L receipts require TLC value ordering/integer mode"));
            }
            if model.view.is_some() || model.symmetry.is_some() {
                return Err(Error::Unsupported("VIEW/symmetry phase-L representatives"));
            }
            let fingerprint = TlcFingerprint::new(polynomial).ok_or_else(|| Error::Input("fingerprint polynomial index".into()))?;
            let plan = tlc_plan::lower_model(model, limits.formula).map_err(Error::Planning)?;
            let tableaux = tlc_tableau::project(&plan.projection, limits.formula).map_err(Error::Tableau)?;
            Ok((fingerprint, plan, tableaux))
        };
        let (fingerprint, plan, tableaux) = build().map_err(|e| failure(e, &progress, None))?;
        Ok(Self { model, plan, tableaux, fingerprint, limits, multiplier: 1, progress })
    }
    /// Actual cumulative diagnostic work; no checking verdict is implied.
    #[must_use]
    pub fn progress(&self) -> &Progress { &self.progress }
    /// Original configured predicates, distinct from derived role names.
    #[must_use]
    pub fn configured_properties(&self) -> &[dive_model::Named] { &self.plan.configured_properties }
    /// Derived PROPERTY-role labels, in planning input order.
    #[must_use]
    pub fn role_names(&self) -> &[String] { &self.plan.projection.properties }
    fn fingerprint(&mut self, state: &State) -> Result<u64, Error> {
        if state.len() != self.model.ir.vars().len() || state.iter().any(Option::is_none) {
            return Err(Error::Input("incomplete or wrong-width concrete state".into()));
        }
        for value in state.iter().flatten() { material(value, self.limits, &mut self.progress, 0)?; }
        // Check material visits before allocating the reordered value vector.
        let values: Vec<_> = self.model.ir.tlc_var_order().iter().map(|v| state[v.index()].clone())
            .collect::<Option<Vec<_>>>().ok_or_else(|| Error::Input("TLC variable permutation".into()))?;
        self.fingerprint.fingerprint_state(&values, Ctx::tlc()).map_err(Error::Value)
    }
    fn atom(&mut self, atom: Atom, state: &St) -> Result<bool, Error> {
        let o = self.plan.projection.occurrences.get(atom.index)
            .ok_or_else(|| Error::Input("occurrence reference".into()))?;
        let predicate = self.plan.inputs.get(o.input).and_then(|p| p.atoms.get(o.atom.index))
            .ok_or_else(|| Error::Input("bound atom reference".into()))?;
        self.progress.predicates += 1;
        predicate.evaluate(self.model, state).map_err(Error::Evaluation)
    }
    fn term(&mut self, root: usize, state: &St, depth: usize) -> Result<bool, Error> {
        if depth > 256 { return Err(Error::Limit("predicate expression depth")); }
        let n = self.plan.projection.nodes().get(root).cloned().ok_or_else(|| Error::Input("predicate root".into()))?;
        match n {
            Node::Bool(b) => Ok(b),
            Node::Atom(a) => self.atom(a, state),
            Node::Not(c) => Ok(!self.term(c, state, depth + 1)?),
            Node::And(cs) => { for c in cs { if !self.term(c, state, depth + 1)? { return Ok(false); } } Ok(true) }
            Node::Or(cs) => { for c in cs { if self.term(c, state, depth + 1)? { return Ok(true); } } Ok(false) }
            Node::Always(_) | Node::Eventually(_) => Err(Error::Input("temporal operator in predicate bin".into())),
        }
    }
    fn tableau_term(&mut self, group: usize, root: usize, state: &St, depth: usize) -> Result<bool, Error> {
        if depth > 256 { return Err(Error::Limit("initial consistency predicate depth")); }
        let n = self.tableaux.get(group).and_then(Option::as_ref).and_then(|t| t.terms().get(root)).copied()
            .ok_or_else(|| Error::Input("initial consistency term".into()))?;
        match n {
            Term::Bool(b) => Ok(b), Term::Atom(a) => self.atom(a, state),
            Term::Not(c) => Ok(!self.tableau_term(group, c, state, depth + 1)?),
            Term::And(a,b) => Ok(self.tableau_term(group,a,state,depth+1)? && self.tableau_term(group,b,state,depth+1)?),
            Term::Or(a,b) => Ok(self.tableau_term(group,a,state,depth+1)? || self.tableau_term(group,b,state,depth+1)?),
            _ => Err(Error::Input("temporal term in initial consistency".into())),
        }
    }
    /// Evaluates each OOS's initial tableau particles in TLC node/predicate
    /// order, stopping at the first false predicate within a particle. Results
    /// are not graph insertion or acceptance evidence; null tableaux have no checks.
    ///
    /// # Errors
    /// Concrete-state/material limits and the first source-bearing predicate error.
    pub fn initial(&mut self, state: &State) -> Result<Vec<Vec<bool>>, Box<Failure>> {
        self.progress.actual_workers = 1;
        let checked = (|| {
            self.fingerprint(state)?;
            let count = self.tableaux.iter().flatten().try_fold(0usize, |n,t| n.checked_add(t.initial_count))
                .ok_or(Error::Limit("initial consistency cells"))?;
            cell_room(count, self.limits, &mut self.progress)?;
            let mut results = Vec::with_capacity(self.tableaux.len());
            let st = St::state(state.clone());
            for g in 0..self.tableaux.len() {
                let count = self.tableaux[g].as_ref().map_or(0, |t| t.initial_count);
                let mut cells = Vec::with_capacity(count);
                for p in 0..count {
                    let roots = self.tableaux[g].as_ref().map(|t| t.particles[p].state_predicates.clone()).unwrap_or_default();
                    let mut holds = true;
                    for root in roots {
                        if !self.tableau_term(g,root,&st,0)? { holds = false; break; }
                    }
                    cells.push(holds);self.progress.completed_cells += 1;
                }
                results.push(cells);
            }
            Ok(results)
        })();
        checked.map_err(|e| failure(e,&self.progress,None))
    }
    /// Consumes accepted concrete successors once, adds implicit stutter, then
    /// evaluates each OOS's action bins over slot order before its state bins.
    /// Logical capacity history advances only after successful bin evaluation.
    /// It is not an enumerator, graph insertion, proof or completed model check.
    ///
    /// # Errors
    /// Named material/slot/work/result limits; malformed states/ordinals; the
    /// deciding predicate error with the completed prefix receipt retained.
    pub fn source(&mut self, source: &State, incoming: &[Incoming]) -> Result<Receipt, Box<Failure>> {
        self.progress.actual_workers = 1;
        let mut receipt = Receipt { multiplier: self.multiplier, capacity: 0, successors: Vec::new(), checks: Vec::new(), graph_insertion: false };
        let result = (|| {
            if incoming.len().checked_add(1).is_none_or(|n| n > self.limits.incoming) { return Err(Error::Limit("source admissions including stutter")); }
            if incoming.windows(2).any(|pair| pair[0].ordinal >= pair[1].ordinal) { return Err(Error::Input("non-increasing successor ordinals".into())); }
            let initial = self.multiplier.checked_mul(16).ok_or(Error::Limit("logical successor capacity"))?;
            // Reserve the maximum representative prefix before retaining any
            // concrete references, including the prefix returned on failure.
            cell_room(incoming.len() + 1,self.limits,&mut self.progress)?;
            let mut set = Slots::new(initial,self.limits)?;
            let collected = (|| {
                for next in incoming {
                    self.progress.admissions += 1;
                    let fp = self.fingerprint(&next.state)?;
                    set.put(Entry { fp,state:next.state.clone(),ordinal:Some(next.ordinal),label:next.label.clone() },self.limits,&mut self.progress)?;
                }
                self.progress.admissions += 1;
                let fp = self.fingerprint(source)?;
                set.put(Entry { fp,state:source.clone(),ordinal:None,label:None },self.limits,&mut self.progress)?;
                Ok(())
            })();
            receipt.capacity = set.slots.len();
            receipt.successors = set.representatives();
            collected?;
            let cells = self.plan.projection.orders.iter().try_fold(0usize,|n,o| {
                n.checked_add(o.check_action.len().checked_mul(set.count)?).and_then(|n| n.checked_add(o.check_state.len()))
            }).ok_or(Error::Limit("action/state bin cells"))?;
            cell_room(cells,self.limits,&mut self.progress)?;
            receipt.checks.reserve(cells);
            for g in 0..self.plan.projection.orders.len() {
                let actions = self.plan.projection.orders[g].check_action.clone();
                let states = self.plan.projection.orders[g].check_state.clone();
                for (sidx, successor) in receipt.successors.iter().enumerate() {
                    let st = St::action(source.clone(),successor.state.clone());
                    for (bin,root) in actions.iter().enumerate() {
                        let holds = self.term(*root,&st,0)?;
                        receipt.checks.push(Check { group:g,kind:AtomKind::Action,bin,successor:Some(sidx),holds });
                        self.progress.completed_cells += 1;
                    }
                }
                let st = St::state(source.clone());
                for (bin,root) in states.iter().enumerate() {
                    let holds = self.term(*root,&st,0)?;
                    receipt.checks.push(Check { group:g,kind:AtomKind::State,bin,successor:None,holds });
                    self.progress.completed_cells += 1;
                }
            }
            if receipt.capacity > initial {
                self.multiplier = self.multiplier.checked_add(1).ok_or(Error::Limit("logical worker multiplier"))?;
            }
            self.progress.sources += 1;
            Ok(())
        })();
        match result { Ok(()) => Ok(receipt), Err(e) => Err(failure(e,&self.progress,Some(receipt))) }
    }
}

/// A synthetic forced-key state for structural SetOfStates diagnostics only.
/// The real model receipt path never accepts caller-supplied fingerprints.
#[derive(Clone, Debug)]
pub struct DiagnosticState {
    /// Forced key; provides no model-fingerprint evidence.
    pub fingerprint: u64,
    /// A scalar value for actual TLC-oriented comparison semantics.
    pub value: Value,
    /// Stable input label, retained only from its first equal admission.
    pub label: String,
}
/// One synthetic structural insertion snapshot, not a model receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// Whether this insertion matched an existing state.
    pub duplicate: bool,
    /// Entry count.
    pub count: usize,
    /// Slot capacity.
    pub capacity: usize,
    /// Occupied slot indices and first-representative input labels.
    pub slots: Vec<(usize,String)>,
}
/// Projects forced-key structural insertions for raw Java oracle controls. It
/// refuses compound values; this is not model execution, fingerprint derivation,
/// graph insertion or proof evidence.
///
/// # Errors
/// Explicit capacity/work/snapshot bounds, compound diagnostic input or value errors.
pub fn diagnostic_slots(capacity: usize, inputs: &[DiagnosticState], limits: Limits) -> Result<Vec<Snapshot>, Box<Failure>> {
    let mut progress = Progress::default();
    let result = (|| {
        if inputs.len() > limits.incoming { return Err(Error::Limit("diagnostic admissions")); }
        let mut slots = Slots::new(capacity,limits)?;
        let mut result = Vec::new();
        for input in inputs {
            if matches!(input.value,Value::Set(_) | Value::Fcn(_)) { return Err(Error::Unsupported("compound forced-key diagnostic")); }
            progress.actual_workers = 1;
            progress.admissions += 1;
            let duplicate = slots.put(Entry { fp:input.fingerprint,state:Arc::from([Some(input.value.clone())]),ordinal:None,label:Some(input.label.clone()) },limits,&mut progress)?;
            cell_room(slots.count.saturating_add(1),limits,&mut progress)?;
            result.push(Snapshot { duplicate,count:slots.count,capacity:slots.slots.len(),slots:slots.slots.iter().enumerate().filter_map(|(i,e)|e.as_ref().and_then(|e|e.label.as_ref()).map(|label|(i,label.clone()))).collect() });
        }
        Ok(result)
    })();
    result.map_err(|e|failure(e,&progress,None))
}
