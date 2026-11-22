//! Exact, per-worker successor fragments for pure branches.
use super::{Code, Error, G, Handle, Item, Machine, NONE, Sink};
use std::sync::Arc;
const MAX: usize = 12;
const TAIL: usize = 4;
const TABLE: usize = 16384;
#[derive(Debug, Clone, PartialEq, Eq)]
struct Key {
    g: G,
    tail: [G; TAIL],
    val: [Handle; MAX],
}
#[derive(Debug)]
struct Entry {
    key: Key,
    words: Option<Arc<Vec<Vec<Handle>>>>,
}
#[derive(Debug, Default)]
pub(crate) struct Images {
    table: Vec<Option<Entry>>,
    hits: u64,
    miss: u64,
    outputs: u64,
    kept: u64,
    sites: Vec<[u64;4]>,
}
impl Drop for Images {
    fn drop(&mut self) {
        if self.hits + self.miss != 0
            && let Some(path) = std::env::var_os("DIVE_DELTA_METRICS")
        {
            use std::io::Write;
            if let Ok(mut f)=std::fs::OpenOptions::new().create(true).append(true).open("/home/ubuntu/delta-bysite") {
                for (i,s) in self.sites.iter().enumerate(){if s[0]+s[1]>100{let _=writeln!(f,"{i} {} {} {} {}",s[0],s[1],s[2],s[3]);}}
            }
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = writeln!(
                    f,
                    "{} {} {} {}",
                    self.hits, self.miss, self.outputs, self.kept
                );
            }
        }
    }
}
fn columns(mut mask: u64) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        if mask == 0 {
            return None;
        }
        let c = mask.trailing_zeros() as usize;
        mask &= mask - 1;
        Some(c)
    })
}
struct Capture<'a> {
    out: &'a mut dyn Sink,
    mask: u64,
    rows: Option<Vec<Vec<Handle>>>,
}
impl Sink for Capture<'_> {
    fn exhaustive(&self) -> bool {
        true
    }
    fn emit(&mut self, m: &mut Machine, c: &Code) -> Result<(), Error> {
        if let Some(rows) = &mut self.rows {
            if m.assigned != m.s1.len() || rows.len() >= 8 {
                self.rows = None;
            } else {
                let mut row = Vec::with_capacity(self.mask.count_ones() as usize);
                for v in columns(self.mask) {
                    let h = m.s1[v];
                    if h.is_empty() {
                        self.rows = None;
                        return self.out.emit(m, c);
                    }
                    let h = if h.is_transient() {
                        let Ok(exact) = m.heap.intern_exact(h) else {
                            self.rows = None;
                            return self.out.emit(m, c);
                        };
                        exact
                    } else {
                        h
                    };
                    row.push(h);
                }
                rows.push(row);
            }
        }
        self.out.emit(m, c)
    }
}
impl Machine {
    pub(super) fn projected_branch(
        &mut self,
        c: &Code,
        g: G,
        acts: u32,
        out: &mut dyn Sink,
    ) -> Option<Result<(), Error>> {
        let main = c.successor_projections.get(g as usize)?.as_ref()?;
        if !main.block {
            return None;
        }
        if let Some(super::GNode::LabelArm(_, test)) = c.gnodes.get(g as usize)
            && self
                .known_label(*test, self.slot(test.slot))
                .is_some_and(|v| v != test.label)
        {
            return Some(Ok(()));
        }
        let mut r = main.read;
        let mut w = main.write;
        let mut cp = main.copy;
        let mut key = Key {
            g,
            tail: [NONE; TAIL],
            val: [Handle::EMPTY; MAX],
        };
        let mut tails = [None; TAIL];
        let mut cur = acts;
        let mut nt = 0;
        while cur != NONE {
            if nt == TAIL {
                return None;
            }
            let next = self.conts.get(cur as usize)?;
            let Item::Conj(id, _) = next.item else {
                return None;
            };
            // An already complete destination takes the nominal boolean continuation.
            let all = if self.s1.len() == 64 {
                u64::MAX
            } else {
                (1u64 << self.s1.len()) - 1
            };
            if w | cp == all {
                return None;
            }
            let item = c.successor_projections.get(id as usize)?.as_ref()?;
            r |= item.read | (cp & (item.copy | item.write)) | (w & item.copy);
            w |= item.write;
            cp |= item.copy;
            key.tail[nt] = id;
            tails[nt] = Some(item);
            nt += 1;
            cur = next.next;
        }
        r |= w & cp;
        if w == 0 || w.count_ones() > 12 || r.count_ones() as usize > MAX {
            return None;
        }
        // Every unexplained column would be stored as though it were copied.
        // Incomplete branches are left to the ordinary sink.
        let all = if self.s1.len() == 64 {
            u64::MAX
        } else {
            (1u64 << self.s1.len()) - 1
        };
        if (w | cp) != all {
            return None;
        }
        let mut n = 0;
        let mut hash = u64::from(g);
        for id in &key.tail[..nt] {
            hash = hash.wrapping_mul(0x9e37_79b9).wrapping_add(u64::from(*id));
        }
        let mut add = |h: Handle| {
            if h.is_transient() || n >= MAX {
                return None;
            }
            key.val[n] = h;
            n += 1;
            hash = (hash ^ h.bits())
                .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                .rotate_left(19);
            Some(())
        };
        for i in columns(r) {
            add(*self.s0.get(i)?)?;
        }
        for k in std::iter::once(main).chain(tails.into_iter().flatten()) {
            for slot in &k.slots {
                add(self.slot(*slot))?;
            }
        }
        let bucket =
            (hash.wrapping_mul(0xbf58_476d_1ce4_e5b9) >> (64 - TABLE.trailing_zeros())) as usize;
        if self.heap.images.table.is_empty() {
            self.heap.images.table.resize_with(TABLE, || None);
            self.heap.images.sites.resize(c.gnodes.len(),[0;4]);
        }
        let present = self
            .heap
            .images
            .table
            .get(bucket)?
            .as_ref()
            .filter(|entry| entry.key == key)
            .map(|entry| entry.words.clone());
        if let Some(words) = present {
            self.heap.images.sites[g as usize][0]+=1;
            self.heap.images.hits += 1;
            let Some(words) = words else {
                return Some(Ok(()));
            };
            self.heap.images.sites[g as usize][2]+=words.len() as u64;
            self.heap.images.outputs += words.len() as u64;
            self.s1.copy_from_slice(&self.s0);
            self.assigned = self.s1.len();
            let mut result = Ok(());
            for row in words.iter() {
                for (col, h) in columns(w).zip(row) {
                    self.s1[col] = *h;
                }
                result = out.emit(self, c);
                if result.is_err() {
                    break;
                }
            }
            self.clear_buffer();
            return Some(result);
        }
        self.heap.images.sites[g as usize][1]+=1;
        self.heap.images.miss += 1;
        self.capturing = true;
        let mut capture = Capture {
            out,
            mask: w,
            rows: Some(Vec::new()),
        };
        let result = self.gen_node_inner(c, g, acts, &mut capture);
        self.capturing = false;
        if result.is_ok()
            && self.warnings.is_empty()
            && let Some(words) = capture.rows
        {
            self.heap.images.sites[g as usize][3]+=words.len() as u64;
            self.heap.images.kept += 1;
            self.heap.images.table[bucket] = Some(Entry {
                key,
                words: (!words.is_empty()).then(|| Arc::new(words)),
            });
        }
        Some(result)
    }
}
