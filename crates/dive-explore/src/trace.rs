//! Counterexample reconstruction (`07-engine.md` §4).
//!
//! The search logs only `(parent, ordinal)` for each new state, in a
//! [`dive_store::predecessor::PredecessorLog`]. A trace is rebuilt by
//! replaying the recorded ordinals with the same generator: the initial
//! predicates are enumerated and the state at the root's ordinal is kept,
//! then each parent's actions are enumerated in order and the successor at
//! the recorded ordinal is kept, with the action that emitted it. The
//! replay runs with memoization off. A replayed state that differs from the
//! state the search expanded is an internal error, never a trace.

use std::sync::Arc;

use dive_core::memo::Memo;
use dive_core::plan::{Action, Code};
use dive_core::{Error, Handle, Machine, Opts, Plan, Sink, Store};
use dive_store::trace::TraceStep;

/// One state of a reconstructed behavior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceState {
    /// The index of the next-state action (in [`Plan::next`]) that
    /// produced it; `None` for an initial state.
    pub action: Option<usize>,
    /// The state's values, as persistent handles of the run's store; empty
    /// handles are unassigned variables.
    pub values: Vec<Handle>,
}

/// Captures the emission at one ordinal.
struct Capture {
    want: u32,
    seen: u32,
    action: usize,
    got: Option<(usize, Vec<Handle>)>,
}

impl Sink for Capture {
    fn emit(&mut self, m: &mut Machine, _code: &Code) -> Result<(), Error> {
        if self.seen == self.want {
            let mut values = Vec::with_capacity(m.s1.len());
            for &h in &m.s1 {
                values.push(if h.is_empty() { h } else { m.heap.intern(h)? });
            }
            self.got = Some((self.action, values));
            return Err(Error::stop());
        }
        self.seen += 1;
        Ok(())
    }
}

fn run(
    m: &mut Machine,
    code: &Code,
    actions: &[Action],
    want: u32,
) -> Result<Option<(usize, Vec<Handle>)>, Error> {
    let mut sink = Capture {
        want,
        seen: 0,
        action: 0,
        got: None,
    };
    for (i, a) in actions.iter().enumerate() {
        sink.action = i;
        let old = m.enter(a.frame);
        for &(slot, v) in &a.preset {
            if let Some(x) = m.frame.get_mut(m.base + slot as usize) {
                *x = v;
            }
        }
        let r = m.run_action(code, a.g, &mut sink);
        m.leave(old);
        match r {
            Ok(()) => {}
            Err(e) if e.kind() == dive_core::ErrorKind::Stop => break,
            Err(e) => return Err(e),
        }
    }
    m.heap.reset();
    Ok(sink.got)
}

/// Replays `path` (root first) and returns its states.
///
/// # Errors
/// An internal error when the replay does not reach a recorded ordinal, or
/// an evaluation error the replay raised.
pub fn replay(
    plan: &Plan,
    store: &Arc<Store>,
    opts: Opts,
    warn: bool,
    path: &[TraceStep],
) -> Result<Vec<TraceState>, Error> {
    let keys: Vec<usize> = plan
        .code
        .sites
        .iter()
        .map(|s| s.vars.len() + s.slots.len())
        .collect();
    let opts = Opts {
        memo: false,
        ..opts
    };
    let mut m = Machine::new(Arc::clone(store), plan.vars.len(), opts, Memo::new(&keys));
    m.warn = warn;
    let mut out: Vec<TraceState> = Vec::with_capacity(path.len());
    for step in path {
        let got = if let Some(prev) = out.last() {
            m.s0.copy_from_slice(&prev.values);
            crate::start_stream(&mut m, plan, Some(&prev.values))?;
            // The predecessor's level (initial states are level 1).
            m.level = i64::try_from(out.len()).unwrap_or(i64::MAX);
            run(&mut m, &plan.code, &plan.next, step.ordinal)?.map(|(a, v)| (Some(a), v))
        } else {
            crate::start_stream(&mut m, plan, None)?;
            m.level = 0;
            run(&mut m, &plan.code, &plan.init, step.ordinal)?.map(|(_, v)| (None, v))
        };
        let Some((action, values)) = got else {
            return Err(Error::internal(format!(
                "trace replay did not reach ordinal {} of state {}",
                step.ordinal, step.id
            )));
        };
        out.push(TraceState { action, values });
    }
    Ok(out)
}
