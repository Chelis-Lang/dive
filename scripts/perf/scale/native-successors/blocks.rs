//! Bound operations used by source-emitted guard and write continuations.
use crate::handle::Handle;
use crate::plan::{Code, GNode, Node, P};
use crate::{Error, Machine};

#[derive(Clone, Copy, Debug)]
enum Atom {
    Constant(Handle),
    State(u16),
    Next(u16),
    Slot(u32),
}

impl Atom {
    fn bind(c: &Code, p: P) -> Option<Self> {
        Some(match *c.nodes.get(p as usize)? {
            Node::Const(h) if !h.is_compound() && !h.is_empty() => Self::Constant(h),
            Node::Var(v) => Self::State(v),
            Node::Primed(v) => Self::Next(v),
            Node::Slot(s) => Self::Slot(s),
            _ => return None,
        })
    }

    fn value(self, m: &Machine) -> Option<Handle> {
        let h = match self {
            Self::Constant(h) => h,
            Self::State(v) => *m.s0.get(usize::from(v))?,
            Self::Next(v) => *m.s1.get(usize::from(v))?,
            Self::Slot(s) => m.slot(s),
        };
        (!h.is_empty()).then_some(h)
    }
}

#[derive(Clone, Copy, Debug)]
struct Cell {
    table: Handle,
    key: Handle,
    pos: usize,
    row: Handle,
}

/// A register lives on the stack for one emitted action. Neither a Store epoch
/// nor an arena rollback can extend its lifetime. Only immutable persistent
/// carriers may be retained across continuations.
#[derive(Default)]
pub(crate) struct Register(Option<Cell>, Option<(Handle, Handle)>);
impl Register {
    pub(crate) fn actor(&mut self, m: &Machine, var: u16, slot: u32) -> Option<Handle> {
        let table = *m.s0.get(usize::from(var))?;
        Some(self.resolve(m, table, m.slot(slot))?.row)
    }
    pub(crate) fn field(&self, m: &Machine, row: Handle, key: Handle) -> Option<Handle> {
        if key.is_empty() {
            Some(row)
        } else {
            m.native_entry(row, key, true).map(|e| e.1)
        }
    }
    /// Test an original label while retaining its checked carrier and position
    /// for a following write. The Core label table stores only verified strings.
    pub(crate) fn scalar_label(
        &mut self,
        m: &mut Machine,
        code: &Code,
        predicate: P,
        test: crate::plan::LabelTest,
    ) -> Result<bool, Error> {
        let key = m.slot(test.slot);
        if !code.native_blocks.enabled || m.known_label(test, key).is_some_and(|v| v != test.label)
        {
            return m.label_matches(code, predicate, test);
        }
        let Some(&table) = m.s0.get(usize::from(test.table)) else {
            return m.label_matches(code, predicate, test);
        };
        if table.is_persistent()
            && !table.is_lazy_persistent()
            && let Some(cell) = self.resolve(m, table, key)
            && test.field.is_empty()
            && cell.row.as_str_id().is_some()
        {
            let label = cell.row;
            m.heap.labels[test.bucket()] = [table, key, test.field, label];
            if code.native_blocks.meter.is_some() {
                code.native_blocks.record(Event::LabelRead);
                self.1 = Some((table, key));
            }
            return Ok(label == test.label);
        }
        m.label_matches(code, predicate, test)
    }
    fn resolve(&mut self, m: &Machine, table: Handle, key: Handle) -> Option<Cell> {
        if let Some(cell) = self.0
            && cell.table == table
            && cell.key == key
        {
            return Some(cell);
        }
        let (pos, row) = m.native_entry(table, key, false)?;
        let cell = Cell {
            table,
            key,
            pos,
            row,
        };
        if table.is_persistent() && !table.is_lazy_persistent() && !row.is_transient() {
            self.0 = Some(cell);
            // A write can replace the register and later revisit the same key.
            // Only a fresh scalar guard may mark the replacement as its result.
            self.1 = None;
        }
        Some(cell)
    }
}

#[derive(Clone, Debug)]
struct Write {
    rhs: P,
    table: Atom,
    actor: Atom,
}

/// Exact operands derived once from the current compiled value nodes.
#[derive(Clone, Debug, Default)]
pub struct Blocks {
    pub(crate) enabled: bool,
    meter: Option<std::path::PathBuf>,
    writes: Vec<Option<Write>>,
    domains: Vec<Option<crate::native_binders::Domain>>,
    quant: std::collections::BTreeMap<u32, crate::native_binders::Domain>,
    history: Vec<Option<Box<crate::native_history::History>>>,
}
impl Blocks {
    pub(crate) fn bind(c: &Code, heap: &crate::heap::Heap) -> Self {
        let mut writes = Vec::with_capacity(c.gnodes.len());
        for node in &c.gnodes {
            let w = match node {
                GNode::Assign { rhs, .. } => {
                    let rhs = *rhs;
                    if let Node::Except {
                        base,
                        updates,
                        row_fields,
                    } = &c.nodes[rhs as usize]
                        && (row_fields.is_some() || matches!(&**updates,[u] if u.path.len()==1))
                    {
                        Atom::bind(c, *base)
                            .zip(
                                updates
                                    .first()
                                    .and_then(|u| u.path.first())
                                    .and_then(|p| Atom::bind(c, *p)),
                            )
                            .map(|(table, actor)| Write { rhs, table, actor })
                    } else {
                        None
                    }
                }
                _ => None,
            };
            writes.push(w);
        }
        let domains = c
            .gnodes
            .iter()
            .map(|g| match g {
                GNode::Exists { binds, .. } => crate::native_binders::Domain::bind(heap, c, binds),
                _ => None,
            })
            .collect();
        let quant = c
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                if let Node::Quant {
                    binds,
                    kind: crate::plan::Quant::Forall | crate::plan::Quant::Exists,
                    ..
                } = v
                {
                    Some((
                        u32::try_from(i).ok()?,
                        crate::native_binders::Domain::bind(heap, c, binds)?,
                    ))
                } else {
                    None
                }
            })
            .collect();
        Self {
            enabled: true,
            meter: std::env::var_os("DIVE_NATIVE_BLOCK_METER").map(Into::into),
            writes,
            domains,
            quant,
            history: crate::native_history::History::sites(c, heap),
        }
    }

    pub(crate) fn domain(&self, g: usize) -> Option<&crate::native_binders::Domain> {
        self.domains.get(g)?.as_ref()
    }
    pub(crate) fn quant(&self, p: u32) -> Option<&crate::native_binders::Domain> {
        self.quant.get(&p)
    }
    pub(crate) fn history(&self, m: &mut Machine, site: u32) -> Option<Handle> {
        let (value, changed) = self.history.get(site as usize)?.as_ref()?.update(m)?;
        self.record(if changed {
            Event::HistoryChanged
        } else {
            Event::History
        });
        Some(value)
    }

    pub(crate) fn write(
        &self,
        m: &mut Machine,
        c: &Code,
        r: &mut Register,
        g: usize,
    ) -> Result<Option<Handle>, Error> {
        let Some(w) = self.writes.get(g).and_then(Option::as_ref) else {
            return Ok(None);
        };
        let Some((table, key)) = w.table.value(m).zip(w.actor.value(m)) else {
            return Ok(None);
        };
        // The source normalizes the carrier before processing its selectors.
        // A canonical persistent function is already in that state.
        if !table.is_persistent() || table.is_lazy_persistent() {
            return Ok(None);
        }
        let from_guard = self.meter.is_some()
            && r.1 == Some((table, key))
            && r.0
                .is_some_and(|cell| cell.table == table && cell.key == key);
        let Some(cell) = r.resolve(m, table, key) else {
            return Ok(None);
        };
        let Node::Except {
            updates,
            row_fields,
            ..
        } = &c.nodes[w.rhs as usize]
        else {
            return Ok(None);
        };
        if let Some(plan) = row_fields {
            if !m.opts.actor_rows {
                return Ok(None);
            }
            let value = m.native_row_write(c, table, updates, plan, cell.pos, cell.row)?;
            if value.is_some() {
                self.record(Event::RowWrite)
            }
            return Ok(value);
        }
        let [update] = &**updates else {
            return Ok(None);
        };
        let v = m.heap.view(table);
        let (lookups, copies) = (v.len(), v.data.len());
        m.charge(lookups)?;
        m.set_slot(update.at, cell.row);
        let new = m.eval_component(c, update.value)?;
        self.record(Event::CellWrite);
        if from_guard {
            self.record(Event::GuardWrite);
        }
        if new == cell.row {
            return Ok(Some(table));
        }
        m.charge(copies)?;
        Ok(Some(m.heap.fcn_with(table, cell.pos, new)))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Event {
    Route,
    RowWrite,
    CellWrite,
    History,
    HistoryChanged,
    Exists,
    Quant,
    LabelRead,
    GuardWrite,
}
impl Blocks {
    pub(crate) fn record(&self, event: Event) {
        use std::io::Write;
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTS: [AtomicU64; 9] = [const { AtomicU64::new(0) }; 9];
        if let Some(path) = &self.meter {
            let n = COUNTS[event as usize].fetch_add(1, Ordering::Relaxed) + 1;
            if n.is_power_of_two()
                && let Ok(mut out) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
            {
                let _ = out.write_all(format!("{event:?} {n}\n").as_bytes());
            }
        }
    }
}
