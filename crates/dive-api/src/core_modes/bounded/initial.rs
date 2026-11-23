//! Exact initial-domain joins for a negated conjunction of state predicates.
//!
//! Visit clauses in their original short-circuit order. An unbound variable
//! is expanded only when the next demanded clause reads it. A false clause
//! excludes all completions of that partial assignment. Any uncertain shape
//! or evaluator error declines to the ordinary generator.

use std::time::Instant;

use dive_core::plan::{Action, Code, G, GNode, Node, P, Quant};
use dive_core::{Error, ErrorKind, Handle, Machine};

use super::super::Native;

const MAX_DOMAIN: usize = 4096;
const MAX_TABLE_CELLS: usize = 262_144;

#[derive(Debug, Default)]
pub(super) struct Stats {
    domain_sizes: Vec<usize>,
    tested_clauses: u64,
    definedness_cells: usize,
    extensions: u64,
    preparation_ms: f64,
    search_ms: f64,
}

impl Stats {
    pub(super) fn to_json(&self) -> serde_json::Value {
        serde_json::json!({"domain_sizes":self.domain_sizes,"definedness_cells":self.definedness_cells,
            "tested_clauses":self.tested_clauses,"extensions":self.extensions,
            "preparation_ms":self.preparation_ms,"search_ms":self.search_ms})
    }
}

pub(super) enum ResultKind {
    Found { row: Vec<Handle>, stats: Stats },
    Empty { product: usize, stats: Stats },
    Declined,
    Stopped,
}

#[derive(Clone, Copy)]
enum Domain {
    One(P),
    Members(P),
}

fn collect_rows(code: &Code, node: G, axes: &mut [Option<Domain>]) -> Option<()> {
    let (var, assign) = match code.gnodes.get(node as usize)? {
        GNode::Conj(items) => {
            return items
                .iter()
                .try_for_each(|&(g, _)| collect_rows(code, g, axes));
        }
        GNode::Scope { body, reset } if reset.is_empty() => return collect_rows(code, *body, axes),
        &GNode::Assign { var, rhs } => (var, Domain::One(rhs)),
        &GNode::AssignIn { var, set } => (var, Domain::Members(set)),
        _ => return None,
    };
    let expression = match assign {
        Domain::One(p) | Domain::Members(p) => p,
    };
    if !matches!(code.nodes.get(expression as usize)?, Node::Const(_))
        || axes.get_mut(usize::from(var))?.replace(assign).is_some()
    {
        return None;
    }
    Some(())
}

fn reads(code: &Code, p: P, depth: u8) -> Option<u64> {
    let next = |child| reads(code, child, depth + 1);
    if depth >= 48 {
        return None;
    }
    let list = |items: &[P]| {
        items
            .iter()
            .try_fold(0, |mask, &child| Some(mask | next(child)?))
    };
    let quant = |binds: &[dive_core::plan::Bind], body| {
        binds.iter().try_fold(next(body)?, |mask, bind| {
            if bind.eligible.is_some() {
                return None;
            }
            Some(mask | next(bind.domain)?)
        })
    };
    match code.nodes.get(p as usize)? {
        Node::Const(_) | Node::Slot(_) => Some(0),
        &Node::Var(var) => 1_u64.checked_shl(u32::from(var)),
        &Node::Not(p)
        | &Node::Neg(p)
        | &Node::Domain(p)
        | &Node::Spec(p)
        | &Node::Select(p, _)
        | &Node::Memo { body: p, .. }
        | &Node::Lazy { body: p, .. }
        | &Node::Scope { body: p, .. } => next(p),
        Node::And(items) | Node::Or(items) | Node::SetEnum(items) | Node::Tuple(items) => {
            list(items)
        }
        &Node::Eq(a, b)
        | &Node::Neq(a, b)
        | &Node::In(a, b)
        | &Node::NotIn(a, b)
        | &Node::Arith(_, a, b)
        | &Node::Cmp(_, a, b)
        | &Node::Range(a, b)
        | &Node::FcnSet(a, b)
        | &Node::Apply(a, b)
        | &Node::SetBin(_, a, b) => Some(next(a)? | next(b)?),
        &Node::Filter {
            ref bind,
            pred,
            subset: None,
        } => quant(std::slice::from_ref(bind), pred),
        &Node::Map { ref binds, body }
        | &Node::FcnCons { ref binds, body }
        | &Node::Quant {
            kind: Quant::Exists | Quant::Forall,
            ref binds,
            body,
        } => quant(binds, body),
        _ => None,
    }
}

fn unwrap_memo(code: &Code, mut p: P) -> Option<P> {
    loop {
        match code.nodes.get(p as usize)? {
            &Node::Memo { body, .. } | &Node::Lazy { body, .. } => p = body,
            Node::Scope { reset, body } if reset.is_empty() => p = *body,
            _ => return Some(p),
        }
    }
}

fn clauses(code: &Code, p: P) -> Option<Vec<(P, u64)>> {
    let &Node::Not(body) = code.nodes.get(unwrap_memo(code, p)? as usize)? else {
        return None;
    };
    let inner = unwrap_memo(code, body)?;
    let Node::And(items) = code.nodes.get(inner as usize)? else {
        return None;
    };
    items
        .iter()
        .map(|&item| Some((item, reads(code, item, 0)?)))
        .collect()
}

fn collect_domain(
    machine: &mut Machine,
    code: &Code,
    domain: Domain,
) -> Result<Vec<Handle>, Error> {
    Ok(match domain {
        Domain::One(p) => {
            let value = machine.eval(code, p)?;
            vec![machine.heap.intern_exact(value)?]
        }
        Domain::Members(p) => {
            let set = machine.eval(code, p)?;
            let mut cursor = machine.heap.cursor(set)?;
            let mut values = Vec::new();
            while let Some(value) = cursor.next(&mut machine.heap)? {
                if machine.heap.work_interrupted() {
                    return Err(Error::stop());
                }
                if values.len() == MAX_DOMAIN {
                    return Err(Error::unsupported(
                        "bounded initial domain exceeds 4096 members",
                    ));
                }
                values.push(machine.heap.intern_exact(value)?);
            }
            values
        }
    })
}

fn domains(
    machine: &mut Machine,
    code: &Code,
    action: &Action,
    axes: &[Option<Domain>],
) -> Result<Vec<Vec<Handle>>, Error> {
    axes.iter()
        .map(|axis| {
            let mark = machine.heap.mark();
            let old = machine.enter(action.frame);
            let result = collect_domain(
                machine,
                code,
                axis.ok_or_else(|| Error::internal("unbound initial variable"))?,
            );
            machine.leave(old);
            machine.heap.rollback(mark);
            result
        })
        .collect()
}

struct Relation {
    columns: Vec<(usize, usize)>,
    reads: u64,
    values: Vec<bool>,
}

fn table(
    machine: &mut Machine,
    code: &Code,
    frame: u32,
    domains: &[Vec<Handle>],
    predicate: P,
    reads: u64,
    limit: usize,
) -> Result<Relation, Error> {
    let mut size = 1_usize;
    let mut columns = Vec::new();
    for (var, domain) in domains
        .iter()
        .enumerate()
        .filter(|(var, _)| reads & (1 << var) != 0)
    {
        columns.push((var, size));
        size = size
            .checked_mul(domain.len())
            .filter(|&size| size <= limit)
            .ok_or_else(|| Error::unsupported("initial definedness table budget"))?;
    }
    if size > limit {
        return Err(Error::unsupported("initial definedness table budget"));
    }
    let mut values = Vec::with_capacity(size);
    for index in 0..size {
        if machine.heap.work_interrupted() {
            return Err(Error::stop());
        }
        for &(var, stride) in &columns {
            machine.s0[var] = domains[var][(index / stride) % domains[var].len()];
        }
        let mark = machine.heap.mark();
        let saved = machine.enter(frame);
        let holds = machine.eval_bool(code, predicate);
        machine.leave(saved);
        machine.heap.rollback(mark);
        values.push(holds?);
    }
    machine.s0.fill(Handle::EMPTY);
    Ok(Relation {
        columns,
        reads,
        values,
    })
}

struct Join<'a> {
    machine: &'a Machine,
    domains: &'a [Vec<Handle>],
    relations: &'a [Relation],
    chosen: Vec<usize>,
    stats: &'a mut Stats,
}

impl Join<'_> {
    fn demand(&mut self, index: usize, assigned: u64) -> Result<bool, Error> {
        let Some(relation) = self.relations.get(index) else {
            return Ok(true);
        };
        if self.machine.heap.work_interrupted() {
            return Err(Error::stop());
        }
        let missing = relation.reads & !assigned;
        if missing != 0 {
            let var = missing.trailing_zeros() as usize;
            for value in 0..self.domains[var].len() {
                self.stats.extensions += 1;
                self.chosen[var] = value;
                if self.demand(index, assigned | 1 << var)? {
                    return Ok(true);
                }
            }
            self.chosen[var] = 0;
            return Ok(false);
        }
        let offset = relation
            .columns
            .iter()
            .map(|&(var, stride)| self.chosen[var] * stride)
            .sum::<usize>();
        self.stats.tested_clauses += 1;
        Ok(relation.values[offset] && self.demand(index + 1, assigned)?)
    }
}

pub(super) fn check(native: &mut Native) -> ResultKind {
    let prepared = Instant::now();
    let plan = &native.plan;
    let ([init], [invariant]) = (&plan.init[..], &plan.invariants[..]) else {
        return ResultKind::Declined;
    };
    if plan.effects.any
        || !plan.constraints.is_empty()
        || !plan.implied_inits.is_empty()
        || !init.preset.is_empty()
        || !(1..=32).contains(&plan.vars.len())
    {
        return ResultKind::Declined;
    }
    let [root] = invariant.conjuncts.as_slice() else {
        return ResultKind::Declined;
    };
    let Some(clauses) = clauses(&plan.code, root.p).filter(|clauses| clauses.len() <= 128) else {
        return ResultKind::Declined;
    };
    let mut axis = vec![None; plan.vars.len()];
    if collect_rows(&plan.code, init.g, &mut axis).is_none() || axis.iter().any(Option::is_none) {
        return ResultKind::Declined;
    }
    let m = &mut native.machine;
    let domains = match domains(m, &plan.code, init, &axis) {
        Ok(values) => values,
        Err(e) if e.kind() == ErrorKind::Stop => return ResultKind::Stopped,
        Err(_) => return ResultKind::Declined,
    };
    let Some(product) = domains
        .iter()
        .try_fold(1_usize, |size, row| size.checked_mul(row.len()))
    else {
        return ResultKind::Declined;
    };
    if product == 0 || !m.warnings.is_empty() || !m.printed.is_empty() {
        return ResultKind::Declined;
    }
    // Check each clause over its whole read projection before accepting any
    // result. A demanded error in an earlier ordinary Init row must not be
    // hidden by a different join order. This stronger totality obligation may
    // decline when a guard only happens to be defined after another guard.
    let mut definedness_cells = 0;
    let relations = clauses
        .iter()
        .map(|&(predicate, reads)| {
            let relation = table(
                m,
                &plan.code,
                invariant.frame,
                &domains,
                predicate,
                reads,
                MAX_TABLE_CELLS - definedness_cells,
            )?;
            definedness_cells += relation.values.len();
            Ok(relation)
        })
        .collect::<Result<Vec<_>, Error>>();
    m.s0.fill(Handle::EMPTY);
    let relations = match relations {
        Ok(relations) => relations,
        Err(error) if error.kind() == ErrorKind::Stop => return ResultKind::Stopped,
        Err(_) => return ResultKind::Declined,
    };
    let mut stats = Stats {
        domain_sizes: domains.iter().map(Vec::len).collect(),
        definedness_cells,
        preparation_ms: prepared.elapsed().as_secs_f64() * 1000.,
        ..Stats::default()
    };
    let started = Instant::now();
    let mut join = Join {
        machine: m,
        domains: &domains,
        relations: &relations,
        chosen: vec![0; domains.len()],
        stats: &mut stats,
    };
    let result = join.demand(0, 0);
    let row = join
        .chosen
        .iter()
        .zip(&domains)
        .map(|(&value, domain)| domain[value])
        .collect();
    stats.search_ms = started.elapsed().as_secs_f64() * 1000.;
    if m.heap.poisoned() || !m.warnings.is_empty() || !m.printed.is_empty() {
        return ResultKind::Declined;
    }
    match result {
        Ok(true) => ResultKind::Found { row, stats },
        Ok(false) => ResultKind::Empty { product, stats },
        Err(e) if e.kind() == ErrorKind::Stop => ResultKind::Stopped,
        Err(_) => ResultKind::Declined,
    }
}
