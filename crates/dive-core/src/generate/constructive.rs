//! Indexed constructive Next traversal; ordinary evaluator queries stay separate.

use num_traits::ToPrimitive;

use super::Sink;
use crate::eval::{Machine, Unit};
use crate::node::Kind;
use crate::plan::{Bind, Code, G};
use crate::{Error, Handle};

struct IndexOrder {
    len: u64,
    start: u64,
    index: u64,
    stride: u64,
    remaining: u64,
}

impl IndexOrder {
    fn new(len: u64, sink: &mut dyn Sink) -> Result<Self, Error> {
        let start = if len == 0 { 0 } else { draw(len, sink)? };
        let stride = if len <= 1 {
            1
        } else {
            let proposed = draw(len - 1, sink)? + 1;
            if gcd(proposed, len) == 1 { proposed } else { 1 }
        };
        Ok(Self {
            len,
            start,
            index: start,
            stride,
            remaining: len,
        })
    }

    fn current(&self) -> Option<u64> {
        (self.remaining != 0).then_some(self.index)
    }

    #[allow(clippy::cast_possible_truncation, reason = "modulus is a positive u64")]
    fn advance(&mut self) -> bool {
        self.remaining = self.remaining.saturating_sub(1);
        if self.remaining == 0 {
            self.index = self.start;
            self.remaining = self.len;
            return false;
        }
        self.index =
            ((u128::from(self.index) + u128::from(self.stride)) % u128::from(self.len)) as u64;
        true
    }
}

fn draw(len: u64, sink: &mut dyn Sink) -> Result<u64, Error> {
    let index = sink.generation_index(len)?;
    if index >= len {
        return Err(Error::unsupported(
            "constructive generation index outside its domain",
        ));
    }
    Ok(index)
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

#[derive(Clone, Copy)]
enum Domain {
    Stored { set: Handle, len: u64 },
    Interval { low: i64, len: u64 },
}

impl Domain {
    fn len(self) -> u64 {
        match self {
            Self::Stored { len, .. } | Self::Interval { len, .. } => len,
        }
    }

    fn at(self, machine: &mut Machine, index: u64) -> Result<Handle, Error> {
        match self {
            Self::Stored { set, len } if index < len => usize::try_from(index)
                .ok()
                .and_then(|index| machine.heap.view(set).data.get(index).copied())
                .ok_or_else(|| Error::internal("constructive stored-domain index")),
            Self::Interval { low, len } if index < len => {
                let value = i64::try_from(i128::from(low) + i128::from(index))
                    .map_err(|_| Error::unsupported("constructive interval value"))?;
                Ok(machine.heap.int_from_i64(value))
            }
            _ => Err(Error::internal("constructive domain index")),
        }
    }
}

impl Machine {
    fn generation_domain(&mut self, value: Handle) -> Result<Domain, Error> {
        if !value.is_compound() {
            return Err(Error::eval("enumerating a value that is not a set"));
        }
        match self.heap.view(value).kind {
            Kind::Interval => {
                let view = self.heap.view(value);
                let low = view.lo;
                let high = view
                    .data
                    .first()
                    .copied()
                    .and_then(|high| self.heap.big(high))
                    .and_then(|high| high.to_i64())
                    .ok_or_else(|| {
                        Error::unsupported("constructive interval endpoints outside i64")
                    })?;
                let len = u64::try_from((i128::from(high) - i128::from(low) + 1).max(0))
                    .map_err(|_| Error::unsupported("constructive interval cardinality"))?;
                if self.heap.int_mode() == crate::int::IntMode::Tlc
                    && len > u64::from(i32::MAX.unsigned_abs())
                {
                    return Err(Error::eval_class(
                        crate::TlcClass::GENERAL,
                        format!(
                            "Size of interval value exceeds the maximum representable size (32bits): {low}..{high}."
                        ),
                    ));
                }
                self.charge(1)?;
                Ok(Domain::Interval { low, len })
            }
            Kind::Set | Kind::SetEnum => {
                // Preserve real local construction normalization before indexing.
                let set = self.work_norm(value)?;
                let len = u64::try_from(self.heap.view(set).data.len())
                    .map_err(|_| Error::unsupported("constructive stored-domain size"))?;
                Ok(Domain::Stored { set, len })
            }
            _ => Err(Error::unsupported(
                "constructive generation requires an interval or stored finite binder domain",
            )),
        }
    }

    pub(super) fn generation_disjunction(
        &mut self,
        code: &Code,
        items: &[G],
        acts: u32,
        sink: &mut dyn Sink,
    ) -> Result<(), Error> {
        let len = u64::try_from(items.len())
            .map_err(|_| Error::unsupported("constructive disjunction size"))?;
        let mut order = IndexOrder::new(len, sink)?;
        while let Some(index) = order.current() {
            self.charge(1)?;
            let index = usize::try_from(index)
                .map_err(|_| Error::unsupported("constructive branch index"))?;
            let before = self.heap.mark();
            let forced = self.thunk_mark();
            self.gen_node(code, items[index], acts, sink)?;
            if self.thunks_released(forced) {
                self.heap.rollback(before);
            }
            if !order.advance() {
                break;
            }
        }
        Ok(())
    }

    pub(super) fn generation_exists(
        &mut self,
        code: &Code,
        binds: &[Bind],
        body: G,
        acts: u32,
        sink: &mut dyn Sink,
    ) -> Result<(), Error> {
        let path = self.path_mark();
        let result = self.generation_exists_inner(code, binds, body, acts, sink);
        self.path_reset(path);
        result
    }

    fn generation_exists_inner(
        &mut self,
        code: &Code,
        binds: &[Bind],
        body: G,
        acts: u32,
        sink: &mut dyn Sink,
    ) -> Result<(), Error> {
        let mut units = Vec::new();
        // All source domains are admitted before emptiness/choice traversal.
        for (index, bind) in binds.iter().enumerate() {
            let value = self.eval(code, bind.domain)?;
            if value.is_compound()
                && self.heap.view(value).kind.is_set()
                && !self.heap.is_finite(value)
            {
                // Simulation draws one member per bound variable; without a
                // sampler this is TLC's "not enumerable" error.
                let vars = if bind.tuple { 1 } else { bind.slots.len() };
                for var in 0..vars {
                    let Some(member) = self.sampled_witness(value)? else {
                        return Err(Error::eval("attempted to enumerate an infinite set"));
                    };
                    let mut one = vec![member];
                    let set = self.heap.set_from(&mut one);
                    units.push((
                        Unit {
                            bind: index,
                            var: (!bind.tuple).then_some(var),
                        },
                        Domain::Stored { set, len: 1 },
                    ));
                }
                continue;
            }
            let domain = self.generation_domain(value)?;
            if bind.tuple {
                units.push((
                    Unit {
                        bind: index,
                        var: None,
                    },
                    domain,
                ));
            } else {
                for var in 0..bind.slots.len() {
                    units.push((
                        Unit {
                            bind: index,
                            var: Some(var),
                        },
                        domain,
                    ));
                }
            }
        }
        let mut orders = units
            .iter()
            .map(|(_, domain)| IndexOrder::new(domain.len(), sink))
            .collect::<Result<Vec<_>, _>>()?;
        if orders.iter().any(|order| order.current().is_none()) {
            return Ok(());
        }
        let slots: Vec<u32> = binds
            .iter()
            .flat_map(|bind| bind.slots.iter().copied())
            .collect();
        loop {
            self.charge(1)?;
            let mark = self.save(slots.iter().copied());
            let before = self.heap.mark();
            let forced = self.thunk_mark();
            let result = (|| {
                for ((unit, domain), order) in units.iter().zip(&orders) {
                    let index = order
                        .current()
                        .ok_or_else(|| Error::internal("empty constructive context"))?;
                    let value = domain.at(self, index)?;
                    self.bind_unit(binds, *unit, value)?;
                }
                self.gen_node(code, body, acts, sink)
            })();
            self.restore(mark, slots.iter().copied());
            // As in the ordinary binding walker, forced transient thunks may
            // escape the context and prevent rollback. Chosen sink Stop keeps
            // its retained state and unwinds through the outer driver cleanup.
            result?;
            if self.thunks_released(forced) {
                self.heap.rollback(before);
            }
            let mut carry = true;
            for order in &mut orders {
                if order.advance() {
                    carry = false;
                    break;
                }
            }
            if carry {
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{IndexOrder, Sink};
    use crate::plan::Code;
    use crate::{Error, Machine};

    struct Chooser {
        draw: u64,
        calls: usize,
    }
    impl Sink for Chooser {
        fn emit(&mut self, _: &mut Machine, _: &Code) -> Result<(), Error> {
            Ok(())
        }
        fn generation_index(&mut self, _: u64) -> Result<u64, Error> {
            self.calls += 1;
            Ok(self.draw)
        }
    }

    #[test]
    fn empty_has_no_draws_and_out_of_range_indices_refuse() -> Result<(), Error> {
        let mut chooser = Chooser { draw: 4, calls: 0 };
        assert!(IndexOrder::new(0, &mut chooser)?.current().is_none());
        assert_eq!(chooser.calls, 0);
        assert!(IndexOrder::new(4, &mut chooser).is_err());
        Ok(())
    }
}
