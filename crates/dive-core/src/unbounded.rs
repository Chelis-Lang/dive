//! Drawing members of sets the generator cannot enumerate, for simulation
//! (`--sample-unbounded`, ADR 0162 §9, D-040).
//!
//! Where a generator would enumerate an infinite set — `x' \in S` or a
//! positive `\E v \in S` in Init or Next — a machine with an [`Unbounded`]
//! sampler draws one member of `S` instead. Only generator positions draw:
//! they are positive, so the drawn member yields states that satisfy the
//! action, and every sampled trace is a behavior of the specification.
//! Boolean quantifiers, `\A` and negated contexts never draw and keep their
//! errors. Nothing is concluded from a draw that leads to no state: the
//! driver must not report a deadlock or an empty Init for a step that drew.
//!
//! **Distributions** (`mean` is `m`, `p = 1 / (m + 1)`):
//! - `Nat`: geometric, `P(k) = p (1 - p)^k`, mean `m`;
//! - `Int`: a geometric magnitude `k`, then for `k > 0` a uniform sign, so
//!   `P(0) = p` and `P(k) = P(-k) = p (1 - p)^k / 2`;
//! - `Seq(S)`: a geometric length, each element drawn from `S`;
//! - `S1 \X ... \X Sn`, `[a : S, ...]` and `[D -> S]` with finite `D`:
//!   each component drawn from its set;
//! - a finite component: uniform over its elements.
//!
//! Every other infinite set (`SUBSET Nat`, a union with an infinite part, a
//! filter over an infinite set, `STRING`) is refused by name. Draws use only
//! uniform integer choices, so a seed reproduces them on every platform.

use crate::eval::{Machine, Unit};
use crate::node::Kind;
use crate::plan::{Bind, Code, Node};
use crate::{Error, Handle};

/// The largest supported mean. A geometric draw costs about `mean` uniform
/// choices.
pub const MAX_MEAN: u64 = 1 << 16;

/// Nesting of sampled sets (`Seq(Seq(Nat))`) a single draw may descend.
const MAX_NESTING: usize = 64;

/// A replayable source of uniform choices, positioned by the driver.
pub trait IndexSource: Send + std::fmt::Debug {
    /// A uniform choice in `0..bound`; `bound` is positive.
    fn next_index(&mut self, bound: u64) -> u64;

    /// Starts the stream for one generation site, named by the driver.
    fn restart(&mut self, site: [u64; 4]);
}

/// A machine's sampler of unbounded sets.
#[derive(Debug)]
pub struct Unbounded {
    mean: u64,
    source: Box<dyn IndexSource>,
    probe: bool,
    draws: u64,
    /// Members drawn for `\E` bindings on the current generator path,
    /// innermost last: the witnesses of an emitted state's existentials.
    path: Vec<Handle>,
    /// Each distinct sampled set, rendered, since last taken.
    sets: Vec<String>,
}

impl Unbounded {
    /// A sampler with the geometric `mean` (positive, at most [`MAX_MEAN`]).
    ///
    /// # Errors
    /// A mean outside `1..=MAX_MEAN`.
    pub fn new(mean: u64, source: Box<dyn IndexSource>) -> Result<Self, Error> {
        if !(1..=MAX_MEAN).contains(&mean) {
            return Err(Error::unsupported(format!(
                "a sampling mean outside 1..={MAX_MEAN}"
            )));
        }
        Ok(Self {
            mean,
            source,
            probe: false,
            draws: 0,
            path: Vec::new(),
            sets: Vec::new(),
        })
    }

    /// In probe mode a draw fails instead of drawing, after counting itself:
    /// the driver learns that a generator reached an unbounded set.
    pub fn set_probe(&mut self, probe: bool) {
        self.probe = probe;
    }

    /// Draws made (or, in probe mode, attempted) since the sampler was made.
    #[must_use]
    pub const fn draws(&self) -> u64 {
        self.draws
    }

    /// Positions the stream for one generation site.
    pub fn restart(&mut self, site: [u64; 4]) {
        self.source.restart(site);
    }

    /// The distinct sets sampled since the last call, rendered as TLA+.
    pub fn take_sets(&mut self) -> Vec<String> {
        std::mem::take(&mut self.sets)
    }

    fn geometric(&mut self) -> u64 {
        let mut k = 0u64;
        while self.source.next_index(self.mean + 1) != 0 {
            k += 1;
        }
        k
    }
}

impl Machine {
    /// The witnesses drawn for `\E` bindings on the current generator path,
    /// outermost first. Empty without a sampler.
    #[must_use]
    pub fn drawn(&self) -> &[Handle] {
        self.unbounded.as_ref().map_or(&[], |u| u.path.as_slice())
    }

    fn path_len(&self) -> usize {
        self.unbounded.as_ref().map_or(0, |u| u.path.len())
    }

    fn truncate_path(&mut self, len: usize) {
        if let Some(unbounded) = &mut self.unbounded {
            unbounded.path.truncate(len);
        }
    }

    fn push_path(&mut self, value: Handle) {
        if let Some(unbounded) = &mut self.unbounded {
            unbounded.path.push(value);
        }
    }

    /// One member of the infinite set `set`, or `None` without a sampler (the
    /// caller then keeps its "not enumerable" error).
    ///
    /// # Errors
    /// A set with no supported distribution, or, in probe mode, any draw.
    pub(crate) fn sample_member(&mut self, set: Handle) -> Result<Option<Handle>, Error> {
        let Some(unbounded) = &mut self.unbounded else {
            return Ok(None);
        };
        unbounded.draws += 1;
        if unbounded.probe {
            return Err(Error::unsupported(
                "an unbounded set reached while enumerating once",
            ));
        }
        let member = self.draw_from(set, 0)?;
        let rendered = self.describe(set, 0);
        if let Some(unbounded) = &mut self.unbounded
            && !unbounded.sets.contains(&rendered)
        {
            unbounded.sets.push(rendered);
        }
        Ok(Some(member))
    }

    /// `set` as TLA+, with its infinite parts by name (`[a : Nat, b : {1, 2}]`).
    fn describe(&mut self, set: Handle, depth: usize) -> String {
        if depth > MAX_NESTING || !set.is_compound() || self.heap.is_finite(set) {
            return self.describe_finite(set);
        }
        let view = self.heap.view(set);
        let (kind, data) = (view.kind, view.data.to_vec());
        match kind {
            Kind::Nat => "Nat".to_owned(),
            Kind::Int => "Int".to_owned(),
            Kind::SeqSet => format!("Seq({})", self.describe(data[0], depth + 1)),
            Kind::Product => data
                .iter()
                .map(|&part| self.describe(part, depth + 1))
                .collect::<Vec<_>>()
                .join(" \\X "),
            Kind::RecSet => {
                let n = data.len() / 2;
                let fields: Vec<String> = (0..n)
                    .map(|i| {
                        let name = crate::print::pretty(&mut self.heap, data[i])
                            .unwrap_or_default()
                            .trim_matches('"')
                            .to_owned();
                        format!("{name} : {}", self.describe(data[n + i], depth + 1))
                    })
                    .collect();
                format!("[{}]", fields.join(", "))
            }
            Kind::FcnSet => format!(
                "[{} -> {}]",
                self.describe(data[0], depth + 1),
                self.describe(data[1], depth + 1)
            ),
            _ => "an unbounded set".to_owned(),
        }
    }

    /// A finite set as TLA+: an interval as `a..b`, else its elements.
    /// The value printer refuses sets of consecutive integers, whose TLC
    /// form depends on how TLC holds them, so elements are printed one by one.
    fn describe_finite(&mut self, set: Handle) -> String {
        if set.is_compound() && self.heap.view(set).kind == Kind::Interval {
            let view = self.heap.view(set);
            if let Some(high) = view.data.first().and_then(|h| h.as_int()) {
                return format!("{}..{high}", view.lo);
            }
        }
        if let Ok(text) = crate::print::pretty(&mut self.heap, set) {
            return text;
        }
        let elements = self
            .heap
            .norm(set)
            .and_then(|normal| self.heap.elements(normal));
        match elements {
            Ok(elements) => {
                let parts: Vec<String> = elements
                    .into_iter()
                    .map(|e| {
                        crate::print::pretty(&mut self.heap, e).unwrap_or_else(|_| "?".to_owned())
                    })
                    .collect();
                format!("{{{}}}", parts.join(", "))
            }
            Err(_) => "a finite set".to_owned(),
        }
    }

    fn uniform(&mut self, bound: u64) -> Result<u64, Error> {
        let unbounded = self
            .unbounded
            .as_mut()
            .ok_or_else(|| Error::internal("unbounded draw without a sampler"))?;
        Ok(unbounded.source.next_index(bound))
    }

    fn geometric(&mut self) -> Result<u64, Error> {
        let unbounded = self
            .unbounded
            .as_mut()
            .ok_or_else(|| Error::internal("unbounded draw without a sampler"))?;
        Ok(unbounded.geometric())
    }

    /// A member of `set`: uniform when it is finite, else by its
    /// distribution.
    fn draw_from(&mut self, set: Handle, depth: usize) -> Result<Handle, Error> {
        if depth > MAX_NESTING {
            return Err(Error::unsupported(
                "sampling unbounded sets nested more than 64 deep",
            ));
        }
        if !set.is_compound() || !self.heap.view(set).kind.is_set() {
            return Err(Error::eval("The right side of \\IN is not a set"));
        }
        self.charge(1)?;
        if self.heap.is_finite(set) {
            return self.uniform_member(set);
        }
        let view = self.heap.view(set);
        let (kind, data) = (view.kind, view.data.to_vec());
        match kind {
            Kind::Nat => {
                let k = self.geometric()?;
                Ok(self.int_of(k))
            }
            Kind::Int => {
                let k = self.geometric()?;
                let value = self.int_of(k);
                if k > 0 && self.uniform(2)? == 1 {
                    let negative = i64::try_from(k)
                        .map(|k| -k)
                        .map_err(|_| Error::unsupported("a sampled integer outside i64"))?;
                    return Ok(self.heap.int_from_i64(negative));
                }
                Ok(value)
            }
            Kind::SeqSet => {
                let base = data[0];
                let len = self.geometric()?;
                let mut elems = Vec::new();
                for _ in 0..len {
                    elems.push(self.draw_from(base, depth + 1)?);
                }
                Ok(self.heap.tuple(&elems))
            }
            Kind::Product => {
                let mut elems = Vec::with_capacity(data.len());
                for component in data {
                    elems.push(self.draw_from(component, depth + 1)?);
                }
                Ok(self.heap.tuple(&elems))
            }
            Kind::RecSet => {
                let n = data.len() / 2;
                let mut values = Vec::with_capacity(n);
                for &component in &data[n..] {
                    values.push(self.draw_from(component, depth + 1)?);
                }
                Ok(self.heap.fcn_sorted(&data[..n], &values))
            }
            Kind::FcnSet if self.heap.is_finite(data[0]) => {
                let domain = self.work_norm(data[0])?;
                let keys = self.work_elements(domain)?;
                let mut values = Vec::with_capacity(keys.len());
                for _ in 0..keys.len() {
                    values.push(self.draw_from(data[1], depth + 1)?);
                }
                Ok(self.heap.fcn_sorted(&keys, &values))
            }
            Kind::FcnSet => Err(Error::unsupported(
                "sampling a function set with an infinite domain",
            )),
            Kind::Subset => Err(Error::unsupported(
                "sampling SUBSET of an infinite set: no stated distribution",
            )),
            Kind::Union => Err(Error::unsupported(
                "sampling a union with an infinite part: no stated distribution",
            )),
            Kind::SetPred => Err(Error::unsupported(
                "sampling a filter over an infinite set: no stated distribution",
            )),
            _ => Err(Error::unsupported(
                "sampling this infinite set: no stated distribution",
            )),
        }
    }

    fn int_of(&mut self, k: u64) -> Handle {
        match i64::try_from(k) {
            Ok(k) => self.heap.int_from_i64(k),
            Err(_) => self.heap.int_from_big(&num_bigint::BigInt::from(k)),
        }
    }

    /// A uniform member of the finite set `set`. An interval is indexed
    /// without materializing it.
    fn uniform_member(&mut self, set: Handle) -> Result<Handle, Error> {
        let view = self.heap.view(set);
        if view.kind == Kind::Interval {
            let low = view.lo;
            let high = view.data.first().and_then(|h| h.as_int());
            if let Some(high) = high {
                let len = i128::from(high) - i128::from(low) + 1;
                if len <= 0 {
                    return Err(Error::unsupported("sampling a member of an empty set"));
                }
                let len = u64::try_from(len)
                    .map_err(|_| Error::unsupported("sampling an interval wider than 2^64"))?;
                let index = self.uniform(len)?;
                let value = i64::try_from(i128::from(low) + i128::from(index))
                    .map_err(|_| Error::unsupported("a sampled integer outside i64"))?;
                return Ok(self.heap.int_from_i64(value));
            }
        }
        let normal = self.work_norm(set)?;
        let elements = self.work_elements(normal)?;
        if elements.is_empty() {
            return Err(Error::unsupported("sampling a member of an empty set"));
        }
        let index = usize::try_from(self.uniform(elements.len() as u64)?)
            .map_err(|_| Error::internal("sampled index"))?;
        Ok(elements[index])
    }

    /// A cursor over `domain`'s elements, or over one drawn member when the
    /// domain is infinite and the machine samples. It binds a filter's
    /// variable to the member it assigns, so it needs no witness.
    pub(crate) fn bound_cursor(&mut self, domain: Handle) -> Result<crate::heap::Cursor, Error> {
        if domain.is_compound()
            && !self.heap.is_finite(domain)
            && let Some(member) = self.sample_member(domain)?
        {
            let mut one = vec![member];
            let set = self.heap.set_from(&mut one);
            return self.heap.cursor(set);
        }
        self.work_cursor(domain)
    }

    pub(crate) fn path_mark(&self) -> usize {
        self.path_len()
    }

    pub(crate) fn path_reset(&mut self, mark: usize) {
        self.truncate_path(mark);
    }

    /// `\E` bindings as [`Machine::units`] builds them, with one drawn member
    /// for each variable bound to an infinite domain. The drawn members are
    /// pushed on the witness path; the caller truncates it afterwards.
    pub(crate) fn sampled_units(
        &mut self,
        code: &Code,
        binds: &[Bind],
    ) -> Result<Vec<(Unit, Vec<Handle>)>, Error> {
        let mut out = Vec::new();
        for (bi, b) in binds.iter().enumerate() {
            let domain = match code.nodes.get(b.domain as usize) {
                Some(Node::Const(h)) => *h,
                _ => self.eval(code, b.domain)?,
            };
            if domain.is_compound() && !self.heap.is_finite(domain) {
                let vars = if b.tuple { 1 } else { b.slots.len() };
                for vi in 0..vars {
                    let Some(member) = self.sample_member(domain)? else {
                        return Err(Error::eval("attempted to enumerate an infinite set"));
                    };
                    self.push_path(member);
                    out.push((
                        Unit {
                            bind: bi,
                            var: (!b.tuple).then_some(vi),
                        },
                        vec![member],
                    ));
                }
                continue;
            }
            let d = match code.nodes.get(b.domain as usize) {
                Some(Node::Const(h)) => self.heap.enumerate_constant(*h)?,
                _ => self.work_norm(domain)?,
            };
            let elems = self.work_elements(d)?;
            if b.tuple {
                out.push((
                    Unit {
                        bind: bi,
                        var: None,
                    },
                    elems,
                ));
            } else {
                for vi in 0..b.slots.len() {
                    self.charge(elems.len() + 1)?;
                    out.push((
                        Unit {
                            bind: bi,
                            var: Some(vi),
                        },
                        elems.clone(),
                    ));
                }
            }
        }
        Ok(out)
    }

    /// One drawn member of `domain` for constructive generation, pushed on
    /// the witness path. `None` without a sampler.
    pub(crate) fn sampled_witness(&mut self, domain: Handle) -> Result<Option<Handle>, Error> {
        let member = self.sample_member(domain)?;
        if let Some(member) = member {
            self.push_path(member);
        }
        Ok(member)
    }
}
