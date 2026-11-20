//! Source-derived finite value layouts for experiment measurements.
//!
//! A type predicate proposes the layout. Encoding validates every member
//! against that layout, so a type invariant need not be assumed to be true.

use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::sync::Arc;

use dive_core::node::{Kind, flags};
use dive_core::plan::{Code, Node, P, SetOp};
use dive_core::{Error, Handle, Heap, Machine, Plan, Sink, Store};
use serde_json::json;

use crate::{Config, Memo};

const ROW_LIMIT: usize = 4096;
const LEVEL_LIMIT: usize = 8;

#[derive(Debug)]
enum Shape {
    Atom(Vec<Handle>),
    Set(Vec<Handle>),
    Function(Vec<Handle>, Box<Self>),
}

impl Shape {
    fn width(&self) -> Option<u32> {
        match self {
            Self::Atom(values) => u32::try_from(values.len().checked_sub(1)?)
                .ok()
                .map(u32::bit_width),
            Self::Set(values) => u32::try_from(values.len()).ok(),
            Self::Function(keys, value) => {
                value.width()?.checked_mul(u32::try_from(keys.len()).ok()?)
            }
        }
    }

    fn pack(&self, heap: &Heap, handle: Handle) -> Option<u64> {
        match self {
            Self::Atom(values) => {
                u64::try_from(values.iter().position(|&item| item == handle)?).ok()
            }
            Self::Set(values) => {
                if !handle.is_compound() {
                    return None;
                }
                let view = heap.view(handle);
                if view.kind != Kind::Set || view.flags & flags::VIEW != 0 {
                    return None;
                }
                let mut bits = 0_u64;
                for &item in view.data {
                    bits |= 1 << values.iter().position(|&candidate| item == candidate)?;
                }
                Some(bits)
            }
            Self::Function(keys, value) => {
                if !handle.is_compound() {
                    return None;
                }
                let view = heap.view(handle);
                if view.len() != keys.len() || view.flags & flags::VIEW != 0 {
                    return None;
                }
                match view.kind {
                    Kind::Fcn
                        if view.keys() == keys && keys.iter().any(|key| key.as_int().is_none()) => {
                    }
                    Kind::IntFcn
                        if keys.iter().enumerate().all(|(i, key)| {
                            key.as_int()
                                == i64::try_from(i).ok().and_then(|i| view.lo.checked_add(i))
                        }) => {}
                    _ => return None,
                }
                let mut result = 0;
                let width = value.width()?;
                for (i, &member) in view.values().iter().enumerate() {
                    let field = value.pack(heap, member)?;
                    let offset = u32::try_from(i).ok()?.checked_mul(width)?;
                    result |= field.checked_shl(offset).or((field == 0).then_some(0))?;
                }
                Some(result)
            }
        }
    }

    fn unpack(&self, heap: &mut Heap, word: u64) -> Option<Handle> {
        match self {
            Self::Atom(values) => values.get(usize::try_from(word).ok()?).copied(),
            Self::Set(values) => {
                let mut result = Vec::with_capacity(word.count_ones() as usize);
                for (i, &member) in values.iter().enumerate() {
                    if word & (1 << i) != 0 {
                        result.push(member);
                    }
                }
                Some(heap.set_sorted(&result))
            }
            Self::Function(keys, value) => {
                let width = value.width()?;
                let mut members = Vec::with_capacity(keys.len());
                for i in 0..keys.len() {
                    let offset = u32::try_from(i).ok()?.checked_mul(width)?;
                    let field = extract(word, offset, width);
                    members.push(value.unpack(heap, field)?);
                }
                Some(heap.fcn_sorted(keys, &members))
            }
        }
    }
}

fn extract(word: u64, offset: u32, width: u32) -> u64 {
    let shifted = word.checked_shr(offset).unwrap_or(0);
    shifted & u64::MAX.checked_shr(64 - width).unwrap_or(0)
}

fn domain(heap: &Heap, handle: Handle) -> Option<Vec<Handle>> {
    if !handle.is_compound() {
        return None;
    }
    let view = heap.view(handle);
    match view.kind {
        Kind::Set
            if view.len() <= 64 && view.data.iter().all(|v| !v.is_compound() && !v.is_empty()) =>
        {
            Some(view.data.to_vec())
        }
        Kind::Interval => {
            let hi = view.data.first()?.as_int()?;
            if hi.checked_sub(view.lo)? >= 64 {
                return None;
            }
            (view.lo..=hi).map(Handle::int).collect()
        }
        _ => None,
    }
}

fn shape_from_value(heap: &Heap, handle: Handle, depth: usize) -> Option<Shape> {
    if depth >= 8 || !handle.is_compound() {
        return None;
    }
    let view = heap.view(handle);
    match view.kind {
        Kind::FcnSet => {
            let [keys, range] = *view.data else {
                return None;
            };
            Some(Shape::Function(
                domain(heap, keys)?,
                Box::new(shape_from_value(heap, range, depth + 1)?),
            ))
        }
        Kind::Subset => Some(Shape::Set(domain(heap, *view.data.first()?)?)),
        _ => Some(Shape::Atom(domain(heap, handle)?)),
    }
}

fn literal(code: &Code, p: P, depth: usize) -> Option<Handle> {
    if depth >= 8 {
        return None;
    }
    match code.nodes.get(p as usize)? {
        Node::Const(value) => Some(*value),
        Node::Spec(p) | Node::Memo { body: p, .. } => literal(code, *p, depth + 1),
        _ => None,
    }
}

fn static_set(code: &Code, heap: &Heap, p: P, depth: usize) -> Option<Vec<Handle>> {
    if depth >= 8 {
        return None;
    }
    if let Some(handle) = literal(code, p, 0) {
        return domain(heap, handle);
    }
    match code.nodes.get(p as usize)? {
        Node::Spec(p) | Node::Memo { body: p, .. } => static_set(code, heap, *p, depth + 1),
        Node::SetBin(SetOp::Cup, left, right) => {
            let mut elements = static_set(code, heap, *left, depth + 1)?;
            elements.extend(static_set(code, heap, *right, depth + 1)?);
            elements.sort_unstable_by(|&a, &b| heap.cmp(a, b));
            elements.dedup();
            (elements.len() <= 64).then_some(elements)
        }
        Node::SetEnum(elements) if elements.len() <= 64 => {
            let mut values = elements
                .iter()
                .map(|&p| literal(code, p, 0))
                .collect::<Option<Vec<_>>>()?;
            if values
                .iter()
                .any(|value| value.is_compound() || value.is_empty())
            {
                return None;
            }
            values.sort_unstable_by(|&a, &b| heap.cmp(a, b));
            values.dedup();
            Some(values)
        }
        _ => None,
    }
}

fn shape_from_node(code: &Code, heap: &Heap, p: P, depth: usize) -> Option<Shape> {
    if depth >= 8 {
        return None;
    }
    match code.nodes.get(p as usize)? {
        Node::Const(handle) => shape_from_value(heap, *handle, depth),
        Node::Spec(p) | Node::Memo { body: p, .. } => shape_from_node(code, heap, *p, depth + 1),
        Node::FcnSet(keys, range) => Some(Shape::Function(
            static_set(code, heap, *keys, 0)?,
            Box::new(shape_from_node(code, heap, *range, depth + 1)?),
        )),
        Node::Powerset(elements) => Some(Shape::Set(static_set(code, heap, *elements, 0)?)),
        _ => Some(Shape::Atom(static_set(code, heap, p, 0)?)),
    }
}

fn infer(code: &Code, heap: &Heap, p: P, fields: &mut [Option<Shape>], depth: usize) {
    if depth >= 16 {
        return;
    }
    match code.nodes.get(p as usize) {
        Some(Node::And(parts)) => {
            for &part in parts {
                infer(code, heap, part, fields, depth + 1);
            }
        }
        Some(Node::Spec(p) | Node::Memo { body: p, .. }) => {
            infer(code, heap, *p, fields, depth + 1);
        }
        Some(
            Node::In(row, ty)
            | Node::InLazy {
                elem: row,
                set: ty,
                neg: false,
            }
            | Node::Subseteq(row, ty),
        ) => {
            if let Some(Node::Var(var)) = code.nodes.get(*row as usize)
                && let Some(field) = fields.get_mut(usize::from(*var))
                && field.is_none()
            {
                *field = if matches!(code.nodes.get(p as usize), Some(Node::Subseteq(_, _))) {
                    static_set(code, heap, *ty, 0).map(Shape::Set)
                } else {
                    shape_from_node(code, heap, *ty, 0)
                };
            }
        }
        _ => {}
    }
}

struct Layout {
    fields: Vec<Shape>,
    widths: Vec<u32>,
}

impl Layout {
    fn build(plan: &Plan, heap: &Heap) -> Option<Self> {
        let mut partial: Vec<_> = (0..plan.vars.len()).map(|_| None).collect();
        for check in &plan.invariants {
            for part in &check.conjuncts {
                infer(&plan.code, heap, part.p, &mut partial, 0);
            }
        }
        let fields: Vec<_> = partial.into_iter().collect::<Option<_>>()?;
        let widths: Vec<_> = fields.iter().map(Shape::width).collect::<Option<_>>()?;
        if widths.iter().copied().try_fold(0_u32, u32::checked_add)? > 64 {
            return None;
        }
        Some(Self { fields, widths })
    }

    fn pack(&self, heap: &Heap, values: &[Handle]) -> Option<u64> {
        let mut row = 0;
        let mut offset = 0;
        for ((shape, width), &value) in self.fields.iter().zip(&self.widths).zip(values) {
            let field = shape.pack(heap, value)?;
            row |= field.checked_shl(offset).unwrap_or(0);
            offset += width;
        }
        Some(row)
    }

    fn round_trip(&self, m: &mut Machine) -> Option<u64> {
        if m.assigned != self.fields.len() {
            return None;
        }
        let row = self.pack(&m.heap, &m.s1)?;
        let mut offset = 0;
        for ((shape, &width), &value) in self.fields.iter().zip(&self.widths).zip(&m.s1) {
            let word = extract(row, offset, width);
            let decoded = shape.unpack(&mut m.heap, word)?;
            if !m.heap.eq(value, decoded) || m.heap.hash(value) != m.heap.hash(decoded) {
                return None;
            }
            offset += width;
        }
        Some(row)
    }

    fn unpack_into(&self, m: &mut Machine, row: u64) -> Option<()> {
        let mut offset = 0;
        for (i, (shape, &width)) in self.fields.iter().zip(&self.widths).enumerate() {
            m.s0[i] = shape.unpack(&mut m.heap, extract(row, offset, width))?;
            offset += width;
        }
        Some(())
    }
}

struct Inspect<'a> {
    layout: &'a Layout,
    rows: usize,
    seen: HashSet<u64>,
    frontier: Vec<u64>,
    miss: bool,
}

impl Sink for Inspect<'_> {
    fn emit(&mut self, m: &mut Machine, _: &Code) -> Result<(), Error> {
        if self.rows >= ROW_LIMIT {
            return Err(Error::stop());
        }
        let Some(word) = self.layout.round_trip(m) else {
            self.miss = true;
            return Err(Error::stop());
        };
        if self.seen.insert(word) {
            self.frontier.push(word);
        }
        self.rows += 1;
        Ok(())
    }
}

fn run_actions(m: &mut Machine, plan: &Plan, sink: &mut dyn Sink, initial: bool) -> bool {
    let actions = if initial { &plan.init } else { &plan.next };
    actions.iter().all(|action| {
        let old = m.enter(action.frame);
        for &(slot, value) in &action.preset {
            if let Some(destination) = m.frame.get_mut(m.base + slot as usize) {
                *destination = value;
            }
        }
        let result = m.run_action(&plan.code, action.g, sink);
        m.leave(old);
        result.is_ok() && !m.heap.poisoned()
    })
}

fn inspect_prefix(m: &mut Machine, plan: &Plan, layout: &Layout) -> serde_json::Value {
    let mut sample = Inspect {
        layout,
        rows: 0,
        seen: HashSet::new(),
        frontier: Vec::new(),
        miss: false,
    };
    let initial_complete = run_actions(m, plan, &mut sample, true);
    let initial_rows = sample.rows;
    let mut completed_waves = 0;
    if initial_complete {
        for _ in 0..LEVEL_LIMIT {
            let frontier = std::mem::take(&mut sample.frontier);
            if frontier.is_empty() {
                break;
            }
            if !frontier.into_iter().all(|word| {
                m.heap.reset();
                layout.unpack_into(m, word).is_some() && run_actions(m, plan, &mut sample, false)
            }) {
                break;
            }
            completed_waves += 1;
        }
    }
    json!({
        "field_bits": plan.vars.iter().zip(&layout.widths).collect::<Vec<_>>(),
        "bits_per_state": layout.widths.iter().sum::<u32>(),
        "initial_rows_checked": initial_rows,
        "initial_check_complete": initial_complete,
        "successor_rows_checked": sample.rows - initial_rows,
        "retained_distinct_words": sample.seen.len(),
        "complete_expansion_waves": completed_waves,
        "shape_miss": sample.miss,
        "row_limit": ROW_LIMIT,
    })
}

pub(crate) fn observe(plan: &Plan, store: &Arc<Store>, config: Config) {
    let Some(path) = std::env::var_os("DIVE_EXPERIMENT_LAYOUT_PROBE") else {
        return;
    };
    if plan.effects.any {
        return;
    }
    let mut m = Machine::new(
        Arc::clone(store),
        plan.vars.len(),
        config.opts,
        Memo::default(),
    );
    let report = if let Some(layout) = Layout::build(plan, &m.heap) {
        inspect_prefix(&mut m, plan, &layout)
    } else {
        json!({"status": "declined"})
    };
    if let Ok(mut stream) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(stream, "{report}");
    }
}

struct WordSearch<'a> {
    plan: &'a Plan,
    layout: &'a Layout,
    seen: HashSet<u32>,
    next: Vec<u32>,
    image: Vec<Handle>,
    permutations: Option<Vec<WordPermutation>>,
    stats: crate::Stats,
    metrics: crate::Metrics,
}

impl WordSearch<'_> {
    fn budget(&mut self, unexpanded: usize) -> Option<crate::Verdict> {
        self.stats.depth += u64::from(!self.next.is_empty());
        self.stats.left_on_queue = u64::try_from(unexpanded.checked_add(self.next.len())?).ok()?;
        Some(crate::Verdict::Budget)
    }
}

struct WordPermutation(Vec<(u32, u32)>);

impl WordPermutation {
    fn lower(layout: &Layout, map: &[(u32, u32)]) -> Option<Self> {
        let mut destinations = [0_u32; 32];
        let mut offset = 0;
        for (shape, &width) in layout.fields.iter().zip(&layout.widths) {
            shape.permuted_positions(map, offset, offset, &mut destinations)?;
            offset += width;
        }
        let mut masks = BTreeMap::<u32, u32>::new();
        let mut used = 0_u32;
        for (source, &destination) in destinations[..offset as usize].iter().enumerate() {
            let bit = 1_u32.checked_shl(destination)?;
            if destination >= offset || used & bit != 0 {
                return None;
            }
            used |= bit;
            let source = u32::try_from(source).ok()?;
            let rotation = (destination + 32 - source) % 32;
            *masks.entry(rotation).or_default() |= 1_u32 << source;
        }
        Some(Self(masks.into_iter().collect()))
    }

    fn apply(&self, word: u32) -> u32 {
        self.0.iter().fold(0, |acc, &(rotation, mask)| {
            acc | (word & mask).rotate_left(rotation)
        })
    }
}

fn moved(handle: Handle, map: &[(u32, u32)]) -> Handle {
    handle
        .as_model_id()
        .and_then(|id| {
            let index = map.binary_search_by_key(&id, |&(from, _)| from).ok()?;
            Some(Handle::model(map[index].1))
        })
        .unwrap_or(handle)
}

impl Shape {
    fn comparable(&self, sym: &dive_core::symmetry::Symmetry, heap: &Heap) -> bool {
        match self {
            Self::Atom(atoms) | Self::Set(atoms) => sym.comparable_domain(heap, atoms),
            // Fixed function keys remain identical throughout the orbit.
            Self::Function(_, value) => value.comparable(sym, heap),
        }
    }

    fn permuted_positions(
        &self,
        map: &[(u32, u32)],
        source: u32,
        destination: u32,
        positions: &mut [u32; 32],
    ) -> Option<()> {
        match self {
            Self::Atom(atoms) => {
                if atoms.iter().any(|&atom| atom != moved(atom, map)) {
                    return None;
                }
                for bit in 0..self.width()? {
                    *positions.get_mut((source + bit) as usize)? = destination + bit;
                }
            }
            Self::Set(atoms) => {
                for (index, &atom) in atoms.iter().enumerate() {
                    let target = atoms
                        .iter()
                        .position(|&target| target == moved(atom, map))?;
                    *positions.get_mut(source as usize + index)? =
                        destination + u32::try_from(target).ok()?;
                }
            }
            Self::Function(keys, value) => {
                let width = value.width()?;
                for (index, &key) in keys.iter().enumerate() {
                    let target = keys.iter().position(|&target| target == moved(key, map))?;
                    value.permuted_positions(
                        map,
                        source + u32::try_from(index).ok()?.checked_mul(width)?,
                        destination + u32::try_from(target).ok()?.checked_mul(width)?,
                        positions,
                    )?;
                }
            }
        }
        Some(())
    }
}

impl Sink for WordSearch<'_> {
    fn emit(&mut self, m: &mut Machine, code: &Code) -> Result<(), Error> {
        self.stats.generated += 1;
        if m.assigned != self.layout.fields.len() {
            return Err(Error::stop());
        }
        let word = self
            .layout
            .pack(&m.heap, &m.s1)
            .and_then(|word| u32::try_from(word).ok())
            .ok_or_else(Error::stop)?;
        let key = if let Some(permutations) = &self.permutations {
            // Exact membership needs the orbit identity. The stored frontier
            // remains the first concrete successor in generator order.
            permutations
                .iter()
                .fold(word, |min, permutation| min.min(permutation.apply(word)))
        } else if let Some(symmetry) = &self.plan.symmetry {
            symmetry.canonical_from(&mut m.heap, &m.s1, &m.s0, &mut self.image)?;
            self.layout
                .pack(&m.heap, &self.image)
                .and_then(|word| u32::try_from(word).ok())
                .ok_or_else(Error::stop)?
        } else {
            word
        };
        self.metrics.probed += 1;
        if self.seen.len() == self.seen.capacity() {
            self.seen.try_reserve(1).map_err(|_| Error::stop())?;
        }
        if self.seen.insert(key) {
            for invariant in &self.plan.invariants {
                if !crate::eval_check(m, code, invariant, None, &mut self.metrics)? {
                    return Err(Error::stop());
                }
            }
            if self.next.len() == self.next.capacity() {
                self.next.try_reserve(1).map_err(|_| Error::stop())?;
            }
            self.next.push(word);
            self.stats.distinct += 1;
        } else {
            self.metrics.discarded_before_intern += 1;
        }
        Ok(())
    }
}

// A holding result requires complete search. A budget stop reports the prefix
// here. Other endings and representation misses run through ordinary Core.
pub(crate) fn try_explore(
    plan: &Plan,
    store: &Arc<Store>,
    config: Config,
    poll: &dyn Fn() -> bool,
    control: Option<&Arc<dyn dive_core::eval::WorkControl>>,
) -> Option<crate::Outcome> {
    if std::env::var_os("DIVE_EXPERIMENT_RANK_EXACT").as_deref() != Some(std::ffi::OsStr::new("1"))
        || plan.effects.any
        || plan.view.is_some()
        || !plan.constraints.is_empty()
        || !plan.action_constraints.is_empty()
        || !plan.implied_inits.is_empty()
        || !plan.implied_actions.is_empty()
        || plan.startup_assumption_false.is_some()
        || plan.next.is_empty()
    {
        return None;
    }
    let start = std::time::Instant::now();
    let mut m = Machine::new(
        Arc::clone(store),
        plan.vars.len(),
        config.opts,
        Memo::default(),
    );
    m.set_work_control(control.cloned());
    let layout = Layout::build(plan, &m.heap)?;
    if layout.widths.iter().sum::<u32>() > 32 {
        return None;
    }
    let permutations = if std::env::var_os("DIVE_EXPERIMENT_RANK_SYMMETRY").as_deref()
        == Some(std::ffi::OsStr::new("1"))
    {
        plan.symmetry.as_ref().and_then(|symmetry| {
            if !layout
                .fields
                .iter()
                .all(|shape| shape.comparable(symmetry, &m.heap))
            {
                return None;
            }
            symmetry
                .moved_model_values()
                .iter()
                .map(|permutation| WordPermutation::lower(&layout, permutation))
                .collect::<Option<Vec<_>>>()
        })
    } else {
        None
    };
    let mut search = WordSearch {
        plan,
        layout: &layout,
        seen: HashSet::new(),
        next: Vec::new(),
        image: Vec::new(),
        permutations,
        stats: crate::Stats::default(),
        metrics: crate::Metrics::default(),
    };
    if !run_actions(&mut m, plan, &mut search, true) || search.next.is_empty() {
        return None;
    }
    search.stats.initial_generated = search.stats.generated;
    let mut frontier = Vec::new();
    let mut verdict = crate::Verdict::Success;
    'waves: while !search.next.is_empty() {
        std::mem::swap(&mut frontier, &mut search.next);
        search.stats.depth += 1;
        for (at, &word) in frontier.iter().enumerate() {
            if at.is_multiple_of(1024) && poll() {
                verdict = search.budget(frontier.len() - at)?;
                break 'waves;
            }
            m.heap.reset();
            layout.unpack_into(&mut m, u64::from(word))?;
            let generated = search.stats.generated;
            if !run_actions(&mut m, plan, &mut search, false) {
                if m.heap.work_interrupted() || poll() {
                    verdict = search.budget(frontier.len() - at)?;
                    break 'waves;
                }
                return None;
            }
            if plan.check_deadlock && generated == search.stats.generated {
                return None;
            }
        }
        frontier.clear();
    }
    search.metrics.explore_time = start.elapsed();
    search.metrics.store = store.stats();
    search.metrics.store_memory = store.memory_usage();
    Some(crate::Outcome {
        verdict,
        stats: search.stats,
        metrics: search.metrics,
        warnings: Vec::new(),
        trace: None,
        failure: None,
        in_init: false,
        expanded: true,
        replayed: None,
        sampled: Vec::new(),
        printed: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dive_core::symmetry::Symmetry;
    use std::collections::HashMap;

    #[test]
    fn ranked_symmetry_preserves_the_orbit_partition() -> Result<(), Error> {
        let mut heap = Heap::new(Arc::new(Store::default()), false, false);
        let ids = vec![Handle::model(1), Handle::model(2)];
        let zero = Handle::int(0).ok_or_else(Error::stop)?;
        let one = Handle::int(1).ok_or_else(Error::stop)?;
        let layout = Layout {
            fields: vec![
                Shape::Function(ids.clone(), Box::new(Shape::Atom(vec![zero, one]))),
                Shape::Set(ids),
            ],
            widths: vec![2, 2],
        };
        let map = vec![(1, 2), (2, 1)];
        let permutation = WordPermutation::lower(&layout, &map)
            .ok_or_else(|| Error::internal("expected an exact bit permutation"))?;
        for symmetry in [
            Symmetry::new(vec![map.clone()]),
            Symmetry::new(vec![map.clone()]).tlc(vec![1, 0]),
        ] {
            let mut canonical_to_word = HashMap::new();
            let mut word_to_canonical = HashMap::new();
            let mut output = Vec::new();
            for rank in 0..16 {
                let mut offset = 0;
                let state = layout
                    .fields
                    .iter()
                    .zip(&layout.widths)
                    .map(|(shape, &width)| {
                        let field = extract(rank, offset, width);
                        offset += width;
                        shape.unpack(&mut heap, field).ok_or_else(Error::stop)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                symmetry.canonical(&mut heap, &state, &mut output)?;
                let ordinary = layout.pack(&heap, &output).ok_or_else(Error::stop)?;
                let rank = u32::try_from(rank).map_err(|_| Error::stop())?;
                let packed = rank.min(permutation.apply(rank));
                assert_eq!(*canonical_to_word.entry(ordinary).or_insert(packed), packed);
                assert_eq!(
                    *word_to_canonical.entry(packed).or_insert(ordinary),
                    ordinary
                );
            }
            assert_eq!(canonical_to_word.len(), 10);
        }
        let moving_atoms = Layout {
            fields: vec![Shape::Atom(vec![Handle::model(1), Handle::model(2)])],
            widths: vec![1],
        };
        assert!(WordPermutation::lower(&moving_atoms, &map).is_none());
        Ok(())
    }
}
