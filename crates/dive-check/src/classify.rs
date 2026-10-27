//! TLC's error classes for another engine's error endings.
//!
//! The new execution core (`--engine=core`, `docs/developer/core/07-engine.md`
//! §5) finds where an evaluation error ends a search and reproduces the error
//! with the reference evaluator. It then classifies the error by its site
//! with the reference engine's table (`class.rs`), so both engines report the
//! same class, message and exit. These are thin public views of that table;
//! the rules and their evidence are documented there.

use dive_eval::EvalError;

use crate::class;
use crate::result::{ErrorSite, Verdict};

/// An error raised at `site` while exploring, as TLC classifies it.
/// `name` is the invariant, constraint or property, where there is one.
#[must_use]
pub fn search_error(e: &EvalError, site: ErrorSite, name: Option<&str>) -> Verdict {
    class::search_error(e, site, name)
}

/// An error raised while generating or checking the initial states. `state`
/// is TLC's text of the initial state being checked, for an error in a
/// check of a state, and `None` for an error while generating them.
#[must_use]
pub fn init_error(v: Verdict, state: Option<&str>) -> Verdict {
    class::init_error(v, state)
}

/// An invariant that failed to evaluate on a state outside the model, which
/// TLC's replay checks again: the replay decides.
#[must_use]
pub fn recurring(v: Verdict) -> Verdict {
    class::recurring(v)
}

/// The class once TLC's call-stack replay meets the further error `e2`
/// after an invariant failed to evaluate on a new state.
#[must_use]
pub fn replay_error(v: Verdict, e2: &EvalError) -> Verdict {
    class::replay_error(v, e2)
}
