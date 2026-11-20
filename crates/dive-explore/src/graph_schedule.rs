//! Scheduling for a wide initial frontier with small deterministic Next actions.

use dive_core::Plan;
use dive_core::plan::{Code, G, GNode, Node, P, Unch};

/// Every permitted branch emits at most one successor. The visit budget also
/// caps recursion and repeated visits through shared code on user input.
pub(super) fn inline_initial_frontier(plan: &Plan, roots: u64) -> bool {
    if roots < 4_096 || plan.vars.len() != 1 || plan.next.len() > 8 {
        return false;
    }
    let mut fuel = 256;
    plan.next
        .iter()
        .all(|action| simple_action(&plan.code, action.g, &mut fuel))
        && plan
            .constraints
            .iter()
            .chain(&plan.action_constraints)
            .chain(&plan.invariants)
            .chain(&plan.implied_actions)
            .all(|check| {
                check
                    .conjuncts
                    .iter()
                    .all(|c| simple_value(&plan.code, c.p, &mut fuel))
            })
}

fn simple_action(code: &Code, node: G, fuel: &mut usize) -> bool {
    let Some(left) = fuel.checked_sub(1) else {
        return false;
    };
    *fuel = left;
    match code.gnodes.get(node as usize) {
        Some(GNode::Conj(parts)) => parts.iter().all(|&(g, _)| simple_action(code, g, fuel)),
        Some(GNode::Scope { body, .. }) => simple_action(code, *body, fuel),
        Some(GNode::Guard(p) | GNode::Assign { rhs: p, .. }) => simple_value(code, *p, fuel),
        Some(GNode::Unchanged(items)) => items.iter().all(|item| matches!(item, Unch::Var { .. })),
        _ => false,
    }
}

fn simple_value(code: &Code, node: P, fuel: &mut usize) -> bool {
    let Some(left) = fuel.checked_sub(1) else {
        return false;
    };
    *fuel = left;
    match code.nodes.get(node as usize) {
        Some(Node::Const(_) | Node::Slot(_) | Node::Var(_) | Node::Primed(_)) => true,
        Some(Node::Select(p, _) | Node::Memo { body: p, .. } | Node::Scope { body: p, .. }
            | Node::Lazy { body: p, .. } | Node::Spec(p) | Node::Not(p)) => {
            simple_value(code, *p, fuel)
        }
        Some(Node::Arith(_, a, b) | Node::Cmp(_, a, b) | Node::Eq(a, b)
            | Node::Neq(a, b) | Node::Equiv(a, b) | Node::Implies(a, b)) => {
            simple_value(code, *a, fuel) && simple_value(code, *b, fuel)
        }
        Some(Node::In(p, domain)) => {
            matches!(code.nodes.get(*domain as usize), Some(Node::Const(_)))
                && simple_value(code, *p, fuel)
        }
        Some(Node::SameVars { eq, .. }) => simple_value(code, *eq, fuel),
        Some(Node::And(ps) | Node::Or(ps)) => ps.iter().all(|&p| simple_value(code, p, fuel)),
        Some(Node::Except { base, updates }) => {
            simple_value(code, *base, fuel) && updates.iter().all(|update| {
                matches!(update.path.as_ref(), [p] if matches!(code.nodes.get(*p as usize), Some(Node::Const(_))))
                    && simple_value(code, update.value, fuel)
            })
        }
        _ => false,
    }
}
