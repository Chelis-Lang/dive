//! The behavior graph of a run, as events for a temporal check
//! (`docs/developer/core/11-liveness.md` §3).
//!
//! A [`GraphSink`] passed to [`crate::explore_with`] receives, in the
//! one-worker commit order at any worker count, the events from which the
//! reference engine's capture builds its `BehaviorGraph`:
//!
//! - [`GraphSink::emitted`] for every generated state, before its checks;
//! - [`GraphSink::state`] for every state inside the model that passed its
//!   checks, new or already seen, with its source, ordinal and action;
//! - [`GraphSink::stutter`] once a source is fully expanded;
//! - [`GraphSink::level_done`] after each completed level, which may stop
//!   the search there ([`crate::Verdict::PrefixStop`]).
//!
//! States outside the model are not reported. Vertex ids are discovery
//! indexes, which are the ids of the predecessor log.

use dive_core::{Handle, Heap};

use crate::Stats;

/// The sink ended the search; it keeps its own reason
/// ([`crate::Verdict::GraphStop`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stopped;

/// One accepted state inside the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step<'a> {
    /// The state's key in the seen set; equal keys are one vertex.
    pub key: u64,
    /// Its values (persistent handles) when it is new, with its new id.
    pub new: Option<(u64, &'a [Handle])>,
    /// The expanded source (`None` for an initial state).
    pub parent: Option<u64>,
    /// Its position among the states generated from the same source (or
    /// among the initial states), from 0.
    pub ordinal: u64,
    /// The index of the next-state action that emitted it (`None` for an
    /// initial state).
    pub action: Option<usize>,
}

/// Receives the behavior graph of a run.
pub trait GraphSink {
    /// The `generated`-th generated state, the `ordinal`-th of its source
    /// (`None` for an initial state), before its checks.
    ///
    /// # Errors
    /// [`Stopped`] ends the search.
    fn emitted(&mut self, generated: u64, ordinal: Option<u64>) -> Result<(), Stopped>;

    /// An accepted state inside the model. `heap` reads its values.
    ///
    /// # Errors
    /// [`Stopped`] ends the search.
    fn state(&mut self, heap: &mut Heap, step: Step<'_>) -> Result<(), Stopped>;

    /// The fully expanded source `source`: its implicit stutter.
    ///
    /// # Errors
    /// [`Stopped`] ends the search.
    fn stutter(&mut self, source: u64) -> Result<(), Stopped>;

    /// The level `level` was expanded and committed (the initial states are
    /// level 1); `stats.left_on_queue` is the next level's size. Returns
    /// whether the search stops here.
    ///
    /// # Errors
    /// [`Stopped`] ends the search.
    fn level_done(&mut self, level: u64, stats: Stats) -> Result<bool, Stopped>;
}
