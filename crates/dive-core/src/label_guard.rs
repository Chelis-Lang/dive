//! Lower pure, unprimed process-label tests without changing action order.
use crate::plan::{Action, Code, GNode, LabelTest, Node, P};
use crate::{Error, Handle, Machine};

pub(crate) fn lower(code: &mut Code, next: &[Action]) {
    for g in &mut code.gnodes {
        if let GNode::Guard(p) = *g
            && let Some(test) = LabelTest::recognize(&code.nodes, p)
        {
            *g = GNode::LabelGuard(p, test);
        }
    }
    let arms: Vec<_> = next
        .iter()
        .map(|a| a.g)
        .chain(
            code.gnodes
                .iter()
                .filter_map(|g| match g {
                    GNode::Disj(arms) => Some(arms.iter().copied()),
                    _ => None,
                })
                .flatten(),
        )
        .collect();
    for arm in arms {
        let mut first = arm;
        let test = loop {
            match &code.gnodes[first as usize] {
                GNode::Conj(parts) if !parts.is_empty() => first = parts[0].0,
                GNode::LabelGuard(_, test) | GNode::LabelArm(_, test) => break Some(*test),
                _ => break None,
            }
        };
        if let Some(test) = test
            && let GNode::Conj(parts) = &mut code.gnodes[arm as usize]
        {
            code.gnodes[arm as usize] = GNode::LabelArm(std::mem::take(parts), test);
        }
    }
}

impl LabelTest {
    pub(crate) fn recognize(nodes: &[Node], predicate: P) -> Option<Self> {
        let mut predicate = predicate;
        while let Node::Memo { body, .. } = nodes[predicate as usize] {
            predicate = body;
        }
        let Node::Eq(selector, literal) = nodes[predicate as usize] else {
            return None;
        };
        let Node::Const(label) = nodes[literal as usize] else {
            return None;
        };
        label.as_str_id()?;
        let (indexed, field) = match nodes[selector as usize] {
            Node::Select(value, field) => (value, field),
            _ => (selector, Handle::EMPTY),
        };
        let Node::Apply(table, index) = nodes[indexed as usize] else {
            return None;
        };
        let Node::Var(table) = nodes[table as usize] else {
            return None;
        };
        let Node::Slot(slot) = nodes[index as usize] else {
            return None;
        };
        Some(Self {
            table,
            slot,
            field,
            selector,
            label,
        })
    }
}

impl LabelTest {
    // The full identity is always compared, including on a bucket collision.
    fn bucket(self) -> usize {
        self.field.as_str_id().map_or(7, |f| f as usize % 8)
    }
}

impl Machine {
    pub(crate) fn known_label(&self, test: LabelTest, index: Handle) -> Option<Handle> {
        let &table = self.s0.get(usize::from(test.table))?;
        let [last_table, last_index, last_field, value] = self.heap.labels[test.bucket()];
        (!value.is_empty() && (table, index, test.field) == (last_table, last_index, last_field))
            .then_some(value)
    }

    pub(crate) fn inactive_split(&self, code: &Code, action: &Action) -> bool {
        let Some(GNode::LabelArm(_, test)) = code.gnodes.get(action.g as usize) else {
            return false;
        };
        if self.fuel != u64::MAX || self.debugger.is_some() || test.slot >= action.frame {
            return false;
        }
        action
            .preset
            .iter()
            .rev()
            .find(|(slot, _)| *slot == test.slot)
            .and_then(|(_, index)| self.known_label(*test, *index))
            .is_some_and(|value| value != test.label)
    }

    pub(crate) fn label_matches(
        &mut self,
        code: &Code,
        predicate: P,
        test: LabelTest,
    ) -> Result<bool, Error> {
        let table = self
            .s0
            .get(usize::from(test.table))
            .copied()
            .unwrap_or(Handle::EMPTY);
        let index = self.slot(test.slot);
        if let Some(value) = self.known_label(test, index) {
            return Ok(value == test.label);
        }
        // Persistent materialized tables survive Arena rollback. A ground
        // scalar index and a string result carry no retainable Arena data.
        if !table.is_persistent()
            || table.is_lazy_persistent()
            || index.is_compound()
            || index.is_empty()
        {
            return self.eval_bool(code, predicate);
        }
        let value = self.eval(code, test.selector)?;
        if value.as_str_id().is_none() {
            // Equality with an untyped model value or a differently typed
            // value uses the current profile's original comparison.
            return self.eval_bool(code, predicate);
        }
        self.heap.labels[test.bucket()] = [table, index, test.field, value];
        Ok(value == test.label)
    }
}
