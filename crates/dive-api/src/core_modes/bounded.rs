//! Exact-layer search over the shared compiled Init, Next and invariant roles.

use std::collections::BTreeSet;

use dive_core::{Error, ErrorKind, Handle};
use dive_eval::next::StateVals;
use dive_model::Model;
use dive_runtime::budget::Budget;
use dive_sim::Outcome;

use super::{
    Native, allowed, assumptions, classify, finish_batch, generate, retain, roles, values,
};

pub(crate) enum ResultKind {
    ReachedBound,
    ExecutionsTooShort,
    Failed(Outcome),
}

pub(crate) struct Run {
    pub(crate) witness: Vec<StateVals>,
    pub(crate) checked_layers: Vec<usize>,
    pub(crate) generated: u64,
    pub(crate) terminal_sources: u64,
}

struct Node {
    row: Vec<Handle>,
    parent: Option<usize>,
}

struct Search {
    native: Native,
    nodes: Vec<Node>,
    next: Vec<usize>,
    seen: BTreeSet<Vec<Handle>>,
}

impl Run {
    pub(crate) fn new() -> Self {
        Self {
            witness: Vec::new(),
            checked_layers: Vec::new(),
            generated: 0,
            terminal_sources: 0,
        }
    }

    pub(crate) fn check(&mut self, model: &Model, bound: usize, budget: &Budget) -> ResultKind {
        if let Some(failure) = assumptions(model) {
            return ResultKind::Failed(failure);
        }
        let mut search = match Native::prepare(model, budget) {
            Ok(native) => Search {
                native,
                nodes: Vec::new(),
                next: Vec::new(),
                seen: BTreeSet::new(),
            },
            Err(error) => return ResultKind::Failed(error.simulation()),
        };
        search
            .run(self, model, bound)
            .unwrap_or_else(ResultKind::Failed)
    }
}

impl Search {
    fn run(&mut self, run: &mut Run, model: &Model, bound: usize) -> Result<ResultKind, Outcome> {
        self.expand(run, model, None)?;
        for depth in 0..=bound {
            let current = std::mem::take(&mut self.next);
            run.checked_layers.push(current.len());
            if current.is_empty() {
                return Ok(ResultKind::ExecutionsTooShort);
            }
            if depth == bound {
                return Ok(ResultKind::ReachedBound);
            }
            // Equal interner rows at different depths must remain visitable.
            self.seen.clear();
            for id in current {
                let admitted = self.expand(run, model, Some(id))?;
                run.terminal_sources += u64::from(!admitted);
            }
        }
        Err(Outcome::Internal("bounded path layer accounting".into()))
    }

    fn expand(
        &mut self,
        run: &mut Run,
        model: &Model,
        parent: Option<usize>,
    ) -> Result<bool, Outcome> {
        let native = &mut self.native;
        if native.budget.poll() != Ok(None) {
            return Err(Outcome::Cancelled);
        }
        let machine = &mut native.machine;
        let plan = &native.plan;
        if let Some(source) = parent {
            machine.s0.clone_from(&self.nodes[source].row);
        } else {
            machine.s0.fill(Handle::EMPTY);
        }
        let actions = if parent.is_some() {
            &plan.next
        } else {
            &plan.init
        };
        let mut admitted = false;
        for action in actions {
            let mark = machine.heap.mark();
            let mut failure = None;
            let result = generate(machine, &plan.code, action, &mut |machine| {
                if machine.assigned != machine.s1.len() {
                    return Err(Error::unsupported("bounded path has an incomplete state"));
                }
                run.generated += 1;
                failure = roles(
                    machine,
                    plan,
                    &plan.invariants,
                    false,
                    "invariant",
                    if parent.is_some() { 2110 } else { 2107 },
                    2111,
                );
                if let Some(Outcome::Violation { .. }) = &failure {
                    let row = retain(machine)?;
                    run.witness
                        .push(values(machine, model, &row).map_err(Error::unsupported)?);
                    return Err(Error::stop());
                }
                if failure.is_some() {
                    return Err(Error::stop());
                }
                if allowed(machine, plan, &plan.constraints, false)?
                    && (parent.is_none() || allowed(machine, plan, &plan.action_constraints, true)?)
                {
                    admitted = true;
                    let row = retain(machine)?;
                    if self.seen.insert(row.clone()) {
                        self.next.push(self.nodes.len());
                        self.nodes.push(Node { row, parent });
                    }
                }
                Ok(())
            });
            if !machine.warnings.is_empty() || !machine.printed.is_empty() {
                return Err(Outcome::Unsupported(
                    "bounded path checking with runtime diagnostics".into(),
                ));
            }
            finish_batch(machine, mark).map_err(|error| classify(&error, None))?;
            if let Err(error) = result
                && (error.kind() != ErrorKind::Stop || failure.is_none())
            {
                return Err(classify(&error, None));
            }
            if let Some(failure) = failure {
                if !run.witness.is_empty() {
                    let mut at = parent;
                    while let Some(index) = at {
                        let node = &self.nodes[index];
                        run.witness
                            .push(values(machine, model, &node.row).map_err(Outcome::Unsupported)?);
                        at = node.parent;
                    }
                    run.witness.reverse();
                }
                return Err(failure);
            }
        }
        Ok(admitted)
    }
}
