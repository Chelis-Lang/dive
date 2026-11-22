//! The exact seen set's discovery ids: with every state key forced equal,
//! the exact store must report the same behavior graph as an uncollided
//! fingerprint run, with each seen target named by its own discovery id.

#![allow(clippy::unwrap_used, reason = "deterministic graph controls")]

use std::collections::HashMap;
use std::sync::Arc;

use dive_core::plan::{Action, Arith, Code, CompileStats, Effects, GNode, Node};
use dive_core::{Error, Handle, Heap, Opts, Plan, Store};

use crate::graph::{GraphSink, SourceRow, Step, Stopped};
use crate::{Config, Stats, Verdict, explore_with_storage, storage};

/// States `x \in 0..1`, actions `x' = (x + 1) % N` and `x' = (2 * x) % N`:
/// every state is reachable, over several levels, with many seen targets.
const N: i64 = 37;
/// [`N`] as a count.
const STATES: usize = 37;

fn plan(store: &Arc<Store>) -> Result<Plan, Error> {
    let mut heap = Heap::new(Arc::clone(store), true, false);
    let initial = heap.interval(0, 1);
    let initial = heap.intern_const(initial)?;
    let int = |v| Handle::int(v).ok_or_else(|| Error::internal("small integer"));
    let action = |name: &str, g| Action {
        name: name.into(),
        g,
        frame: 0,
        preset: Vec::new(),
    };
    Ok(Plan {
        code: Code {
            nodes: vec![
                Node::Const(initial),
                Node::Var(0),
                Node::Const(int(1)?),
                Node::Arith(Arith::Add, 1, 2),
                Node::Const(int(N)?),
                Node::Arith(Arith::Mod, 3, 4),
                Node::Const(int(2)?),
                Node::Arith(Arith::Mul, 1, 6),
                Node::Arith(Arith::Mod, 7, 4),
            ],
            gnodes: vec![
                GNode::AssignIn { var: 0, set: 0 },
                GNode::Assign { var: 0, rhs: 5 },
                GNode::Assign { var: 0, rhs: 8 },
            ],
            ..Code::default()
        },
        vars: vec!["x".into()],
        init: vec![action("Init", 0)],
        next: vec![action("Inc", 1), action("Dbl", 2)],
        invariants: Vec::new(),
        constraints: Vec::new(),
        action_constraints: Vec::new(),
        implied_inits: Vec::new(),
        implied_actions: Vec::new(),
        check_deadlock: true,
        symmetry: None,
        effects: Effects::default(),
        view: None,
        stats: CompileStats::default(),
        bound_preset: Vec::new(),
        startup_assumptions: Vec::new(),
        startup_assumption_false: None,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Emitted(u64, Option<u64>),
    State {
        key: u64,
        new: Option<(u64, Vec<i64>)>,
        seen: Option<u64>,
        original: Vec<i64>,
        parent: Option<u64>,
        ordinal: u64,
        action: Option<usize>,
    },
    Stutter(u64),
    Source(u64, u64),
    Level(u64, Stats),
}

#[derive(Default)]
struct Recorder {
    events: Vec<Event>,
}

fn ints(row: &[Handle]) -> Vec<i64> {
    row.iter().map(|h| h.as_int().unwrap()).collect()
}

impl GraphSink for Recorder {
    fn max_expansion_units(&self) -> usize {
        64
    }

    fn max_successors_per_unit(&self) -> u64 {
        1 << 20
    }

    fn emitted(&mut self, generated: u64, ordinal: Option<u64>) -> Result<(), Stopped> {
        self.events.push(Event::Emitted(generated, ordinal));
        Ok(())
    }

    fn state(&mut self, _heap: &mut Heap, step: Step<'_>) -> Result<(), Stopped> {
        self.events.push(Event::State {
            key: step.key,
            new: step.new.map(|(id, row)| (id, ints(row))),
            seen: step.seen,
            original: ints(step.original),
            parent: step.parent,
            ordinal: step.ordinal,
            action: step.action,
        });
        Ok(())
    }

    fn stutter(&mut self, source: u64) -> Result<(), Stopped> {
        self.events.push(Event::Stutter(source));
        Ok(())
    }

    fn source_row(&mut self, _heap: &mut Heap, row: SourceRow<'_>) -> Result<(), Stopped> {
        self.events.push(Event::Source(row.source, row.emitted));
        Ok(())
    }

    fn level_done(&mut self, level: u64, stats: Stats) -> Result<bool, Stopped> {
        self.events.push(Event::Level(level, stats));
        Ok(false)
    }
}

fn run(
    exact: bool,
    collide: Option<u64>,
    workers: usize,
    capture: bool,
) -> (Verdict, Stats, usize, Vec<Event>) {
    let store = Arc::new(Store::new(true));
    let plan = plan(&store).unwrap();
    let mut sink = Recorder::default();
    let out = explore_with_storage(
        &plan,
        &store,
        Config {
            opts: Opts::default(),
            max_states: None,
            max_depth: None,
            workers,
            warn: true,
            sample: None,
        },
        &|| false,
        capture.then_some(&mut sink as &mut dyn GraphSink),
        None,
        &storage::Storage {
            exact,
            collide_keys: collide,
            ..storage::Storage::default()
        },
    );
    (
        out.verdict,
        out.stats,
        out.metrics.storage.exact_states,
        sink.events,
    )
}

/// The events with every seen target resolved to a discovery id: through
/// `seen` when the store reports one, otherwise through the key.
fn resolved(events: &[Event]) -> Vec<Event> {
    let mut by_key = HashMap::new();
    let mut rows = HashMap::new();
    events
        .iter()
        .map(|event| match event {
            Event::State {
                key,
                new,
                seen,
                original,
                parent,
                ordinal,
                action,
            } => {
                let target = if let Some((id, row)) = new {
                    assert_eq!(*seen, None, "a new state carries no seen id");
                    by_key.insert(*key, *id);
                    rows.insert(*id, row.clone());
                    *id
                } else {
                    let id = seen.unwrap_or_else(|| by_key[key]);
                    assert_eq!(
                        rows[&id], *original,
                        "a seen target's id names a vertex with its own values"
                    );
                    id
                };
                Event::State {
                    key: 0,
                    new: new.clone(),
                    seen: Some(target),
                    original: original.clone(),
                    parent: *parent,
                    ordinal: *ordinal,
                    action: *action,
                }
            }
            other => other.clone(),
        })
        .collect()
}

#[test]
fn exact_collisions_report_the_fingerprint_graph_with_discovery_ids() {
    let (verdict, stats, _, reference) = run(false, None, 1, true);
    assert_eq!(verdict, Verdict::Success);
    assert_eq!(stats.distinct, STATES as u64);
    assert!(stats.depth > 3, "several levels: {stats:?}");
    let expected = resolved(&reference);
    for workers in [1, 2, 4] {
        let (fp_verdict, fp_stats, _, fp_events) = run(false, None, workers, true);
        assert_eq!((fp_verdict, fp_stats), (Verdict::Success, stats));
        assert_eq!(fp_events, reference, "fingerprint at {workers} workers");
        for collide in [None, Some(0), Some(7)] {
            let (verdict, exact_stats, retained, events) = run(true, collide, workers, true);
            assert_eq!(verdict, Verdict::Success, "{workers} {collide:?}");
            assert_eq!(exact_stats, stats, "{workers} {collide:?}");
            assert_eq!(retained, STATES);
            let mut seen_steps = 0;
            for event in &events {
                if let Event::State {
                    new: None, seen, ..
                } = event
                {
                    assert!(seen.is_some(), "exact seen steps carry their id");
                    seen_steps += 1;
                }
                if let (Event::State { key, .. }, Some(collide)) = (event, collide) {
                    assert_eq!(*key, collide);
                }
            }
            assert!(seen_steps > STATES, "many seen targets");
            assert_eq!(
                resolved(&events),
                expected,
                "exact at {workers} workers with keys {collide:?}"
            );
        }
    }
}

#[test]
fn fingerprint_collisions_merge_states_without_ids() {
    // The control: the same forced collision in fingerprint mode loses every
    // state but the first of each key, so the exact equality above is not
    // vacuous.
    let (_, stats, retained, events) = run(false, Some(0), 1, true);
    assert_eq!(stats.distinct, 1);
    assert_eq!(retained, 0);
    assert!(events.iter().all(|event| match event {
        Event::State { seen, .. } => seen.is_none(),
        _ => true,
    }));
}

#[test]
fn resident_exact_hits_preserve_collisions_without_graph_capture() {
    let (verdict, stats, _, _) = run(false, None, 1, false);
    assert_eq!(verdict, Verdict::Success);
    assert_eq!(stats.distinct, STATES as u64);
    for workers in [2, 8] {
        let (verdict, exact_stats, retained, _) = run(true, Some(0), workers, false);
        assert_eq!(
            (verdict, exact_stats, retained),
            (Verdict::Success, stats, STATES)
        );
    }
}
