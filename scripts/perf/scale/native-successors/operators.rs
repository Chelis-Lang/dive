//! Typed operands of shared source operators in a prepared worker.
//! Each compilation binds fresh handles, frame slots and residual Core nodes.
use super::{Machine, bool_of, eval_err};
use crate::Error;
use crate::handle::Handle;
use crate::plan::{Code, Node, P};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Arg {
    index: u32,
    pub(crate) value: Handle,
}

pub(crate) type Body = fn(&mut Machine, &Code, &[Arg]) -> Result<Handle, Error>;

#[derive(Clone, Debug)]
struct Site {
    body: Body,
    args: Box<[Arg]>,
    origin: u32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Operators {
    indices: Vec<u32>,
    sites: Vec<Site>,
    meter: Option<(std::path::PathBuf, Arc<[AtomicU64]>)>,
}

#[derive(Default)]
struct Signature {
    tokens: Vec<String>,
    args: Vec<Arg>,
}

impl Signature {
    fn node(&mut self, c: &Code, p: P, depth: usize) -> Option<()> {
        if depth > 16 || self.tokens.len() >= 80 {
            return None;
        }
        let node = c.nodes.get(p as usize)?;
        let mut arg = Arg {
            index: p,
            value: Handle::EMPTY,
        };
        let (tag, children): (&str, &[P]) = match node {
            Node::Const(h) => {
                arg.value = *h;
                ("c", &[])
            }
            Node::Var(v) => {
                arg.index = (*v).into();
                ("v", &[])
            }
            Node::Primed(v) => {
                arg.index = (*v).into();
                ("p", &[])
            }
            Node::Slot(s) => {
                arg.index = *s;
                ("s", &[])
            }
            Node::Select(r, h) => {
                arg.value = *h;
                ("field", std::slice::from_ref(r))
            }
            Node::Not(a) => ("not", std::slice::from_ref(a)),
            Node::And(xs) | Node::Or(xs) if (1..=12).contains(&xs.len()) => {
                self.tokens.push(format!(
                    "{}{}",
                    if matches!(node, Node::And(_)) {
                        "and"
                    } else {
                        "or"
                    },
                    xs.len()
                ));
                for x in xs {
                    self.node(c, *x, depth + 1)?;
                }
                return Some(());
            }
            Node::Eq(a, b)
            | Node::Neq(a, b)
            | Node::In(a, b)
            | Node::NotIn(a, b)
            | Node::Implies(a, b)
            | Node::Apply(a, b) => {
                self.tokens.push(
                    match node {
                        Node::Eq(..) => "eq",
                        Node::Neq(..) => "ne",
                        Node::In(..) => "in",
                        Node::NotIn(..) => "ni",
                        Node::Implies(..) => "imp",
                        _ => "at",
                    }
                    .into(),
                );
                self.node(c, *a, depth + 1)?;
                self.node(c, *b, depth + 1)?;
                return Some(());
            }
            // A residual subtree is evaluated only on its original demand.
            // In particular a Lazy, Scope or Memo stays under Core's authority.
            _ => ("x", &[]),
        };
        self.tokens.push(tag.into());
        if tag != "not" {
            self.args.push(arg);
        }
        for x in children {
            self.node(c, *x, depth + 1)?;
        }
        Some(())
    }
}

impl Operators {
    /// Keep wrappers and quantifier binders in Core, admitting their actual
    /// bodies only when evaluated. A site is identified by the compiled node.
    fn candidates(c: &Code) -> BTreeMap<P, u32> {
        let mut roots = BTreeMap::new();
        for &(mut p, origin) in &c.operator_roots {
            for _ in 0..16 {
                match c.nodes.get(p as usize) {
                    Some(
                        Node::Memo { body, .. }
                        | Node::Lazy { body, .. }
                        | Node::Scope { body, .. }
                        | Node::Quant { body, .. },
                    ) => p = *body,
                    _ => break,
                }
            }
            roots.entry(p).or_insert(origin);
        }
        roots
    }
    fn signature(c: &Code, p: P) -> Option<(String, Box<[Arg]>)> {
        let mut shape = Signature::default();
        shape.node(c, p, 0)?;
        // A single residual or trivial leaf is cheaper in the evaluator.
        if shape.tokens.len() < 3 {
            return None;
        }
        Some((shape.tokens.join(" "), shape.args.into()))
    }
    pub(crate) fn shapes(c: &Code) -> BTreeSet<String> {
        Self::candidates(c)
            .keys()
            .filter_map(|p| Self::signature(c, *p).map(|x| x.0))
            .collect()
    }
    pub(crate) fn bind(c: &Code) -> Self {
        let mut out = Self::default();
        let mut shapes = BTreeMap::new();
        out.indices.resize(c.nodes.len(), 0);
        for (p, origin) in Self::candidates(c) {
            let Some((shape, args)) = Self::signature(c, p) else {
                continue;
            };
            let kernel = *shapes
                .entry(shape.clone())
                .or_insert_with(|| crate::generate::native_study::operator(&shape));
            if let Some(body) = kernel {
                out.sites.push(Site { body, args, origin });
                out.indices[p as usize] = out.sites.len() as u32;
            }
        }
        if let Some(path) = std::env::var_os("DIVE_NATIVE_OPERATOR_METER") {
            out.meter = Some((
                path.into(),
                (0..out.sites.len()).map(|_| AtomicU64::new(0)).collect(),
            ));
        }
        out
    }
    #[inline]
    pub(crate) fn run(&self, m: &mut Machine, c: &Code, p: P) -> Option<Result<Handle, Error>> {
        let index = self.indices.get(p as usize)?.checked_sub(1)? as usize;
        if m.fuel != u64::MAX
            || m.debugger.is_some()
            || m.transient_bound != usize::MAX
            || m.heap.speculative
        {
            return None;
        }
        let site = &self.sites[index];
        if let Some((path, counters)) = &self.meter {
            use std::io::Write;
            let count = counters[index].fetch_add(1, Ordering::Relaxed) + 1;
            if count.is_power_of_two()
                && let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
            {
                let line = format!("{} {p} {count}\n", site.origin);
                let _ = f.write_all(line.as_bytes());
            }
        }
        Some((site.body)(m, c, &site.args))
    }
}

pub(crate) fn state(m: &Machine, a: Arg, prime: bool) -> Result<Handle, Error> {
    let value = if prime { &m.s1 } else { &m.s0 }
        .get(a.index as usize)
        .copied()
        .unwrap_or(Handle::EMPTY);
    if value.is_empty() {
        Err(eval_err(if prime {
            "read of an unassigned primed variable"
        } else {
            "read of an unassigned variable"
        }))
    } else {
        Ok(value)
    }
}
pub(crate) fn slot(m: &Machine, a: Arg) -> Result<Handle, Error> {
    let value = m.slot(a.index);
    if value.is_empty() {
        Err(Error::internal("read of an unbound slot"))
    } else {
        Ok(value)
    }
}
pub(crate) fn residual(m: &mut Machine, c: &Code, a: Arg) -> Result<Handle, Error> {
    m.eval(c, a.index)
}
pub(crate) fn field(m: &mut Machine, v: Handle, a: Arg) -> Result<Handle, Error> {
    m.select(v, a.value)
}
pub(crate) fn boolean(v: Handle) -> Result<bool, Error> {
    bool_of(v)
}
