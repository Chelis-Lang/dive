//! Bind a two-dimensional table constructed from its old sets and guarded unions.
//! A fully checked false guard contributes the empty set. A true guard can add
//! one inline integer only when its original payload is exactly the recognised
//! singleton projection. Other cases use the original source expression.
use crate::compare;
use crate::heap::Heap;
use crate::node::{Kind, flags};
use crate::plan::{Code, GNode, Node, P, SetOp};
use crate::{Handle, Machine};

#[derive(Clone, Debug)]
struct Watch {
    variable: u16,
    field: Handle,
    sentinel: Handle,
    payload_key: Option<Handle>,
}

#[derive(Clone, Debug)]
pub(crate) struct History {
    old: u16,
    actors: Box<[Handle]>,
    paths: Box<[Handle]>,
    watch: Box<[Watch]>,
}

fn unwrap(c: &Code, mut p: P) -> Option<P> {
    while let Node::Spec(body) = *c.nodes.get(p as usize)? {
        p = body;
    }
    Some(p)
}

fn atom(c: &Code, p: P) -> Option<Handle> {
    match *c.nodes.get(unwrap(c, p)? as usize)? {
        Node::Const(h) => Some(h),
        _ => None,
    }
}

fn dimension(c: &Code, p: P) -> Option<(u32, Handle, P)> {
    let Node::FcnCons { binds, body } = &c.nodes[unwrap(c, p)? as usize] else {
        return None;
    };
    let [bind] = &**binds else { return None };
    let [slot] = &*bind.slots else { return None };
    if bind.tuple || bind.eligible.is_some() {
        return None;
    }
    Some((*slot, atom(c, bind.domain)?, *body))
}

fn at(c: &Code, p: P, s: u32) -> Option<P> {
    let Node::Apply(base, k) = c.nodes[unwrap(c, p)? as usize] else {
        return None;
    };
    matches!(c.nodes[k as usize],Node::Slot(slot) if slot==s).then_some(base)
}

fn watched(c: &Code, p: P, s: u32, k: u32) -> Option<(bool, u16, Handle)> {
    let Node::Select(base, field) = c.nodes[at(c, p, k)? as usize] else {
        return None;
    };
    match c.nodes[at(c, base, s)? as usize] {
        Node::Var(v) => Some((false, v, field)),
        Node::Primed(v) => Some((true, v, field)),
        _ => None,
    }
}

impl History {
    fn bind(c: &Code, heap: &Heap, p: P) -> Option<Self> {
        let (s, actors, inner) = dimension(c, p)?;
        let (k, paths, body) = dimension(c, inner)?;
        if s == k {
            return None;
        }
        let mut left = unwrap(c, body)?;
        let mut watch = Vec::new();
        while let Node::SetBin(SetOp::Cup, prev, cond) = c.nodes[left as usize] {
            if watch.len() == 4 {
                return None;
            }
            let Node::If(guard, payload, empty) = c.nodes[unwrap(c, cond)? as usize] else {
                return None;
            };
            let Node::And(parts) = &c.nodes[unwrap(c, guard)? as usize] else {
                return None;
            };
            let [nonempty, changed] = &**parts else {
                return None;
            };
            let Node::Neq(a, nil) = c.nodes[unwrap(c, *nonempty)? as usize] else {
                return None;
            };
            let Node::Neq(b, d) = c.nodes[unwrap(c, *changed)? as usize] else {
                return None;
            };
            let (true, v, field) = watched(c, a, s, k)? else {
                return None;
            };
            if watched(c, b, s, k)? != (true, v, field) || watched(c, d, s, k)? != (false, v, field)
            {
                return None;
            }
            let sentinel = atom(c, nil)?;
            let empty = atom(c, empty)?;
            if sentinel.is_empty()
                || sentinel.is_compound()
                || !empty.is_persistent()
                || empty.is_lazy_persistent()
            {
                return None;
            }
            let empty = heap.view(empty);
            if empty.kind != Kind::Set || !empty.data.is_empty() {
                return None;
            }
            let payload_key = (|| {
                let Node::SetEnum(items) = &c.nodes[unwrap(c, payload)? as usize] else {
                    return None;
                };
                let [item] = &**items else { return None };
                let Node::Apply(source, key) = c.nodes[unwrap(c, *item)? as usize] else {
                    return None;
                };
                if watched(c, source, s, k)? != (true, v, field) {
                    return None;
                }
                let key = atom(c, key)?;
                key.as_int().map(|_| key)
            })();
            watch.push(Watch {
                variable: v,
                field,
                sentinel,
                payload_key,
            });
            left = unwrap(c, prev)?;
        }
        if watch.is_empty() {
            return None;
        }
        let Node::Var(old) = c.nodes[at(c, at(c, left, k)?, s)? as usize] else {
            return None;
        };
        let domains = |h: Handle| {
            if !h.is_persistent() || h.is_lazy_persistent() {
                return None;
            }
            let v = heap.view(h);
            if v.kind != Kind::Set
                || v.flags & flags::VIEW != 0
                || v.data.is_empty()
                || v.data.len() > 16
            {
                return None;
            }
            let first = *v.data.first()?;
            let names = first.as_model_id().and_then(|id| heap.model_type(id));
            // The original domain must be a normalized, comparable ground set.
            if !v.data.iter().all(|a| {
                heap.proved_concrete(*a)
                    && ((first.as_str_id().is_some() && a.as_str_id().is_some())
                        || (first.as_model_id().is_some()
                            && a.as_model_id()
                                .is_some_and(|id| heap.model_type(id) == names)))
            }) {
                return None;
            }
            Some(Box::<[Handle]>::from(v.data))
        };
        let (actors, paths) = (domains(actors)?, domains(paths)?);
        Some(Self {
            old,
            actors,
            paths,
            watch: watch.into(),
        })
    }

    pub(crate) fn sites(c: &Code, heap: &Heap) -> Vec<Option<Box<Self>>> {
        let mut plans = vec![None; c.sites.len()];
        for g in &c.gnodes {
            if let GNode::Assign { rhs, .. } = g
                && let Node::Memo { site, body } = c.nodes[*rhs as usize]
                && let Some(h) = Self::bind(c, heap, body)
            {
                plans[site as usize] = Some(Box::new(h));
            }
        }
        plans
    }

    pub(crate) fn update(&self, m: &mut Machine) -> Option<(Handle, bool)> {
        let heap = &m.heap;
        let (actors, paths) = (&*self.actors, &*self.paths);
        let table = |h: Handle, domain: &[Handle]| {
            if !h.is_compound() {
                return None;
            }
            let v = heap.view(h);
            (v.kind == Kind::Fcn && !v.is_lazy() && v.keys() == domain)
                .then_some(&v.data[v.len()..])
        };
        let canon = |h: Handle| h.is_persistent() && !h.is_lazy_persistent();
        let old = *m.s0.get(usize::from(self.old))?;
        if !canon(old) {
            return None;
        }
        let oldrows = table(old, actors)?;
        for &row in oldrows {
            if !canon(row) {
                return None;
            }
            for &cell in table(row, paths)? {
                if !canon(cell) {
                    return None;
                }
                let v = heap.view(cell);
                if v.kind != Kind::Set
                    || v.data.len() > 32
                    || !v.data.iter().all(|h| h.as_int().is_some())
                {
                    return None;
                }
            }
        }
        // Prove operands and collect only changed integer set cells. No
        // allocation in the success path for a completely unchanged table.
        let mut additions = Vec::new();
        for w in &self.watch {
            let current = table(*m.s0.get(usize::from(w.variable))?, actors)?;
            let next = table(*m.s1.get(usize::from(w.variable))?, actors)?;
            for (i, (&a, &b)) in current.iter().zip(next).enumerate() {
                let before = table(m.native_entry(a, w.field, true)?.1, paths)?;
                let after = table(m.native_entry(b, w.field, true)?.1, paths)?;
                for (j, (&x, &y)) in before.iter().zip(after).enumerate() {
                    // The demands of both source comparisons must succeed.
                    // Symbolic normalization is left to the original RHS.
                    if (x.is_compound() && heap.view(x).kind.is_symbolic())
                        || (y.is_compound() && heap.view(y).kind.is_symbolic())
                    {
                        return None;
                    }
                    if !compare::equals(heap, y, w.sentinel).ok()?
                        && !compare::equals(heap, y, x).ok()?
                    {
                        let key = w.payload_key?;
                        let val = m.native_entry(y, key, false)?.1;
                        val.as_int()?;
                        let old_cell = table(oldrows[i], paths)?[j];
                        if !heap.view(old_cell).data.contains(&val) {
                            additions.push((i, j, val))
                        }
                    }
                }
            }
        }
        let changed = !additions.is_empty();
        let mut result = old;
        for (i, j, add) in additions {
            let row = m.heap.view(result).values()[i];
            let cell = m.heap.view(row).values()[j];
            let mut values = m.heap.view(cell).data.to_vec();
            values.push(add);
            let new = m.heap.set_from(&mut values);
            let row = m.heap.fcn_with(row, j, new);
            result = m.heap.fcn_with(result, i, row);
        }
        Some((result, changed))
    }
}
