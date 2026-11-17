//! The effects analysis.
//!
//! Effects are computed bottom-up over expressions and to a fixed point over
//! definitions, during lowering. They are conservative: a flag that is clear
//! is a proof, a flag that is set is only a possibility.
//!
//! They decide:
//! - which laziness points may be [`crate::Pass::Strict`];
//! - whether the checker may run in parallel;
//! - what `dive-compile` may hoist or share.

use serde::{Deserialize, Serialize};

/// A set of effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Effects(u8);

impl Effects {
    /// No effects: pure, cannot fail, no `ENABLED`, not temporal.
    pub const NONE: Effects = Effects(0);
    /// Evaluation can do something besides compute a value: `Print`,
    /// `PrintT`, `TLCSet`, `RandomElement` and `Randomization`, `JavaTime`,
    /// `TLCGet` of a register or live counter, I/O, or reading checker state.
    pub const IMPURE: Effects = Effects(1);
    /// Reads a value the checker changes while it runs: `TLCGet` of
    /// `"generated"`, `"distinct"`, `"queue"`, `"diameter"`, `"duration"`,
    /// `"stats"`, `"all"`, `"all:named"`, an integer or `"s:"` register, or a
    /// key that is not a literal.
    pub const LIVE_TLCGET: Effects = Effects(2);
    /// Evaluation may raise an error. Conservative: everything but literals,
    /// bound variables, and sets, tuples and records of them.
    pub const MAY_FAIL: Effects = Effects(4);
    /// Contains `ENABLED`, directly or through a definition.
    pub const USES_ENABLED: Effects = Effects(8);
    /// Contains a temporal operator, directly or through a definition.
    pub const TEMPORAL: Effects = Effects(16);
    /// Reads or applies a constant, which the configuration may bind to a
    /// definition with `<-`. Its effects are only known once the model is
    /// bound; `dive-model` completes the analysis. (An operator parameter
    /// is analysed as every operator passed as an argument anywhere.)
    pub const OPEN: Effects = Effects(32);

    /// Whether every flag of `other` is set.
    #[must_use]
    pub fn contains(self, other: Effects) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether any flag of `other` is set.
    #[must_use]
    pub fn intersects(self, other: Effects) -> bool {
        self.0 & other.0 != 0
    }

    /// The union.
    #[must_use]
    pub fn union(self, other: Effects) -> Effects {
        Effects(self.0 | other.0)
    }

    /// Without the flags of `other`.
    #[must_use]
    pub fn without(self, other: Effects) -> Effects {
        Effects(self.0 & !other.0)
    }

    /// Whether the node is pure and cannot fail, so that evaluating it early
    /// or sharing it is indistinguishable from TLC.
    #[must_use]
    pub fn is_inert(self) -> bool {
        !self.intersects(
            Effects::IMPURE
                .union(Effects::LIVE_TLCGET)
                .union(Effects::MAY_FAIL)
                .union(Effects::OPEN),
        )
    }

    /// The flag names, for printing.
    #[must_use]
    pub fn names(self) -> Vec<&'static str> {
        let all = [
            (Effects::IMPURE, "impure"),
            (Effects::LIVE_TLCGET, "live_tlcget"),
            (Effects::MAY_FAIL, "may_fail"),
            (Effects::USES_ENABLED, "uses_enabled"),
            (Effects::TEMPORAL, "temporal"),
            (Effects::OPEN, "open"),
        ];
        all.iter()
            .filter(|(f, _)| self.contains(*f))
            .map(|(_, n)| *n)
            .collect()
    }
}

impl std::ops::BitOr for Effects {
    type Output = Effects;
    fn bitor(self, rhs: Effects) -> Effects {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for Effects {
    fn bitor_assign(&mut self, rhs: Effects) {
        *self = self.union(rhs);
    }
}

/// `TLCGet` keys whose values the checker changes while it runs
/// (`tlc2/module/TLCGetSet.java:189-488`).
pub const LIVE_TLCGET_KEYS: &[&str] = &[
    "generated",
    "distinct",
    "queue",
    "diameter",
    "duration",
    "stats",
    "all",
    "all:named",
];

/// Whether `TLCGet(key)` with a literal string key reads a live value or a
/// register. `"s:"` keys are registers.
#[must_use]
pub fn tlcget_string_is_live(key: &str) -> bool {
    LIVE_TLCGET_KEYS.contains(&key) || key.starts_with("s:")
}
