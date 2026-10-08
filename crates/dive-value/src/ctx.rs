//! Per-run value semantics: integer mode and value order.

/// How integers behave.
///
/// ADR 0010: arbitrary precision is the owner-approved dive default, and TLC's 32-bit
/// integers with its overflow error are always built and selected by `--int=tlc`
/// (part of `--compat=tlc`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum IntMode {
    /// TLC's semantics: values are Java `int`s, arithmetic that leaves
    /// `[-2^31, 2^31 - 1]` is `EC.TLC_MODULE_OVERFLOW`, and literals above `2^31 - 1`
    /// are `EC.TLC_INTEGER_TOO_BIG`.
    Tlc,
    /// Mathematical integers: an inline 64-bit fast path that spills to a big integer.
    #[default]
    Big,
}

/// How names (strings, model values and record field names) are ordered. The order of
/// every other value is built from it, so this decides set order, printing order and
/// which element `CHOOSE` picks.
///
/// TLC orders names by their position in its global intern table
/// (`util.UniqueString.compareTo` compares intern tokens), not by their characters.
/// That order depends on the order in which SANY and TLC first met each name: adding
/// an unused definition `U == "alpha"` can change `CHOOSE e \in {"zeta", "alpha"} : TRUE`.
/// The owner decided (open question 8, ADR 0011) that dive's default is a canonical,
/// text-independent order, and TLC's order is `--choose-order=tlc`, included in
/// `--compat=tlc`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum ValueOrder {
    /// dive's canonical order (the default): names by the lexicographic order of their
    /// UTF-8 bytes, which is the order of their Unicode scalar values. Total, stable,
    /// and independent of module text and evaluation order.
    #[default]
    Canonical,
    /// TLC's order (`--choose-order=tlc`): names by intern token. Exact when the
    /// [`Interner`](crate::Interner) is fed names in SANY/TLC's encounter order.
    Tlc,
}

/// The value semantics of one run. Cheap to copy; pass it to every operation that may
/// compare, normalize, enumerate or print values.
///
/// All values compared within a run must have been built under the same `Ctx` and
/// the same [`Interner`](crate::Interner): sets are kept sorted in the run's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Ctx {
    /// The integer semantics.
    pub int_mode: IntMode,
    /// The order of names, and so of all values.
    pub order: ValueOrder,
}

impl Ctx {
    /// The `--compat=tlc` semantics: 32-bit integers and TLC's intern order.
    #[must_use]
    pub const fn tlc() -> Self {
        Self {
            int_mode: IntMode::Tlc,
            order: ValueOrder::Tlc,
        }
    }

    /// dive's default semantics: big integers and the canonical order (both
    /// owner-approved defaults; ADR 0010, ADR 0011).
    #[must_use]
    pub const fn dive() -> Self {
        Self {
            int_mode: IntMode::Big,
            order: ValueOrder::Canonical,
        }
    }
}
