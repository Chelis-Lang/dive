//! Seeded simulation over the compiled plan: per-trace sampling and the
//! worker driver that commits traces in id order.
//!
//! Trace `id` draws only from `Streams::simulation_trace(id)`, and its
//! sampling reads nothing another trace writes, so its states, generated
//! count, output and verdict are a function of the seed and `id`. Workers
//! claim ids from one counter and hand finished traces to a committer that
//! takes them in id order. The first deciding trace committed (a violation,
//! an error, a refusal or a limit) is therefore the lowest deciding id, and
//! the statistics, output and kept traces are those of the ids before it, at
//! any worker count (ADR 0162 §7).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::Duration;

use dive_core::memo::Memo;
use dive_core::plan::Plan;
use dive_core::{Error, ErrorKind, Handle, Machine};
use dive_model::Model;
use dive_sim::{Outcome, Report, TemporalEnding, Trace, Unchecked};
use dive_stdlib::random::{RandomSource, Stream, Streams};

use crate::simulate_temporal::{Checker, NativeTemporal, TraceGraph, Verdict, Word};

use super::{
    Native, allowed, assumptions, at_level, classify, finish_batch, generate, generate_selected,
    implied_initial, level_of, pending, preflight_report, retain, roles, values,
};

/// Admitted successors kept exactly before successor choice switches to
/// reservoir sampling. Up to this many, the choice is the draw
/// `next_index(n)` over the `n` candidates; beyond it, one candidate is kept
/// and the `i`-th replaces it with probability `1/i`, uniform over all of
/// them in constant memory.
const EXACT_CANDIDATES: usize = 1024;

/// Finished traces a worker may hold ahead of the next trace to commit, per
/// worker, which bounds buffered output and states.
const WINDOW_PER_WORKER: u64 = 4;

use dive_sim::SAMPLE_ATTEMPTS;

/// The replayable draw stream of `--sample-unbounded` (D-040): one dive
/// stream per trace, step, action and attempt (`Streams::simulation_draws`).
#[derive(Debug)]
struct DrawSource {
    streams: Streams,
    stream: Stream,
}

impl dive_core::unbounded::IndexSource for DrawSource {
    fn next_index(&mut self, bound: u64) -> u64 {
        self.stream.next_index(bound)
    }

    fn restart(&mut self, [trace, step, action, attempt]: [u64; 4]) {
        self.stream = self.streams.simulation_draws(trace, step, action, attempt);
    }
}

/// The sampler `options` ask for, if any.
fn sampler(options: &dive_sim::Options) -> Result<Option<dive_core::unbounded::Unbounded>, Error> {
    let dive_sim::SampleUnbounded::Geometric { mean } = options.sample_unbounded else {
        return Ok(None);
    };
    let streams = Streams::new(options.seed, options.mode);
    dive_core::unbounded::Unbounded::new(
        mean,
        Box::new(DrawSource {
            streams,
            stream: streams.simulation_draws(0, 0, 0, 0),
        }),
    )
    .map(Some)
}

/// Positions `machine`'s draw streams at `site`: the unbounded sampler's
/// and the random operators' (`sim_random`). Returns the draw count so far.
fn restart_draws(machine: &mut Machine, random: Option<Streams>, site: [u64; 4]) -> u64 {
    if let Some(streams) = random {
        super::sim_random::position(machine, streams, site);
    }
    if let Some(unbounded) = machine.unbounded.as_mut() {
        unbounded.restart(site);
    }
    draw_count(machine)
}

/// Draws made so far: members of unbounded sets and random operators'.
fn draw_count(machine: &Machine) -> u64 {
    machine
        .unbounded
        .as_ref()
        .map_or(0, dive_core::unbounded::Unbounded::draws)
        + machine.draws
}

/// The values the random operators drew at the current position so far.
fn random_drawn(machine: &Machine) -> Vec<Handle> {
    super::sim_random::drawn(machine).to_vec()
}

/// Records `drawn` as the draws of the step into row `row`, if any.
fn push_drawn(list: &mut Vec<(usize, Vec<Handle>)>, row: usize, drawn: Vec<Handle>) {
    if !drawn.is_empty() {
        list.push((row, drawn));
    }
}

/// Records the sets the last action drew from, with the action's label.
fn note_sets(machine: &mut Machine, label: &str, sampled: &mut Vec<String>) {
    if let Some(unbounded) = &mut machine.unbounded {
        for set in unbounded.take_sets() {
            sampled.push(format!("{set} in {label}"));
        }
    }
}

/// The interned witnesses of the current generator path, if any.
fn witnesses(machine: &mut Machine) -> Result<Vec<Handle>, Error> {
    if machine.drawn().is_empty() {
        return Ok(Vec::new());
    }
    let drawn = machine.drawn().to_vec();
    drawn
        .into_iter()
        .map(|value| machine.heap.intern_exact(value))
        .collect()
}

/// How one trace ended.
enum End {
    /// Its depth or a state without successors (deadlock checking off).
    Completed,
    /// A violation, error, refusal or limit that decides the run.
    Deciding(Outcome),
    /// A lower trace decided the run first; this one was abandoned.
    Superseded,
    /// The run's budget stopped it.
    Stopped,
}

/// Receives every finished trace (`--trace-dir`): `write` runs on the
/// worker that sampled it, before commit, and `commit` when the trace commits
/// in id order. A trace that never commits (after the deciding one, or when
/// the run stops) is never committed.
pub(crate) trait TraceExport: Sync {
    /// Validates and writes `trace`, which decides the run with `deciding`
    /// if given. An error decides the run at this trace instead.
    fn write(&self, trace: &Trace, deciding: Option<&Outcome>) -> Result<(), Outcome>;
    /// Makes the written trace `id` part of the output.
    fn commit(&self, id: u64) -> Result<(), String>;
}

/// One sampled trace before commit: interned rows, so no conversion is paid
/// for a trace the report does not keep.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent facts about one trace, each read at a different commit step"
)]
struct Run {
    id: u64,
    rows: Vec<Vec<Handle>>,
    actions: Vec<usize>,
    generated: u64,
    /// Initial candidates generated by constructive Init.
    initial: u64,
    printed: Vec<String>,
    /// Chosen transitions as (from, to) action ids, for `--stats basic`.
    flow: Vec<(usize, usize)>,
    /// Existential witnesses drawn for the step into a row, by row index.
    witnesses: Vec<(usize, Vec<Handle>)>,
    /// Values random operators drew for the step into a row (Init for row
    /// 0), by row index.
    draws: Vec<(usize, Vec<Handle>)>,
    /// Unbounded sets drawn from, with the drawing action.
    sampled: Vec<String>,
    /// Ended early: no drawn member led to an admitted state.
    stalled: bool,
    /// Written by the run's [`TraceExport`], to be committed with it.
    exported: bool,
    end: End,
    /// The last state had no admitted successor.
    terminal: bool,
    /// The trace continued past the depth (`--liveness-extend`).
    extended: bool,
    /// The trace graph, once built for the temporal check.
    graph: Option<TraceGraph>,
    /// A temporal verdict reached at the depth, when extension was not needed.
    verdict: Option<Verdict>,
    /// The trace's class and per-property coverage, with temporal properties.
    class: Option<Class>,
    coverage: Vec<Option<Unchecked>>,
    /// A temporal counterexample word.
    word: Option<Word>,
}

/// How a completed trace ended, with temporal properties (ADR 0162 §4.2).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Closed,
    Terminal,
    Truncated,
    ExtendedClosed,
    ExtendedTerminal,
}

/// The initial states traces start from.
enum Init {
    /// Enumerated once; each trace draws one.
    Enumerated(Vec<(Vec<Handle>, usize)>),
    /// Under `--constructive-init`, each trace constructs its own.
    Constructive,
    /// Init draws from an unbounded set, so each trace enumerates it with
    /// its own draws.
    Sampled,
}

/// What every trace reads.
struct Context<'a> {
    plan: &'a Plan,
    model: &'a Model,
    /// The run's temporal checker, when the model has temporal properties.
    temporal: Option<&'a Checker>,
    /// The run's budget, which bounds each worker's native atom evaluation.
    budget: &'a dive_runtime::budget::Budget,
    options: &'a dive_sim::Options,
    init: &'a Init,
    init_ids: &'a [usize],
    next_ids: &'a [usize],
    streams: Streams,
    /// The random operators' streams, when the plan draws (`sim_random`).
    random: Option<Streams>,
    best: &'a AtomicU64,
    poll: &'a (dyn Fn() -> bool + Sync),
    flows: bool,
    /// Output a trace may retain: what the run's allowance leaves after
    /// startup and enumerated Init.
    output: (usize, usize),
    /// Action labels for the report's sampled-set notes: Init's, Next's.
    init_labels: &'a [String],
    next_labels: &'a [String],
    /// Where every finished trace is written, if anywhere (`--trace-dir`).
    export: Option<&'a dyn TraceExport>,
}

/// A kept trace: interned rows, actions, generated count and witnesses, or
/// for a temporal violation its counterexample word.
struct Kept {
    id: u64,
    rows: Vec<Vec<Handle>>,
    actions: Vec<usize>,
    generated: u64,
    witnesses: Vec<(usize, Vec<Handle>)>,
    draws: Vec<(usize, Vec<Handle>)>,
    word: Option<Word>,
    /// The last state had no admitted successor ([`Trace::terminal`]).
    terminal: bool,
}

/// The committed prefix of the run.
struct Commit {
    next: u64,
    pending: BTreeMap<u64, Run>,
    finished: bool,
    report: Report,
    rows: Vec<Kept>,
    kept: u64,
    retained: usize,
    output: (usize, usize),
    printed: Vec<String>,
    sampled: BTreeSet<String>,
    stalled: u64,
}

struct Shared {
    next: AtomicU64,
    best: AtomicU64,
    stop: AtomicBool,
    /// `Commit::next`, readable without the lock.
    committed: AtomicU64,
    /// Workers waiting for the window to move; commit notifies only then.
    waiting: AtomicUsize,
    commit: Mutex<Commit>,
    ready: Condvar,
}

impl Native {
    pub(crate) fn simulate(
        &mut self,
        model: &Model,
        options: &dive_sim::Options,
        runtime: &dive_sim::Runtime,
        poll: &(dyn Fn() -> bool + Sync),
        export: Option<&dyn TraceExport>,
    ) -> Report {
        if let Some(outcome) = dive_sim::preflight(model, options) {
            return preflight_report(outcome);
        }
        let checker = match Checker::new(model, options.bad_prefix, options.liveness_budget) {
            Ok(checker) => checker,
            Err(outcome) => return preflight_report(outcome),
        };
        runtime.simulate_with_backend(model, |remaining| {
            let started = || runtime.start_sampling();
            let mut report = self.sample(
                model,
                checker.as_ref(),
                options,
                poll,
                (remaining, &started),
                export,
            );
            if self.machine.heap.poisoned() {
                report.outcome = Outcome::Internal("native finite mode read a stale value".into());
            }
            report
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "startup, Init and the worker hand-off stay in one driver"
    )]
    fn sample(
        &mut self,
        model: &Model,
        checker: Option<&Checker>,
        options: &dive_sim::Options,
        poll: &(dyn Fn() -> bool + Sync),
        (remaining, started): (&dyn Fn() -> (usize, usize), &dyn Fn()),
        export: Option<&dyn TraceExport>,
    ) -> Report {
        let mut report = preflight_report(Outcome::Sampled);
        if options.action_stats {
            match dive_sim::ActionFlow::new(model) {
                Ok(flow) => report.action_flow = Some(flow),
                Err(error) => {
                    report.outcome = Outcome::Unsupported(error.to_string());
                    return report;
                }
            }
        }
        if poll() {
            report.outcome = Outcome::Cancelled;
            return report;
        }
        report.stage = "assumptions";
        if let Some(outcome) = assumptions(model) {
            report.outcome = outcome;
            return report;
        }
        // Start-up drew what it draws; Core draws from each trace's streams.
        started();
        let workers = options.workers.max(1);
        match Self::prepare_with_options(model, &self.budget.clone(), self.machine.opts, workers) {
            Ok(actual) => *self = actual,
            Err(error) => {
                report.outcome = error.simulation();
                return report;
            }
        }
        let sampling = match sampler(options) {
            Ok(sampling) => sampling.is_some(),
            Err(error) => {
                report.outcome = classify(&error, None);
                return report;
            }
        };
        // Init is first enumerated once; a draw there means each trace must
        // enumerate it with its own draws, so the probe only detects it.
        self.machine.unbounded = sampler(options).ok().flatten();
        if let Some(unbounded) = &mut self.machine.unbounded {
            unbounded.set_probe(true);
        }
        if self.plan.effects.draws {
            super::sim_random::probe(&mut self.machine);
        }
        let (events, bytes) = remaining();
        self.machine.set_output_limits(events, bytes);
        // TLC's simulator: `TLCGet("level")` is 0 in Init, a state's 1-based
        // trace position in its state checks and in the next-state relation
        // and action checks of the step it starts.
        self.machine.allow_finite_level();
        self.machine.level = 0;
        report.stage = "initialization";
        let init = if options.constructive_init {
            Init::Constructive
        } else {
            let Some(init) = enumerate_init(
                &mut self.machine,
                &self.plan,
                model,
                options,
                poll,
                &mut report,
            ) else {
                self.flush_driver_output(&mut report);
                return report;
            };
            init
        };
        if let Some(unbounded) = &mut self.machine.unbounded {
            unbounded.set_probe(false);
        }
        let (printed, failure) = self.machine.take_output();
        let used = (
            printed.len(),
            printed.iter().map(String::len).sum::<usize>(),
        );
        if let Some(reason) = failure {
            report.outcome = Outcome::Limit(reason.into());
        }
        report
            .runtime_output
            .extend(
                printed
                    .into_iter()
                    .map(|text| dive_eval::capture::Captured {
                        text,
                        warning: false,
                    }),
            );
        if !self.machine.warnings.is_empty() {
            report.outcome =
                Outcome::Unsupported("native simulation warning/output interleaving".into());
        }
        if !matches!(report.outcome, Outcome::Sampled) {
            return report;
        }
        report.stage = "sampling";
        let output = (events.saturating_sub(used.0), bytes.saturating_sub(used.1));
        let init_ids: Vec<usize> = model
            .init_actions
            .iter()
            .map(dive_eval::next::Action::id)
            .collect();
        let next_ids: Vec<usize> = model
            .actions
            .iter()
            .map(dive_eval::next::Action::id)
            .collect();
        let flows = report.action_flow.is_some();
        let init_labels: Vec<String> = model
            .init_actions
            .iter()
            .map(|action| action.label().to_owned())
            .collect();
        let next_labels: Vec<String> = model
            .actions
            .iter()
            .map(|action| action.label().to_owned())
            .collect();
        report.temporal = checker.map(|checker| dive_sim::TemporalCoverage {
            classes: dive_sim::TraceClasses::default(),
            properties: checker
                .names()
                .map(|name| dive_sim::PropertyCoverage {
                    name: name.to_owned(),
                    ..Default::default()
                })
                .collect(),
        });
        let shared = Shared {
            next: AtomicU64::new(options.first_trace),
            best: AtomicU64::new(u64::MAX),
            stop: AtomicBool::new(false),
            committed: AtomicU64::new(options.first_trace),
            waiting: AtomicUsize::new(0),
            commit: Mutex::new(Commit {
                next: options.first_trace,
                pending: BTreeMap::new(),
                finished: false,
                report,
                rows: Vec::new(),
                kept: 0,
                retained: 0,
                output,
                printed: Vec::new(),
                sampled: BTreeSet::new(),
                stalled: 0,
            }),
            ready: Condvar::new(),
        };
        let cx = Context {
            plan: &self.plan,
            model,
            temporal: checker,
            budget: &self.budget,
            options,
            init: &init,
            init_ids: &init_ids,
            next_ids: &next_ids,
            streams: Streams::new(options.seed, options.mode),
            random: self
                .plan
                .effects
                .draws
                .then(|| Streams::new(options.seed, options.mode)),
            best: &shared.best,
            poll,
            flows,
            output,
            init_labels: &init_labels,
            next_labels: &next_labels,
            export,
        };
        let budget = &self.budget;
        let store = std::sync::Arc::clone(self.machine.heap.store());
        let opts = self.machine.opts;
        let failed_start = std::thread::scope(|scope| {
            let mut failed = None;
            for _ in 1..workers {
                let started = std::thread::Builder::new()
                    .stack_size(dive_eval::RECOMMENDED_STACK)
                    .spawn_scoped(scope, || {
                        let mut machine = worker_machine(&store, cx.plan, opts, budget);
                        if sampling {
                            machine.unbounded = sampler(options).ok().flatten();
                        }
                        work(&mut machine, &cx, &shared);
                    });
                if let Err(error) = started {
                    failed = Some(format!("cannot start simulation worker: {error}"));
                    shared.stop.store(true, Ordering::SeqCst);
                    break;
                }
            }
            work(&mut self.machine, &cx, &shared);
            failed
        });
        let mut commit = shared
            .commit
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(message) = failed_start {
            commit.report.outcome = Outcome::Internal(message);
        } else if !commit.finished {
            // Every claimed trace was committed or the run stopped first.
            commit.report.outcome = Outcome::Cancelled;
        }
        let mut report = std::mem::replace(&mut commit.report, preflight_report(Outcome::Sampled));
        report.sampled_sets = std::mem::take(&mut commit.sampled).into_iter().collect();
        report.traces_stalled = commit.stalled;
        report
            .runtime_output
            .extend(
                commit
                    .printed
                    .drain(..)
                    .map(|text| dive_eval::capture::Captured {
                        text,
                        warning: false,
                    }),
            );
        for kept in std::mem::take(&mut commit.rows) {
            match self.kept_trace(model, kept) {
                Ok(trace) => report.traces.push(trace),
                Err(message) => {
                    report.outcome = Outcome::Unsupported(message);
                    report.traces.clear();
                    return report;
                }
            }
        }
        report
    }

    /// Converts a kept trace's interned rows and witnesses to values.
    fn kept_trace(&mut self, model: &Model, kept: Kept) -> Result<Trace, String> {
        if let Some(word) = kept.word {
            return Ok(word_trace(kept.id, kept.generated, word));
        }
        trace_values(
            &mut self.machine,
            model,
            kept.id,
            &kept.rows,
            kept.actions,
            kept.generated,
            (&kept.witnesses, &kept.draws),
        )
        .map(|trace| Trace {
            terminal: kept.terminal,
            ..trace
        })
    }

    fn flush_driver_output(&mut self, report: &mut Report) {
        let (printed, failure) = self.machine.take_output();
        report
            .runtime_output
            .extend(
                printed
                    .into_iter()
                    .map(|text| dive_eval::capture::Captured {
                        text,
                        warning: false,
                    }),
            );
        if let Some(reason) = failure {
            report.outcome = Outcome::Limit(reason.into());
        }
        if !self.machine.warnings.is_empty() {
            report.outcome =
                Outcome::Unsupported("native simulation warning/output interleaving".into());
        }
    }
}

/// Values recorded at a state (its index), interned.
type Recorded = (usize, Vec<Handle>);

/// A trace's interned rows, witnesses and draws as values.
fn trace_values(
    machine: &mut Machine,
    model: &Model,
    id: u64,
    rows: &[Vec<Handle>],
    actions: Vec<usize>,
    generated: u64,
    (witnesses, random): (&[Recorded], &[Recorded]),
) -> Result<Trace, String> {
    let mut states = Vec::with_capacity(rows.len());
    for row in rows {
        states.push(values(machine, model, row)?);
    }
    let mut members = |list: &[(usize, Vec<Handle>)]| -> Result<_, String> {
        let mut out = Vec::with_capacity(list.len());
        for (state, row) in list {
            let values = values(machine, model, row)?;
            out.push((*state, values.iter().flatten().cloned().collect()));
        }
        Ok(out)
    };
    let witnesses = members(witnesses)?;
    let draws = members(random)?;
    Ok(Trace {
        id,
        states,
        actions,
        generated,
        witnesses,
        draws,
        temporal: None,
        terminal: false,
    })
}

/// A temporal counterexample word as the trace it decides.
fn word_trace(id: u64, generated: u64, word: Word) -> Trace {
    Trace {
        id,
        states: word.states,
        actions: word.actions,
        generated,
        witnesses: Vec::new(),
        draws: Vec::new(),
        temporal: Some((word.ending, word.properties)),
        terminal: false,
    }
}

/// Hands a finished trace to the run's [`TraceExport`]; a failure decides
/// the run at this trace.
fn export(machine: &mut Machine, cx: &Context<'_>, run: &mut Run) {
    let Some(export) = cx.export else {
        return;
    };
    let deciding = match &run.end {
        End::Completed => None,
        End::Deciding(outcome) => Some(outcome),
        End::Superseded | End::Stopped => return,
    };
    let trace = if let Some(word) = &run.word {
        Ok(word_trace(run.id, run.generated, word.clone()))
    } else if run.rows.is_empty() {
        return;
    } else {
        trace_values(
            machine,
            cx.model,
            run.id,
            &run.rows,
            run.actions.clone(),
            run.generated,
            (&run.witnesses, &run.draws),
        )
        .map(|trace| Trace {
            terminal: run.terminal,
            ..trace
        })
    };
    let written = trace
        .map_err(Outcome::Unsupported)
        .and_then(|trace| export.write(&trace, deciding));
    match written {
        Ok(()) => run.exported = true,
        Err(outcome) => run.end = End::Deciding(outcome),
    }
}

fn worker_machine(
    store: &std::sync::Arc<dive_core::Store>,
    plan: &Plan,
    opts: dive_core::Opts,
    budget: &dive_runtime::budget::Budget,
) -> Machine {
    let lengths: Vec<_> = plan
        .code
        .sites
        .iter()
        .map(dive_core::plan::MemoSite::key_len)
        .collect();
    let mut machine = Machine::new(
        std::sync::Arc::clone(store),
        plan.vars.len(),
        opts,
        Memo::new(&lengths),
    );
    machine.deny_checker_access();
    machine.set_work_control(Some(crate::core_engine::search_work_control(budget)));
    machine.allow_finite_level();
    machine
}

/// Enumerates Init once and checks each initial state as `dive check` does:
/// invariants, implied initial predicates, then constraints. `None` when
/// the run is decided here, with the report's outcome set.
#[allow(
    clippy::too_many_lines,
    reason = "generation, the initial checks and their outcomes stay in one pass"
)]
fn enumerate_init(
    machine: &mut Machine,
    plan: &Plan,
    model: &Model,
    options: &dive_sim::Options,
    poll: &(dyn Fn() -> bool + Sync),
    report: &mut Report,
) -> Option<Init> {
    let mut initial = Vec::new();
    for (index, action) in plan.init.iter().enumerate() {
        let mark = machine.heap.mark();
        let mut limited = false;
        let result = generate(machine, &plan.code, action, &mut |machine| {
            if poll() {
                return Err(Error::stop());
            }
            if initial.len() >= options.max_candidates
                || report.initial_candidates >= options.max_generated
            {
                limited = true;
                return Err(Error::stop());
            }
            report.initial_candidates += 1;
            initial.push((retain(machine)?, index));
            Ok(())
        });
        if let Err(error) = finish_batch(machine, mark) {
            report.outcome = classify(&error, None);
            return None;
        }
        if result.is_err() && draw_count(machine) > 0 {
            return Some(sampled_init_instead(machine, report));
        }
        if let Err(error) = result {
            report.outcome = if limited {
                Outcome::Limit("simulation initial-state bound".into())
            } else {
                classify_init(&error)
            };
            return None;
        }
    }
    report.generated = report.initial_candidates;
    let mut eligible = Vec::new();
    for (state, action) in initial {
        let mark = machine.heap.mark();
        if state.iter().any(|value| value.is_empty()) {
            report.outcome = Outcome::Error {
                code: 2106,
                message: "incomplete initial state".into(),
            };
            return None;
        }
        machine.s1.clone_from(&state);
        machine.level = 1;
        // As `dive check` orders an initial state's checks: invariants,
        // then implied initial predicates (2108, D-036), then constraints.
        if let Some(outcome) = roles(
            machine,
            plan,
            &plan.invariants,
            false,
            "invariant",
            2107,
            2102,
        )
        .or_else(|| implied_initial(machine, plan))
        {
            if options.max_retained == 0 {
                report.pending_violation = pending(&outcome);
                report.outcome = Outcome::Limit("simulation retained-state bound".into());
                return None;
            }
            report.outcome = outcome;
            match values(machine, model, &state) {
                // Init is checked before any trace, so its violation is the
                // first trace of the run.
                Ok(state) => report.traces.push(Trace {
                    id: options.first_trace,
                    states: vec![state],
                    actions: Vec::new(),
                    generated: 0,
                    witnesses: Vec::new(),
                    draws: Vec::new(),
                    temporal: None,
                    terminal: false,
                }),
                Err(message) => report.outcome = Outcome::Unsupported(message),
            }
            return None;
        }
        match allowed(machine, plan, &plan.constraints, false) {
            Ok(true) => eligible.push((state, action)),
            Ok(false) => {}
            Err(error) => {
                report.outcome = classify(&error, None);
                return None;
            }
        }
        if let Err(error) = finish_batch(machine, mark) {
            report.outcome = classify(&error, None);
            return None;
        }
    }
    machine.level = 0;
    if eligible.is_empty() {
        report.outcome = Outcome::Error {
            code: if report.initial_candidates == 0 {
                2118
            } else {
                2256
            },
            message: "no initial state satisfies the specification and constraints".into(),
        };
        return None;
    }
    Some(Init::Enumerated(eligible))
}

/// Init reached an unbounded set while enumerated once: nothing enumerated
/// so far counts, and nothing it printed is kept; each trace enumerates Init
/// with its own draws.
fn sampled_init_instead(machine: &mut Machine, report: &mut Report) -> Init {
    report.initial_candidates = 0;
    let _ = machine.take_output();
    machine.warnings.clear();
    Init::Sampled
}

/// One worker: claims trace ids in order and commits each finished trace.
fn work(machine: &mut Machine, cx: &Context<'_>, shared: &Shared) {
    let window = WINDOW_PER_WORKER.saturating_mul(cx.options.workers.max(1) as u64);
    // This worker's own Core machine for temporal atoms: no evaluator
    // state is shared with another worker.
    let mut native = cx.temporal.and_then(|checker| {
        checker.native(cx.model, machine.heap.store(), cx.budget, machine.opts)
    });
    loop {
        if shared.stop.load(Ordering::SeqCst) || (cx.poll)() {
            break;
        }
        let id = shared.next.fetch_add(1, Ordering::SeqCst);
        if id >= cx.options.end_trace() || id > shared.best.load(Ordering::SeqCst) {
            break;
        }
        if !shared.wait_window(id, window, cx.poll) {
            break;
        }
        let run = trace(machine, &mut native, cx, id);
        if matches!(run.end, End::Deciding(_)) {
            shared.best.fetch_min(id, Ordering::SeqCst);
        }
        let stopped = matches!(run.end, End::Stopped);
        shared.submit(run, cx);
        if stopped {
            break;
        }
    }
}

impl Shared {
    /// Waits until `id` is within `window` traces of the next to commit.
    /// False when the run ended meanwhile.
    fn wait_window(&self, id: u64, window: u64, poll: &(dyn Fn() -> bool + Sync)) -> bool {
        if id < self.committed.load(Ordering::SeqCst).saturating_add(window) {
            return !self.stop.load(Ordering::SeqCst);
        }
        self.waiting.fetch_add(1, Ordering::SeqCst);
        let mut commit = self.commit.lock().unwrap_or_else(PoisonError::into_inner);
        let mut open = true;
        while id >= commit.next.saturating_add(window) {
            if commit.finished || self.stop.load(Ordering::SeqCst) || poll() {
                open = false;
                break;
            }
            commit = self
                .ready
                .wait_timeout(commit, Duration::from_millis(5))
                .map_or_else(|e| e.into_inner().0, |(guard, _)| guard);
        }
        let open = open && !commit.finished;
        drop(commit);
        self.waiting.fetch_sub(1, Ordering::SeqCst);
        open
    }

    fn submit(&self, run: Run, cx: &Context<'_>) {
        let mut commit = self.commit.lock().unwrap_or_else(PoisonError::into_inner);
        if commit.finished {
            return;
        }
        commit.pending.insert(run.id, run);
        while !commit.finished {
            let next = commit.next;
            let Some(run) = commit.pending.remove(&next) else {
                break;
            };
            commit.apply(run, cx);
            commit.next += 1;
            if !commit.finished && commit.next >= cx.options.end_trace() {
                commit.finished = true;
            }
        }
        if commit.finished {
            commit.pending.clear();
            self.stop.store(true, Ordering::SeqCst);
        }
        self.committed.store(commit.next, Ordering::SeqCst);
        drop(commit);
        if self.waiting.load(Ordering::SeqCst) > 0 {
            self.ready.notify_all();
        }
    }
}

impl Commit {
    fn end(&mut self, outcome: Outcome) {
        self.report.outcome = outcome;
        self.finished = true;
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one trace's commit: bounds, output, statistics and retention in order"
    )]
    fn apply(&mut self, run: Run, cx: &Context<'_>) {
        let Run {
            id,
            rows,
            actions,
            generated,
            initial,
            printed,
            flow,
            witnesses,
            draws,
            sampled,
            stalled,
            exported,
            end,
            terminal,
            class,
            coverage,
            word,
            ..
        } = run;
        let outcome = match end {
            End::Stopped => {
                if let Some(temporal) = &mut self.report.temporal {
                    temporal.classes.cancelled += 1;
                }
                return self.end(Outcome::Cancelled);
            }
            End::Superseded => {
                return self.end(Outcome::Internal(
                    "a superseded simulation trace reached commit".into(),
                ));
            }
            End::Completed => None,
            End::Deciding(outcome) => Some(outcome),
        };
        let total = self
            .report
            .generated
            .checked_add(generated)
            .filter(|&total| total <= cx.options.max_generated);
        let Some(total) = total else {
            return self.end(Outcome::Limit("simulation generated-state bound".into()));
        };
        // Output is admitted in trace order against the run's allowance.
        for text in printed {
            let bytes = self.output.1.checked_sub(text.len());
            match bytes {
                Some(bytes) if self.output.0 > 0 => {
                    self.output = (self.output.0 - 1, bytes);
                    self.printed.push(text);
                }
                _ => {
                    let reason = if self.output.0 == 0 {
                        "simulation output event bound"
                    } else {
                        "simulation output byte bound"
                    };
                    return self.end(Outcome::Limit(reason.into()));
                }
            }
        }
        self.report.generated = total;
        self.report.initial_candidates += initial;
        self.report.traces_started += 1;
        self.stalled += u64::from(stalled);
        self.sampled.extend(sampled);
        if let Some(temporal) = &mut self.report.temporal {
            record(
                temporal,
                id,
                outcome.is_some(),
                class,
                &coverage,
                word.as_ref(),
            );
        }
        if let Some(action_flow) = &mut self.report.action_flow {
            for (from, to) in flow {
                if let Err(error) = action_flow.record(from, to) {
                    return self.end(Outcome::Internal(error.to_string()));
                }
            }
        }
        if exported
            && let Some(export) = cx.export
            && let Err(message) = export.commit(id)
        {
            return self.end(Outcome::Unsupported(message));
        }
        let length = word.as_ref().map_or(rows.len(), |word| word.states.len());
        let kept = Kept {
            id,
            rows,
            actions,
            generated,
            witnesses,
            draws,
            word,
            terminal,
        };
        let fits = self
            .retained
            .checked_add(length)
            .is_some_and(|total| total <= cx.options.max_retained);
        match outcome {
            None => {
                // A trace that stalled before its first state has nothing
                // to keep.
                if self.kept < cx.options.keep_traces && !kept.rows.is_empty() {
                    if !fits {
                        return self.end(Outcome::Limit("simulation retained-state bound".into()));
                    }
                    self.kept += 1;
                    self.retained += length;
                    self.rows.push(kept);
                }
            }
            Some(mut outcome) => {
                if !fits {
                    if matches!(outcome, Outcome::Violation { .. }) {
                        self.report.pending_violation = pending(&outcome);
                    }
                    return self.end(Outcome::Limit("simulation retained-state bound".into()));
                }
                if let Outcome::Violation { trace, .. } = &mut outcome {
                    *trace = self.rows.len();
                }
                // An error before the trace had a state (constructive Init)
                // leaves nothing to keep.
                if length > 0 {
                    self.retained += length;
                    self.rows.push(kept);
                }
                self.end(outcome);
            }
        }
    }
}

/// Recognizes the generator's "not enumerable" errors, which TLC reports as
/// GENERAL 1000 in simulation (oracle G3 to G6).
fn not_enumerable(error: &Error) -> bool {
    error.kind() == ErrorKind::Eval
        && matches!(
            error.message(),
            "The right side of \\IN is not enumerable" | "attempted to enumerate an infinite set"
        )
}

/// Classifies a reached error; a set the generator cannot enumerate is
/// TLC's GENERAL 1000 error.
fn classify_step(error: &Error, wrapper: Option<u32>) -> Outcome {
    if not_enumerable(error) {
        Outcome::Error {
            code: 1000,
            message: error.to_string(),
        }
    } else {
        classify(error, wrapper)
    }
}

fn classify_init(error: &Error) -> Outcome {
    classify_step(error, Some(1000))
}

/// Samples trace `id`. Reads only the shared plan, the initial states and
/// the trace's own stream; output goes to `machine` and is taken here.
#[allow(
    clippy::too_many_lines,
    reason = "emitted-candidate checks and the chosen transition stay in one loop"
)]
fn trace(
    machine: &mut Machine,
    native: &mut Option<NativeTemporal>,
    cx: &Context<'_>,
    id: u64,
) -> Run {
    let mut run = Run {
        id,
        rows: Vec::new(),
        actions: Vec::new(),
        generated: 0,
        initial: 0,
        printed: Vec::new(),
        flow: Vec::new(),
        witnesses: Vec::new(),
        draws: Vec::new(),
        sampled: Vec::new(),
        stalled: false,
        exported: false,
        end: End::Completed,
        terminal: false,
        extended: false,
        graph: None,
        verdict: None,
        class: None,
        coverage: Vec::new(),
        word: None,
    };
    machine.set_output_limits(cx.output.0, cx.output.1);
    let rng = RefCell::new(cx.streams.simulation_trace(id));
    let mut end = sample_into(machine, native, cx, id, &rng, &mut run);
    if matches!(end, End::Completed)
        && let Some(checker) = cx.temporal
    {
        match finish_temporal(machine, native, cx, checker, &mut run) {
            Ok(None) => {}
            Ok(Some(outcome)) | Err(outcome) => end = End::Deciding(outcome),
        }
    }
    let (printed, failure) = machine.take_output();
    run.printed = printed;
    run.end = match (end, failure) {
        (End::Stopped, _) => End::Stopped,
        (_, Some(reason)) => End::Deciding(Outcome::Limit(reason.into())),
        (end, None) if !machine.warnings.is_empty() => {
            machine.warnings.clear();
            match end {
                End::Superseded => End::Superseded,
                _ => End::Deciding(Outcome::Unsupported(
                    "native simulation warning/output interleaving".into(),
                )),
            }
        }
        (end, None) => end,
    };
    if machine.heap.poisoned() {
        run.end = End::Deciding(Outcome::Internal(
            "native finite mode read a stale value".into(),
        ));
    }
    // Every finished trace reaches the export before its rows can go.
    export(machine, cx, &mut run);
    // Ids commit in order, so only the first `keep_traces` ids are kept
    // unless they decide the run: the rows of any other finished trace go now.
    if matches!(run.end, End::Completed)
        && id.saturating_sub(cx.options.first_trace) >= cx.options.keep_traces
    {
        run.rows = Vec::new();
        run.actions = Vec::new();
        run.witnesses = Vec::new();
        run.draws = Vec::new();
    }
    if run.word.is_some() {
        run.rows = Vec::new();
    }
    run.graph = None;
    run
}

/// Adds a committed trace to the temporal coverage, in id order.
fn record(
    temporal: &mut dive_sim::TemporalCoverage,
    id: u64,
    deciding: bool,
    class: Option<Class>,
    coverage: &[Option<Unchecked>],
    word: Option<&Word>,
) {
    let classes = &mut temporal.classes;
    if deciding {
        classes.violated += 1;
        let refuted: Vec<&str> = match word {
            Some(Word {
                ending: TemporalEnding::BadPrefix(name),
                ..
            }) => vec![name.as_str()],
            Some(word) => word.properties.iter().map(String::as_str).collect(),
            None => Vec::new(),
        };
        for property in &mut temporal.properties {
            if refuted.contains(&property.name.as_str()) {
                property.refuted_by_trace.get_or_insert(id);
            }
        }
        return;
    }
    match class {
        Some(Class::Closed) => classes.closed += 1,
        Some(Class::Terminal) => classes.terminal += 1,
        Some(Class::Truncated) => classes.truncated += 1,
        Some(Class::ExtendedClosed) => classes.extended_closed += 1,
        Some(Class::ExtendedTerminal) => classes.extended_terminal += 1,
        None => {}
    }
    for (property, reason) in temporal.properties.iter_mut().zip(coverage) {
        match reason {
            None => property.checked += 1,
            Some(reason) => *property.unchecked.entry(*reason).or_default() += 1,
        }
    }
}

/// The trace graph of the rows sampled so far.
fn graph_of(
    machine: &mut Machine,
    native: &mut Option<NativeTemporal>,
    model: &Model,
    run: &Run,
) -> Result<TraceGraph, Outcome> {
    let mut graph = TraceGraph::default();
    for (position, row) in run.rows.iter().enumerate() {
        let state = state_of(machine, native, model, row).map_err(Outcome::Unsupported)?;
        let action = position
            .checked_sub(1)
            .and_then(|step| run.actions.get(step).copied());
        graph
            .push(model, state, row, action)
            .map_err(Outcome::Unsupported)?;
    }
    Ok(graph)
}

/// The value of an interned trace row, converted once per worker when the
/// worker has a native temporal evaluator.
fn state_of(
    machine: &mut Machine,
    native: &mut Option<NativeTemporal>,
    model: &Model,
    row: &[Handle],
) -> Result<dive_eval::next::StateVals, String> {
    match native {
        Some(native) => native.state(row, || values(machine, model, row)),
        None => values(machine, model, row),
    }
}

/// At the depth: whether to extend the trace, and by how many steps. A
/// trace extends only when it neither closed nor terminated and some
/// temporal property is unchecked on it because it was cut (D-039).
fn extension(
    machine: &mut Machine,
    native: &mut Option<NativeTemporal>,
    cx: &Context<'_>,
    run: &mut Run,
) -> Result<usize, Outcome> {
    let Some(checker) = cx.temporal else {
        return Ok(0);
    };
    if cx.options.liveness_extend == 0 {
        return Ok(0);
    }
    let graph = graph_of(machine, native, cx.model, run)?;
    if graph.closed_at().is_some() {
        run.graph = Some(graph);
        return Ok(0);
    }
    let verdict = checker.check(cx.model, native.as_mut(), &graph, &run.actions, true)?;
    run.graph = Some(graph);
    if matches!(&verdict, Verdict::Coverage(reasons) if reasons.contains(&Some(Unchecked::Truncated)))
    {
        run.extended = true;
        Ok(cx.options.liveness_extend)
    } else {
        run.verdict = Some(verdict);
        Ok(0)
    }
}

/// After an extension step: adds the new state to the trace graph; true
/// when the step closed a loop, which ends the extension.
fn extended_step(
    machine: &mut Machine,
    native: &mut Option<NativeTemporal>,
    cx: &Context<'_>,
    run: &mut Run,
) -> Result<bool, Outcome> {
    let (Some(row), Some(&action)) = (run.rows.last(), run.actions.last()) else {
        return Ok(false);
    };
    let state = state_of(machine, native, cx.model, row).map_err(Outcome::Unsupported)?;
    match &mut run.graph {
        Some(graph) => graph
            .push(cx.model, state, row, Some(action))
            .map_err(Outcome::Unsupported),
        None => Ok(false),
    }
}

/// Checks a completed trace's temporal properties: its class, and either a
/// counterexample word (which decides the run once validated) or coverage.
fn finish_temporal(
    machine: &mut Machine,
    native: &mut Option<NativeTemporal>,
    cx: &Context<'_>,
    checker: &Checker,
    run: &mut Run,
) -> Result<Option<Outcome>, Outcome> {
    let graph = match run.graph.take() {
        Some(graph) => graph,
        None => graph_of(machine, native, cx.model, run)?,
    };
    let class = match graph.closed_at() {
        Some(at) if run.extended && at > cx.options.depth => Class::ExtendedClosed,
        Some(_) => Class::Closed,
        None if run.terminal && run.extended => Class::ExtendedTerminal,
        None if run.terminal => Class::Terminal,
        None => Class::Truncated,
    };
    let verdict = match run.verdict.take() {
        Some(verdict) => verdict,
        None => checker.check(
            cx.model,
            native.as_mut(),
            &graph,
            &run.actions,
            class == Class::Truncated,
        )?,
    };
    run.class = Some(class);
    match verdict {
        // The word's steps are trace-graph edges, which do not carry the
        // members a step drew from an unbounded set, so it could not be
        // validated (D-040).
        Verdict::Violation(_) if !run.witnesses.is_empty() => Err(Outcome::Unsupported(
            "simulation temporal counterexample through members drawn from unbounded sets".into(),
        )),
        Verdict::Violation(_) if !run.draws.is_empty() => Err(Outcome::Unsupported(
            "simulation temporal counterexample through RandomElement or Randomization draws"
                .into(),
        )),
        Verdict::Violation(word) => {
            let name = match &word.ending {
                TemporalEnding::BadPrefix(name) => name.clone(),
                _ => word.properties.first().cloned().unwrap_or_default(),
            };
            run.word = Some(word);
            Ok(Some(Outcome::Violation {
                kind: "temporal",
                name: Some(name),
                code: 2116,
                trace: 0,
            }))
        }
        Verdict::Coverage(reasons) => {
            run.coverage = reasons;
            Ok(None)
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "emitted-candidate checks and the chosen transition stay in one loop"
)]
fn sample_into(
    machine: &mut Machine,
    native: &mut Option<NativeTemporal>,
    cx: &Context<'_>,
    id: u64,
    rng: &RefCell<Stream>,
    run: &mut Run,
) -> End {
    let options = cx.options;
    let plan = cx.plan;
    let poll = cx.poll;
    for _ in 0..options.aril {
        if poll() {
            return End::Stopped;
        }
        let _ = rng.borrow_mut().next_double();
    }
    let (mut current, action) = match cx.init {
        Init::Enumerated(eligible) => {
            let choice =
                usize::try_from(rng.borrow_mut().next_index(eligible.len() as u64)).unwrap_or(0);
            eligible[choice].clone()
        }
        Init::Constructive | Init::Sampled => {
            let chosen = if matches!(cx.init, Init::Sampled) {
                sampled_init(machine, cx, id, rng, run)
            } else {
                construct_init(machine, cx, id, rng, run)
            };
            match chosen {
                Ok((row, action, drawn, random)) => {
                    if !drawn.is_empty() {
                        run.witnesses.push((0, drawn));
                    }
                    push_drawn(&mut run.draws, 0, random);
                    (row, action)
                }
                Err(end) => return end,
            }
        }
    };
    let mut previous = cx.init_ids[action];
    run.rows.push(current.clone());
    let mut limit = options.depth;
    let mut steps = 0;
    loop {
        if steps == limit {
            if run.extended {
                break;
            }
            match extension(machine, native, cx, run) {
                Ok(0) => break,
                Ok(extra) => limit = limit.saturating_add(extra),
                Err(outcome) => return End::Deciding(outcome),
            }
        }
        steps += 1;
        if poll() {
            return End::Stopped;
        }
        if cx.best.load(Ordering::Relaxed) < id {
            return End::Superseded;
        }
        machine.s0.clone_from(&current);
        let source = level_of(run.rows.len());
        let target = level_of(run.rows.len() + 1);
        machine.level = source;
        // Each candidate: its row, its witnesses and how many of the
        // position's draws were made when it was emitted.
        let mut exact: Vec<(Vec<Handle>, Vec<Handle>, usize)> = Vec::new();
        let mut kept: Option<(Vec<Handle>, Vec<Handle>, usize)> = None;
        let mut admitted = 0u64;
        let start =
            usize::try_from(rng.borrow_mut().next_index(plan.next.len() as u64)).unwrap_or(0);
        let mut index;
        let sampled_prime = usize::try_from(rng.borrow_mut().next_prime()).unwrap_or(0);
        let prime = if options.generation { 1 } else { sampled_prime };
        let mut failed = None;
        let step = run.rows.len() as u64;
        let mut attempt = 0u64;
        let mut drew;
        loop {
            index = start;
            drew = 0;
            for _ in 0..plan.next.len() {
                let mark = machine.heap.mark();
                let mut selected = false;
                let before = restart_draws(machine, cx.random, [id, step, index as u64, attempt]);
                let generated = &mut run.generated;
                let rows = &mut run.rows;
                let actions = &mut run.actions;
                let run_witnesses = &mut run.witnesses;
                let run_draws = &mut run.draws;
                let mut emit = |machine: &mut Machine| {
                    if poll() {
                        return Err(Error::stop());
                    }
                    if machine.assigned != machine.s1.len() {
                        failed = Some(Outcome::Error {
                            code: 2109,
                            message: "incomplete successor state".into(),
                        });
                        return Err(Error::stop());
                    }
                    *generated += 1;
                    failed = at_level(machine, target, |machine| {
                        roles(
                            machine,
                            plan,
                            &plan.invariants,
                            false,
                            "invariant",
                            2110,
                            2111,
                        )
                    })
                    .or_else(|| {
                        roles(
                            machine,
                            plan,
                            &plan.implied_actions,
                            true,
                            "property",
                            2112,
                            2113,
                        )
                    });
                    if failed.is_some() {
                        match retain(machine).and_then(|row| Ok((row, witnesses(machine)?))) {
                            Ok((row, drawn)) => {
                                rows.push(row);
                                actions.push(index);
                                if !drawn.is_empty() {
                                    run_witnesses.push((rows.len() - 1, drawn));
                                }
                                push_drawn(run_draws, rows.len() - 1, random_drawn(machine));
                            }
                            Err(error) => failed = Some(classify(&error, None)),
                        }
                        return Err(Error::stop());
                    }
                    if at_level(machine, target, |machine| {
                        allowed(machine, plan, &plan.constraints, false)
                    })? && allowed(machine, plan, &plan.action_constraints, true)?
                    {
                        admitted += 1;
                        let draws = super::sim_random::drawn(machine).len();
                        if options.generation {
                            kept = Some((retain(machine)?, witnesses(machine)?, draws));
                            selected = true;
                            return Err(Error::stop());
                        }
                        if kept.is_none() && exact.len() < EXACT_CANDIDATES {
                            exact.push((retain(machine)?, witnesses(machine)?, draws));
                        } else {
                            let mut rng = rng.borrow_mut();
                            if kept.is_none() {
                                let choice = usize::try_from(rng.next_index(exact.len() as u64))
                                    .unwrap_or(0);
                                kept = Some(exact.swap_remove(choice));
                                exact = Vec::new();
                            }
                            if rng.next_index(admitted) == 0 {
                                kept = Some((retain(machine)?, witnesses(machine)?, draws));
                            }
                        }
                    }
                    Ok(())
                };
                let mut choose = |bound| rng.borrow_mut().next_index(bound);
                let choices = if options.generation {
                    Some(&mut choose as &mut dyn FnMut(u64) -> u64)
                } else {
                    None
                };
                let result =
                    generate_selected(machine, &plan.code, &plan.next[index], &mut emit, choices);
                drew += draw_count(machine) - before;
                note_sets(machine, &cx.next_labels[index], &mut run.sampled);
                if let Err(error) = finish_batch(machine, mark) {
                    failed = Some(classify(&error, None));
                }
                if let Err(error) = result
                    && failed.is_none()
                    && !(selected && error.kind() == ErrorKind::Stop)
                {
                    failed = Some(match error.kind() {
                        ErrorKind::Stop if poll() => return End::Stopped,
                        _ => classify_step(&error, None),
                    });
                }
                if failed.is_some() || admitted > 0 {
                    break;
                }
                index = (index + prime) % plan.next.len();
            }
            // Draws that led nowhere are drawn again; they never show that the
            // state has no successor.
            if failed.is_some() || admitted > 0 || drew == 0 || attempt + 1 >= SAMPLE_ATTEMPTS {
                break;
            }
            attempt += 1;
        }
        if let Some(outcome) = failed {
            return End::Deciding(outcome);
        }
        if admitted == 0 {
            if drew > 0 {
                run.stalled = true;
                return End::Completed;
            }
            if plan.check_deadlock {
                return End::Deciding(Outcome::Violation {
                    kind: "deadlock",
                    name: None,
                    code: 2114,
                    trace: 0,
                });
            }
            run.terminal = true;
            return End::Completed;
        }
        let (chosen, drawn, random) = if let Some(kept) = kept {
            kept
        } else {
            let choice =
                usize::try_from(rng.borrow_mut().next_index(exact.len() as u64)).unwrap_or(0);
            exact.swap_remove(choice)
        };
        current = chosen;
        run.rows.push(current.clone());
        if !drawn.is_empty() {
            run.witnesses.push((run.rows.len() - 1, drawn));
        }
        // Every candidate came from this position's one evaluation, so its
        // draws are the log's first `random` values.
        let log = super::sim_random::drawn(machine);
        let prefix = log[..random.min(log.len())].to_vec();
        push_drawn(&mut run.draws, run.rows.len() - 1, prefix);
        run.actions.push(index);
        let to = cx.next_ids[index];
        if cx.flows {
            run.flow.push((previous, to));
        }
        previous = to;
        if run.extended {
            match extended_step(machine, native, cx, run) {
                Ok(true) => break,
                Ok(false) => {}
                Err(outcome) => return End::Deciding(outcome),
            }
        }
    }
    End::Completed
}

/// The checks of an initial candidate in `machine.s1`, as enumerated Init
/// makes them: its deciding violation, else whether the constraints admit it.
fn initial_checks(machine: &mut Machine, plan: &Plan) -> Option<Outcome> {
    if machine.s1.iter().any(|value| value.is_empty()) {
        return Some(Outcome::Error {
            code: 2106,
            message: "incomplete initial state".into(),
        });
    }
    at_level(machine, 1, |machine| {
        roles(
            machine,
            plan,
            &plan.invariants,
            false,
            "invariant",
            2107,
            2102,
        )
        .or_else(|| implied_initial(machine, plan))
    })
}

/// No initial state: 2118 when Init generated none, else 2256.
fn no_initial_state(generated: bool) -> End {
    End::Deciding(Outcome::Error {
        code: if generated { 2256 } else { 2118 },
        message: "no initial state satisfies the specification and constraints".into(),
    })
}

/// An initial row, its action, its witnesses and its draws.
type Chosen = (Vec<Handle>, usize, Vec<Handle>, Vec<Handle>);

/// Init draws from an unbounded set: trace `id` enumerates Init with its own
/// draws, checks every candidate as enumerated Init does, and chooses one
/// admitted candidate uniformly (reservoir). Only an attempt that drew
/// nothing shows there is no initial state; otherwise Init is drawn again,
/// and the trace stalls after [`SAMPLE_ATTEMPTS`].
fn sampled_init(
    machine: &mut Machine,
    cx: &Context<'_>,
    id: u64,
    rng: &RefCell<Stream>,
    run: &mut Run,
) -> Result<Chosen, End> {
    let plan = cx.plan;
    let poll = cx.poll;
    for attempt in 0..SAMPLE_ATTEMPTS {
        machine.s0.fill(Handle::EMPTY);
        machine.level = 0;
        let mut kept: Option<Chosen> = None;
        let mut admitted = 0u64;
        let mut candidates = false;
        let mut drew = 0;
        for index in 0..plan.init.len() {
            let before = restart_draws(machine, cx.random, [id, 0, index as u64, attempt]);
            let mark = machine.heap.mark();
            let mut failed = None;
            let initial = &mut run.initial;
            let generated = &mut run.generated;
            let rows = &mut run.rows;
            let run_witnesses = &mut run.witnesses;
            let run_draws = &mut run.draws;
            let mut emit = |machine: &mut Machine| {
                if poll() {
                    return Err(Error::stop());
                }
                *initial += 1;
                *generated += 1;
                candidates = true;
                failed = initial_checks(machine, plan);
                if failed.is_some() {
                    match retain(machine).and_then(|row| Ok((row, witnesses(machine)?))) {
                        Ok((row, drawn)) => {
                            rows.push(row);
                            if !drawn.is_empty() {
                                run_witnesses.push((0, drawn));
                            }
                            push_drawn(run_draws, 0, random_drawn(machine));
                        }
                        Err(error) => failed = Some(classify(&error, None)),
                    }
                    return Err(Error::stop());
                }
                if at_level(machine, 1, |machine| {
                    allowed(machine, plan, &plan.constraints, false)
                })? {
                    admitted += 1;
                    if rng.borrow_mut().next_index(admitted) == 0 {
                        kept = Some((
                            retain(machine)?,
                            index,
                            witnesses(machine)?,
                            random_drawn(machine),
                        ));
                    }
                }
                Ok(())
            };
            let result = generate(machine, &plan.code, &plan.init[index], &mut emit);
            drew += draw_count(machine) - before;
            note_sets(machine, &cx.init_labels[index], &mut run.sampled);
            if let Err(error) = finish_batch(machine, mark) {
                return Err(End::Deciding(classify(&error, None)));
            }
            if let Some(outcome) = failed {
                return Err(End::Deciding(outcome));
            }
            if let Err(error) = result {
                if poll() {
                    return Err(End::Stopped);
                }
                return Err(End::Deciding(classify_init(&error)));
            }
        }
        if let Some(chosen) = kept {
            return Ok(chosen);
        }
        if drew == 0 {
            return Err(no_initial_state(candidates));
        }
    }
    run.stalled = true;
    Err(End::Completed)
}

/// Under `--constructive-init`: constructs the trace's initial state with
/// randomized Init choices, checking each candidate as enumerated Init does. The generator
/// searches every remaining alternative before giving up, so finding no
/// admitted state shows there is none (2118 or 2256).
fn construct_init(
    machine: &mut Machine,
    cx: &Context<'_>,
    id: u64,
    rng: &RefCell<Stream>,
    run: &mut Run,
) -> Result<Chosen, End> {
    let plan = cx.plan;
    let poll = cx.poll;
    machine.s0.fill(Handle::EMPTY);
    machine.level = 0;
    let count = plan.init.len();
    let mut index = usize::try_from(rng.borrow_mut().next_index(count as u64)).unwrap_or(0);
    let start = index;
    let mut drew = 0;
    let mut attempt = 0;
    for _ in 0..count.saturating_mul(usize::try_from(SAMPLE_ATTEMPTS).unwrap_or(1)) {
        let before = restart_draws(machine, cx.random, [id, 0, index as u64, attempt]);
        let mark = machine.heap.mark();
        let mut kept = None;
        let mut failed = None;
        let initial = &mut run.initial;
        let generated = &mut run.generated;
        let rows = &mut run.rows;
        let run_witnesses = &mut run.witnesses;
        let run_draws = &mut run.draws;
        let mut emit = |machine: &mut Machine| {
            if poll() {
                return Err(Error::stop());
            }
            *initial += 1;
            *generated += 1;
            failed = initial_checks(machine, plan);
            if failed.is_some() {
                match retain(machine).and_then(|row| Ok((row, witnesses(machine)?))) {
                    Ok((row, drawn)) => {
                        rows.push(row);
                        if !drawn.is_empty() {
                            run_witnesses.push((0, drawn));
                        }
                        push_drawn(run_draws, 0, random_drawn(machine));
                    }
                    Err(error) => failed = Some(classify(&error, None)),
                }
                return Err(Error::stop());
            }
            if at_level(machine, 1, |machine| {
                allowed(machine, plan, &plan.constraints, false)
            })? {
                kept = Some((retain(machine)?, witnesses(machine)?, random_drawn(machine)));
                return Err(Error::stop());
            }
            Ok(())
        };
        let mut choose = |bound| rng.borrow_mut().next_index(bound);
        let result = generate_selected(
            machine,
            &plan.code,
            &plan.init[index],
            &mut emit,
            Some(&mut choose),
        );
        drew += draw_count(machine) - before;
        note_sets(machine, &cx.init_labels[index], &mut run.sampled);
        if let Err(error) = finish_batch(machine, mark) {
            return Err(End::Deciding(classify(&error, None)));
        }
        if let Some(outcome) = failed {
            return Err(End::Deciding(outcome));
        }
        if let Some((row, drawn, draws)) = kept {
            return Ok((row, index, drawn, draws));
        }
        if let Err(error) = result {
            if poll() {
                return Err(End::Stopped);
            }
            return Err(End::Deciding(classify_init(&error)));
        }
        index = (index + 1) % count;
        if index == start {
            // Every Init alternative was searched: without draws, there is
            // no initial state; with them, Init is drawn again.
            if drew == 0 {
                return Err(no_initial_state(run.initial > 0));
            }
            drew = 0;
            attempt += 1;
        }
    }
    run.stalled = true;
    Err(End::Completed)
}
