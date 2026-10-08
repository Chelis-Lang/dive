//! Evaluation options: the run's value semantics plus the evaluator's own
//! profile-dependent choices (ADR 0010).

use dive_value::{Ctx, IntMode, ValueOrder};

/// The two profiles of ADR 0010.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Profile {
    /// dive's default profile: TLC-compatible except where the owner approved a
    /// better default (big integers, canonical `CHOOSE` order).
    #[default]
    Dive,
    /// `--compat=tlc`: every behaviour exactly TLC's.
    Tlc,
}

/// In which order the evaluator *explores* sets: the bound sets of `\E` and
/// `\A`, set filters `{x \in S : P}`, set maps `{e : x \in S}`, function
/// constructors and (in `dive_eval::next`) `x' \in S`.
///
/// `CHOOSE` always follows the run's [`ValueOrder`] (owner decision, open
/// question 8). Exploration order is open question 9: whether the default
/// profile explores in the canonical order too (option (a)) or keeps TLC's
/// order (option (b)). Until the owner answers, the default is TLC's order in
/// both profiles, so the default profile reports the same first violation as
/// TLC. Changing [`Options::dive`]'s `exploration` is the one-line switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum ExplorationOrder {
    /// TLC's normalized order (names by intern token), whatever the run's
    /// [`ValueOrder`]. `--explore-order=tlc`; part of `--compat=tlc`.
    #[default]
    Tlc,
    /// The run's [`ValueOrder`] (canonical in the default profile):
    /// `--explore-order=value`. Open question 9, option (a).
    Value,
}

/// How `=` and `\in` treat values that TLC refuses to compare (D-009, open
/// question 12).
///
/// TLC's rules are asymmetric and order-dependent: `[a |-> 1, b |-> 1] =
/// <<1>>` is an error while `<<1>> = [a |-> 1, b |-> 1]` is `FALSE`. The
/// proposal `--comparisons=tla` answers such tests as TLA+ defines them. It is
/// result-changing and not adopted: the default is TLC's rules in both
/// profiles until the owner answers. Changing [`Options::dive`]'s
/// `comparisons` is the one-line switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Comparisons {
    /// TLC's checked `equals` and `member`, with TLC's errors.
    /// `--comparisons=tlc`; part of `--compat=tlc`.
    #[default]
    Tlc,
    /// TLA+'s answers for `=` and `\in` between functions (including records
    /// and tuples) and sets: symmetric and independent of element order. The
    /// error is kept only where TLA+ leaves the answer unspecified (`1 = "a"`,
    /// `{1} = 1`). `--comparisons=tla` (proposed, D-009).
    Tla,
}

/// The evaluator's options.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Options {
    /// The value semantics: integer mode and value (`CHOOSE`) order.
    pub ctx: Ctx,
    /// The exploration order (open question 9).
    pub exploration: ExplorationOrder,
    /// Comparison semantics (open question 12, D-009).
    pub comparisons: Comparisons,
    /// The maximum evaluation depth (nested operator applications, function
    /// applications and set constructions). Exceeding it is a resource limit
    /// naming the construct, never a stack overflow.
    pub max_depth: u32,
}

/// The stack size evaluation needs at [`DEFAULT_MAX_DEPTH`], with room to
/// spare, in an unoptimized build. Run evaluation on a thread with at least
/// this much stack (ADR 0014 §8: workers run on threads with large stacks).
pub const RECOMMENDED_STACK: usize = 256 << 20;

/// The default evaluation depth limit. Evaluation runs on the caller's stack;
/// callers that raise this must provide a correspondingly large stack.
pub const DEFAULT_MAX_DEPTH: u32 = 400;

impl Options {
    /// `--compat=tlc`: every option at its TLC setting.
    #[must_use]
    pub const fn tlc() -> Self {
        Self {
            ctx: Ctx::tlc(),
            exploration: ExplorationOrder::Tlc,
            comparisons: Comparisons::Tlc,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }

    /// dive's default profile.
    ///
    /// - Big integers and canonical `CHOOSE` order (owner-approved, ADR 0010).
    /// - Exploration in TLC's order, pending open question 9.
    /// - TLC's comparison rules, pending open question 12.
    #[must_use]
    pub const fn dive() -> Self {
        Self {
            ctx: Ctx::dive(),
            exploration: ExplorationOrder::Tlc,
            comparisons: Comparisons::Tlc,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }

    /// The options of a profile.
    #[must_use]
    pub const fn for_profile(profile: Profile) -> Self {
        match profile {
            Profile::Dive => Self::dive(),
            Profile::Tlc => Self::tlc(),
        }
    }

    /// Whether every option is at its TLC setting (a pure `--compat=tlc` run).
    #[must_use]
    pub fn is_tlc(&self) -> bool {
        self.ctx.int_mode == IntMode::Tlc
            && self.ctx.order == ValueOrder::Tlc
            && self.exploration == ExplorationOrder::Tlc
            && self.comparisons == Comparisons::Tlc
    }

    /// The order sets are explored in.
    #[must_use]
    pub(crate) fn explore_order(self) -> ValueOrder {
        match self.exploration {
            ExplorationOrder::Tlc => ValueOrder::Tlc,
            ExplorationOrder::Value => self.ctx.order,
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::dive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tlc_profile_is_pure_tlc() {
        assert!(Options::tlc().is_tlc());
        assert!(!Options::dive().is_tlc());
        assert_eq!(Options::for_profile(Profile::Tlc), Options::tlc());
    }

    #[test]
    fn pending_owner_questions_default_to_tlc() {
        // Open questions 9 and 12: TLC's behaviour until the owner answers.
        assert_eq!(Options::dive().exploration, ExplorationOrder::Tlc);
        assert_eq!(Options::dive().comparisons, Comparisons::Tlc);
    }
}
