//! The typed AST views.

use dive_syntax::ast::{AstNode as _, Expr, Unit};
use dive_syntax::parse_module;

fn units(src: &str) -> Vec<Unit> {
    let p = parse_module(src);
    assert!(p.is_ok(), "{:?}", p.diagnostics());
    p.module().map(|m| m.units().collect()).unwrap_or_default()
}

fn def_body(def: &str) -> Expr {
    let src = format!("---- MODULE M ----\n{def}\n====\n");
    let Some(Unit::OperatorDef(d)) = units(&src).into_iter().last() else {
        unreachable!("no definition")
    };
    let Some(e) = d.body() else {
        unreachable!("no body")
    };
    e
}

fn text(e: Option<Expr>) -> String {
    e.map(|e| e.syntax().text().to_string()).unwrap_or_default()
}

#[test]
fn module_units() {
    let src = "---- MODULE M ----\nEXTENDS Naturals, Sequences\nCONSTANT N\nVARIABLES x, y\n\
               LOCAL F(a, _ + _) == a\nf[i \\in 1..N] == i\nI == INSTANCE Naturals\nINSTANCE TLC\n\
               ASSUME A == N > 0\nTHEOREM T == TRUE OBVIOUS\n---- MODULE Inner ----\n====\n====\n";
    let p = parse_module(src);
    assert!(p.is_ok(), "{:?}", p.diagnostics());
    let Some(m) = p.module() else {
        unreachable!("no module")
    };
    assert_eq!(m.name().as_deref(), Some("M"));
    assert_eq!(m.extends(), ["Naturals", "Sequences"]);
    assert!(m.is_closed());
    let mut kinds = Vec::new();
    for u in m.units() {
        kinds.push(match u {
            Unit::ConstantDecl(_) => "constant",
            Unit::VariableDecl(v) => {
                assert_eq!(v.names(), ["x", "y"]);
                "variable"
            }
            Unit::OperatorDef(d) => {
                assert!(d.is_local());
                assert_eq!(d.name().as_deref(), Some("F"));
                assert_eq!(d.parameters(), ["a", "+"]);
                "def"
            }
            Unit::FunctionDef(f) => {
                assert_eq!(f.name().as_deref(), Some("f"));
                assert_eq!(
                    f.bounds().flat_map(|b| b.names()).collect::<Vec<_>>(),
                    ["i"]
                );
                "fdef"
            }
            Unit::ModuleDef(d) => {
                assert_eq!(
                    d.instantiation().and_then(|i| i.module_name()).as_deref(),
                    Some("Naturals")
                );
                "moddef"
            }
            Unit::Instance(i) => {
                assert!(!i.is_local());
                "instance"
            }
            Unit::Assumption(a) => {
                assert_eq!(a.name().as_deref(), Some("A"));
                "assume"
            }
            Unit::Theorem(t) => {
                assert_eq!(t.name().as_deref(), Some("T"));
                assert!(t.proof().is_some());
                "theorem"
            }
            Unit::Module(n) => {
                assert_eq!(n.name().as_deref(), Some("Inner"));
                "module"
            }
            Unit::RecursiveDecl(_) | Unit::UseOrHide(_) | Unit::Error(_) => "other",
        });
    }
    assert_eq!(
        kinds,
        [
            "constant", "variable", "def", "fdef", "moddef", "instance", "assume", "theorem",
            "module"
        ]
    );
}

#[test]
fn let_if_case() {
    let Expr::Let(let_expr) = def_body("x == LET a == 1\n         RECURSIVE g(_)\n     IN a")
    else {
        unreachable!()
    };
    assert_eq!(let_expr.definitions().len(), 2);
    assert_eq!(text(let_expr.body()), "a");

    let Expr::If(if_expr) = def_body("x == IF a THEN b ELSE c") else {
        unreachable!()
    };
    assert_eq!(
        [
            text(if_expr.condition()),
            text(if_expr.then_branch()),
            text(if_expr.else_branch())
        ],
        ["a", "b", "c"]
    );

    let Expr::Case(case) = def_body("x == CASE a -> 1 [] b -> 2 [] OTHER -> 3") else {
        unreachable!()
    };
    assert_eq!(case.arms().len(), 2);
    assert_eq!(text(case.other()), "3");
}

#[test]
fn binders() {
    let Expr::Quant(quant) = def_body("x == \\A a, b \\in S, <<c, d>> \\in T : P") else {
        unreachable!()
    };
    assert!(quant.is_forall() && !quant.is_temporal());
    let names: Vec<String> = quant.bounds().flat_map(|bound| bound.names()).collect();
    assert_eq!(names, ["a", "b", "c", "d"]);
    assert_eq!(text(quant.body()), "P");

    let Expr::Quant(temporal) = def_body("x == \\EE v : P") else {
        unreachable!()
    };
    assert!(temporal.is_temporal() && !temporal.is_forall());
    assert_eq!(temporal.unbounded_names(), ["v"]);

    let Expr::Choose(choose) = def_body("x == CHOOSE v \\in S : v > 1") else {
        unreachable!()
    };
    assert_eq!(choose.names(), ["v"]);
    assert_eq!(text(choose.set()), "S");
}

#[test]
fn lists_applications_and_labels() {
    let Expr::Junction(list) = def_body("x == \\/ a\n     \\/ b") else {
        unreachable!()
    };
    assert!(!list.is_conjunction());
    assert_eq!(list.items().len(), 2);

    let Expr::FcnAppl(fcn) = def_body("x == f[1, 2]") else {
        unreachable!()
    };
    assert_eq!(text(fcn.function()), "f");
    assert_eq!(fcn.arguments().len(), 2);

    let Expr::Field(field) = def_body("x == r.ELSE") else {
        unreachable!()
    };
    assert_eq!(field.field().as_deref(), Some("ELSE"));

    let Expr::Label(label) = def_body("x == lab(a) :: a") else {
        unreachable!()
    };
    assert_eq!(label.name().as_deref(), Some("lab"));

    let Expr::Apply(app) = def_body("x == F(LAMBDA p : p, +)") else {
        unreachable!()
    };
    assert_eq!(app.arguments().len(), 2);
    let lambda = app
        .arguments()
        .into_iter()
        .find_map(dive_syntax::ast::Lambda::cast);
    assert_eq!(lambda.map(|l| l.parameters()), Some(vec!["p".to_owned()]));
}
