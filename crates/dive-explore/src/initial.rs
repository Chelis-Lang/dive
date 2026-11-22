//! Parallel generation and checks over a closed finite Init product.
//!
//! Workers own successive slices of the first domain. They buffer an ordered
//! stream of exact rows while the admission and diagnostic state stays in the
//! explorer. If any worker cannot establish a pure, fully successful slice,
//! the explorer evaluates the original Init in order to classify its first stop.

use std::cmp::Ordering;
use std::thread;

use dive_core::plan::{Action, Code, G, GNode, Node, P};
use dive_core::{Error, Handle, Machine, Plan, Sink};

use crate::{Explorer, Metrics, eval_check, fingerprintable};

const MIN_COMBINATIONS: u64 = 4096;
const MAX_DOMAIN_SIZE: u64 = 256;
const MAX_SHARDS: usize = 32;
// Bound all worker row buffers, including rejected input combinations.
const MAX_PRODUCT_BYTES: u64 = 96 << 20;

struct Product {
    code: Code,
    split: G,
    variable: u16,
    choices: Vec<Handle>,
    shard_rows: usize,
}

pub(super) fn parallel(
    machines: &mut [Machine],
    ex: &mut Explorer<'_>,
) -> Option<Result<(), Error>> {
    let plan = ex.plan;
    if machines.len() < 2
        || plan.effects.any
        || plan.code.source_map.is_some()
        || plan.code.debugger.is_some()
        || ex.packed.is_none()
        || ex.disk.is_some()
        || ex.candidate_storage.predecessor_memory_bytes.is_some()
        || !plan.constraints.is_empty()
        || plan.view.is_some()
        || plan.symmetry.is_some()
        || ex.max_states.is_some_and(|n| n < MIN_COMBINATIONS)
        || ex.max_depth == Some(0)
    {
        return None;
    }
    let [action] = plan.init.as_slice() else {
        return None;
    };
    let machine = machines.first_mut()?;
    // Complete continuations use the usual Boolean, short-circuit Init path.
    // The warning-off generator has different occurrence traversal rules.
    if !machine.warn || !(2..=64).contains(&plan.vars.len()) {
        return None;
    }
    let product = Product::prepare(machine, plan, action)?;
    let combinations = product.shard_rows as u64 * product.choices.len() as u64;
    if ex
        .max_states
        .is_some_and(|max| combinations > max.saturating_mul(4))
    {
        return None;
    }
    let width = machines.len().min(product.choices.len());
    let outputs = thread::scope(|scope| {
        let mut started = Vec::new();
        let mut spawn_failed = false;
        let groups = product
            .choices
            .chunks(product.choices.len().div_ceil(width));
        for (machine, choices) in machines.iter_mut().zip(groups) {
            let product = &product;
            let poll = ex.poll;
            let thread = thread::Builder::new().spawn_scoped(scope, move || {
                product.generate(machine, plan, action, choices, poll)
            });
            if let Ok(thread) = thread {
                started.push(thread);
            } else {
                spawn_failed = true;
                break;
            }
        }
        // Join every worker before deciding whether the serial path is safe
        // to use. A panicked machine cannot be assumed to be reusable.
        let completed: Vec<_> = started
            .into_iter()
            .map(thread::ScopedJoinHandle::join)
            .collect();
        completed
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map(|completed| {
                if spawn_failed {
                    None
                } else {
                    completed.into_iter().collect::<Option<Vec<_>>>()
                }
            })
    });
    let Ok(outputs) = outputs else {
        return Some(Err(Error::internal("initial-state worker panicked")));
    };
    if outputs.is_none() {
        // No output or warning was ever committed. Replay demanded warnings
        // with the original generator instead of publishing speculative ones.
        for machine in &mut *machines {
            machine.warnings.clear();
            machine.heap.reset();
        }
    }
    let outputs = outputs?;
    ex.metrics.initial_workers = outputs.len();
    let m = machines.first_mut()?;
    Some(commit(m, ex, outputs))
}

impl Product {
    fn prepare(machine: &mut Machine, plan: &Plan, action: &Action) -> Option<Self> {
        let variables = plan.vars.len();
        let generators = leading_domains(&plan.code, action, variables)?;
        let mut domains = Vec::with_capacity(variables);
        let mut combinations = 1u64;
        for (g, var, domain) in generators {
            let mark = machine.heap.mark();
            let values = exact_members(machine, &plan.code, domain);
            machine.heap.rollback(mark);
            let values = values?;
            if domains.is_empty() && !(2..=MAX_SHARDS).contains(&values.len()) {
                return None;
            }
            combinations = combinations.checked_mul(values.len() as u64)?;
            if combinations.checked_mul((variables * std::mem::size_of::<Handle>()) as u64)?
                > MAX_PRODUCT_BYTES
            {
                return None;
            }
            domains.push((g, var, values));
        }
        if combinations < MIN_COMBINATIONS {
            return None;
        }
        let (split, variable, choices) = domains.first()?.clone();
        let mut code = plan.code.clone();
        for (g, var, values) in domains {
            // Reuse the original cursor order, including function spaces. The
            // new set is an array of exactly the representations it produced.
            let mark = machine.heap.mark();
            let set = machine.heap.set_sorted(&values);
            let set = machine.heap.intern_exact(set);
            machine.heap.rollback(mark);
            let node = u32::try_from(code.nodes.len()).ok()?;
            code.nodes.push(Node::Const(set.ok()?));
            code.gnodes[g as usize] = GNode::AssignIn { var, set: node };
        }
        let shard_rows = usize::try_from(combinations / choices.len() as u64).ok()?;
        Some(Self {
            code,
            split,
            variable,
            choices,
            shard_rows,
        })
    }

    fn generate(
        &self,
        machine: &mut Machine,
        plan: &Plan,
        action: &Action,
        choices: &[Handle],
        poll: &(dyn Fn() -> bool + Sync),
    ) -> Option<Shard> {
        let mut code = self.code.clone();
        let rhs = u32::try_from(code.nodes.len()).ok()?;
        code.nodes.push(Node::Const(Handle::EMPTY));
        code.gnodes[self.split as usize] = GNode::Assign {
            var: self.variable,
            rhs,
        };
        let mut sink = Worker {
            plan,
            rows: Vec::new(),
            max_handles: self
                .shard_rows
                .checked_mul(choices.len())?
                .checked_mul(plan.vars.len())?,
            scratch: vec![Handle::EMPTY; plan.vars.len()],
            metrics: Metrics::default(),
            poll,
        };
        sink.rows.try_reserve_exact(sink.max_handles).ok()?;
        for &member in choices {
            code.nodes[rhs as usize] = Node::Const(member);
            machine.level = 0;
            machine.run_bound_action(&code, action, &mut sink).ok()?;
        }
        if !machine.warnings.is_empty() || machine.heap.poisoned() {
            return None;
        }
        Some(Shard {
            rows: sink.rows,
            conjuncts: sink.metrics.conjuncts_total,
        })
    }
}

fn exact_members(machine: &mut Machine, code: &Code, domain: P) -> Option<Vec<Handle>> {
    let set = machine.eval(code, domain).ok()?;
    let count = machine.heap.cardinality(set).ok()?;
    if !(1..=MAX_DOMAIN_SIZE).contains(&count) {
        return None;
    }
    let mut cursor = machine.heap.cursor(set).ok()?;
    let mut values = Vec::new();
    while let Some(value) = cursor.next(&mut machine.heap).ok()? {
        if values.len() as u64 == count
            || values
                .last()
                .is_some_and(|&prev| machine.heap.cmp(prev, value) != Ordering::Less)
        {
            return None;
        }
        values.push(machine.heap.intern_exact(value).ok()?);
    }
    (values.len() as u64 == count).then_some(values)
}

fn leading_domains(code: &Code, action: &Action, variables: usize) -> Option<Vec<(G, u16, P)>> {
    let mut todo = vec![action.g];
    let mut assigned = 0u64;
    let mut domains = Vec::new();
    // A compiled Plan is normally a DAG. Cap optional inspection work even
    // for an unusually deep nested conjunction.
    for _ in 0..256 {
        let g = todo.pop()?;
        match code.gnodes.get(g as usize)? {
            GNode::Conj(items) => todo.extend(items.iter().rev().map(|&(g, _)| g)),
            GNode::Scope { body, .. } => todo.push(*body),
            GNode::AssignIn { var, set }
                if usize::from(*var) < variables && closed_domain(code, *set) =>
            {
                let bit = 1u64 << var;
                if assigned & bit != 0 {
                    return None;
                }
                assigned |= bit;
                domains.push((g, *var, *set));
                if domains.len() == variables {
                    return Some(domains);
                }
            }
            _ => return None,
        }
    }
    None
}

fn closed_domain(code: &Code, p: P) -> bool {
    match code.nodes.get(p as usize) {
        Some(Node::Const(_)) => true,
        Some(Node::Powerset(sub)) => matches!(code.nodes.get(*sub as usize), Some(Node::Const(_))),
        Some(Node::FcnSet(keys, values)) => {
            matches!(code.nodes.get(*keys as usize), Some(Node::Const(_)))
                && match code.nodes.get(*values as usize) {
                    Some(Node::Const(_)) => true,
                    Some(Node::Powerset(sub)) => {
                        matches!(code.nodes.get(*sub as usize), Some(Node::Const(_)))
                    }
                    _ => false,
                }
        }
        _ => false,
    }
}

struct Shard {
    rows: Vec<Handle>,
    conjuncts: u64,
}

struct Worker<'a> {
    plan: &'a Plan,
    rows: Vec<Handle>,
    max_handles: usize,
    scratch: Vec<Handle>,
    metrics: Metrics,
    poll: &'a (dyn Fn() -> bool + Sync),
}

impl Sink for Worker<'_> {
    fn emit(&mut self, m: &mut Machine, code: &Code) -> Result<(), Error> {
        if (self.poll)()
            || self.rows.len() >= self.max_handles
            || m.assigned != m.s1.len()
            || m.s1.iter().any(|value| value.is_transient())
        {
            return Err(Error::stop());
        }
        let mark = m.heap.mark();
        let forced = m.thunk_mark();
        self.scratch.copy_from_slice(&m.s1);
        let level = m.level;
        m.level = 1;
        let checked = (|| {
            if !fingerprintable(m, true)? {
                return Err(Error::stop());
            }
            for check in self.plan.invariants.iter().chain(&self.plan.implied_inits) {
                if !eval_check(m, code, check, None, &mut self.metrics)? {
                    return Err(Error::stop());
                }
            }
            self.rows.extend_from_slice(&self.scratch);
            Ok(())
        })();
        m.level = level;
        m.s1.copy_from_slice(&self.scratch);
        if m.thunks_released(forced) {
            m.heap.rollback(mark);
        }
        checked
    }
}

fn commit(m: &mut Machine, ex: &mut Explorer<'_>, shards: Vec<Shard>) -> Result<(), Error> {
    ex.init_action = Some(0);
    for shard in shards {
        ex.metrics.conjuncts_total += shard.conjuncts;
        for row in shard.rows.chunks_exact(ex.plan.vars.len()) {
            m.s1.copy_from_slice(row);
            m.assigned = row.len();
            ex.emit_with_init_proof(m, true)?;
        }
    }
    m.clear_buffer();
    Ok(())
}
