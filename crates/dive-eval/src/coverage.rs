//! Opt-in native source-work counters. These are actual evaluator/generator
//! visits, not TLC's action-context cost model or a model-correctness proof.
use dive_ir::{ExprId, Ir};

thread_local! { static PAUSED: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) }; }

/// A thread-local pause of one collector during diagnostic reconstruction.
/// The strong owner prevents address reuse, and the guard cannot move between
/// threads. Other producers and other threads remain observable.
#[derive(Debug)]
pub struct SourceCoveragePause {
    owner: Arc<SourceCoverage>,
    not_send: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl Drop for SourceCoveragePause {
    fn drop(&mut self) {
        let owner = Arc::as_ptr(&self.owner) as usize;
        PAUSED.with(|paused| {
            let mut paused = paused.borrow_mut();
            if let Some(index) = paused.iter().rposition(|p| *p == owner) {
                paused.remove(index);
            }
        });
    }
}
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

/// An exact original source location, with JavaCC/TLC line and column semantics.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceLocation {
    /// The module owning the source text.
    pub module: String,
    /// The file name retained by lowering.
    pub file: String,
    /// Original UTF-8 byte range, with an exclusive end.
    pub bytes: (u32, u32),
    /// One-based line and column of the first source character.
    pub begin: (u32, u32),
    /// One-based line and inclusive column of the final source character.
    pub end: (u32, u32),
    /// Static instantiated definition (`I!Op`) retained by the IR, when present.
    /// Outer dynamic caller paths are not reconstructed from this origin.
    pub instance: Option<String>,
}
/// Actual producer work performed at a source location.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceCounts {
    /// Expression evaluator entries, including successful and failing demands.
    pub evaluations: u64,
    /// Visits by the initial-state generator's source walker.
    pub init_steps: u64,
    /// Visits by the successor generator's source walker.
    pub next_steps: u64,
    /// Initial generator emissions, before the consumer accepts or rejects them.
    pub init_emissions: u64,
    /// Successor generator emissions, before the consumer accepts or rejects them.
    /// Actual producer queries such as ENABLED can perform generator work too.
    pub next_emissions: u64,
}
impl SourceCounts {
    fn add(&mut self, other: Self, overflowed: &mut bool) {
        fn sum(a: &mut u64, b: u64, overflowed: &mut bool) {
            if let Some(total) = a.checked_add(b) {
                *a = total;
            } else {
                *a = u64::MAX;
                *overflowed = true;
            }
        }
        sum(&mut self.evaluations, other.evaluations, overflowed);
        sum(&mut self.init_steps, other.init_steps, overflowed);
        sum(&mut self.next_steps, other.next_steps, overflowed);
        sum(&mut self.init_emissions, other.init_emissions, overflowed);
        sum(&mut self.next_emissions, other.next_emissions, overflowed);
    }
}
/// A source row. Coincident static source/instance locations aggregate their work;
/// VM or IR node indices are never used as user-facing source locations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceCoverageRow {
    /// Exact source and instantiation context.
    pub location: SourceLocation,
    /// Actual source work.
    pub counts: SourceCounts,
}
/// An observation snapshot, independent of verdict or witness certification.
#[derive(Clone, Debug)]
pub struct SourceCoverageSnapshot {
    /// Nonstandard source locations, including unvisited locations with zeros.
    pub rows: Vec<SourceCoverageRow>,
    /// Work at synthetic or unavailable origins, reported separately.
    pub originless: SourceCounts,
    /// Whether observation is still enabled. A live snapshot is not coherent
    /// across concurrent entries; file checking stops collection before reporting.
    pub active: bool,
    /// A counter or location aggregate saturated; exact counts are unavailable.
    pub overflowed: bool,
}
#[derive(Debug)]
struct Counters([AtomicU64; 5]);
impl Counters {
    fn new() -> Self {
        Self(std::array::from_fn(|_| AtomicU64::new(0)))
    }
    fn snapshot(&self) -> SourceCounts {
        let count = self.0.each_ref().map(|value| value.load(Ordering::Relaxed));
        SourceCounts {
            evaluations: count[0],
            init_steps: count[1],
            next_steps: count[2],
            init_emissions: count[3],
            next_emissions: count[4],
        }
    }
}
/// Counters bound to one immutable IR and explicitly attached producer evaluators.
///
/// No counter callback evaluates, formats or normalizes a value. Fresh replay
/// evaluators are unobserved unless a caller explicitly attaches this collector.
/// Ordinary evaluators allocate no counter storage. This measures native work,
/// which can differ from TLC's logical costs and can depend on enabled caches.
#[derive(Debug)]
pub struct SourceCoverage {
    ir: Arc<Ir>,
    counts: Vec<Counters>,
    active: AtomicBool,
    overflowed: AtomicBool,
}
impl SourceCoverage {
    /// Creates zero counters for the exact supplied IR, before producer startup.
    #[must_use]
    pub fn new(ir: Arc<Ir>) -> Self {
        let counts = (0..ir.nodes().len()).map(|_| Counters::new()).collect();
        Self {
            ir,
            counts,
            active: AtomicBool::new(true),
            overflowed: AtomicBool::new(false),
        }
    }
    pub(crate) fn is_for(&self, ir: &Arc<Ir>) -> bool {
        Arc::ptr_eq(&self.ir, ir)
    }
    pub(crate) fn record(&self, expression: ExprId, kind: usize) {
        if !self.active.load(Ordering::Relaxed)
            || PAUSED.with(|paused| {
                paused
                    .borrow()
                    .contains(&(std::ptr::from_ref(self) as usize))
            })
        {
            return;
        }
        let Some(counter) = self
            .counts
            .get(expression.index())
            .and_then(|c| c.0.get(kind))
        else {
            self.overflowed.store(true, Ordering::Relaxed);
            return;
        };
        if counter
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .is_err()
        {
            self.overflowed.store(true, Ordering::Relaxed);
        }
    }
    /// Pauses this collector on the calling thread until the returned guard
    /// drops. Used for reconstruction that must not inflate producer work.
    #[must_use]
    pub fn pause(self: &Arc<Self>) -> SourceCoveragePause {
        let owner = Arc::as_ptr(self) as usize;
        PAUSED.with(|paused| paused.borrow_mut().push(owner));
        SourceCoveragePause {
            owner: self.clone(),
            not_send: std::marker::PhantomData,
        }
    }
    /// Disables further observation. Call after producer workers finish and
    /// before independent replay or presentation; this does not certify a run.
    pub fn stop(&self) {
        self.active.store(false, Ordering::Relaxed);
    }
    /// Returns source-associated counts without evaluating any specification.
    /// Embedded standard-module locations are excluded. The original IR keeps
    /// its exact UTF-8 and `JavaCC` positions; no source reconstruction occurs.
    #[must_use]
    pub fn snapshot(&self) -> SourceCoverageSnapshot {
        let mut locations = BTreeMap::<SourceLocation, SourceCounts>::new();
        let mut overflowed = self.overflowed.load(Ordering::Relaxed);
        let mut originless = SourceCounts::default();
        for (node, counters) in self.ir.nodes().iter().zip(&self.counts) {
            let counts = counters.snapshot();
            let origin = node.origin;
            let Some(file) = origin
                .file
                .and_then(|file| self.ir.files().get(file.index()))
            else {
                originless.add(counts, &mut overflowed);
                continue;
            };
            if file.standard {
                continue;
            }
            let instance = origin
                .instance
                .and_then(|id| self.ir.defs().get(id.index()))
                .map(|d| d.name.clone());
            let location = SourceLocation {
                module: file.module.clone(),
                file: file.display.clone(),
                bytes: origin.range,
                begin: origin.begin,
                end: origin.end,
                instance,
            };
            locations
                .entry(location)
                .or_default()
                .add(counts, &mut overflowed);
        }
        SourceCoverageSnapshot {
            rows: locations
                .into_iter()
                .map(|(location, counts)| SourceCoverageRow { location, counts })
                .collect(),
            originless,
            overflowed,
            active: self.active.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "source counter ownership/overflow controls"
    )]
    use super::*;
    use crate::{Evaluator, Model, Options, St};
    fn ir() -> Arc<Ir> {
        let analysis = dive_sema::analyze_source(
            "C.tla",
            "---- MODULE C ----\nEXTENDS Naturals\nN == 1+2\n====\n",
            &dive_sema::Options::default(),
        );
        Arc::new(dive_ir::lower(&analysis).unwrap())
    }
    #[test]
    fn observer_requires_exact_ir_and_stop_prevents_later_evaluation_counts() {
        let source = ir();
        let coverage = Arc::new(SourceCoverage::new(source.clone()));
        let foreign =
            Evaluator::new_source_covered(Model::new(ir()), Options::dive(), coverage.clone());
        assert_eq!(
            foreign.unwrap_err().kind(),
            crate::ContextErrorKind::ForeignContext
        );
        let ev = Evaluator::new_source_covered(
            Model::new(source.clone()),
            Options::dive(),
            coverage.clone(),
        )
        .unwrap();
        let value = source.root_def("N").unwrap();
        ev.eval_def(value, &St::constant()).unwrap();
        let first = coverage.snapshot();
        assert!(first.rows.iter().any(|row| row.counts.evaluations > 0));
        coverage.stop();
        ev.eval_def(value, &St::constant()).unwrap();
        assert_eq!(first.rows, coverage.snapshot().rows);
        assert!(!coverage.snapshot().active);
    }
    #[test]
    fn overflow_saturates_and_is_reported_instead_of_wrapping() {
        let source = ir();
        let expression = source.def(source.root_def("N").unwrap()).body;
        let coverage = SourceCoverage::new(source);
        coverage.counts[expression.index()].0[0].store(u64::MAX, Ordering::Relaxed);
        coverage.record(expression, 0);
        let snapshot = coverage.snapshot();
        assert!(snapshot.overflowed);
        assert!(
            snapshot
                .rows
                .iter()
                .any(|row| row.counts.evaluations == u64::MAX)
        );
    }
    #[test]
    fn diagnostic_pauses_nest_and_do_not_suspend_another_collector() {
        let source = ir();
        let expression = source.def(source.root_def("N").unwrap()).body;
        let first = Arc::new(SourceCoverage::new(source.clone()));
        let second = Arc::new(SourceCoverage::new(source));
        let outer = first.pause();
        first.record(expression, 0);
        {
            let _inner = first.pause();
            first.record(expression, 0);
            second.record(expression, 0);
        }
        first.record(expression, 0);
        assert_eq!(
            first.counts[expression.index()].0[0].load(Ordering::Relaxed),
            0
        );
        drop(outer);
        first.record(expression, 0);
        assert_eq!(
            first.counts[expression.index()].0[0].load(Ordering::Relaxed),
            1
        );
        assert_eq!(
            second.counts[expression.index()].0[0].load(Ordering::Relaxed),
            1
        );
    }
}
