//! Successor sampling for the shadow comparison
//! (`docs/developer/core/08-correctness.md` §4).
//!
//! A run with [`Config::sample`](crate::Config::sample) keeps a uniform
//! sample of the states it expanded (reservoir sampling, seeded, so the same
//! seed picks the same states at any worker count), retaining each source's
//! actual evaluation level. [`successors`] then
//! regenerates each sampled state's successors, action by action, for the
//! caller to compare with the reference evaluator's.

use std::sync::Arc;

use dive_core::memo::Memo;
use dive_core::plan::Code;
use dive_core::{Error, Handle, Machine, Opts, Plan, Sink, Store};

/// What to sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    /// The seed of the selection.
    pub seed: u64,
    /// At most this many states.
    pub states: usize,
}

/// An expanded state and the evaluation level used for its successors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampledState {
    /// Persistent state handles, in the plan's variable order.
    pub values: Vec<Handle>,
    /// The source state's `TLCGet("level")` (initial states have level 1).
    pub level: i64,
}

/// A reservoir of expanded states (Algorithm R).
#[derive(Debug)]
pub(crate) struct Reservoir {
    want: usize,
    rng: u64,
    seen: u64,
    pub(crate) states: Vec<SampledState>,
}

impl Reservoir {
    pub(crate) fn new(s: Sample) -> Self {
        Self {
            want: s.states,
            rng: s.seed ^ 0x9e37_79b9_7f4a_7c15,
            seen: 0,
            states: Vec::new(),
        }
    }

    fn next(&mut self) -> u64 {
        // SplitMix64.
        self.rng = self.rng.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Offers the state about to be expanded.
    pub(crate) fn offer(&mut self, state: &[Handle], level: i64) {
        if self.want == 0 {
            return;
        }
        self.seen += 1;
        if self.states.len() < self.want {
            self.states.push(SampledState {
                values: state.to_vec(),
                level,
            });
            return;
        }
        let j = self.next() % self.seen;
        if let Some(slot) = usize::try_from(j).ok().and_then(|j| self.states.get_mut(j)) {
            *slot = SampledState {
                values: state.to_vec(),
                level,
            };
        }
    }
}

/// The successors of one state by one action: every emission in order,
/// as persistent handles (empty handles are unassigned variables), or the
/// core's message for the evaluation error that ended the action.
pub type ActionSuccessors = Result<Vec<Vec<Handle>>, String>;

struct Collect {
    states: Vec<Vec<Handle>>,
}

impl Sink for Collect {
    fn emit(&mut self, m: &mut Machine, _code: &Code) -> Result<(), Error> {
        let mut values = Vec::with_capacity(m.s1.len());
        for &h in &m.s1.clone() {
            values.push(if h.is_empty() { h } else { m.heap.intern(h)? });
        }
        self.states.push(values);
        Ok(())
    }
}

/// The successors of `s0` under each action of [`Plan::next`], in order,
/// evaluated at the sampled source state's actual `level`.
///
/// # Errors
/// An error other than an evaluation error (a refusal or a core bug).
pub fn successors(
    plan: &Plan,
    store: &Arc<Store>,
    opts: Opts,
    warn: bool,
    s0: &[Handle],
    level: i64,
) -> Result<Vec<ActionSuccessors>, Error> {
    let keys: Vec<usize> = plan
        .code
        .sites
        .iter()
        .map(dive_core::plan::MemoSite::key_len)
        .collect();
    let mut m = Machine::new(Arc::clone(store), plan.vars.len(), opts, Memo::new(&keys));
    m.warn = warn;
    m.level = level;
    if s0.len() != m.s0.len() {
        return Err(Error::internal("a sampled state of the wrong width"));
    }
    let mut out = Vec::with_capacity(plan.next.len());
    for a in &plan.next {
        m.s0.copy_from_slice(s0);
        let mut sink = Collect { states: Vec::new() };
        let old = m.enter(a.frame);
        for &(slot, v) in &a.preset {
            if let Some(x) = m.frame.get_mut(m.base + slot as usize) {
                *x = v;
            }
        }
        let r = m.run_action(&plan.code, a.g, &mut sink);
        m.leave(old);
        m.heap.reset();
        out.push(match r {
            Ok(()) => Ok(sink.states),
            Err(e) if e.kind() == dive_core::ErrorKind::Eval => Err(e.message().to_owned()),
            Err(e) => return Err(e),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{Reservoir, Sample, SampledState};
    use dive_core::Handle;

    fn picks(seed: u64, n: i64) -> Vec<SampledState> {
        let mut r = Reservoir::new(Sample { seed, states: 4 });
        for i in 0..n {
            r.offer(&[Handle::int(i).unwrap_or(Handle::EMPTY)], i + 1);
        }
        r.states
    }

    #[test]
    fn the_seed_decides_the_sample() {
        assert_eq!(picks(1, 3).len(), 3);
        assert_eq!(picks(1, 1000).len(), 4);
        assert_eq!(picks(1, 1000), picks(1, 1000));
        assert_ne!(picks(1, 1000), picks(2, 1000));
        let mut r = Reservoir::new(Sample { seed: 1, states: 0 });
        r.offer(&[Handle::EMPTY], 1);
        assert_eq!(r.states.len(), 0);
    }

    #[test]
    fn levels_are_retained_without_changing_algorithm_r_selection() {
        let sample = picks(1, 1000);
        let selected: Vec<_> = sample.iter().map(|s| s.values[0].as_int()).collect();
        assert_eq!(selected, vec![Some(669), Some(231), Some(180), Some(745)]);
        for state in sample {
            assert_eq!(state.values[0].as_int().map(|n| n + 1), Some(state.level));
        }
    }
}
