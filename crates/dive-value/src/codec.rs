//! A compact, deterministic byte encoding of values.
//!
//! State storage (`dive-store`) keeps states as bytes: frontier blocks, the exact
//! store, disk spill and checkpoints. This module encodes one value; a state is the
//! concatenation of its variables' encodings. Two encodings are provided.
//!
//! - [`encode`] is **lossless**. Decoding gives back the same representation, not just
//!   an equal value: `1..3` stays an interval, an unnormalized set stays unnormalized
//!   (with TLC's `isNorm` flag), a record literal stays a record literal. TLC's
//!   observable behaviour depends on representation (printing, comparability), so the
//!   frontier and disk spill use this encoding.
//! - [`encode_canonical`] is the **exact-store key**. Within one run (one
//!   [`Interner`](crate::Interner) and one [`Ctx`]), two values have equal canonical
//!   bytes exactly when they are equal (`==`). Sets are written as their normalized
//!   elements, a non-empty set of consecutive integers as its bounds (so `1..3` and
//!   `{1, 2, 3}` agree, and `0..2147483646` costs O(1)), and records ignore the
//!   record-literal bit. It fails exactly where TLC's fingerprinting fails (`Nat` in a
//!   state, say) and normalizes what fingerprinting normalizes. Decoding it gives an
//!   equal value, possibly with another representation.
//!
//! Both encodings are prefix-free, so a concatenation of encodings decodes uniquely.
//! They are deterministic and platform-independent: integers are LEB128 varints
//! (zigzag for signed values), and names are written as their intern token
//! ([`Atom::tok`]), which [`decode`] resolves through the run's interner
//! ([`Interner::resolve`](crate::Interner::resolve)).
//!
//! Encoding and decoding are iterative, so any nesting depth is safe.
//!
//! The structural tuple/function distinction was ambiguous in version 1: both
//! used `0x08`. Persisted value-bearing formats must reject old archives, since
//! the missing literal bit cannot be inferred. Raw old `0x08` decodes only as a
//! literal tuple; it is not a migration of an old tuple-domain function.
//! Canonical keys deliberately keep `0x08` for both equal representations.
//!
//! # Structural format (version 2)
//!
//! | Tag | Value | Payload |
//! |---|---|---|
//! | `0x00`, `0x01` | `FALSE`, `TRUE` | |
//! | `0x02` | integer in `i64` range | zigzag varint |
//! | `0x03` | integer outside `i64` range | sign byte, varint length, magnitude bytes (little-endian) |
//! | `0x04` | string | varint token |
//! | `0x05` | model value | varint token |
//! | `0x08` | literal tuple (also the unchanged canonical-key tuple tag) | varint `n`, `n` values |
//! | `0x0A` | nonliteral tuple-domain function | varint `n`, `n` values |
//! | `0x10` | normalized set | varint `n`, `n` values in the set's order |
//! | `0x12` | unnormalized set | `isNorm` byte, varint `n`, `n` values as constructed |
//! | `0x14` | `lo..hi` | two integers |
//! | `0x16` | `SUBSET S` | `S` |
//! | `0x18` | `[S -> T]` | `S`, `T` |
//! | `0x1A` | `[f1 : S1, ...]` | varint `n`, `n` × (token, value) |
//! | `0x1C` | `S1 \X ...` | varint `n`, `n` values |
//! | `0x1E`, `0x20`, `0x22` | `Nat`, `Int`, `STRING` | |
//! | `0x24` | `Seq(S)` | `S` |
//! | `0x26` | record literal | varint `n`, `n` × (token, value) |
//! | `0x28` | function with a string domain | varint `n`, `n` × (token, value) |
//! | `0x2A` | other function | varint `n`, `n` × (argument, value) |
//!
//! For tags `0x10` and above, bit 0 is the [`ValueOrder`] the set or function is
//! sorted in (`1` for [`ValueOrder::Tlc`]). Evaluator-driven lazy sets
//! ([`DeferredSet`](crate::DeferredSet)) cannot be written as their definition; both
//! encodings write their elements as a normalized set, which needs a [`Ctx`] and can
//! fail with the evaluator's error.
//!
//! ```
//! use dive_value::{codec, Ctx, Int, Interner, Value};
//!
//! let names = Interner::new();
//! let ctx = Ctx::tlc();
//! let v = Value::tuple(vec![
//!     Value::interval(Int::from(1), Int::from(3), ctx),
//!     Value::string(&names, "a"),
//! ]);
//! let mut bytes = Vec::new();
//! codec::encode(&v, ctx, &mut bytes)?;
//! let back = codec::decode(&mut bytes.as_slice(), &mut |t| names.resolve(t))?;
//! assert_eq!(back, v);
//!
//! // The canonical key does not depend on representation.
//! let (mut k1, mut k2) = (Vec::new(), Vec::new());
//! codec::encode_canonical(&Value::interval(Int::from(1), Int::from(3), ctx), ctx, &mut k1)?;
//! let enumerated = Value::set(vec![Value::Int(3), Value::Int(1), Value::Int(2)], ctx)?;
//! codec::encode_canonical(&enumerated, ctx, &mut k2)?;
//! assert_eq!(k1, k2);
//! # Ok::<(), dive_value::ValueError>(())
//! ```

use num_bigint::{BigInt, Sign};

use crate::atom::Atom;
use crate::ctx::{Ctx, ValueOrder};
use crate::error::{ErrorKind, Result, ValueError};
use crate::fp64::{is_fingerprintable, not_fingerprintable};
use crate::int::Int;
use crate::stream::elems_in;
use crate::value::{Fcn, FcnRepr, Set, SetRepr, Value};

mod tag {
    pub const FALSE: u8 = 0x00;
    pub const TRUE: u8 = 0x01;
    pub const INT: u8 = 0x02;
    pub const BIGINT: u8 = 0x03;
    pub const STR: u8 = 0x04;
    pub const MODEL: u8 = 0x05;
    pub const TUPLE: u8 = 0x08;
    pub const TUPLEFCN: u8 = 0x0A;
    pub const SET: u8 = 0x10;
    pub const RAW: u8 = 0x12;
    pub const INTERVAL: u8 = 0x14;
    pub const SUBSET: u8 = 0x16;
    pub const FCNSET: u8 = 0x18;
    pub const RCDSET: u8 = 0x1A;
    pub const PRODUCT: u8 = 0x1C;
    pub const NAT: u8 = 0x1E;
    pub const INTSET: u8 = 0x20;
    pub const STRINGS: u8 = 0x22;
    pub const SEQ: u8 = 0x24;
    pub const RECORD: u8 = 0x26;
    pub const RCDFCN: u8 = 0x28;
    pub const FCN: u8 = 0x2A;
}

fn order_bit(order: ValueOrder) -> u8 {
    match order {
        ValueOrder::Canonical => 0,
        ValueOrder::Tlc => 1,
    }
}

fn order_of(tag: u8) -> ValueOrder {
    if tag & 1 == 1 {
        ValueOrder::Tlc
    } else {
        ValueOrder::Canonical
    }
}

/// Appends `x` as an unsigned LEB128 varint.
pub fn write_varint(out: &mut Vec<u8>, mut x: u64) {
    while x >= 0x80 {
        out.push(x.to_le_bytes()[0] | 0x80);
        x >>= 7;
    }
    out.push(x.to_le_bytes()[0]);
}

/// Reads an unsigned LEB128 varint written by [`write_varint`].
///
/// # Errors
/// [`ErrorKind::Codec`] on truncated or overlong input.
pub fn read_varint(input: &mut &[u8]) -> Result<u64> {
    let mut x = 0u64;
    for shift in (0..64).step_by(7) {
        let b = read_u8(input)?;
        let low = u64::from(b & 0x7F);
        if shift == 63 && low > 1 {
            return Err(corrupt("varint overflows 64 bits"));
        }
        x |= low << shift;
        if b & 0x80 == 0 {
            return Ok(x);
        }
    }
    Err(corrupt("varint longer than 10 bytes"))
}

fn zigzag(v: i64) -> u64 {
    #[allow(clippy::cast_sign_loss, reason = "zigzag is a bit reinterpretation")]
    let z = ((v << 1) ^ (v >> 63)) as u64;
    z
}

fn unzigzag(z: u64) -> i64 {
    #[allow(
        clippy::cast_possible_wrap,
        reason = "zigzag is a bit reinterpretation"
    )]
    let v = ((z >> 1) as i64) ^ -((z & 1) as i64);
    v
}

fn corrupt(what: &str) -> ValueError {
    ValueError::new(ErrorKind::Codec, format!("invalid value encoding: {what}"))
}

fn read_u8(input: &mut &[u8]) -> Result<u8> {
    let (&b, rest) = input.split_first().ok_or_else(|| corrupt("truncated"))?;
    *input = rest;
    Ok(b)
}

fn write_len(out: &mut Vec<u8>, n: usize) {
    write_varint(out, n as u64);
}

fn write_tok(out: &mut Vec<u8>, a: &Atom) {
    write_varint(out, u64::from(a.tok()));
}

fn write_int(out: &mut Vec<u8>, i: &Int) {
    if let Some(v) = i.to_i64() {
        out.push(tag::INT);
        write_varint(out, zigzag(v));
    } else {
        let (sign, mag) = i.to_big().to_bytes_le();
        out.push(tag::BIGINT);
        out.push(u8::from(sign == Sign::Minus));
        write_len(out, mag.len());
        out.extend_from_slice(&mag);
    }
}

/// One pending item of an iterative encoding.
enum Frame<'a> {
    /// A value borrowed from the value being encoded.
    Ref(&'a Value),
    /// A value generated on the way, encoded by a nested traversal.
    Own(Value),
    /// A name written as its token.
    Tok(&'a Atom),
}

/// Maximum nodes inspected by one callback-free frontier codec admission.
pub const FRONTIER_MAX_NODES: usize = 65_536;

/// A conservative byte bound for callback-free lossless frontier encoding.
/// This walks representation only, never normalizes a set or evaluates a
/// deferred callback. Its traversal is capped at 65,536 nodes. The ordinary
/// [`encode`] contract remains unchanged.
///
/// # Errors
/// [`ErrorKind::NotEnumerable`] for any evaluator-driven deferred set, or
/// [`ErrorKind::DiveLimit`] when the byte/traversal admission limit is exceeded.
#[allow(clippy::too_many_lines, reason = "all lossless representation forms")]
pub fn frontier_size_bound(value: &Value, maximum: usize) -> Result<usize> {
    let limit = || {
        ValueError::new(
            ErrorKind::DiveLimit,
            "frontier codec byte or 65536-node admission limit",
        )
    };
    let mut stack = vec![value];
    let mut visited = 0usize;
    let mut bytes = 0usize;
    while let Some(v) = stack.pop() {
        visited += 1;
        if visited + stack.len() > FRONTIER_MAX_NODES {
            return Err(limit());
        }
        let mut overhead = 11usize;
        match v {
            Value::Bool(_) => overhead = 1,
            Value::Int(_) | Value::Str(_) | Value::Model(_) => {}
            Value::BigInt(b) => {
                overhead = usize::try_from(b.bits().div_ceil(8))
                    .map_err(|_| limit())?
                    .saturating_add(12);
            }
            Value::Fcn(f) => match &f.0.repr {
                FcnRepr::Tuple(e) => {
                    if visited + stack.len() + e.len() > FRONTIER_MAX_NODES {
                        return Err(limit());
                    }
                    stack.extend(e.iter());
                }
                FcnRepr::Record(fields) => {
                    if visited + stack.len() + fields.len() > FRONTIER_MAX_NODES {
                        return Err(limit());
                    }
                    overhead = overhead.saturating_add(fields.len().saturating_mul(10));
                    stack.extend(fields.iter().map(|(_, v)| v));
                }
                FcnRepr::General(pairs) => {
                    if visited + stack.len() + pairs.len().saturating_mul(2) > FRONTIER_MAX_NODES {
                        return Err(limit());
                    }
                    for (d, v) in pairs {
                        stack.push(d);
                        stack.push(v);
                    }
                }
            },
            Value::Set(set) => match set.repr() {
                SetRepr::Deferred(_) => {
                    return Err(ValueError::new(
                        ErrorKind::NotEnumerable,
                        "frontier spill refuses evaluator-driven deferred values without evaluating them",
                    ));
                }
                SetRepr::Enum => {
                    if let Some(e) = set.0.elems.get() {
                        if visited + stack.len() + e.len() > FRONTIER_MAX_NODES {
                            return Err(limit());
                        }
                        stack.extend(e.iter());
                    }
                }
                SetRepr::Raw(e) | SetRepr::Product(e) => {
                    if visited + stack.len() + e.len() > FRONTIER_MAX_NODES {
                        return Err(limit());
                    }
                    overhead += 1;
                    stack.extend(e.iter());
                }
                SetRepr::Subset(v) | SetRepr::Seq(v) => stack.push(v),
                SetRepr::FcnSet(d, r) => {
                    stack.push(d);
                    stack.push(r);
                }
                SetRepr::RcdSet(fields) => {
                    if visited + stack.len() + fields.len() > FRONTIER_MAX_NODES {
                        return Err(limit());
                    }
                    overhead = overhead.saturating_add(fields.len().saturating_mul(10));
                    stack.extend(fields.iter().map(|(_, v)| v));
                }
                SetRepr::Interval(a, b) => {
                    for i in [a, b] {
                        let n = match i {
                            Int::Small(_) => 11,
                            Int::Big(v) => usize::try_from(v.bits().div_ceil(8))
                                .map_err(|_| limit())?
                                .saturating_add(12),
                        };
                        overhead = overhead.saturating_add(n);
                    }
                }
                SetRepr::Nat | SetRepr::Int | SetRepr::Strings => overhead = 1,
            },
        }
        bytes = bytes.checked_add(overhead).ok_or_else(limit)?;
        if bytes > maximum {
            return Err(limit());
        }
    }
    Ok(bytes)
}

/// Encodes `v` losslessly, appending to `out` (see the module documentation).
///
/// `ctx` is needed only to enumerate evaluator-driven lazy sets.
///
/// # Errors
/// Only from enumerating an evaluator-driven lazy set.
pub fn encode(v: &Value, ctx: Ctx, out: &mut Vec<u8>) -> Result<()> {
    let mut stack = vec![Frame::Ref(v)];
    while let Some(frame) = stack.pop() {
        match frame {
            Frame::Tok(a) => write_tok(out, a),
            Frame::Own(v) => encode(&v, ctx, out)?,
            Frame::Ref(v) => encode_step(v, ctx, out, &mut stack)?,
        }
    }
    Ok(())
}

fn tuple_tag(f: &Fcn) -> u8 {
    if f.is_tuple_literal() {
        tag::TUPLE
    } else {
        tag::TUPLEFCN
    }
}

#[allow(
    clippy::many_single_char_names,
    reason = "short names mirror the format table: value, set, function, order"
)]
fn encode_step<'a>(
    v: &'a Value,
    ctx: Ctx,
    out: &mut Vec<u8>,
    stack: &mut Vec<Frame<'a>>,
) -> Result<()> {
    match v {
        Value::Bool(b) => out.push(if *b { tag::TRUE } else { tag::FALSE }),
        Value::Int(_) | Value::BigInt(_) => write_int(out, &v.as_int().unwrap_or(Int::ZERO)),
        Value::Str(a) => {
            out.push(tag::STR);
            write_tok(out, a);
        }
        Value::Model(a) => {
            out.push(tag::MODEL);
            write_tok(out, a);
        }
        Value::Set(s) => {
            let o = order_bit(s.order());
            match s.repr() {
                SetRepr::Enum | SetRepr::Deferred(_) => {
                    if matches!(s.repr(), SetRepr::Deferred(_)) {
                        s.elements(ctx)?;
                    }
                    let e = s.0.elems.get().map_or(&[][..], |e| &e[..]);
                    out.push(tag::SET | o);
                    write_len(out, e.len());
                    stack.extend(e.iter().rev().map(Frame::Ref));
                }
                SetRepr::Raw(e) => {
                    out.push(tag::RAW | o);
                    out.push(u8::from(s.is_tlc_normalized()));
                    write_len(out, e.len());
                    stack.extend(e.iter().rev().map(Frame::Ref));
                }
                SetRepr::Interval(lo, hi) => {
                    out.push(tag::INTERVAL | o);
                    write_int(out, lo);
                    write_int(out, hi);
                }
                SetRepr::Subset(b) => {
                    out.push(tag::SUBSET | o);
                    stack.push(Frame::Ref(b));
                }
                SetRepr::Seq(b) => {
                    out.push(tag::SEQ | o);
                    stack.push(Frame::Ref(b));
                }
                SetRepr::FcnSet(d, r) => {
                    out.push(tag::FCNSET | o);
                    stack.push(Frame::Ref(r));
                    stack.push(Frame::Ref(d));
                }
                SetRepr::RcdSet(fs) => {
                    out.push(tag::RCDSET | o);
                    write_len(out, fs.len());
                    for (a, x) in fs.iter().rev() {
                        stack.push(Frame::Ref(x));
                        stack.push(Frame::Tok(a));
                    }
                }
                SetRepr::Product(fs) => {
                    out.push(tag::PRODUCT | o);
                    write_len(out, fs.len());
                    stack.extend(fs.iter().rev().map(Frame::Ref));
                }
                SetRepr::Nat => out.push(tag::NAT | o),
                SetRepr::Int => out.push(tag::INTSET | o),
                SetRepr::Strings => out.push(tag::STRINGS | o),
            }
        }
        Value::Fcn(f) => {
            let o = order_bit(f.order());
            match &f.0.repr {
                FcnRepr::Tuple(e) => {
                    out.push(tuple_tag(f));
                    write_len(out, e.len());
                    stack.extend(e.iter().rev().map(Frame::Ref));
                }
                FcnRepr::Record(fs) => {
                    let t = if f.is_record_literal() {
                        tag::RECORD
                    } else {
                        tag::RCDFCN
                    };
                    out.push(t | o);
                    write_len(out, fs.len());
                    for (a, x) in fs.iter().rev() {
                        stack.push(Frame::Ref(x));
                        stack.push(Frame::Tok(a));
                    }
                }
                FcnRepr::General(ps) => {
                    out.push(tag::FCN | o);
                    write_len(out, ps.len());
                    for (d, x) in ps.iter().rev() {
                        stack.push(Frame::Ref(x));
                        stack.push(Frame::Ref(d));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Encodes `v` as its canonical exact-store key, appending to `out` (see the module
/// documentation): within one run, equal values, and only equal values, have equal
/// keys.
///
/// # Errors
/// Exactly TLC's fingerprinting errors: a set that cannot be fingerprinted (such as
/// `Nat`), and size, enumeration and normalization errors.
pub fn encode_canonical(v: &Value, ctx: Ctx, out: &mut Vec<u8>) -> Result<()> {
    let mut stack = vec![Frame::Ref(v)];
    while let Some(frame) = stack.pop() {
        match frame {
            Frame::Tok(a) => write_tok(out, a),
            Frame::Own(v) => encode_canonical(&v, ctx, out)?,
            Frame::Ref(v) => canonical_step(v, ctx, out, &mut stack)?,
        }
    }
    Ok(())
}

/// The bounds `(lo, hi)` if the normalized `elems` are exactly the integers `lo..hi`.
fn int_range(elems: &[Value]) -> Option<(Int, Int)> {
    let first = elems.first()?.as_int()?;
    let last = elems.last()?.as_int()?;
    if !elems
        .iter()
        .all(|e| matches!(e, Value::Int(_) | Value::BigInt(_)))
    {
        return None;
    }
    // Normalized integer sets are sorted ascending without duplicates (in both
    // orders), so they are consecutive exactly when they span `len` values.
    let span = last.to_big() - first.to_big() + 1u32;
    (span == BigInt::from(elems.len())).then_some((first, last))
}

fn canonical_step<'a>(
    v: &'a Value,
    ctx: Ctx,
    out: &mut Vec<u8>,
    stack: &mut Vec<Frame<'a>>,
) -> Result<()> {
    match v {
        Value::Set(s) => canonical_set(v, s, ctx, out, stack),
        Value::Fcn(f) => {
            canonical_fcn(f, ctx, out, stack);
            Ok(())
        }
        _ => encode_step(v, ctx, out, stack),
    }
}

fn canonical_set<'a>(
    v: &'a Value,
    s: &'a Set,
    ctx: Ctx,
    out: &mut Vec<u8>,
    stack: &mut Vec<Frame<'a>>,
) -> Result<()> {
    if !is_fingerprintable(s) {
        return Err(not_fingerprintable(v, ctx));
    }
    // As TLC's fingerprinting, size (and normalize) the set first.
    s.size(ctx)?;
    let o = order_bit(ctx.order);
    if let SetRepr::Interval(lo, hi) = s.repr() {
        if hi < lo {
            out.push(tag::SET | o);
            write_len(out, 0);
        } else {
            out.push(tag::INTERVAL | o);
            write_int(out, lo);
            write_int(out, hi);
        }
        return Ok(());
    }
    if s.order() == ctx.order {
        s.elements(ctx)?;
        let e = s.0.elems.get().map_or(&[][..], |e| &e[..]);
        if let Some((lo, hi)) = int_range(e) {
            out.push(tag::INTERVAL | o);
            write_int(out, &lo);
            write_int(out, &hi);
        } else {
            out.push(tag::SET | o);
            write_len(out, e.len());
            stack.extend(e.iter().rev().map(Frame::Ref));
        }
    } else {
        let e = elems_in(s, ctx, true)?;
        if let Some((lo, hi)) = int_range(&e) {
            out.push(tag::INTERVAL | o);
            write_int(out, &lo);
            write_int(out, &hi);
        } else {
            out.push(tag::SET | o);
            write_len(out, e.len());
            stack.extend(e.iter().rev().cloned().map(Frame::Own));
        }
    }
    Ok(())
}

fn canonical_fcn<'a>(f: &'a Fcn, ctx: Ctx, out: &mut Vec<u8>, stack: &mut Vec<Frame<'a>>) {
    let o = order_bit(ctx.order);
    match &f.0.repr {
        FcnRepr::Tuple(e) => {
            out.push(tag::TUPLE);
            write_len(out, e.len());
            stack.extend(e.iter().rev().map(Frame::Ref));
        }
        FcnRepr::Record(fs) if f.order() == ctx.order => {
            out.push(tag::RCDFCN | o);
            write_len(out, fs.len());
            for (a, x) in fs.iter().rev() {
                stack.push(Frame::Ref(x));
                stack.push(Frame::Tok(a));
            }
        }
        FcnRepr::General(ps) if f.order() == ctx.order => {
            out.push(tag::FCN | o);
            write_len(out, ps.len());
            for (d, x) in ps.iter().rev() {
                stack.push(Frame::Ref(x));
                stack.push(Frame::Ref(d));
            }
        }
        FcnRepr::Record(fs) => {
            let mut fs: Vec<&(Atom, Value)> = fs.iter().collect();
            fs.sort_by(|a, b| a.0.cmp_with(&b.0, ctx.order));
            out.push(tag::RCDFCN | o);
            write_len(out, fs.len());
            for (a, x) in fs.into_iter().rev() {
                stack.push(Frame::Ref(x));
                stack.push(Frame::Tok(a));
            }
        }
        FcnRepr::General(ps) => {
            let mut ps: Vec<(&Value, &Value)> = ps.iter().collect();
            ps.sort_by(|a, b| a.0.cmp_with(b.0, ctx.order));
            out.push(tag::FCN | o);
            write_len(out, ps.len());
            for (d, x) in ps.into_iter().rev() {
                stack.push(Frame::Ref(x));
                stack.push(Frame::Ref(d));
            }
        }
    }
}

/// A compound value being decoded: its tag, the items read so far, and how many it
/// needs.
struct Build {
    tag: u8,
    /// `isNorm` for an unnormalized set.
    flag: bool,
    need: usize,
    names: Vec<Atom>,
    items: Vec<Value>,
}

impl Build {
    fn named(&self) -> bool {
        matches!(self.tag & !1, tag::RCDSET | tag::RECORD | tag::RCDFCN)
    }

    fn finish(self) -> Value {
        let order = order_of(self.tag);
        let ctx = Ctx {
            order,
            ..Ctx::default()
        };
        let mut items = self.items.into_iter();
        let mut next = || items.next().unwrap_or(Value::FALSE);
        match self.tag & !1 {
            tag::TUPLE => Value::Fcn(Fcn::tuple(items.collect())),
            tag::TUPLEFCN => Value::Fcn(Fcn::tuple_fcn(items.collect())),
            tag::SET => Value::Set(Set::from_sorted(order, items.collect())),
            tag::RAW => {
                let v = Value::set_raw(items.collect(), ctx);
                if let (true, Value::Set(s)) = (self.flag, &v) {
                    s.mark_normalized();
                }
                v
            }
            tag::SUBSET => Value::Set(Set::new(order, SetRepr::Subset(next()))),
            tag::SEQ => Value::Set(Set::new(order, SetRepr::Seq(next()))),
            tag::FCNSET => {
                let d = next();
                let r = next();
                Value::Set(Set::new(order, SetRepr::FcnSet(d, r)))
            }
            tag::PRODUCT => Value::Set(Set::new(
                order,
                SetRepr::Product(items.collect::<Vec<_>>().into_boxed_slice()),
            )),
            tag::RCDSET => Value::Set(Set::new(
                order,
                SetRepr::RcdSet(self.names.into_iter().zip(items).collect()),
            )),
            tag::RECORD | tag::RCDFCN => Value::Fcn(Fcn::record(
                self.names.into_iter().zip(items).collect(),
                order,
                self.tag & !1 == tag::RECORD,
            )),
            _ => {
                // tag::FCN: arguments and values alternate.
                let mut pairs = Vec::with_capacity(self.need / 2);
                while let (Some(d), Some(x)) = (items.next(), items.next()) {
                    pairs.push((d, x));
                }
                Value::Fcn(Fcn::general(pairs, order))
            }
        }
    }
}

fn read_tok<A: FnMut(u32) -> Option<Atom>>(input: &mut &[u8], atoms: &mut A) -> Result<Atom> {
    let tok = u32::try_from(read_varint(input)?).map_err(|_| corrupt("token out of range"))?;
    atoms(tok).ok_or_else(|| corrupt("unknown name token"))
}

/// Reads a tagged integer.
fn read_int(input: &mut &[u8]) -> Result<Int> {
    let t = read_u8(input)?;
    read_int_body(t, input)
}

/// Reads an integer's payload after its tag `t`.
fn read_int_body(t: u8, input: &mut &[u8]) -> Result<Int> {
    match t {
        tag::INT => Ok(Int::Small(unzigzag(read_varint(input)?))),
        tag::BIGINT => {
            let neg = read_u8(input)? != 0;
            let n = read_count(input)?;
            if n > input.len() {
                return Err(corrupt("truncated integer"));
            }
            let (mag, rest) = input.split_at(n);
            *input = rest;
            let sign = if neg { Sign::Minus } else { Sign::Plus };
            Ok(Int::from_big(BigInt::from_bytes_le(sign, mag)))
        }
        _ => Err(corrupt("expected an integer")),
    }
}

fn read_count(input: &mut &[u8]) -> Result<usize> {
    usize::try_from(read_varint(input)?).map_err(|_| corrupt("count out of range"))
}

/// Decodes one value written by [`encode`] or [`encode_canonical`] from the front of
/// `input`, advancing it past the value.
///
/// `atoms` maps an intern token back to its name; pass `&mut |t| names.resolve(t)`
/// for the run's [`Interner`](crate::Interner), or a caching wrapper.
///
/// # Errors
/// [`ErrorKind::Codec`] if `input` does not start with a valid encoding or names an
/// unknown token. Decoding never panics, whatever the input.
pub fn decode<A: FnMut(u32) -> Option<Atom>>(input: &mut &[u8], atoms: &mut A) -> Result<Value> {
    let mut stack: Vec<Build> = Vec::new();
    loop {
        if let Some(top) = stack.last_mut()
            && top.named()
            && top.names.len() == top.items.len()
        {
            top.names.push(read_tok(input, atoms)?);
        }
        let t = read_u8(input)?;
        let leaf = match t {
            tag::FALSE => Some(Value::FALSE),
            tag::TRUE => Some(Value::TRUE),
            tag::INT | tag::BIGINT => Some(Value::int(read_int_body(t, input)?)),
            tag::STR => Some(Value::Str(read_tok(input, atoms)?)),
            tag::MODEL => Some(Value::Model(read_tok(input, atoms)?)),
            _ => {
                let order = order_of(t);
                match t & !1 {
                    tag::INTERVAL => {
                        let lo = read_int(input)?;
                        let hi = read_int(input)?;
                        Some(Value::Set(Set::new(order, SetRepr::Interval(lo, hi))))
                    }
                    tag::NAT => Some(Value::Set(Set::new(order, SetRepr::Nat))),
                    tag::INTSET => Some(Value::Set(Set::new(order, SetRepr::Int))),
                    tag::STRINGS => Some(Value::Set(Set::new(order, SetRepr::Strings))),
                    tag::SUBSET | tag::SEQ | tag::FCNSET => {
                        let need = if t & !1 == tag::FCNSET { 2 } else { 1 };
                        stack.push(build(t, false, need, input));
                        None
                    }
                    tag::TUPLE
                    | tag::TUPLEFCN
                    | tag::SET
                    | tag::RAW
                    | tag::RCDSET
                    | tag::PRODUCT
                    | tag::RECORD
                    | tag::RCDFCN
                    | tag::FCN => {
                        let flag = if t & !1 == tag::RAW {
                            read_u8(input)? != 0
                        } else {
                            false
                        };
                        let n = read_count(input)?;
                        let need = if t & !1 == tag::FCN {
                            n.checked_mul(2)
                                .ok_or_else(|| corrupt("count out of range"))?
                        } else {
                            n
                        };
                        let b = build(t, flag, need, input);
                        if need == 0 {
                            Some(b.finish())
                        } else {
                            stack.push(b);
                            None
                        }
                    }
                    _ => return Err(corrupt("unknown tag")),
                }
            }
        };
        let Some(mut v) = leaf else { continue };
        loop {
            let Some(top) = stack.last_mut() else {
                return Ok(v);
            };
            top.items.push(v);
            if top.items.len() < top.need {
                break;
            }
            let Some(done) = stack.pop() else {
                return Err(corrupt("internal decoder state"));
            };
            v = done.finish();
        }
    }
}

/// Starts a compound value, reserving no more than the remaining input could hold
/// (every item takes at least one byte), so corrupt counts cannot exhaust memory.
fn build(t: u8, flag: bool, need: usize, input: &[u8]) -> Build {
    let cap = need.min(input.len());
    let named = matches!(t & !1, tag::RCDSET | tag::RECORD | tag::RCDFCN);
    Build {
        tag: t,
        flag,
        need,
        names: Vec::with_capacity(if named { cap } else { 0 }),
        items: Vec::with_capacity(cap),
    }
}

/// Encoded byte length used by size regression controls.
#[cfg(test)]
fn encoded_len(v: &Value) -> usize {
    let mut out = Vec::new();
    let _ = encode(v, Ctx::default(), &mut out);
    out.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_round_trip() {
        for x in [0u64, 1, 127, 128, 300, u64::from(u32::MAX), u64::MAX] {
            let mut out = Vec::new();
            write_varint(&mut out, x);
            assert_eq!(read_varint(&mut out.as_slice()).ok(), Some(x));
        }
        for v in [0i64, -1, 1, i64::MIN, i64::MAX] {
            assert_eq!(unzigzag(zigzag(v)), v);
        }
    }

    #[test]
    fn small_values_are_small() {
        assert_eq!(encoded_len(&Value::Int(5)), 2);
        assert_eq!(encoded_len(&Value::TRUE), 1);
        assert_eq!(
            encoded_len(&Value::tuple(vec![Value::Int(1), Value::Int(2)])),
            6
        );
    }

    #[test]
    fn truncated_input_is_an_error() {
        let v = Value::tuple(vec![Value::Int(1000), Value::Int(2)]);
        let mut out = Vec::new();
        assert!(encode(&v, Ctx::default(), &mut out).is_ok());
        for cut in 0..out.len() {
            let r = decode(&mut &out[..cut], &mut |_| None);
            assert!(r.is_err(), "prefix of {cut} bytes decoded");
        }
    }
}
