//! Shared continuations for an isolated source-built experiment.

use super::{Code, Error, GNode, Handle, Item, Machine, NONE, Sink};
use crate::native_blocks::Register;
type Finish = fn(&mut Machine, &Code, &mut dyn Sink, &mut Register) -> Result<(), Error>;
#[inline]
fn end(m: &mut Machine, c: &Code, s: &mut dyn Sink, _r: &mut Register) -> Result<(), Error> {
    s.emit(m, c)
}
#[inline]
fn generic(
    m: &mut Machine,
    c: &Code,
    s: &mut dyn Sink,
    _r: &mut Register,
    g: u32,
    k: &[(u32, u32)],
) -> Result<(), Error> {
    let mark = m.conts.len();
    let mut t = NONE;
    for &(id, p) in k.iter().rev() {
        t = m.push_cont(Item::Conj(id, p), t)
    }
    let result = m.gen_node(c, g, t, s);
    m.conts.truncate(mark);
    result
}
#[inline]
fn assign(
    m: &mut Machine,
    c: &Code,
    s: &mut dyn Sink,
    r: &mut Register,
    site: usize,
    v: u16,
    rhs: u32,
    k: Finish,
) -> Result<(), Error> {
    let value = match c.native_blocks.write(m,c,r,site)? {
        Some(value) => value, None => m.eval_component(c,rhs)?,
    };
    let present = m.s1.get(usize::from(v)).copied().unwrap_or(Handle::EMPTY);
    if present.is_empty() {
        m.bind_var(v, value);
        let out = k(m, c, s, r);
        m.unbind_var(v);
        out
    } else if m.demand_equals(present, value)? {
        k(m, c, s, r)
    } else {
        Ok(())
    }
}
#[inline]
fn copy(
    m: &mut Machine,
    c: &Code,
    s: &mut dyn Sink,
    r: &mut Register,
    indices: &[u16],
    g: u32,
    t: &[(u32, u32)],
    k: Finish,
) -> Result<(), Error> {
    if indices.iter().any(|&v| {
        m.s1.get(usize::from(v)).is_none_or(|x| !x.is_empty())
            || m.s0.get(usize::from(v)).is_none_or(|x| x.is_empty())
    }) {
        return generic(m, c, s, r, g, t);
    }
    for &v in indices {
        m.s1[usize::from(v)] = m.s0[usize::from(v)];
    }
    m.assigned += indices.len();
    let out = k(m, c, s, r);
    m.assigned -= indices.len();
    for &v in indices {
        m.s1[usize::from(v)] = Handle::EMPTY;
    }
    out
}
#[inline]
fn scope(
    m: &mut Machine,
    c: &Code,
    s: &mut dyn Sink,
    r: &mut Register,
    slots: &[u32],
    k: Finish,
) -> Result<(), Error> {
    let mark = m.save(slots.iter().copied());
    for &v in slots {
        m.set_slot(v, Handle::EMPTY);
    }
    let result = k(m, c, s, r);
    m.restore(mark, slots.iter().copied());
    result
}
#[inline]
fn exists(
    m: &mut Machine,
    c: &Code,
    s: &mut dyn Sink,
    r: &mut Register,
    g: u32,
    slots: &[u32],
    k: Finish,
) -> Result<(), Error> {
    let GNode::Exists { binds, .. } = &c.gnodes[g as usize] else {
        return Err(Error::internal("native binder shape"));
    };
    m.charge(slots.len())?;
    let mark = m.save(slots.iter().copied());
    let out = m
        .existential_bindings(c, binds, &mut |m| {
            k(m, c, s, r)?;
            Ok(true)
        })
        .map(|_| ());
    m.restore(mark, slots.iter().copied());
    out
}

#[inline]
fn test(m:&mut Machine,c:&Code,r:&mut Register,g:usize,p:u32)->Result<bool,Error>{
    match c.native_blocks.test(m,r,g){Some(value)=>Ok(value),None=>m.eval_bool(c,p)}
}
