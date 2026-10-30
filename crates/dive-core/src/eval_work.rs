//! Finite optional query work using the existing set cursors. Ordinary
//! unbounded machines retain their existing enumeration/materialization path.

use super::{Machine, Unit};
use crate::Error;
use crate::Handle;
use crate::heap::Cursor;
use crate::node::Kind;
use crate::plan::{Bind, Code};

fn quota() -> Error {
    Error::unsupported("the optional domain materialization budget")
}

impl Machine {
    /// Preflight every allocation made by Heap cursor construction/norm before
    /// calling that unmetered primitive. Large intervals can stream at the top;
    /// nested product components are materialized by Heap and must fit first.
    pub(crate) fn domain_work(
        &mut self,
        h: Handle,
        materialize: bool,
        depth: u8,
    ) -> Result<usize, Error> {
        self.charge(1)?;
        if depth >= 64 || !h.is_compound() {
            return Err(quota());
        }
        let v = self.heap.view(h);
        let (kind, lo, len) = (v.kind, v.lo, v.data.len());
        match kind {
            Kind::Interval => {
                let hi = v.data.first().and_then(|h| h.as_int()).ok_or_else(quota)?;
                let count = if hi < lo {
                    0
                } else {
                    hi.checked_sub(lo)
                        .and_then(|n| n.checked_add(1))
                        .and_then(|n| usize::try_from(n).ok())
                        .ok_or_else(quota)?
                };
                if materialize {
                    self.charge(count.checked_mul(3).ok_or_else(quota)?)?;
                }
                Ok(count)
            }
            Kind::Set | Kind::SetEnum => {
                self.charge(len.checked_mul(3).ok_or_else(quota)?)?;
                Ok(len)
            }
            Kind::Product => {
                self.charge(len)?;
                let children = self.heap.view(h).data.to_vec();
                let mut count = 1usize;
                for child in children {
                    let size = self.domain_work(child, true, depth + 1)?;
                    count = count.checked_mul(size).ok_or_else(quota)?;
                }
                if materialize {
                    self.charge(
                        count
                            .checked_mul(len.checked_add(3).ok_or_else(quota)?)
                            .ok_or_else(quota)?,
                    )?;
                }
                Ok(count)
            }
            // These constructors need different eager Heap work bounds. They
            // stay on reference fallback before invoking any such primitive.
            _ => Err(Error::unsupported(
                "unproved optional domain materialization shape",
            )),
        }
    }

    /// Central bounded boundary for every interpreter/library normalization.
    pub(crate) fn work_norm(&mut self, h: Handle) -> Result<Handle, Error> {
        if self.fuel != u64::MAX && h.is_compound() && self.heap.view(h).kind.is_symbolic() {
            self.domain_work(h, true, 0)?;
        }
        self.heap.norm(h)
    }

    /// Central bounded boundary for every eager interpreter/library enumeration.
    pub(crate) fn work_elements(&mut self, h: Handle) -> Result<Vec<Handle>, Error> {
        if self.fuel != u64::MAX {
            self.domain_work(h, true, 0)?;
        }
        self.heap.elements(h)
    }

    pub(crate) fn work_cardinality(&mut self, h: Handle) -> Result<u64, Error> {
        if self.fuel != u64::MAX
            && h.is_compound()
            && !matches!(self.heap.view(h).kind, Kind::Interval | Kind::Set)
        {
            self.domain_work(h, true, 0)?;
        }
        self.heap.cardinality(h)
    }

    pub(crate) fn work_materialize_cached(&mut self, h: Handle) -> Result<Handle, Error> {
        if self.fuel == u64::MAX {
            return self.heap.materialize_cached(h);
        }
        self.domain_work(h, true, 0)?;
        // Optional analysis must not intern a normalized result in the producer
        // Store. Its isolated arena owns this temporary projection.
        self.heap.materialize(h)
    }

    pub(crate) fn work_cursor(&mut self, h: Handle) -> Result<Cursor, Error> {
        if self.fuel != u64::MAX {
            self.domain_work(h, false, 0)?;
        }
        self.heap.cursor(h)
    }

    pub(crate) fn work_next(&mut self, cursor: &mut Cursor) -> Result<Option<Handle>, Error> {
        let work = match cursor {
            Cursor::Odometer { comps, shape, .. } => {
                comps.len()
                    + match shape {
                        crate::heap::Shape::Tuple => 1,
                        crate::heap::Shape::Fcn(keys) => keys.len() + 1,
                    }
            }
            Cursor::Subset { elems, .. } => elems.len() + 1,
            Cursor::Range { .. } | Cursor::Array { .. } => 1,
        };
        self.charge(work)?;
        cursor.next(&mut self.heap)
    }

    pub(crate) fn binding_stream(
        &mut self,
        code: &Code,
        binds: &[Bind],
        f: &mut dyn FnMut(&mut Self) -> Result<bool, Error>,
    ) -> Result<bool, Error> {
        let mut units = Vec::new();
        let mut values = Vec::new();
        for (bi, bind) in binds.iter().enumerate() {
            self.charge(1)?;
            let domain = self.eval(code, bind.domain)?;
            let width = if bind.tuple { 1 } else { bind.slots.len() };
            self.charge(width.checked_mul(3).ok_or_else(quota)?)?;
            for vi in 0..width {
                let cursor = self.work_cursor(domain)?;
                units.push((
                    Unit {
                        bind: bi,
                        var: (!bind.tuple).then_some(vi),
                    },
                    domain,
                    cursor,
                ));
            }
        }
        // Evaluate/build every original domain before recognizing an empty
        // one, preserving the existing error/demand order.
        for (_, _, cursor) in &mut units {
            let Some(value) = self.work_next(cursor)? else {
                return Ok(true);
            };
            values.push(value);
        }
        loop {
            self.charge(units.len() + 1)?;
            for ((unit, _, _), &value) in units.iter().zip(&values) {
                self.bind_unit(binds, *unit, value)?;
            }
            if !f(self)? {
                return Ok(false);
            }
            // TLC binding order: the first variable advances fastest. Reset a
            // cursor under the quota again; never clone a large product buffer.
            let mut index = 0;
            loop {
                if index == units.len() {
                    return Ok(true);
                }
                if let Some(next) = self.work_next(&mut units[index].2)? {
                    values[index] = next;
                    break;
                }
                if index + 1 == units.len() {
                    return Ok(true);
                }
                units[index].2 = self.work_cursor(units[index].1)?;
                values[index] = self.work_next(&mut units[index].2)?.ok_or_else(quota)?;
                index += 1;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, reason = "bounded primitive regression inputs")]
mod tests {
    use super::*;
    use crate::plan::Node;
    use crate::{Opts, Store};
    use std::sync::Arc;

    #[test]
    fn large_materialization_and_nested_product_components_decline_before_allocation() {
        let mut machine = Machine::new(
            Arc::new(Store::default()),
            1,
            Opts::default(),
            crate::memo::Memo::default(),
        );
        let large = machine.heap.interval(1, 1_000_000_000);
        let small = machine.heap.interval(1, 2);
        let product = machine.heap.symbolic(Kind::Product, 0, &[small, large]);
        let nested = machine.heap.symbolic(Kind::Product, 0, &[product, small]);
        let code = Code {
            nodes: vec![Node::Const(large)],
            ..Code::default()
        };
        let binds = [Bind {
            slots: vec![0].into(),
            tuple: false,
            domain: 0,
        }];
        machine.set_fuel(4096);
        assert!(
            machine.eval_norm(&code, 0).is_err(),
            "candidate value normalization is also preflighted"
        );
        assert_eq!(
            machine.fuel, 0,
            "preflight quota denied the payload before Heap normalization"
        );

        for domain in [large, product, nested] {
            machine.set_fuel(4096);
            let before = machine.heap.arena().peak();
            if domain == large {
                let mut cursor = machine.work_cursor(domain).expect("large interval streams");
                assert_eq!(
                    machine.work_next(&mut cursor).expect("first"),
                    Handle::int(1)
                );
                assert!(
                    machine.units(&code, &binds).is_err(),
                    "eager materialization is preflighted"
                );
            } else {
                assert!(
                    machine.work_cursor(domain).is_err(),
                    "large nested component is never materialized"
                );
            }
            assert_eq!(
                machine.heap.arena().peak(),
                before,
                "preflight creates no heap payload"
            );
        }
    }

    #[test]
    fn small_product_stream_preserves_order_and_empty_domain_error_demand() {
        let mut machine = Machine::new(
            Arc::new(Store::default()),
            1,
            Opts::default(),
            crate::memo::Memo::default(),
        );
        let a = machine.heap.interval(1, 2);
        let b = machine.heap.interval(3, 4);
        let product = machine.heap.symbolic(Kind::Product, 0, &[a, b]);
        let expected = machine.heap.elements(product).expect("ordinary product");
        machine.set_fuel(4096);
        let mut cursor = machine.work_cursor(product).expect("bounded small product");
        let mut got = Vec::new();
        while let Some(element) = machine.work_next(&mut cursor).expect("next") {
            got.push(element);
        }
        assert_eq!(got.len(), expected.len());
        for (actual, expected) in got.into_iter().zip(expected) {
            assert!(machine.heap.eq(actual, expected));
        }
        let empty = machine.heap.interval(2, 1);
        let code = Code {
            nodes: vec![Node::Const(empty), Node::Const(Handle::TRUE)],
            ..Code::default()
        };
        let binds = [
            Bind {
                slots: vec![0].into(),
                tuple: false,
                domain: 0,
            },
            Bind {
                slots: vec![1].into(),
                tuple: false,
                domain: 1,
            },
        ];
        let frame = machine.enter(2);
        assert!(
            machine
                .for_each_binding(&code, &binds, &mut |_| Ok(true))
                .is_err(),
            "the second original domain is still evaluated/validated"
        );
        machine.leave(frame);
    }
    #[test]
    fn admitted_higher_order_sequence_copies_are_charged_before_allocation() {
        use crate::stdlib::{Ho, OpFn};
        let mut machine = Machine::new(
            Arc::new(Store::default()),
            0,
            Opts::default(),
            crate::memo::Memo::default(),
        );
        let huge = machine.heap.tuple(&vec![Handle::TRUE; 10000]);
        let small = machine.heap.tuple(&[Handle::TRUE, Handle::TRUE]);
        let code = Code {
            nodes: vec![Node::Const(Handle::FALSE)],
            ..Code::default()
        };
        let frame = machine.enter(2);
        for (op, params) in [
            (dive_ir::StdOp::TlcSortSeq, vec![0, 1]),
            (dive_ir::StdOp::SequencesSelectSeq, vec![0]),
        ] {
            machine.set_fuel(4096);
            let before = machine.heap.arena().peak();
            let function = OpFn::new(&code, &params, 0);
            assert!(
                crate::stdlib::call_ho(&mut machine, op, &[Ho::Value(huge), Ho::Op(function)])
                    .is_err()
            );
            assert_eq!(
                machine.fuel, 0,
                "copy is denied before allocating its vector"
            );
            assert_eq!(machine.heap.arena().peak(), before);
            machine.set_fuel(4096);
            assert!(
                crate::stdlib::call_ho(&mut machine, op, &[Ho::Value(small), Ho::Op(function)])
                    .is_ok()
            );
        }
        machine.leave(frame);
    }
    #[test]
    fn utf16_len_precharges_a_constant_time_utf8_bound() {
        let mut machine = Machine::new(
            Arc::new(Store::default()),
            0,
            Opts::default(),
            crate::memo::Memo::default(),
        );
        let long = machine
            .heap
            .string(&"a".repeat(10000))
            .expect("owned long name");
        machine.set_fuel(4096);
        assert!(machine.len(long).is_err());
        assert_eq!(machine.fuel, 0, "denied before UTF16 traversal");
        for (text, len) in [("", 0), ("a", 1), ("é😀", 3)] {
            let value = machine.heap.string(text).expect("owned name");
            machine.set_fuel(4096);
            assert_eq!(
                machine.len(value).expect("UTF16 length").as_int(),
                Some(len)
            );
            assert_eq!(machine.fuel, 4096 - text.len() as u64);
        }
    }
}
