//! Iteration programs for small, closed source domains.
use crate::heap::Heap;
use crate::node::Kind;
use crate::plan::{Bind, Code, Node, Quant};
use crate::{Error, Handle, Machine};

#[derive(Clone, Debug)]
pub(crate) struct Domain {
    slots: Box<[u32]>,
    rows: Box<[[Handle; 2]]>,
}

impl Domain {
    pub(crate) fn bind(heap: &Heap, c: &Code, binds: &[Bind]) -> Option<Self> {
        if binds.is_empty() || binds.len() > 2 {
            return None;
        }
        let mut slots = Vec::new();
        let mut rows = vec![[Handle::EMPTY; 2]];
        for b in binds {
            let [slot] = &*b.slots else { return None };
            if b.tuple || b.eligible.is_some() || slots.contains(slot) {
                return None;
            }
            let Node::Const(set) = c.nodes[b.domain as usize] else {
                return None;
            };
            if !set.is_persistent() || set.is_lazy_persistent() {
                return None;
            }
            let v = heap.view(set);
            if v.kind != Kind::Set
                || v.data.len() > 32
                || v.data.len() * rows.len() > 64
                || v.data
                    .iter()
                    .any(|h| h.is_compound() || !heap.proved_concrete(*h))
            {
                return None;
            }
            let mut next = Vec::with_capacity(v.data.len() * rows.len());
            for &val in v.data {
                for row in &rows {
                    let mut row = *row;
                    row[slots.len()] = val;
                    next.push(row);
                }
            }
            slots.push(*slot);
            rows = next;
        }
        Some(Self {
            slots: slots.into(),
            rows: rows.into(),
        })
    }

    pub(crate) fn each(
        &self,
        m: &mut Machine,
        mut visit: impl FnMut(&mut Machine) -> Result<bool, Error>,
    ) -> Result<bool, Error> {
        // Each domain already belongs to this bound Code. Charge before the
        // first visit too, so an empty Cartesian product still polls for work.
        m.charge(self.slots.len())?;
        for values in &self.rows {
            m.charge(1)?;
            for (&s, &v) in self.slots.iter().zip(values) {
                m.set_slot(s, v);
            }
            if !visit(m)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(crate) fn quant(
        &self,
        m: &mut Machine,
        c: &Code,
        kind: Quant,
        body: u32,
    ) -> Result<Handle, Error> {
        let want = matches!(kind, Quant::Forall);
        let completed = self.each(m, |m| Ok(m.eval_bool(c, body)? == want))?;
        Ok(Handle::bool(completed == want))
    }
}
