//! SANY's built-in operators (`tla2sany/semantic/BuiltInOperators.java`).
//!
//! Each entry gives the name, the arity (-1 for variadic), the operator's
//! own level, and per-argument maximum levels and weights. A `None` level
//! table means SANY leaves the level arrays unset (only `.` and `\neg`,
//! which are never applied directly: `.` is parsed as `$RcdSelect` and
//! `\neg` resolves to its synonym `\lnot`).

/// One built-in operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltIn {
    /// SANY's name for it.
    pub name: &'static str,
    /// Arity; -1 for variadic.
    pub arity: i32,
    /// The level of the operator itself.
    pub level: u8,
    /// Maximum argument levels and weights, or `None`.
    pub levels: Option<(&'static [u8], &'static [u8])>,
}

const fn b(
    name: &'static str,
    arity: i32,
    level: u8,
    max: &'static [u8],
    weights: &'static [u8],
) -> BuiltIn {
    BuiltIn {
        name,
        arity,
        level,
        levels: Some((max, weights)),
    }
}

const fn bare(name: &'static str, arity: i32) -> BuiltIn {
    BuiltIn {
        name,
        arity,
        level: 0,
        levels: None,
    }
}

/// The table, in SANY's order.
pub const BUILT_INS: &[BuiltIn] = &[
    b("STRING", 0, 0, &[], &[]),
    b("FALSE", 0, 0, &[], &[]),
    b("TRUE", 0, 0, &[], &[]),
    b("BOOLEAN", 0, 0, &[], &[]),
    b("=", 2, 0, &[2, 2], &[1, 1]),
    b("/=", 2, 0, &[2, 2], &[1, 1]),
    bare(".", 2),
    b("'", 1, 2, &[1], &[0]),
    b("\\lnot", 1, 0, &[3], &[1]),
    bare("\\neg", 1),
    b("\\land", 2, 0, &[3, 3], &[1, 1]),
    b("\\lor", 2, 0, &[3, 3], &[1, 1]),
    b("\\equiv", 2, 0, &[3, 3], &[1, 1]),
    b("=>", 2, 0, &[3, 3], &[1, 1]),
    b("SUBSET", 1, 0, &[2], &[1]),
    b("UNION", 1, 0, &[2], &[1]),
    b("DOMAIN", 1, 0, &[2], &[1]),
    b("\\subseteq", 2, 0, &[2, 2], &[1, 1]),
    b("\\in", 2, 0, &[2, 2], &[1, 1]),
    b("\\notin", 2, 0, &[2, 2], &[1, 1]),
    b("\\", 2, 0, &[2, 2], &[1, 1]),
    b("\\intersect", 2, 0, &[2, 2], &[1, 1]),
    b("\\union", 2, 0, &[2, 2], &[1, 1]),
    b("\\times", 2, 0, &[2, 2], &[1, 1]),
    b("~>", 2, 3, &[3, 3], &[0, 0]),
    b("[]", 1, 3, &[3], &[0]),
    b("<>", 1, 3, &[3], &[0]),
    b("ENABLED", 1, 1, &[2], &[0]),
    b("UNCHANGED", 1, 2, &[1], &[0]),
    b("\\cdot", 2, 2, &[2, 2], &[0, 0]),
    b("-+->", 2, 3, &[3, 3], &[0, 0]),
    b("$AngleAct", 2, 2, &[2, 1], &[0, 0]),
    b("$BoundedChoose", -1, 0, &[2], &[1]),
    b("$BoundedExists", -1, 0, &[3], &[1]),
    b("$BoundedForall", -1, 0, &[3], &[1]),
    b("$CartesianProd", -1, 0, &[2], &[1]),
    b("$Case", -1, 0, &[3], &[1]),
    b("$ConjList", -1, 0, &[3], &[1]),
    b("$DisjList", -1, 0, &[3], &[1]),
    b("$Except", -1, 0, &[2], &[1]),
    b("$FcnApply", 2, 0, &[2, 2], &[1, 1]),
    b("$FcnConstructor", -1, 0, &[3], &[1]),
    b("$IfThenElse", 3, 0, &[3, 3, 3], &[1, 1, 1]),
    b("$NonRecursiveFcnSpec", 1, 0, &[3], &[1]),
    b("$Pair", 2, 0, &[3, 3], &[1, 1]),
    b("$RcdConstructor", -1, 0, &[3], &[1]),
    b("$RcdSelect", 2, 0, &[2, 0], &[1, 1]),
    b("$RecursiveFcnSpec", 1, 0, &[3], &[1]),
    b("$Seq", -1, 0, &[2], &[1]),
    b("$SetEnumerate", -1, 0, &[2], &[1]),
    b("$SetOfAll", -1, 0, &[2], &[1]),
    b("$SetOfFcns", -1, 0, &[2], &[1]),
    b("$SetOfRcds", -1, 0, &[2], &[1]),
    b("$SF", 2, 3, &[1, 2], &[0, 0]),
    b("$SquareAct", 2, 2, &[2, 1], &[0, 0]),
    b("$SubsetOf", 1, 0, &[2], &[1]),
    b("$TemporalExists", 1, 3, &[3], &[0]),
    b("$TemporalForall", 1, 3, &[3], &[0]),
    b("$TemporalWhile", 2, 3, &[3, 3], &[0, 0]),
    b("$Tuple", -1, 0, &[2], &[1]),
    b("$UnboundedChoose", 1, 0, &[2], &[1]),
    b("$UnboundedExists", 1, 0, &[3], &[1]),
    b("$UnboundedForall", 1, 0, &[3], &[1]),
    b("$WF", 2, 3, &[1, 2], &[0, 0]),
    b("$Nop", 1, 0, &[3], &[1]),
    b("$Qed", 0, 0, &[], &[]),
    b("$Pfcase", 1, 0, &[3], &[1]),
    b("$Have", 1, 0, &[3], &[1]),
    b("$Take", 1, 0, &[3], &[1]),
    b("$Pick", 1, 0, &[3], &[1]),
    b("$Witness", -1, 0, &[2], &[1]),
    b("$Suffices", 1, 0, &[3], &[1]),
];

/// SANY's `Operators.resolveSynonym`: the canonical name of an operator
/// symbol (`\cup` → `\union`, `~` → `\lnot`, `∈` → `\in`), or the text itself.
#[must_use]
pub fn resolve_synonym(text: &str) -> &str {
    dive_syntax::ops::canonical_spelling(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for op in BUILT_INS {
            assert!(seen.insert(op.name), "{}", op.name);
        }
        assert_eq!(BUILT_INS.len(), 72);
    }

    #[test]
    fn synonyms() {
        assert_eq!(resolve_synonym("\\cup"), "\\union");
        assert_eq!(resolve_synonym("~"), "\\lnot");
        assert_eq!(resolve_synonym("\\neg"), "\\lnot");
        assert_eq!(resolve_synonym("(+)"), "\\oplus");
        assert_eq!(resolve_synonym("Foo"), "Foo");
    }
}
