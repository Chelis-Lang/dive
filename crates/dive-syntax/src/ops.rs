//! Operator precedence and associativity.
//!
//! This is SANY's table (`tla2sany/parser/Operators.java`). Every operator
//! has a precedence *range* `low..=high`. Two operators conflict unless one
//! range lies wholly above the other. Synonyms such as `/\`, `\land` and `∧`
//! share one entry.

/// Associativity of an operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assoc {
    /// Not associative: `a = b = c` is an error.
    None,
    /// Left-associative: `a + b + c` is `(a + b) + c`.
    Left,
}

/// How an operator is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fixity {
    /// `op e`.
    Prefix,
    /// `e op`.
    Postfix,
    /// `a op b`.
    Infix,
    /// `a op b op c` collected into one n-ary node (only `\X`).
    Nfix,
}

/// Precedence information for one operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpInfo {
    /// The canonical spelling; synonyms share it.
    pub canonical: &'static str,
    /// Lowest precedence in the operator's range.
    pub low: u8,
    /// Highest precedence in the operator's range.
    pub high: u8,
    /// Associativity.
    pub assoc: Assoc,
    /// Fixity.
    pub fixity: Fixity,
}

impl OpInfo {
    /// Whether the operator is prefix.
    #[must_use]
    pub fn is_prefix(&self) -> bool {
        self.fixity == Fixity::Prefix
    }

    /// Whether the operator is infix (including n-ary `\X`).
    #[must_use]
    pub fn is_infix(&self) -> bool {
        matches!(self.fixity, Fixity::Infix | Fixity::Nfix)
    }

    /// Whether the operator is postfix.
    #[must_use]
    pub fn is_postfix(&self) -> bool {
        self.fixity == Fixity::Postfix
    }

    /// SANY's `Operator.succ(left, right)`: `left` binds tighter.
    #[must_use]
    pub fn succ(&self, right: &OpInfo) -> bool {
        self.low > right.high
    }

    /// SANY's `Operator.prec(left, right)`: `right` binds tighter.
    #[must_use]
    pub fn prec(&self, right: &OpInfo) -> bool {
        self.high < right.low
    }

    /// Identity in SANY's sense: synonyms are the same operator.
    #[must_use]
    pub fn same(&self, other: &OpInfo) -> bool {
        self.canonical == other.canonical
    }
}

const fn op(canonical: &'static str, low: u8, high: u8, assoc: Assoc, fixity: Fixity) -> OpInfo {
    OpInfo {
        canonical,
        low,
        high,
        assoc,
        fixity,
    }
}

use Assoc::{Left as L, None as N};
use Fixity::{Infix as I, Nfix as X, Postfix as Po, Prefix as Pr};

/// The canonical operators (`Operators.CanonicalOperators`).
const CANONICAL: &[OpInfo] = &[
    op("[", 160, 160, L, Po),
    op(".", 170, 170, L, I),
    op("'", 150, 150, N, Po),
    op("^", 140, 140, N, I),
    op("/", 130, 130, N, I),
    op("*", 130, 130, L, I),
    op("-", 110, 110, L, I),
    op("-.", 120, 120, N, Pr),
    op("+", 100, 100, L, I),
    op("=", 50, 50, N, I),
    op("\\lnot", 40, 40, L, Pr),
    op("\\land", 30, 30, L, I),
    op("\\lor", 30, 30, L, I),
    op("~>", 20, 20, N, I),
    op("=>", 10, 10, N, I),
    op("[]", 40, 150, N, Pr),
    op("<>", 40, 150, N, Pr),
    op("ENABLED", 40, 150, N, Pr),
    op("UNCHANGED", 40, 150, N, Pr),
    op("SUBSET", 100, 130, N, Pr),
    op("UNION", 100, 130, N, Pr),
    op("DOMAIN", 100, 130, N, Pr),
    op("^+", 150, 150, N, Po),
    op("^*", 150, 150, N, Po),
    op("^#", 150, 150, N, Po),
    op("\\cdot", 50, 140, L, I),
    op("\\equiv", 20, 20, N, I),
    op("-+->", 20, 20, N, I),
    op("/=", 50, 50, N, I),
    op("\\subseteq", 50, 50, N, I),
    op("\\in", 50, 50, N, I),
    op("\\notin", 50, 50, N, I),
    op("<", 50, 50, N, I),
    op("\\leq", 50, 50, N, I),
    op(">", 50, 50, N, I),
    op("\\geq", 50, 50, N, I),
    op("\\times", 100, 130, L, X),
    op("\\", 80, 80, N, I),
    op("\\intersect", 80, 80, L, I),
    op("\\union", 80, 80, L, I),
    op("...", 90, 90, N, I),
    op("..", 90, 90, N, I),
    op("|", 100, 110, L, I),
    op("||", 100, 110, L, I),
    op("&&", 130, 130, L, I),
    op("&", 130, 130, L, I),
    op("$$", 90, 130, L, I),
    op("$", 90, 130, L, I),
    op("??", 90, 130, L, I),
    op("%%", 100, 110, L, I),
    op("%", 100, 110, N, I),
    op("##", 90, 130, L, I),
    op("++", 100, 100, L, I),
    op("--", 110, 110, L, I),
    op("**", 130, 130, L, I),
    op("//", 130, 130, N, I),
    op("^^", 140, 140, N, I),
    op("@@", 60, 60, L, I),
    op("!!", 90, 130, N, I),
    op("|-", 50, 50, N, I),
    op("|=", 50, 50, N, I),
    op("-|", 50, 50, N, I),
    op("=|", 50, 50, N, I),
    op("<:", 70, 70, N, I),
    op(":>", 70, 70, N, I),
    op(":=", 50, 50, N, I),
    op("::=", 50, 50, N, I),
    op("\\oplus", 100, 100, L, I),
    op("\\ominus", 110, 110, L, I),
    op("\\odot", 130, 130, L, I),
    op("\\oslash", 130, 130, N, I),
    op("\\otimes", 130, 130, L, I),
    op("\\uplus", 90, 130, L, I),
    op("\\sqcap", 90, 130, L, I),
    op("\\sqcup", 90, 130, L, I),
    op("\\div", 130, 130, N, I),
    op("\\wr", 90, 140, N, I),
    op("\\star", 130, 130, L, I),
    op("\\o", 130, 130, L, I),
    op("\\bigcirc", 130, 130, L, I),
    op("\\bullet", 130, 130, L, I),
    op("\\prec", 50, 50, N, I),
    op("\\succ", 50, 50, N, I),
    op("\\preceq", 50, 50, N, I),
    op("\\succeq", 50, 50, N, I),
    op("\\sim", 50, 50, N, I),
    op("\\simeq", 50, 50, N, I),
    op("\\ll", 50, 50, N, I),
    op("\\gg", 50, 50, N, I),
    op("\\asymp", 50, 50, N, I),
    op("\\subset", 50, 50, N, I),
    op("\\supset", 50, 50, N, I),
    op("\\supseteq", 50, 50, N, I),
    op("\\approx", 50, 50, N, I),
    op("\\cong", 50, 50, N, I),
    op("\\sqsubset", 50, 50, N, I),
    op("\\sqsubseteq", 50, 50, N, I),
    op("\\sqsupset", 50, 50, N, I),
    op("\\sqsupseteq", 50, 50, N, I),
    op("\\doteq", 50, 50, N, I),
    op("\\propto", 50, 50, N, I),
];

/// Synonyms (`Operators.OperatorSynonyms`): canonical spelling first.
const SYNONYMS: &[&[&str]] = &[
    &["\\lnot", "~", "\\neg", "¬"],
    &["[]", "□"],
    &["<>", "◇"],
    &["\\land", "/\\", "∧"],
    &["\\lor", "\\/", "∨"],
    &["\\equiv", "<=>", "≡", "⇔"],
    &["/=", "#", "≠"],
    &["\\leq", "<=", "=<", "≤"],
    &["\\geq", ">=", "≥"],
    &["\\times", "\\X", "×"],
    &["\\intersect", "\\cap", "∩"],
    &["\\union", "\\cup", "∪"],
    &["\\o", "\\circ", "∘"],
    &["\\oplus", "(+)", "⊕"],
    &["\\ominus", "(-)", "⊖"],
    &["\\odot", "(.)", "⊙"],
    &["\\oslash", "(/)", "⊘"],
    &["\\otimes", "(\\X)", "⊗"],
    &["\\approx", "≈"],
    &[":=", "≔"],
    &["\\asymp", "≍"],
    &["\\bigcirc", "◯"],
    &["::=", "⩴"],
    &["\\bullet", "●"],
    &["\\cdot", "⋅"],
    &["\\cong", "≅"],
    &["\\div", "÷"],
    &["\\doteq", "≐"],
    &["..", "‥"],
    &["...", "…"],
    &["!!", "‼"],
    &["\\gg", "≫"],
    &["=>", "⇒"],
    &["\\in", "∈"],
    &["=|", "⫤"],
    &["~>", "↝"],
    &["\\ll", "≪"],
    &["-|", "⊣"],
    &["\\notin", "∉"],
    &["-+->", "⇸"],
    &["\\prec", "≺"],
    &["\\preceq", "⪯"],
    &["\\propto", "∝"],
    &["??", "⁇"],
    &["|=", "⊨"],
    &["|-", "⊢"],
    &["\\sim", "∼"],
    &["\\simeq", "≃"],
    &["\\sqcap", "⊓"],
    &["\\sqcup", "⊔"],
    &["\\sqsubset", "⊏"],
    &["\\sqsubseteq", "⊑"],
    &["\\sqsupset", "⊐"],
    &["\\sqsupseteq", "⊒"],
    &["\\star", "⋆"],
    &["\\subset", "⊂"],
    &["\\subseteq", "⊆"],
    &["\\succ", "≻"],
    &["\\succeq", "⪰"],
    &["\\supset", "⊃"],
    &["\\supseteq", "⊇"],
    &["\\uplus", "⊎"],
    &["||", "‖"],
    &["\\wr", "≀"],
    &["^+", "⁺"],
];

/// Precomputed tables: spelling (canonical or synonym) to index in
/// [`CANONICAL`], and each operator's mixfix form.
struct Tables {
    by_spelling: std::collections::HashMap<&'static str, usize>,
    mixfix: Vec<Option<usize>>,
}

fn tables() -> &'static Tables {
    static TABLES: std::sync::OnceLock<Tables> = std::sync::OnceLock::new();
    TABLES.get_or_init(|| {
        let mut by_spelling = std::collections::HashMap::new();
        for (i, o) in CANONICAL.iter().enumerate() {
            by_spelling.insert(o.canonical, i);
        }
        for group in SYNONYMS {
            if let Some(&i) = group.first().and_then(|c| by_spelling.get(c)) {
                for s in group.iter().skip(1) {
                    by_spelling.insert(*s, i);
                }
            }
        }
        let mixfix = CANONICAL
            .iter()
            .enumerate()
            .map(|(i, o)| {
                if o.is_prefix() {
                    Some(i)
                } else {
                    let name = format!("{}.", o.canonical);
                    by_spelling.get(name.as_str()).copied()
                }
            })
            .collect();
        Tables {
            by_spelling,
            mixfix,
        }
    })
}

/// The canonical spelling of an operator symbol, resolving synonyms.
#[must_use]
pub fn canonical_spelling(symbol: &str) -> &str {
    lookup(symbol).map_or(symbol, |o| o.canonical)
}

/// Looks up the precedence of an operator by its spelling.
#[must_use]
pub fn lookup(symbol: &str) -> Option<&'static OpInfo> {
    tables()
        .by_spelling
        .get(symbol)
        .and_then(|&i| CANONICAL.get(i))
}

/// The function-application pseudo-operator `[`.
pub(crate) fn fcn_op() -> &'static OpInfo {
    &CANONICAL[0]
}

/// SANY's `VoidOperator`, pushed to force the final reduction.
pub(crate) const VOID: OpInfo = op("$$_void", 0, 0, N, I);

/// SANY's `Operators.getMixfix`: a prefix operator is its own mixfix form;
/// otherwise the operator named by appending `.` to the canonical spelling,
/// if any. That gives `-` → `-.`, and (a SANY quirk) `..` → `...`.
pub(crate) fn mixfix(o: &'static OpInfo) -> Option<&'static OpInfo> {
    if o.is_prefix() {
        return Some(o);
    }
    let t = tables();
    let i = *t.by_spelling.get(o.canonical)?;
    t.mixfix
        .get(i)
        .copied()
        .flatten()
        .and_then(|j| CANONICAL.get(j))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::OPERATORS;

    #[test]
    fn every_operator_token_has_precedence() {
        for (text, _) in OPERATORS {
            assert!(lookup(text).is_some(), "{text}");
        }
    }

    #[test]
    fn synonyms_share_an_entry() {
        let a = lookup("/\\");
        let b = lookup("∧");
        assert!(a.is_some_and(|a| b.is_some_and(|b| a.same(b))));
        assert_eq!(canonical_spelling("#"), "/=");
    }

    #[test]
    fn ranges() {
        let (Some(cdot), Some(eq)) = (lookup("\\cdot"), lookup("=")) else {
            unreachable!("table entries exist")
        };
        assert!(!cdot.succ(eq) && !cdot.prec(eq));
        assert!(mixfix(lookup("-").unwrap_or(&VOID)).is_some());
        assert!(mixfix(lookup("+").unwrap_or(&VOID)).is_none());
    }
}
