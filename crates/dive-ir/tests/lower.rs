//! Lowering tests on small specifications: each IR property ADR 0013 asks
//! for, and the invariants of `dive_ir::validate`.

use dive_ir::{
    Arg, DefId, DefKind, DefScope, Effects, Expr, Ir, Level, OpRef, Pass, StdOp, Symbol, lower,
};
use dive_sema::{NodeId, Options, analyze_source};

fn analysis(src: &str) -> dive_sema::Analysis {
    let a = analyze_source("M.tla", src, &Options::default());
    assert!(
        a.accepted(),
        "not accepted: {:?} {:?}",
        a.diagnostics(),
        a.syntax_diagnostics()
    );
    a
}

fn ir(src: &str) -> Ir {
    let a = analysis(src);
    let ir = match lower(&a) {
        Ok(ir) => ir,
        Err(e) => unreachable!("lowering failed: {e}"),
    };
    let violations = dive_ir::validate::check(&ir);
    assert!(violations.is_empty(), "{violations:#?}\n{}", ir.pretty());
    ir
}

fn def(ir: &Ir, name: &str) -> DefId {
    let d = ir.root_def(name);
    assert!(d.is_some(), "no definition {name}");
    d.unwrap_or(DefId(0))
}

fn body<'a>(ir: &'a Ir, name: &str) -> &'a Expr {
    ir.expr(ir.def(def(ir, name)).body)
}

#[test]
fn operator_arguments_are_laziness_points() {
    let ir = ir("---- MODULE M ----
EXTENDS Naturals
VARIABLE x
F(a, b) == a
G == F(1, x + 1)
H(p) == LET y == p + 1
            z == <<3, \"s\">>
        IN y
====
");
    let Expr::Apply { callee, args } = body(&ir, "G") else {
        unreachable!("G is not an application");
    };
    assert_eq!(*callee, OpRef::Def(def(&ir, "F")));
    // A literal may be passed strictly; `x + 1` must stay lazy.
    assert!(matches!(
        args[0],
        Arg::Expr {
            pass: Pass::Strict,
            ..
        }
    ));
    assert!(matches!(args[1], Arg::Expr { pass: Pass::Lazy, expr }
        if matches!(ir.expr(expr), Expr::Apply { callee: OpRef::Std(StdOp::NaturalsPlus), args }
            if args.iter().all(|a| matches!(a, Arg::Expr { pass: Pass::Direct, .. })))));
    let Expr::Let { defs, .. } = body(&ir, "H") else {
        unreachable!("H is not a LET");
    };
    let passes: Vec<_> = defs
        .iter()
        .map(|&d| match ir.def(d).scope {
            DefScope::LetValue { pass, .. } => Some(pass),
            _ => None,
        })
        .collect();
    assert_eq!(passes, vec![Some(Pass::Lazy), Some(Pass::Strict)]);
}

#[test]
fn variables_are_not_strict() {
    // `F(x)` in `Init` must stay lazy: TLC's enumerator sees through the
    // argument to the variable (`getVar`) and assigns it.
    let ir = ir("---- MODULE M ----
VARIABLE x
F(v) == v \\in {1, 2}
Init == F(x)
====
");
    let Expr::Apply { args, .. } = body(&ir, "Init") else {
        unreachable!("Init is not an application");
    };
    assert!(matches!(
        args[0],
        Arg::Expr {
            pass: Pass::Lazy,
            ..
        }
    ));
}

#[test]
fn instances_are_substitutions_resolved_dynamically() {
    let ir = ir("---- MODULE M ----
VARIABLE x
---- MODULE Inner ----
CONSTANT c
VARIABLE v
Op2 == c
Op == Op2
Next == v' = v
====
CONSTANT K
I == INSTANCE Inner WITH c <- K, v <- x
Use == I!Op
====
");
    let iop = def(&ir, "I!Op");
    assert_eq!(
        ir.def(iop).source.map(|s| ir.def(s).name.as_str()),
        Some("Op")
    );
    let Expr::Subst { bindings, body } = ir.expr(ir.def(iop).body) else {
        unreachable!("I!Op is not a substitution");
    };
    assert_eq!(bindings.len(), 2);
    // The body still calls the instantiated module's own Op2, whose
    // reference to `c` is looked up in the substitution context.
    let Expr::Def(op2) = ir.expr(*body) else {
        unreachable!("the body is not a use of Op2");
    };
    assert_eq!(ir.def(*op2).name, "Op2");
    let Expr::SubstRef {
        decl,
        primed: false,
    } = ir.expr(ir.def(*op2).body)
    else {
        unreachable!("c is not a substitution reference");
    };
    assert_eq!(ir.decl(*decl).name, "c");
    assert!(ir.decl(*decl).substituted);
    // `v'` under the substitution is a primed substitution reference.
    let next = def(&ir, "I!Next");
    let text = ir.pretty();
    assert!(text.contains("subst:decl"), "{text}");
    let Expr::Subst { body, .. } = ir.expr(ir.def(next).body) else {
        unreachable!("I!Next is not a substitution");
    };
    let Expr::Builtin { args, .. } = ir.expr(*body) else {
        unreachable!("I!Next's body is not =");
    };
    assert!(matches!(
        ir.expr(args[0]),
        Expr::SubstRef { primed: true, .. }
    ));
    // The root's own variable is a plain variable.
    assert_eq!(ir.vars().len(), 1);
}

#[test]
fn recursion_is_explicit() {
    let ir = ir("---- MODULE M ----
EXTENDS Naturals
RECURSIVE Fact(_)
Fact(n) == IF n = 0 THEN 1 ELSE n * Fact(n - 1)
f[n \\in Nat] == IF n = 0 THEN 1 ELSE n * f[n - 1]
g[n \\in Nat] == n
L == LET RECURSIVE Sum(_)
         Sum(k) == IF k = 0 THEN 0 ELSE k + Sum(k - 1)
     IN Sum(3)
====
");
    assert!(ir.def(def(&ir, "Fact")).recursive);
    assert_eq!(ir.def(def(&ir, "f")).kind, DefKind::Function);
    assert!(matches!(body(&ir, "f"), Expr::RecFcn { .. }));
    assert!(matches!(body(&ir, "g"), Expr::FcnCons { .. }));
    let Expr::Let { defs, .. } = body(&ir, "L") else {
        unreachable!("L is not a LET");
    };
    let sum = ir.def(defs[0]);
    assert!(sum.recursive);
    assert!(matches!(sum.scope, DefScope::Local { .. }));
}

#[test]
fn java_overrides_are_matched_like_tlc() {
    let ir = ir("---- MODULE M ----
EXTENDS Sequences, Naturals
S == INSTANCE Sequences
A == Len(<<1>>)
B == S!Len(<<1>>)
C == 7 \\div 2
====
");
    let std_of = |name: &str| match body(&ir, name) {
        Expr::Apply {
            callee: OpRef::Std(s),
            ..
        } => Some(*s),
        _ => None,
    };
    assert_eq!(std_of("A"), Some(StdOp::SequencesLen));
    assert_eq!(std_of("B"), Some(StdOp::SequencesLen));
    // Only Naturals is loaded, so Naturals.Divide is the override.
    assert_eq!(std_of("C"), Some(StdOp::NaturalsDiv));
    assert_eq!(ir.def(def(&ir, "S!Len")).std, Some(StdOp::SequencesLen));

    // With Integers loaded, Integers.Divide replaces it (it also checks
    // `-2147483648 \div -1`).
    let ir2 = ir_of("---- MODULE M ----\nEXTENDS Integers\nC == 7 \\div 2\n====\n");
    assert!(matches!(
        body(&ir2, "C"),
        Expr::Apply {
            callee: OpRef::Std(StdOp::IntegersDiv),
            ..
        }
    ));

    // A user's own `Len` is not an override.
    let ir3 = ir_of("---- MODULE M ----\nLen(s) == 0\nA == Len(1)\n====\n");
    assert!(matches!(
        body(&ir3, "A"),
        Expr::Apply {
            callee: OpRef::Def(_),
            ..
        }
    ));
}

fn ir_of(src: &str) -> Ir {
    ir(src)
}

#[test]
fn unsupported_overrides_are_refused_by_name() {
    let ir = ir("---- MODULE M ----
EXTENDS TLCExt, TLC
A == Trace
B == Print(1, TRUE)
====
");
    let names: Vec<String> = ir.refusals().iter().map(|r| r.feature.name()).collect();
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(names[0].starts_with("TLCExt!Trace"), "{names:?}");
    assert_eq!(ir.refusals()[0].def, def(&ir, "A"));
}

#[test]
fn effects() {
    let ir = ir("---- MODULE M ----
EXTENDS TLC, Naturals
CONSTANT N
VARIABLE x
P == Print(x, TRUE)
Live == TLCGet(\"distinct\") > 0
Reg == TLCGet(1) > 0
Lvl == TLCGet(\"level\") > 0
En == ENABLED (x' = 1)
Tmp == []P
UsesP == P
Open == N + 1
Pure == {1, 2}
====
");
    let e = |n: &str| ir.def(def(&ir, n)).effects;
    assert!(e("P").contains(Effects::IMPURE));
    assert!(
        e("UsesP").contains(Effects::IMPURE),
        "effects propagate through uses"
    );
    assert!(e("Live").contains(Effects::LIVE_TLCGET));
    assert!(e("Reg").contains(Effects::LIVE_TLCGET | Effects::IMPURE));
    assert!(!e("Lvl").intersects(Effects::LIVE_TLCGET | Effects::IMPURE));
    assert!(e("En").contains(Effects::USES_ENABLED));
    assert!(e("Tmp").contains(Effects::TEMPORAL));
    assert!(e("Open").contains(Effects::OPEN));
    assert!(!e("Pure").intersects(Effects::IMPURE | Effects::OPEN));
}

#[test]
fn levels_are_preserved_from_sema() {
    let src = "---- MODULE M ----
EXTENDS Naturals
CONSTANT N
VARIABLE x
Init == x = N
Next == x' = x + 1 /\\ UNCHANGED <<>>
Spec == Init /\\ [][Next]_x
====
";
    let a = analysis(src);
    let Ok(ir) = lower(&a) else {
        unreachable!("lowering failed");
    };
    check_levels(&a, &ir);
    assert_eq!(ir.def(def(&ir, "Init")).level, Level::State);
    assert_eq!(ir.def(def(&ir, "Next")).level, Level::Action);
    assert_eq!(ir.def(def(&ir, "Spec")).level, Level::Temporal);
}

/// Every expression's level equals SANY's level of the node it came from.
fn check_levels(a: &dive_sema::Analysis, ir: &Ir) {
    let m = a.model();
    let mut ids: Vec<Option<NodeId>> = vec![None; m.nodes().len()];
    let mut work: Vec<NodeId> = m.modules().to_vec();
    while let Some(n) = work.pop() {
        let Some(slot) = ids.get_mut(n.index()) else {
            continue;
        };
        if slot.is_some() {
            continue;
        }
        *slot = Some(n);
        work.extend(m.children(n));
    }
    for (i, node) in ir.nodes().iter().enumerate() {
        let e = dive_ir::ExprId(u32::try_from(i).unwrap_or(0));
        let Some(Some(sema)) = ir.sema_node(e).and_then(|s| ids.get(s).copied()) else {
            continue;
        };
        assert_eq!(node.level, Level::from(m.level(sema)), "#{i}");
    }
}

#[test]
fn origins_carry_sany_positions() {
    let ir = ir("---- MODULE M ----
VARIABLE x
\tA == x' = 1
====
");
    let a = ir.def(def(&ir, "A"));
    // The tab puts `A` at column 9, as JavaCC counts.
    assert_eq!(a.origin.begin, (3, 9));
    assert!(ir.location(&a.origin).ends_with("of module M"));
    assert!(
        ir.location(&a.origin)
            .starts_with("line 3, col 9 to line 3, col ")
    );
}

#[test]
fn lambdas_and_selectors_are_lowered_inline() {
    let ir = ir("---- MODULE M ----
EXTENDS Naturals
Ap(F(_), v) == F(v)
A == Ap(LAMBDA y : y + 1, 2)
COp(q) == \\E r \\in {1} : LET G(a) == {a, q, r} IN G(q) = {}
B == COp(1)!(2)!G(3)
====
");
    let text = ir.pretty();
    let inline = ir
        .defs()
        .iter()
        .filter(|d| matches!(d.scope, DefScope::Inline { .. }))
        .count();
    assert!(inline >= 2, "{text}");
    assert!(!text.contains("unbound"), "{text}");
}

#[test]
fn every_construct_lowers() {
    let ir = ir("---- MODULE M ----
EXTENDS Naturals, Sequences
CONSTANT C(_)
VARIABLE x, y
R == [a |-> 1, b |-> <<2, 3>>]
RS == [a : {1}, b : BOOLEAN]
E == [R EXCEPT !.a = @ + 1, !.b[1] = 0]
FS == [{1} -> {2}]
CP == {1} \\X {2} \\X {3}
Fil == {v \\in 1..3 : v > 1}
Map == {v + w : v \\in 1..3, w \\in {1}}
Tup == {<<v, w>> \\in {1} \\X {2} : v = w}
Ch == CHOOSE v \\in 1..3 : v > 2
Unb == \\E v : v = 1
Cs == CASE x = 1 -> 1 [] x = 2 -> 2 [] OTHER -> 3
Ifs == IF x = 1 THEN 1 ELSE 2
Sel == R.a + R[\"a\"]
Fn == [v \\in 1..3 |-> v][2]
Act == [x' = x]_<<x, y>> /\\ <<x' = x>>_x
Fair == WF_x(x' = 1) /\\ SF_<<x>>(x' = 2)
Tmp == [](x = 1) ~> <>(x = 2)
TE == \\EE z : z = x
Lab == lab :: x = 1
Cdt == (x' = 1) \\cdot (x' = 2)
Op == C(1) /\\ ~ (x = 1) /\\ (x = 1 => x = 2) /\\ (x = 1 <=> x = 2)
Dec == 1.5
S1 == SUBSET {1} \\cup UNION {{1}}
S2 == ({1} \\cap {1}) \\ {2}
S3 == {1} \\subseteq DOMAIN R /\\ 1 \\notin STRING
====
");
    let text = ir.pretty();
    for want in [
        "(record",
        "(recordset",
        "(except",
        "@",
        "(->",
        "(\\X",
        "(filter",
        "(map",
        "(Choose",
        "(case",
        "(if",
        "(.a",
        "(apply",
        "([]_",
        "(<>_",
        "(WF",
        "(SF",
        "(~>",
        "(EE",
        "lab::",
        "(cdot",
        "(not",
        "(=>",
        "(<=>",
        "1.5",
        "(SUBSET",
        "(\\union",
    ] {
        assert!(text.contains(want), "missing {want} in\n{text}");
    }
    assert!(!text.contains("unbound"), "{text}");
    assert!(!text.contains("unevaluable"), "{text}");
}

#[test]
fn deterministic_and_serializable() {
    let src = "---- MODULE M ----
EXTENDS Naturals
VARIABLE x
---- MODULE Inner ----
CONSTANT c
Op == c + 1
====
I == INSTANCE Inner WITH c <- x
Init == x = 0
Next == x' = I!Op /\\ LET a == 1 IN x' > a
====
";
    let a = ir(src);
    let b = ir(src);
    assert_eq!(a, b);
    assert_eq!(a.pretty(), b.pretty());
    let json = serde_json::to_string(&a).unwrap_or_default();
    let back: Result<Ir, _> = serde_json::from_str(&json);
    assert!(back.as_ref().is_ok_and(|back| *back == a), "{back:?}");
    for d in a.defs() {
        assert_ne!(d.hash, 0);
    }
}

#[test]
fn context_lists_names_and_assumptions() {
    let ir = ir("---- MODULE M ----
CONSTANT N
ASSUME N = N
ASSUME Named == N = N
THEOREM N = N
A == N
====
");
    let root = ir.module(ir.root());
    assert!(matches!(ir.lookup(ir.root(), "N"), Some(Symbol::Decl(_))));
    assert!(matches!(ir.lookup(ir.root(), "A"), Some(Symbol::Def(_))));
    assert_eq!(root.assumptions.len(), 2);
    assert_eq!(root.theorems.len(), 1);
    for &d in &root.assumptions {
        assert_eq!(ir.def(d).kind, DefKind::Assumption);
    }
}
