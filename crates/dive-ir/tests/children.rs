//! Direct-child order is observable in effects, structural admission and hashes.

use dive_ir::{
    Arg, Bound, BuiltinOp, CaseArm, ConstId, DeclId, DefId, ExceptUpdate, Expr, ExprId, Field,
    JunctionKind, Level, LocalRef, OpRef, Pass, QuantKind, Slot, SubstBinding, Unevaluable, VarId,
};

fn check(expr: &Expr, expected: &[u32]) {
    let expected: Vec<_> = expected.iter().copied().map(ExprId).collect();
    let mut seen = Vec::new();
    expr.for_each_child(|child| seen.push(child));
    assert_eq!(seen, expected, "{}", expr.tag());
    assert_eq!(expr.children(), expected, "{} owned", expr.tag());
    for stop in 0..=expected.len() {
        let mut seen = Vec::new();
        let result = expr.try_for_each_child(|child| {
            seen.push(child);
            if seen.len() == stop + 1 {
                Err(child)
            } else {
                Ok(())
            }
        });
        assert_eq!(result, expected.get(stop).copied().map_or(Ok(()), Err));
        assert_eq!(seen, expected[..expected.len().min(stop + 1)]);
    }
}

fn bound(domain: Option<u32>) -> Bound {
    Bound {
        vars: Box::from([Slot(42)]),
        tuple: true,
        domain: domain.map(ExprId),
    }
}

fn bounds() -> Box<[Bound]> {
    Box::from([bound(None), bound(Some(11)), bound(None), bound(Some(2))])
}

fn args() -> Box<[Arg]> {
    Box::from([
        Arg::Op {
            op: OpRef::Def(DefId(99)),
            level: Level::Constant,
        },
        Arg::Expr {
            expr: ExprId(11),
            pass: Pass::Lazy,
        },
        Arg::Op {
            op: OpRef::Const(ConstId(88)),
            level: Level::State,
        },
        Arg::Expr {
            expr: ExprId(2),
            pass: Pass::Strict,
        },
        Arg::Expr {
            expr: ExprId(11),
            pass: Pass::Direct,
        },
    ])
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "explicit child-order expectations for every expression variant"
)]
fn every_expression_keeps_child_order_and_stops_on_first_error() {
    let leaves = [
        Expr::Bool(true),
        Expr::Num("7".into()),
        Expr::Decimal {
            int: "1".into(),
            frac: "25".into(),
        },
        Expr::Str("s".into()),
        Expr::Var {
            var: VarId(91),
            primed: true,
        },
        Expr::Const(ConstId(99)),
        Expr::Local(LocalRef {
            up: 3,
            slot: Slot(91),
        }),
        Expr::SubstRef {
            decl: DeclId(99),
            primed: true,
        },
        Expr::Def(DefId(88)),
        Expr::ExceptAt,
        Expr::Unbound("x".into()),
        Expr::Unevaluable(Unevaluable::Proof),
    ];
    for expr in leaves {
        check(&expr, &[]);
    }
    for expr in [
        Expr::Not(ExprId(11)),
        Expr::Selected(ExprId(11)),
        Expr::Prime(ExprId(11)),
        Expr::Unchanged(ExprId(11)),
        Expr::Enabled(ExprId(11)),
        Expr::Always(ExprId(11)),
        Expr::Eventually(ExprId(11)),
        Expr::RecordSelect {
            record: ExprId(11),
            field: "field".into(),
        },
        Expr::Let {
            defs: Box::from([DefId(99), DefId(88)]),
            body: ExprId(11),
        },
        Expr::Label {
            name: "label".into(),
            body: ExprId(11),
        },
        Expr::TemporalQuant {
            exists: true,
            vars: Box::from([Slot(99)]),
            body: ExprId(11),
        },
    ] {
        check(&expr, &[11]);
    }
    for expr in [
        Expr::Implies(ExprId(11), ExprId(2)),
        Expr::Equiv(ExprId(11), ExprId(2)),
        Expr::Cdot(ExprId(11), ExprId(2)),
        Expr::LeadsTo(ExprId(11), ExprId(2)),
        Expr::WhilePlus(ExprId(11), ExprId(2)),
        Expr::FcnSet {
            domain: ExprId(11),
            range: ExprId(2),
        },
        Expr::FcnApply {
            fcn: ExprId(11),
            arg: ExprId(2),
        },
        Expr::StepBox {
            action: ExprId(11),
            sub: ExprId(2),
        },
        Expr::StepAngle {
            action: ExprId(11),
            sub: ExprId(2),
        },
        Expr::Wf {
            sub: ExprId(11),
            action: ExprId(2),
        },
        Expr::Sf {
            sub: ExprId(11),
            action: ExprId(2),
        },
    ] {
        check(&expr, &[11, 2]);
    }
    check(
        &Expr::If {
            cond: ExprId(11),
            then: ExprId(2),
            other: ExprId(7),
        },
        &[11, 2, 7],
    );
    check(
        &Expr::Apply {
            callee: OpRef::Def(DefId(99)),
            args: args(),
        },
        &[11, 2, 11],
    );
    for expr in [
        Expr::Quant {
            kind: QuantKind::Exists,
            bounds: bounds(),
            body: ExprId(7),
        },
        Expr::FcnCons {
            bounds: bounds(),
            body: ExprId(7),
        },
        Expr::RecFcn {
            name: Slot(99),
            bounds: bounds(),
            body: ExprId(7),
        },
        Expr::SetMap {
            bounds: bounds(),
            expr: ExprId(7),
        },
    ] {
        check(&expr, &[11, 2, 7]);
    }
    check(
        &Expr::SetFilter {
            bound: bound(Some(11)),
            pred: ExprId(2),
        },
        &[11, 2],
    );
    check(
        &Expr::SetFilter {
            bound: bound(None),
            pred: ExprId(2),
        },
        &[2],
    );
    for other in [None, Some(ExprId(7))] {
        let expr = Expr::Case {
            arms: Box::from([
                CaseArm {
                    guard: ExprId(11),
                    value: ExprId(2),
                },
                CaseArm {
                    guard: ExprId(5),
                    value: ExprId(11),
                },
            ]),
            other,
        };
        check(
            &expr,
            if other.is_some() {
                &[11, 2, 5, 11, 7]
            } else {
                &[11, 2, 5, 11]
            },
        );
    }
    for length in [0, 1, 2, 8, 9, 257] {
        let ids: Vec<u32> = (0..length).rev().collect();
        let items: Box<[_]> = ids.iter().copied().map(ExprId).collect();
        for expr in [
            Expr::Builtin {
                op: BuiltinOp::Eq,
                args: items.clone(),
            },
            Expr::Junction {
                kind: JunctionKind::And,
                bulleted: true,
                items: items.clone(),
            },
            Expr::SetEnum(items.clone()),
            Expr::Tuple(items.clone()),
            Expr::Cartesian(items),
        ] {
            check(&expr, &ids);
        }
        let fields: Box<[_]> = ids
            .iter()
            .copied()
            .map(|id| Field {
                name: "field".into(),
                value: ExprId(id),
            })
            .collect();
        check(&Expr::Record(fields.clone()), &ids);
        check(&Expr::RecordSet(fields), &ids);
    }
    check(
        &Expr::Except {
            base: ExprId(7),
            updates: Box::from([
                ExceptUpdate {
                    path: Box::from([ExprId(11), ExprId(2)]),
                    value: ExprId(5),
                },
                ExceptUpdate {
                    path: Box::from([]),
                    value: ExprId(3),
                },
                ExceptUpdate {
                    path: Box::from([ExprId(7)]),
                    value: ExprId(11),
                },
            ]),
        },
        &[7, 11, 2, 5, 3, 7, 11],
    );
    let bindings: Box<[_]> = args()
        .into_vec()
        .into_iter()
        .map(|value| SubstBinding {
            decl: DeclId(99),
            value,
        })
        .collect();
    check(
        &Expr::Subst {
            bindings,
            body: ExprId(7),
        },
        &[11, 2, 11, 7],
    );
    check(
        &Expr::Subst {
            bindings: Box::from([]),
            body: ExprId(7),
        },
        &[7],
    );
    check(
        &Expr::Case {
            arms: Box::from([]),
            other: None,
        },
        &[],
    );
    check(
        &Expr::Case {
            arms: Box::from([]),
            other: Some(ExprId(7)),
        },
        &[7],
    );
    check(
        &Expr::Apply {
            callee: OpRef::Def(DefId(99)),
            args: Box::from([]),
        },
        &[],
    );
    check(
        &Expr::Quant {
            kind: QuantKind::Forall,
            bounds: Box::from([]),
            body: ExprId(7),
        },
        &[7],
    );
    check(
        &Expr::Except {
            base: ExprId(7),
            updates: Box::from([]),
        },
        &[7],
    );
}
