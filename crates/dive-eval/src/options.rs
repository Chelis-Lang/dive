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
/// question 8). Exploration follows it too in the default profile (owner
/// decision on open question 9, option (a), 2026-10-08), so the default
/// profile is independent of the module text; `--compat=tlc` keeps TLC's
/// order (D-010).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum ExplorationOrder {
    /// TLC's normalized order (names by intern token), whatever the run's
    /// [`ValueOrder`]. `--explore-order=tlc`; part of `--compat=tlc`.
    #[default]
    Tlc,
    /// The run's [`ValueOrder`] (canonical in the default profile):
    /// `--explore-order=value`. The default profile's setting (D-010).
    Value,
}

/// How `=` and `\in` treat values that TLC refuses to compare (D-009, open
/// question 12).
///
/// TLC's rules are asymmetric and order-dependent: `[a |-> 1, b |-> 1] =
/// <<1>>` is an error while `<<1>> = [a |-> 1, b |-> 1]` is `FALSE`.
/// `--comparisons=tla` answers such tests as TLA+ defines them. It is
/// result-changing and is the default profile's setting (owner decision on
/// open question 12, 2026-10-08); `--compat=tlc` keeps TLC's rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Comparisons {
    /// TLC's checked `equals` and `member`, with TLC's errors.
    /// `--comparisons=tlc`; part of `--compat=tlc`.
    #[default]
    Tlc,
    /// TLA+'s answers for `=` and `\in` between functions (including records
    /// and tuples) and sets: symmetric and independent of element order. The
    /// error is kept only where TLA+ leaves the answer unspecified (`1 = "a"`,
    /// `{1} = 1`). `--comparisons=tla`; the default profile's setting (D-009).
    Tla,
}

/// Whether and how `A \cdot B` (action composition) is evaluated (D-022).
///
/// TLC refuses action composition unless the JVM runs with
/// `-Dtlc2.tool.impl.Tool.cdot=true`, which enables what TLC calls an
/// incomplete implementation (`Tool.java`, `OPCODE_cdot`). dive implements the
/// operator by default, with its TLA+ meaning: `s -(A \cdot B)-> u` iff some
/// state `t` has `s -A-> t` and `t -B-> u`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum ActionComposition {
    /// TLC without the switch: every use is TLC's error
    /// (`--action-composition=off`; part of `--compat=tlc`).
    Off,
    /// TLC with `-Dtlc2.tool.impl.Tool.cdot=true`, reproduced exactly
    /// (`--action-composition=tlc`): the intermediate states count as
    /// generated (so a state whose only `A` steps have no `B` step is not a
    /// deadlock), the rest of an enclosing conjunction is evaluated after `A`
    /// as well as after `B`, and a variable the conjunction already assigned
    /// is pre-set in the intermediate state to its current value.
    Tlc,
    /// The TLA+ meaning (`--action-composition=tla`, the default profile's
    /// setting): only composed successors are generated, and a composition
    /// whose result TLC's algorithm would distort (one with a continuation or
    /// with variables already assigned) is refused by name, never
    /// approximated.
    #[default]
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
    /// TLC's warning switch (`TLCGlobals.warn`; `-nowarning` turns it off).
    /// Besides printing warnings, it enables TLC's all-assigned fast path in
    /// successor generation (`Tool.getNextStatesAllAssigned`), which changes
    /// how many successors are generated, so it is part of the semantics.
    /// `true` in both profiles, as in TLC.
    pub warnings: bool,
    /// Action composition (`\cdot`, D-022).
    pub action_composition: ActionComposition,
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
            warnings: true,
            action_composition: ActionComposition::Off,
        }
    }

    /// dive's default profile.
    ///
    /// - Big integers and canonical `CHOOSE` order (owner-approved, ADR 0010).
    /// - Exploration in the canonical order (owner decision, D-010).
    /// - TLA+ comparison answers (owner decision, D-009).
    /// - Action composition with its TLA+ meaning (D-022).
    #[must_use]
    pub const fn dive() -> Self {
        Self {
            ctx: Ctx::dive(),
            exploration: ExplorationOrder::Value,
            comparisons: Comparisons::Tla,
            max_depth: DEFAULT_MAX_DEPTH,
            warnings: true,
            action_composition: ActionComposition::Tla,
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
            && self.action_composition != ActionComposition::Tla
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
    fn owner_decisions_set_the_default_profile() {
        // Open questions 9 and 12, answered 2026-10-08: canonical
        // exploration (D-010) and TLA+ comparisons (D-009) by default;
        // `--compat=tlc` keeps TLC's behaviour.
        assert_eq!(Options::dive().exploration, ExplorationOrder::Value);
        assert_eq!(Options::dive().comparisons, Comparisons::Tla);
        assert_eq!(Options::tlc().exploration, ExplorationOrder::Tlc);
        assert_eq!(Options::tlc().comparisons, Comparisons::Tlc);
    }
}
