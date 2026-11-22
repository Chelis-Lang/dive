//! Execute a compiled deterministic prefix without a continuation per assignment.
use super::{Code, Error, G, Handle, Item, Machine, P, Sink, row_slices};
use crate::plan::{GNode, LabelTest};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Op {
    Guard(P),
    Label(P, LabelTest),
    Write(u16, P),
    Copy(u64),
}
pub(crate) fn compile(node: &GNode, code: &Code) -> Box<[Op]> {
    let (GNode::Conj(items) | GNode::LabelArm(items, _)) = node else {
        return Box::default();
    };
    let ops: Vec<_> = items
        .iter()
        .map_while(|&(g, _)| match code.gnodes.get(g as usize)? {
            GNode::Guard(p) => Some(Op::Guard(*p)),
            GNode::LabelGuard(p, label) => Some(Op::Label(*p, *label)),
            GNode::Assign { var, rhs } => Some(Op::Write(*var, *rhs)),
            GNode::Unchanged(_) => code
                .unchanged_masks
                .get(g as usize)
                .copied()
                .filter(|&m| m != 0)
                .map(Op::Copy),
            _ => None,
        })
        .collect();
    if ops.len() < 2 {
        Box::default()
    } else {
        ops.into()
    }
}
impl Machine {
    pub(super) fn copy_columns(&mut self, mask: u64) -> bool {
        if !row_slices(mask).all(|range| {
            self.s1
                .get(range.clone())
                .is_some_and(|v| v.iter().all(|h| h.is_empty()))
                && self
                    .s0
                    .get(range)
                    .is_some_and(|v| v.iter().all(|h| !h.is_empty()))
        }) {
            return false;
        }
        for range in row_slices(mask) {
            self.s1[range.clone()].copy_from_slice(&self.s0[range]);
        }
        self.assigned += mask.count_ones() as usize;
        true
    }
    pub(super) fn clear_columns(&mut self, mask: u64) {
        for range in row_slices(mask) {
            self.s1[range].fill(Handle::EMPTY);
        }
        self.assigned -= mask.count_ones() as usize;
    }
    pub(super) fn plain_conjunction(
        &mut self,
        c: &Code,
        items: &[(G, P)],
        acts: u32,
        sink: &mut dyn Sink,
    ) -> Result<(), Error> {
        let Some((&(first, _), rest)) = items.split_first() else {
            return self.cont(c, acts, sink);
        };
        let mark = self.conts.len();
        let mut head = acts;
        for &(g, p) in rest.iter().rev() {
            head = self.push_cont(Item::Conj(g, p), head);
        }
        let result = self.gen_node(c, first, head, sink);
        self.conts.truncate(mark);
        result
    }
    pub(super) fn linear_conjunction(
        &mut self,
        c: &Code,
        items: &[(G, P)],
        ops: &[Op],
        acts: u32,
        sink: &mut dyn Sink,
    ) -> Result<(), Error> {
        let mut written = 0;
        let result = (|| {
            for (i, &(_, predicate)) in items.iter().enumerate() {
                if i != 0 && self.warn && self.assigned == self.s1.len() {
                    if !self.eval_bool(c, predicate)? {
                        return Ok(());
                    }
                    continue;
                }
                match ops.get(i) {
                    Some(Op::Guard(p)) => {
                        if !self.eval_bool(c, *p)? {
                            return Ok(());
                        }
                    }
                    Some(Op::Label(p, label)) => {
                        if !self.label_matches(c, *p, *label)? {
                            return Ok(());
                        }
                    }
                    Some(Op::Write(var, p)) => {
                        let v = self.eval_component(c, *p)?;
                        let cur = self.s1[usize::from(*var)];
                        if cur.is_empty() {
                            self.bind_var(*var, v);
                            written |= 1u64 << var;
                        } else if !self.demand_equals(cur, v)? {
                            return Ok(());
                        }
                    }
                    Some(Op::Copy(mask)) if self.copy_columns(*mask) => {
                        written |= mask;
                    }
                    _ => return self.plain_conjunction(c, &items[i..], acts, sink),
                }
            }
            self.cont(c, acts, sink)
        })();
        self.clear_columns(written);
        result
    }
}
