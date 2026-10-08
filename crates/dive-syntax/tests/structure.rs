//! Tree shapes: precedence, associativity, junction lists, labels, proofs.
//!
//! Each test parses a definition and compares a compact S-expression of the
//! CST: names and literals print as their text, other nodes as
//! `(KIND children...)`, tokens as their text; trivia is omitted.

use dive_syntax::ast::{AstNode as _, Expr, Proof, Unit};
use dive_syntax::{SyntaxKind, SyntaxNode, parse_module};

fn sexp(n: &SyntaxNode) -> String {
    use SyntaxKind::*;
    if matches!(n.kind(), GENERAL_ID | NUMBER_LIT | STRING_LIT | DECIMAL_LIT) {
        return n
            .children_with_tokens()
            .filter(|e| !e.kind().is_trivia())
            .map(|e| e.to_string())
            .collect::<String>();
    }
    let mut s = format!("({}", n.kind().name());
    for e in n.children_with_tokens() {
        match e {
            rowan::NodeOrToken::Node(c) => {
                s.push(' ');
                s.push_str(&sexp(&c));
            }
            rowan::NodeOrToken::Token(t) if !t.kind().is_trivia() => {
                s.push(' ');
                s.push_str(t.text());
            }
            rowan::NodeOrToken::Token(_) => {}
        }
    }
    s.push(')');
    s
}

/// The S-expression of the body of the single definition `x == ...`.
fn body(def: &str) -> String {
    let src = format!("---- MODULE M ----\n{def}\n====\n");
    let p = parse_module(&src);
    assert!(p.is_ok(), "{def:?}: {:?}", p.diagnostics());
    let Some(Unit::OperatorDef(d)) = p.module().and_then(|m| m.units().last()) else {
        unreachable!("no definition in {def:?}")
    };
    let Some(e) = d.body() else {
        unreachable!("no body in {def:?}")
    };
    sexp(Expr::syntax(&e))
}

fn rejected(def: &str) -> String {
    let src = format!("---- MODULE M ----\n{def}\n====\n");
    let p = parse_module(&src);
    assert!(!p.is_ok(), "{def:?} should be rejected");
    p.diagnostics()[0].code.as_str().to_owned()
}

#[test]
fn precedence() {
    assert_eq!(
        body("x == 1 + 2 * 3"),
        "(INFIX_EXPR 1 + (INFIX_EXPR 2 * 3))"
    );
    assert_eq!(
        body("x == 1 * 2 + 3"),
        "(INFIX_EXPR (INFIX_EXPR 1 * 2) + 3)"
    );
    assert_eq!(
        body("x == a = b /\\ c"),
        "(INFIX_EXPR (INFIX_EXPR a = b) /\\ c)"
    );
    assert_eq!(
        body("x == a /\\ b => c"),
        "(INFIX_EXPR (INFIX_EXPR a /\\ b) => c)"
    );
    assert_eq!(
        body("x == a \\in S \\cup T"),
        "(INFIX_EXPR a \\in (INFIX_EXPR S \\cup T))"
    );
    assert_eq!(
        body("x == 1 .. 2 + 3"),
        "(INFIX_EXPR 1 .. (INFIX_EXPR 2 + 3))"
    );
}

#[test]
fn prefix_and_postfix() {
    // Prefix minus binds looser than ^ but tighter than *.
    assert_eq!(body("x == -a ^ 2"), "(PREFIX_EXPR - (INFIX_EXPR a ^ 2))");
    assert_eq!(body("x == -a + b"), "(INFIX_EXPR (PREFIX_EXPR - a) + b)");
    assert_eq!(body("x == ~a = b"), "(PREFIX_EXPR ~ (INFIX_EXPR a = b))");
    assert_eq!(
        body("x == ~a /\\ b"),
        "(INFIX_EXPR (PREFIX_EXPR ~ a) /\\ b)"
    );
    assert_eq!(
        body("x == []P => Q"),
        "(INFIX_EXPR (PREFIX_EXPR [] P) => Q)"
    );
    assert_eq!(body("x == a - -b"), "(INFIX_EXPR a - (PREFIX_EXPR - b))");
    assert_eq!(body("x == ~ ~ a"), "(PREFIX_EXPR ~ (PREFIX_EXPR ~ a))");
    assert_eq!(body("x == a' + b"), "(INFIX_EXPR (POSTFIX_EXPR a ') + b)");
    assert_eq!(body("x == f[1]'"), "(POSTFIX_EXPR (FCN_APPL f [ 1 ]) ')");
    assert_eq!(body("x == a'.b"), "(RECORD_FIELD (POSTFIX_EXPR a ') . b)");
    assert_eq!(
        body("x == SUBSET a \\cup b"),
        "(INFIX_EXPR (PREFIX_EXPR SUBSET a) \\cup b)"
    );
}

#[test]
fn associativity() {
    assert_eq!(
        body("x == a - b - c"),
        "(INFIX_EXPR (INFIX_EXPR a - b) - c)"
    );
    assert_eq!(
        body("x == a /\\ b /\\ c"),
        "(INFIX_EXPR (INFIX_EXPR a /\\ b) /\\ c)"
    );
    assert_eq!(
        body("x == a \\o b \\o c"),
        "(INFIX_EXPR (INFIX_EXPR a \\o b) \\o c)"
    );
    // \X is n-ary; synonyms join one product.
    assert_eq!(
        body("x == A \\X B \\times C"),
        "(CARTESIAN_PRODUCT A \\X B \\times C)"
    );
    assert_eq!(
        body("x == (A \\X B) \\X C"),
        "(CARTESIAN_PRODUCT (PAREN_EXPR ( (CARTESIAN_PRODUCT A \\X B) )) \\X C)"
    );
    // Non-associative operators and conflicting ranges are rejected.
    for bad in [
        "x == a = b = c",
        "x == a => b => c",
        "x == a ^ b ^ c",
        "x == a \\cup b \\cap c",
        "x == a /\\ b \\/ c",
        "x == a * -b",
        "x == a \\cdot b = c",
        "x == SUBSET a \\X b",
    ] {
        assert_eq!(rejected(bad), "DIVE-S0012", "{bad}");
    }
}

#[test]
fn junction_lists() {
    assert_eq!(
        body("x == /\\ a\n     /\\ b"),
        "(CONJ_LIST (CONJ_ITEM /\\ a) (CONJ_ITEM /\\ b))"
    );
    // A bullet further right continues the item as an infix operator.
    assert_eq!(
        body("x == /\\ a\n      /\\ b"),
        "(CONJ_LIST (CONJ_ITEM /\\ (INFIX_EXPR a /\\ b)))"
    );
    // A bullet further left ends the list; the operator applies to it.
    assert_eq!(
        body("x == /\\ a\n    /\\ b"),
        "(INFIX_EXPR (CONJ_LIST (CONJ_ITEM /\\ a)) /\\ b)"
    );
    // A different bullet at the same column ends the list too.
    assert_eq!(
        body("x == /\\ a\n     \\/ b"),
        "(INFIX_EXPR (CONJ_LIST (CONJ_ITEM /\\ a)) \\/ b)"
    );
    assert_eq!(
        body("x == \\/ /\\ a\n        /\\ b\n     \\/ c"),
        "(DISJ_LIST (DISJ_ITEM \\/ (CONJ_LIST (CONJ_ITEM /\\ a) (CONJ_ITEM /\\ b))) (DISJ_ITEM \\/ c))"
    );
    assert_eq!(
        rejected("x == /\\ IF a\n        THEN b\n    ELSE c"),
        "DIVE-S0015"
    );
    // A tab advances to the next multiple of 8 (column 9 here).
    assert_eq!(
        rejected("x ==\n        /\\ IF a\n\tTHEN b ELSE c"),
        "DIVE-S0015"
    );
    assert_ne!(body("x ==\n        /\\ IF a\n\t THEN b ELSE c"), "");
}

#[test]
fn brackets_and_binders() {
    assert_eq!(
        body("x == {a \\in S : a > 1}").split(' ').next(),
        Some("(SET_FILTER")
    );
    assert_eq!(
        body("x == {a + 1 : a \\in S}").split(' ').next(),
        Some("(SET_MAP")
    );
    assert_eq!(
        body("x == {a \\in S}"),
        "(SET_ENUM { (INFIX_EXPR a \\in S) })"
    );
    assert_eq!(
        body("x == [f EXCEPT ![1].a = @]"),
        "(EXCEPT [ f EXCEPT (EXCEPT_SPEC ! (EXCEPT_COMPONENT [ 1 ]) (EXCEPT_COMPONENT . a) = @) ])"
    );
    assert_eq!(body("x == [a |-> 1]"), "(RECORD [ (FIELD_VAL a |-> 1) ])");
    assert_eq!(body("x == [S -> T]"), "(SET_OF_FCNS [ S -> T ])");
    assert_eq!(body("x == [A]_v"), "(ACTION_EXPR [ A ]_ v)");
    assert_eq!(body("x == WF_v(A)"), "(FAIRNESS_EXPR WF_ v ( A ))");
    assert_eq!(
        body("x == \\A a, b \\in S : P"),
        "(BOUNDED_QUANT \\A (QUANT_BOUND a , b \\in S) : P)"
    );
    assert_eq!(body("x == \\E a, b : P"), "(UNBOUNDED_QUANT \\E a , b : P)");
    assert_eq!(body("x == CHOOSE a : P"), "(CHOOSE CHOOSE a : P)");
    assert_eq!(
        body("x == IF a THEN b ELSE c"),
        "(IF_EXPR IF a THEN b ELSE c)"
    );
    assert_eq!(
        body("x == CASE a -> b [] OTHER -> c"),
        "(CASE_EXPR CASE (CASE_ARM a -> b) [] (OTHER_ARM OTHER -> c))"
    );
    assert_eq!(
        body("x == LET y == 1 IN y"),
        "(LET_EXPR LET (LET_DEFS (OPERATOR_DEF (IDENT_LHS y) == 1)) IN y)"
    );
}

#[test]
fn names_labels_and_subexpressions() {
    assert_eq!(
        body("x == M!Op(1)"),
        "(OP_APPLICATION M!Op (OP_ARGS ( 1 )))"
    );
    assert_eq!(body("x == lab :: a"), "(LABEL lab :: a)");
    assert_eq!(body("x == Op!1!<<"), "Op!1!<<");
    assert_eq!(
        body("x == -(1, 2)"),
        "(OP_APPLICATION - (OP_ARGS ( 1 , 2 )))"
    );
    assert_eq!(rejected("x == 2 * lab :: 1 + 2"), "DIVE-S0031");
}

#[test]
fn proofs() {
    let src = "---- MODULE M ----\nTHEOREM T == TRUE\n<1>1. TRUE\n  <2>1. TRUE OBVIOUS\n  <2>2. QED BY <2>1\n<1>2. QED\n  OBVIOUS\n====\n";
    let p = parse_module(src);
    assert!(p.is_ok(), "{:?}", p.diagnostics());
    let Some(Unit::Theorem(t)) = p.module().and_then(|m| m.units().next()) else {
        unreachable!("no theorem")
    };
    assert_eq!(t.name().as_deref(), Some("T"));
    let Some(proof) = t.proof().and_then(Proof::cast) else {
        unreachable!("no proof")
    };
    let steps: Vec<String> = proof
        .steps()
        .filter_map(|s| s.step_token().map(|t| t.text().to_owned()))
        .collect();
    assert_eq!(steps, ["<1>1.", "<1>2."]);
    // Indentation does not matter to proofs: these are three level-1 steps.
    let ok = "---- MODULE M ----\nTHEOREM TRUE\n<1>1. TRUE\n<1>2. TRUE\n  <1>3. QED\n====\n";
    assert!(parse_module(ok).is_ok());
    // A deeper step starts a sub-proof, which must end in its own QED
    // (tests/sany-cases/ProofBadLevel.tla).
    let bad = "---- MODULE M ----\nTHEOREM TRUE\n<1>1. TRUE\n<2>2. QED\n====\n";
    assert!(!parse_module(bad).is_ok());
    // Only assertion, CASE, PICK and QED steps take proofs.
    let bad = "---- MODULE M ----\nTHEOREM TRUE\n<1>1. HAVE TRUE\n  OBVIOUS\n<1>2. QED\n====\n";
    assert_eq!(
        parse_module(bad).diagnostics()[0].code.as_str(),
        "DIVE-S0022"
    );
}

#[test]
fn recovery_keeps_later_units() {
    let src = "---- MODULE M ----\nA == 1 +\nB == 2\nVARIABLE v\nC == (3\nD == 4\n====\n";
    let p = parse_module(src);
    assert_eq!(p.syntax().to_string(), src);
    assert!(!p.is_ok());
    let names: Vec<String> = p
        .module()
        .iter()
        .flat_map(dive_syntax::ast::Module::operator_definitions)
        .filter_map(|d| d.name())
        .collect();
    assert_eq!(names, ["B", "D"]);
    assert_eq!(p.diagnostics().len(), 2);
}
