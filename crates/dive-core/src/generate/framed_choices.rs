//! Exact narrowing of a function-set cursor before an all-assigned
//! continuation. A frame fixes every key except one. Testing the earlier
//! guard on all writable values proves that skipping the rejected products
//! cannot hide evaluation errors.

use dive_ir::StdOp;

use crate::eval::Machine;
use crate::handle::Handle;
use crate::heap::{Cursor, Shape, fcn_key};
use crate::node::{Kind, flags};
use crate::plan::{Bind, Code, Node, P, Quant, SetOp};
use crate::{Error, compare};

use super::Item;

/// Bounds the optional proof by finite coordinates and cursor elements.
const MAX_KEYS: usize = 32;
const MAX_CHOICES: usize = 256;

fn node(code: &Code, mut p: P) -> Option<&Node> {
    for _ in 0..8 {
        match code.nodes.get(p as usize)? {
            Node::Memo { body, .. } | Node::Spec(body) => p = *body,
            Node::Scope { reset, body } if reset.is_empty() => p = *body,
            other => return Some(other),
        }
    }
    None
}

fn applied(code: &Code, p: P) -> Option<(&Node, P)> {
    let Node::Apply(function, key) = node(code, p)? else {
        return None;
    };
    Some((node(code, *function)?, *key))
}

fn frame_domain(code: &Code, p: P, var: u16) -> Option<P> {
    let Node::Quant {
        kind: Quant::Forall,
        binds,
        body,
    } = node(code, p)?
    else {
        return None;
    };
    let [
        Bind {
            slots,
            tuple: false,
            domain,
            ..
        },
    ] = binds.as_ref()
    else {
        return None;
    };
    let [slot] = slots.as_ref() else { return None };
    let Node::Eq(left, right) = node(code, *body)? else {
        return None;
    };
    let (Node::Primed(next), left_key) = applied(code, *left)? else {
        return None;
    };
    let (Node::Var(old), right_key) = applied(code, *right)? else {
        return None;
    };
    (var == *next
        && var == *old
        && matches!(node(code, left_key), Some(Node::Slot(key)) if key == slot)
        && matches!(node(code, right_key), Some(Node::Slot(key)) if key == slot))
    .then_some(*domain)
}

impl Machine {
    fn closed_slot(&self, code: &Code, p: P) -> Option<Handle> {
        let value = match node(code, p)? {
            Node::Const(value) => *value,
            Node::Slot(slot) => self.slot(*slot),
            _ => return None,
        };
        (!value.is_empty()).then_some(value)
    }

    fn small_key_set(&self, value: Handle) -> Option<&[Handle]> {
        if self.heap.kind(value) != Some(Kind::Set) {
            return None;
        }
        let view = self.heap.view(value);
        (view.data.len() <= MAX_KEYS
            && view
                .data
                .iter()
                .all(|key| !key.is_compound() && !key.is_empty()))
        .then_some(view.data)
    }

    /// Admit only a small, closed set or a difference of such a set
    /// and a singleton lexical value. Evaluate the actual expression before
    /// relying on its membership, including its normal comparison errors.
    fn small_frame_expression(&self, code: &Code, domain: P) -> Option<()> {
        if let Some(value) = self.closed_slot(code, domain) {
            return self.small_key_set(value).map(|_| ());
        }
        let Node::SetBin(SetOp::Minus, all, except) = node(code, domain)? else {
            return None;
        };
        self.small_key_set(self.closed_slot(code, *all)?)?;
        let Node::SetEnum(except) = node(code, *except)? else {
            return None;
        };
        let [except] = except.as_ref() else {
            return None;
        };
        let excluded = self.closed_slot(code, *except)?;
        (!excluded.is_compound()).then_some(())
    }

    fn writable_key(&mut self, code: &Code, domain: P, keys: &[Handle]) -> Option<usize> {
        self.small_frame_expression(code, domain)?;
        let fixed = self.eval(code, domain).ok()?;
        let fixed = self.small_key_set(fixed)?;
        if fixed.len() + 1 != keys.len() {
            return None;
        }
        let write = keys.iter().position(|key| !fixed.contains(key))?;
        keys.iter()
            .enumerate()
            .filter(|(index, _)| *index != write)
            .map(|(_, &key)| key)
            .eq(fixed.iter().copied())
            .then_some(write)
    }

    fn prefix_key(&self, code: &Code, p: P, var: u16) -> Option<Handle> {
        let Node::Lib {
            op: StdOp::SequencesExtIsPrefix,
            args,
            ..
        } = node(code, p)?
        else {
            return None;
        };
        let [left, right] = args.as_ref() else {
            return None;
        };
        let (Node::Var(old), old_key) = applied(code, *left)? else {
            return None;
        };
        if *old != var || self.closed_slot(code, old_key)?.is_compound() {
            return None;
        }
        let (Node::Primed(next), new_key) = applied(code, *right)? else {
            return None;
        };
        (*next == var).then_some(())?;
        let key = self.closed_slot(code, new_key)?;
        (!key.is_compound()).then_some(key)
    }

    pub(super) fn restrict_function_choices(
        &mut self,
        code: &Code,
        var: u16,
        acts: u32,
        cursor: &mut Cursor,
    ) -> Result<(), Error> {
        let Cursor::Odometer {
            shape: Shape::Fcn(keys),
            comps,
            done,
            ..
        } = cursor
        else {
            return Ok(());
        };
        if *done
            || !(2..=MAX_KEYS).contains(&keys.len())
            || !(4..=MAX_CHOICES).contains(&comps[0].len())
            || keys.iter().any(|key| key.is_compound())
        {
            return Ok(());
        }
        let Some((domain, prefix)) = self.find_choice_frame(code, var, acts) else {
            return Ok(());
        };
        let old = self.s0[usize::from(var)];
        if !matches!(self.heap.kind(old), Some(Kind::Fcn | Kind::IntFcn)) {
            return Ok(());
        }
        let prev = self.heap.view(old);
        if prev.flags & flags::VIEW != 0
            || prev.len() != keys.len()
            || keys
                .iter()
                .enumerate()
                .any(|(i, &key)| fcn_key(&prev, i) != key)
        {
            return Ok(());
        }

        let previous = prev.values().to_vec();
        let scratch = self.heap.mark();
        let speculative = self.heap.set_speculative(true);
        let narrowed = (|| {
            let write = self.writable_key(code, domain, keys)?;
            if prefix.is_some_and(|p| self.prefix_key(code, p, var) != Some(keys[write])) {
                return None;
            }
            self.prove_choice_frame(code, var, keys, comps, write, &previous, prefix)
                .ok()
                .flatten()
        })();
        self.heap.rollback(scratch);
        self.heap.set_speculative(speculative);
        if self.heap.work_interrupted() {
            return Err(Error::stop());
        }
        if let Some(fixed) = narrowed {
            *comps = fixed;
            *done = comps.iter().any(Vec::is_empty);
        }
        Ok(())
    }

    fn find_choice_frame(&self, code: &Code, var: u16, acts: u32) -> Option<(P, Option<P>)> {
        let mut current = self.conts.get(acts as usize)?;
        let Item::Conj(_, first) = current.item else {
            return None;
        };
        if let Some(domain) = frame_domain(code, first, var) {
            return Some((domain, None));
        }
        self.prefix_key(code, first, var)?;
        current = self.conts.get(current.next as usize)?;
        let Item::Conj(_, second) = current.item else {
            return None;
        };
        Some((frame_domain(code, second, var)?, Some(first)))
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the preflight needs the original cursor and continuation coordinates"
    )]
    fn prove_choice_frame(
        &mut self,
        code: &Code,
        var: u16,
        keys: &[Handle],
        comps: &[Vec<Handle>],
        write: usize,
        previous: &[Handle],
        prefix: Option<P>,
    ) -> Result<Option<Vec<Vec<Handle>>>, Error> {
        // Materialized operands keep the equality proof free of lazy demand
        // and let it use the exact comparison of the original frame.
        for &value in previous.iter().chain(&comps[write]) {
            if !compare::materialized(&self.heap, value, 0)? {
                return Ok(None);
            }
        }
        if let Some(prefix) = prefix {
            let mut row: Vec<_> = comps.iter().map(|values| values[0]).collect();
            for &value in &comps[write] {
                let scratch = self.heap.mark();
                row[write] = value;
                let candidate = self.heap.fcn_sorted(keys, &row);
                self.bind_var(var, candidate);
                let tested = self.eval_bool(code, prefix);
                self.unbind_var(var);
                self.heap.rollback(scratch);
                tested?;
            }
        }
        let mut fixed = Vec::with_capacity(comps.len());
        for (index, (choices, &old)) in comps.iter().zip(previous).enumerate() {
            if index == write {
                fixed.push(choices.clone());
                continue;
            }
            let mut selected = Vec::new();
            for &value in choices {
                self.charge(1)?;
                if self.demand_equals(value, old)? {
                    selected.push(value);
                }
            }
            fixed.push(selected);
        }
        Ok(Some(fixed))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, reason = "fixed test handles and node plans")]

    use std::sync::Arc;

    use crate::generate::Sink;
    use crate::plan::GNode;
    use crate::{Opts, Store, memo::Memo};

    use super::*;

    #[derive(Default)]
    struct Rows(Vec<Vec<Handle>>);

    impl Sink for Rows {
        fn exhaustive(&self) -> bool {
            true
        }

        fn emit(&mut self, m: &mut Machine, _: &Code) -> Result<(), Error> {
            self.0.push(m.heap.view(m.s1[0]).values().to_vec());
            Ok(())
        }
    }

    fn integer(value: i64) -> Handle {
        Handle::int(value).expect("small integer")
    }

    fn fixture(values: &[i64], enabled: bool) -> (Machine, Code) {
        let opts = Opts {
            framed_function_choices: enabled,
            ..Opts::default()
        };
        let mut m = Machine::new(Arc::new(Store::default()), 1, opts, Memo::default());
        m.enter(1);
        let keys = [integer(0), integer(1), integer(2)];
        let range = [integer(0), integer(1), integer(2), integer(3)];
        let old: Vec<_> = values.iter().copied().map(integer).collect();
        m.s0[0] = m.heap.fcn_sorted(&keys[..old.len()], &old);
        let code = Code {
            nodes: vec![
                Node::Const(m.heap.set_sorted(&keys)),
                Node::Const(m.heap.set_sorted(&range)),
                Node::FcnSet(0, 1),
                Node::Primed(0),
                Node::Var(0),
                Node::Slot(0),
                Node::Apply(3, 5),
                Node::Apply(4, 5),
                Node::Eq(6, 7),
                Node::Const(m.heap.set_sorted(&[keys[0], keys[2]])),
                Node::Quant {
                    kind: Quant::Forall,
                    binds: Box::new([Bind {
                        slots: Box::new([0]),
                        tuple: false,
                        domain: 9,
                        eligible: None,
                    }]),
                    body: 8,
                },
            ],
            gnodes: vec![
                GNode::AssignIn { var: 0, set: 2 },
                GNode::Guard(10),
                GNode::Conj(Box::new([(0, 2), (1, 10)])),
                GNode::Disj(Box::new([2, 2])),
            ],
            ..Code::default()
        };
        (m, code)
    }

    #[test]
    fn framed_choice_cursor_keeps_the_original_order_and_multiplicity() -> Result<(), Error> {
        let (mut m, code) = fixture(&[1, 0, 2], true);
        let set = m.eval(&code, 2)?;
        let mut cursor = m.work_cursor(set)?;
        let tail = m.push_cont(Item::Conj(1, 10), super::super::NONE);
        m.restrict_function_choices(&code, 0, tail, &mut cursor)?;
        let mut output = Vec::new();
        while let Some(row) = m.work_next(&mut cursor)? {
            output.push(m.heap.view(row).values().to_vec());
        }
        let expected: Vec<_> = (0..4)
            .map(|value| vec![integer(1), integer(value), integer(2)])
            .collect();
        assert_eq!(output, expected);
        for enabled in [false, true] {
            let (mut m, code) = fixture(&[1, 0, 2], enabled);
            let mut rows = Rows::default();
            m.run_action(&code, 3, &mut rows)?;
            assert!(rows.0.iter().eq(expected.iter().chain(&expected)));
        }
        Ok(())
    }

    #[test]
    fn framed_choice_cursor_handles_out_of_range_and_missing_coordinates() -> Result<(), Error> {
        for enabled in [false, true] {
            let (mut m, code) = fixture(&[4, 0, 2], enabled);
            let mut rows = Rows::default();
            m.run_action(&code, 3, &mut rows)?;
            assert_eq!(rows.0, [] as [Vec<Handle>; 0]);
            let (mut m, code) = fixture(&[1, 0], enabled);
            let error = m
                .run_action(&code, 3, &mut rows)
                .expect_err("the original frame reads an unbound key");
            assert_eq!(error.kind(), crate::ErrorKind::Eval);
            assert_eq!(rows.0, [] as [Vec<Handle>; 0]);
        }
        Ok(())
    }

    #[derive(Debug)]
    struct Interrupt;

    impl crate::eval::WorkControl for Interrupt {
        fn proceed(&self) -> bool {
            false
        }
    }

    #[test]
    fn interrupted_preflight_cannot_narrow_the_cursor() -> Result<(), Error> {
        let (mut m, code) = fixture(&[1, 0, 2], true);
        let set = m.eval(&code, 2)?;
        let mut cursor = m.work_cursor(set)?;
        let tail = m.push_cont(Item::Conj(1, 10), super::super::NONE);
        m.set_work_control(Some(Arc::new(Interrupt)));
        let error = m
            .restrict_function_choices(&code, 0, tail, &mut cursor)
            .expect_err("real budget stop");
        assert_eq!(error.kind(), crate::ErrorKind::Stop);
        let Cursor::Odometer { comps, .. } = cursor else {
            return Err(Error::internal("expected original odometer"));
        };
        assert!(comps.iter().all(|choices| choices.len() == 4));
        assert_eq!(m.s1, vec![Handle::EMPTY]);
        Ok(())
    }
}
