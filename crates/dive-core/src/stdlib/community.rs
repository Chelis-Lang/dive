//! CommunityModules operators that take only values: `SequencesExt`
//! (`Contains`, `RemoveFirst`, `Suffixes`, `AllSubSeqs`, `SetToSeq`,
//! `IsPrefix`, `LongestCommonPrefix`, and `ReplaceFirstSubSeq` and
//! `ReplaceAllSubSeqs` on strings and pure scalar sequences), `Functions` (`IsInjective`),
//! `DyadicRationals` (`Reduce`), `Bitwise`, and the `And` of the pinned
//! Examples Hanoi `Bits.class` (`StdOp::BitsAnd`). The reference is `dive-eval`'s
//! `community` module (`call_ext`), which follows the Java overrides of
//! `tlc2.overrides`.

use dive_ir::StdOp;

use super::{arg, arg_error, enumerable, exc, refuse};
use crate::compare;
use crate::eval::Machine;
use crate::node::Kind;
use crate::{Error, Handle};

pub(super) fn implements(op: StdOp) -> bool {
    matches!(
        op,
        StdOp::SequencesExtContains
            | StdOp::SequencesExtRemoveFirst
            | StdOp::SequencesExtSuffixes
            | StdOp::SequencesExtSetToSeq
            | StdOp::SequencesExtSetToSeqs
            | StdOp::SequencesExtIsPrefix
            | StdOp::SequencesExtAllSubSeqs
            | StdOp::SequencesExtLongestCommonPrefix
            | StdOp::SequencesExtReplaceFirstSubSeq
            | StdOp::SequencesExtReplaceAllSubSeqs
            | StdOp::FunctionsIsInjective
            | StdOp::FunctionsAntiFunction
            | StdOp::BitsAnd
            | StdOp::BitwiseAnd
            | StdOp::BitwiseOr
            | StdOp::BitwiseXor
            | StdOp::BitwiseNot
            | StdOp::BitwiseShiftR
            | StdOp::DyadicRationalsReduce
    )
}

pub(super) fn call(m: &mut Machine, op: StdOp, args: &[Handle]) -> Result<Handle, Error> {
    match op {
        StdOp::SequencesExtContains => {
            let Some(t) = tuple(m, op, arg(args, 0)?)? else {
                return Err(one_arg_error("Contains", "sequence"));
            };
            let x = arg(args, 1)?;
            for y in t {
                if compare::equals(&m.heap, y, x)? {
                    return Ok(Handle::TRUE);
                }
            }
            Ok(Handle::FALSE)
        }
        StdOp::SequencesExtRemoveFirst => {
            let Some(t) = tuple(m, op, arg(args, 0)?)? else {
                return Err(arg_error("first", "RemoveFirst", "sequence"));
            };
            let x = arg(args, 1)?;
            let mut out = Vec::with_capacity(t.len());
            let mut found = false;
            for y in t {
                if !found && compare::equals(&m.heap, y, x)? {
                    found = true;
                } else {
                    out.push(y);
                }
            }
            Ok(m.heap.tuple(&out))
        }
        StdOp::SequencesExtSuffixes => {
            let Some(t) = tuple(m, op, arg(args, 0)?)? else {
                return Err(one_arg_error("Suffixes", "sequence"));
            };
            let mut out = Vec::with_capacity(t.len() + 1);
            for i in 0..=t.len() {
                out.push(m.heap.tuple(&t[i..]));
            }
            Ok(m.heap.set_from(&mut out))
        }
        StdOp::SequencesExtSetToSeq => {
            let Some(elems) = enumerable(m, op, arg(args, 0)?)? else {
                return Err(refuse(
                    op,
                    "a value that is not an enumerable set (TLC fails with a NullPointerException)",
                ));
            };
            Ok(m.heap.tuple(&elems))
        }
        StdOp::SequencesExtAllSubSeqs => all_sub_seqs(m, op, arg(args, 0)?),
        StdOp::SequencesExtLongestCommonPrefix => longest_common_prefix(m, op, arg(args, 0)?),
        StdOp::SequencesExtReplaceFirstSubSeq | StdOp::SequencesExtReplaceAllSubSeqs => {
            replace_sub_seq(m, op, arg(args, 0)?, arg(args, 1)?, arg(args, 2)?)
        }
        StdOp::SequencesExtSetToSeqs => set_to_seqs(m, op, arg(args, 0)?),
        StdOp::SequencesExtIsPrefix => is_prefix(m, op, arg(args, 0)?, arg(args, 1)?),
        StdOp::FunctionsIsInjective => is_injective(m, op, arg(args, 0)?),
        StdOp::FunctionsAntiFunction => anti_function(m, op, arg(args, 0)?),
        StdOp::BitsAnd => {
            // The pinned Hanoi `Bits.And(IntValue x, IntValue y, IntValue n,
            // IntValue m)` is `IntValue.gen(x.val & y.val)`: Java's `&` on
            // two's-complement `int`s. Its signature casts all four arguments
            // to `IntValue`, so every one is checked, although only the first
            // two are read; TLC fails a cast with error 2154, which the core
            // refuses by name, as `dive-eval` does.
            let mut v = [0i32; 4];
            for (i, slot) in v.iter_mut().enumerate() {
                *slot = java_int(op, arg(args, i)?)?;
            }
            int(i64::from(v[0] & v[1]))
        }
        StdOp::BitwiseAnd | StdOp::BitwiseOr | StdOp::BitwiseXor => {
            let x = java_int(op, arg(args, 0)?)?;
            let y = java_int(op, arg(args, 1)?)?;
            java_int(op, arg(args, 2)?)?;
            java_int(op, arg(args, 3)?)?;
            let r = match op {
                StdOp::BitwiseAnd => x & y,
                StdOp::BitwiseOr => x | y,
                _ => x ^ y,
            };
            int(i64::from(r))
        }
        StdOp::BitwiseNot => {
            let x = java_int(op, arg(args, 0)?)?;
            // `Integer.MAX_VALUE >> Integer.numberOfLeadingZeros(x) - 1`:
            // Java masks the shift distance to five bits.
            let shift = x.leading_zeros().wrapping_sub(1) & 31;
            let mask = i32::MAX >> shift;
            int(i64::from(!x & mask))
        }
        StdOp::BitwiseShiftR => {
            let n = java_int(op, arg(args, 0)?)?;
            let pos = java_int(op, arg(args, 1)?)?;
            // `n >>> pos`.
            let r = (n.cast_unsigned() >> (pos.cast_unsigned() & 31)).cast_signed();
            int(i64::from(r))
        }
        StdOp::DyadicRationalsReduce => reduce_dyadic(m, op, arg(args, 0)?),
        _ => Err(Error::internal("not a CommunityModules operator")),
    }
}

/// `DyadicRationals.reduce`: the record `[num |-> n, den |-> d]` divided
/// by `gcd(n, d)`, or the argument itself when the gcd is 1. Like
/// `dive-eval`, the core refuses the cases where TLC fails with a
/// Java exception (an operand outside 32 bits, a zero or minimal
/// numerator and denominator); it also refuses an argument that is not a
/// record literal or lacks a field, whose TLC error it does not reproduce.
fn reduce_dyadic(m: &mut Machine, op: StdOp, val: Handle) -> Result<Handle, Error> {
    if !val.is_compound() {
        return Err(refuse(op, "an argument that is not a record"));
    }
    let view = m.heap.view(val);
    if view.kind != Kind::Fcn || !view.is_record_literal() {
        return Err(refuse(op, "an argument that is not a record literal"));
    }
    let (keys, values) = (view.keys().to_vec(), view.values().to_vec());
    let den_key = m.heap.string("den")?;
    let num_key = m.heap.string("num")?;
    let field = |key: Handle| {
        keys.iter()
            .position(|&k| k == key)
            .map(|i| values[i])
            .ok_or_else(|| refuse(op, "a record without a num or den field"))
    };
    let (den, num) = (field(den_key)?, field(num_key)?);
    let small = |h: Handle| h.as_int().filter(|i| i32::try_from(*i).is_ok());
    let (Some(d), Some(n)) = (small(den), small(num)) else {
        return Err(refuse(
            op,
            "a numerator or denominator that is not a 32-bit integer (TLC fails with a ClassCastException)",
        ));
    };
    let g = gcd(n, d);
    if g == 0 || n == i64::from(i32::MIN) || d == i64::from(i32::MIN) {
        return Err(refuse(
            op,
            "a zero or minimal numerator and denominator (TLC fails with an ArithmeticException)",
        ));
    }
    if g == 1 {
        return Ok(val);
    }
    // Java's truncating division.
    let mut pairs = [(den_key, int(d / g)?), (num_key, int(n / g)?)];
    pairs.sort_unstable_by(|a, b| m.heap.cmp(a.0, b.0));
    let (keys, values): (Vec<Handle>, Vec<Handle>) = pairs.into_iter().unzip();
    Ok(m.heap.record(&keys, &values))
}

/// `ArithmeticUtils.gcd(int, int)` for arguments other than
/// `Integer.MIN_VALUE`: the non-negative greatest common divisor.
fn gcd(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// `SetToSeqs(S)`: every ordering of `S` as a sequence. TLC builds them
/// unnormalized, in the lexicographic order of the index permutations
/// over `S`'s normalized elements; for tuples of one length that is the
/// normalized order, in either profile, so the core returns the set.
fn set_to_seqs(m: &mut Machine, op: StdOp, s: Handle) -> Result<Handle, Error> {
    let Some(elems) = enumerable(m, op, s)? else {
        return Err(super::wrong_value("SetToSeqs", "a finite set"));
    };
    if elems.len() > 12 {
        // TLC's `int factorial` overflows from 13 elements on.
        return Err(refuse(op, "a set of more than 12 elements"));
    }
    let mut idx: Vec<usize> = (0..elems.len()).collect();
    let mut out = Vec::new();
    loop {
        let vals: Vec<Handle> = idx.iter().map(|&i| elems[i]).collect();
        out.push(m.heap.tuple(&vals));
        if !super::tlc::next_permutation(&mut idx) {
            break;
        }
    }
    Ok(m.heap.set_from(&mut out))
}

/// `AllSubSeqs(s)`. TLC builds the set unnormalized, in the order of the
/// bit masks over the indices (a `Kind::SetEnum` under TLC's comparisons).
fn all_sub_seqs(m: &mut Machine, op: StdOp, s: Handle) -> Result<Handle, Error> {
    let Some(t) = tuple(m, op, s)? else {
        return Err(one_arg_error("AllSubSeqs", "sequence"));
    };
    let n = t.len();
    if n > 24 {
        return Err(refuse(
            op,
            "a sequence of more than 24 elements (2^n subsequences)",
        ));
    }
    let mut out = Vec::with_capacity(1 << n);
    let mut sub = Vec::with_capacity(n);
    for i in 0u32..(1 << n) {
        sub.clear();
        sub.extend((0..n).filter(|j| i & (1 << j) != 0).map(|j| t[j]));
        out.push(m.heap.tuple(&sub));
    }
    // The order is observable only through TLC's comparisons: a membership
    // test scans an unnormalized set. TLA+ comparisons answer it in the default
    // profile, so there the set is normalized at once.
    if m.heap.store().semantics().tlc_comparisons {
        Ok(m.heap.set_enum(&out))
    } else {
        Ok(m.heap.set_from(&mut out))
    }
}

/// `LongestCommonPrefix(S)` (`SequencesExt.longestCommonPrefix`): the
/// first element in `S`'s order, cut where another element differs. On
/// strings it compares UTF-16 units, as Java does; an element shorter than
/// the prefix so far is TLC's Java exception, which the core refuses (it
/// occurs only in TLC's string order, `--compat=tlc`).
fn longest_common_prefix(m: &mut Machine, op: StdOp, s: Handle) -> Result<Handle, Error> {
    let bad = || one_arg_error("LongestCommonPrefix", "non-empty set");
    let Some(elems) = enumerable(m, op, s)? else {
        return Err(bad());
    };
    let Some(&first) = elems.first() else {
        return Err(bad());
    };
    let not_seq = || one_arg_error("LongestCommonPrefix", "sequence");
    if let Some(id) = first.as_str_id() {
        let text = |m: &Machine, id: u32| {
            m.heap
                .store()
                .strings()
                .text(id)
                .map(|t| t.encode_utf16().collect::<Vec<u16>>())
                .ok_or_else(|| Error::internal("unknown string"))
        };
        let prefix = text(m, id)?;
        let mut upper = prefix.len();
        for &other in &elems[1..] {
            let Some(o) = other.as_str_id() else {
                return Err(not_seq());
            };
            let other = text(m, o)?;
            for (idx, &p) in prefix.iter().enumerate().take(upper) {
                let Some(&c) = other.get(idx) else {
                    return Err(refuse(
                        op,
                        "a string shorter than the first in the set's order (TLC fails with a StringIndexOutOfBoundsException) (M1 core)",
                    ));
                };
                if p != c {
                    upper = idx;
                    break;
                }
            }
            if upper == 0 {
                break;
            }
        }
        return m.heap.string(&String::from_utf16_lossy(&prefix[..upper]));
    }
    let Some(prefix) = tuple(m, op, first)? else {
        return Err(not_seq());
    };
    let mut upper = prefix.len();
    for &other in &elems[1..] {
        let Some(other) = tuple(m, op, other)? else {
            return Err(not_seq());
        };
        for (idx, &x) in prefix.iter().enumerate().take(upper) {
            let Some(&y) = other.get(idx) else {
                return Err(refuse(
                    op,
                    "a sequence shorter than the first in the set's order (TLC fails with an ArrayIndexOutOfBoundsException)",
                ));
            };
            if !compare::equals(&m.heap, x, y)? {
                upper = idx;
                break;
            }
        }
        if upper == 0 {
            break;
        }
    }
    Ok(m.heap.tuple(&prefix[..upper]))
}

/// `ReplaceFirstSubSeq(r, s, t)` and `ReplaceAllSubSeqs(r, s, t)` on
/// strings, as TLC's override computes them, or pure scalar sequences,
/// matching the fallback definition in `SequencesExt.tla`.
fn replace_sub_seq(
    m: &mut Machine,
    op: StdOp,
    r: Handle,
    s: Handle,
    t: Handle,
) -> Result<Handle, Error> {
    let (Some(r), Some(s), Some(t)) = (r.as_str_id(), s.as_str_id(), t.as_str_id()) else {
        return replace_sequence(m, op, r, s, t);
    };
    let strings = m.heap.store().strings();
    let (Some(r), Some(s), Some(t)) = (strings.text(r), strings.text(s), strings.text(t)) else {
        return Err(Error::internal("unknown string"));
    };
    let (r, s, t) = (&*r, &*s, &*t);
    let out = if op == StdOp::SequencesExtReplaceFirstSubSeq {
        if s.is_empty() {
            format!("{r}{t}")
        } else {
            t.replacen(s, r, 1)
        }
    } else if s.is_empty() {
        if t.chars().any(|c| u32::from(c) > 0xFFFF) {
            return Err(refuse(
                op,
                "a string with characters outside the Basic Multilingual Plane (TLC splits them into UTF-16 halves)",
            ));
        }
        let mut out = String::from(r);
        for (i, c) in t.chars().enumerate() {
            if i != 0 {
                out.push_str(r);
            }
            out.push(c);
        }
        out
    } else {
        t.replace(s, r)
    };
    m.heap.string(&out)
}

/// `ReplaceFirst` chooses the least prefix containing the pattern. For All,
/// only non-overlapping occurrences are admitted after the early CASE arms:
/// every maximum-count partition must then contain every occurrence, and
/// replacing them has the same flattened result in every such partition.
/// Overlaps are refused: the fallback compares partition-component indices,
/// not absolute positions, before CHOOSE uses normalized tuple order.
/// Uniform scalar classes avoid skipped fallback normalization errors.
fn replace_sequence(
    m: &mut Machine,
    op: StdOp,
    replacement: Handle,
    pattern: Handle,
    text: Handle,
) -> Result<Handle, Error> {
    let (Some(r), Some(s), Some(t)) = (
        tuple(m, op, replacement)?,
        tuple(m, op, pattern)?,
        tuple(m, op, text)?,
    ) else {
        return Err(refuse(
            op,
            "arguments that are neither all strings nor all materialized sequences",
        ));
    };
    if [r.len(), s.len(), t.len()]
        .into_iter()
        .any(|len| len > crate::heap::MAX_MATERIALIZE)
    {
        return Err(refuse(op, "a sequence argument of more than 2^24 elements"));
    }
    m.charge(r.len() + s.len() + t.len())?;
    let mut class = None;
    for &value in r.iter().chain(&s).chain(&t) {
        let next = compare::class(&m.heap, value);
        if !matches!(
            next,
            compare::Class::Bool | compare::Class::Int | compare::Class::Str
        ) || class.is_some_and(|previous| previous != next)
        {
            return Err(refuse(
                op,
                "sequence elements outside one scalar class (booleans, integers or strings), whose fallback comparison order is not implemented",
            ));
        }
        class = Some(next);
    }
    let all = op == StdOp::SequencesExtReplaceAllSubSeqs;
    let equal = |a: &[Handle], b: &[Handle]| {
        a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| m.heap.eq(x, y))
    };
    // These are the first two CASE arms, before the empty-pattern arm.
    if all && equal(&s, &t) {
        return Ok(replacement);
    }
    if all && equal(&r, &s) {
        return Ok(text);
    }
    let mut out = Vec::new();
    if s.is_empty() {
        if all {
            for value in t {
                replacement_extend(m, op, &mut out, &r)?;
                replacement_extend(m, op, &mut out, &[value])?;
            }
        } else {
            replacement_extend(m, op, &mut out, &r)?;
            replacement_extend(m, op, &mut out, &t)?;
        }
        return Ok(m.heap.tuple(&out));
    }
    if s.len() > t.len() {
        return Ok(text);
    }
    let failure = replacement_failure(m, &s)?;
    let mut copied = 0;
    let mut matched = 0;
    for (i, &value) in t.iter().enumerate() {
        while matched > 0 && !m.heap.eq(value, s[matched]) {
            m.charge(1)?;
            matched = failure[matched - 1];
        }
        m.charge(1)?;
        if m.heap.eq(value, s[matched]) {
            matched += 1;
        }
        if matched == s.len() {
            let start = i + 1 - s.len();
            if start < copied {
                return Err(refuse(
                    op,
                    "overlapping sequence occurrences, whose partition CHOOSE order is not implemented",
                ));
            }
            replacement_extend(m, op, &mut out, &t[copied..start])?;
            replacement_extend(m, op, &mut out, &r)?;
            copied = i + 1;
            if !all {
                break;
            }
            // Continue through the prefix table to detect every occurrence,
            // including an overlap that must refuse instead of being skipped.
            matched = failure[matched - 1];
        }
    }
    if copied == 0 {
        return Ok(text);
    }
    replacement_extend(m, op, &mut out, &t[copied..])?;
    Ok(m.heap.tuple(&out))
}

/// Prefix fallback lengths for the pattern, including overlapping matches.
fn replacement_failure(m: &mut Machine, pattern: &[Handle]) -> Result<Vec<usize>, Error> {
    let mut failure = vec![0; pattern.len()];
    let mut matched = 0;
    for i in 1..pattern.len() {
        while matched > 0 && !m.heap.eq(pattern[i], pattern[matched]) {
            m.charge(1)?;
            matched = failure[matched - 1];
        }
        m.charge(1)?;
        if m.heap.eq(pattern[i], pattern[matched]) {
            matched += 1;
        }
        failure[i] = matched;
    }
    Ok(failure)
}

/// Bounds materialized output before allocating or copying it, and accounts
/// for that work in the compiler's existing constant-fold fuel.
fn replacement_extend(
    m: &mut Machine,
    op: StdOp,
    out: &mut Vec<Handle>,
    values: &[Handle],
) -> Result<(), Error> {
    if out.len().saturating_add(values.len()) > crate::heap::MAX_MATERIALIZE {
        return Err(refuse(
            op,
            "a replacement result of more than 2^24 elements",
        ));
    }
    m.charge(values.len())?;
    out.extend_from_slice(values);
    Ok(())
}

/// `EC.TLC_MODULE_ONE_ARGUMENT_ERROR` (2283).
fn one_arg_error(op: &str, what: &str) -> Error {
    exc(
        2283,
        format!("The argument of {op} should be a {what}, but instead it is:"),
    )
}

fn int(n: i64) -> Result<Handle, Error> {
    Handle::int(n).ok_or_else(|| Error::internal("a 32-bit integer"))
}

/// A Java `int` argument; any other value is refused, as the reference
/// engine refuses it (TLC fails with a cast or range error).
fn java_int(op: StdOp, h: Handle) -> Result<i32, Error> {
    match h.as_int() {
        Some(i) => {
            i32::try_from(i).map_err(|_| refuse(op, "an integer outside TLC's 32-bit range"))
        }
        None => Err(refuse(op, "an argument that is not a TLC IntValue")),
    }
}

/// The elements of a sequence (a function over `1..n`); `None` for any
/// other value.
fn tuple(m: &Machine, op: StdOp, h: Handle) -> Result<Option<Vec<Handle>>, Error> {
    if !h.is_compound() {
        return Ok(None);
    }
    let v = m.heap.view(h);
    match v.kind {
        Kind::IntFcn if v.lo == 1 || v.data.is_empty() => Ok(Some(v.data.to_vec())),
        Kind::Lambda => Err(refuse(op, "a function argument the core keeps lazy")),
        _ => Ok(None),
    }
}

/// `IsPrefix(s, t)`: string prefix on two strings, else elementwise on
/// sequences.
fn is_prefix(m: &mut Machine, op: StdOp, lhs: Handle, rhs: Handle) -> Result<Handle, Error> {
    if let (Some(p), Some(q)) = (lhs.as_str_id(), rhs.as_str_id()) {
        let strings = m.heap.store().strings();
        let (Some(prefix), Some(whole)) = (strings.text(p), strings.text(q)) else {
            return Err(Error::internal("unknown string"));
        };
        return Ok(Handle::bool(whole.starts_with(&*prefix)));
    }
    let (Some(short), Some(long)) = (tuple(m, op, lhs)?, tuple(m, op, rhs)?) else {
        return Err(refuse(
            op,
            "an argument that is not a sequence (TLC fails with a NullPointerException)",
        ));
    };
    if short.len() > long.len() {
        return Ok(Handle::FALSE);
    }
    for (&x, &y) in short.iter().zip(&long) {
        if !compare::equals(&m.heap, x, y)? {
            return Ok(Handle::FALSE);
        }
    }
    Ok(Handle::TRUE)
}

/// `IsInjective(f)`: no two values of `f` are equal. A comparison TLC
/// cannot make is refused: which fails first depends on how TLC holds the
/// function.
fn is_injective(m: &mut Machine, op: StdOp, f: Handle) -> Result<Handle, Error> {
    let values = if f.is_compound() {
        let v = m.heap.view(f);
        match v.kind {
            Kind::IntFcn => Some(v.data.to_vec()),
            Kind::Fcn => Some(v.values().to_vec()),
            Kind::Lambda => return Err(refuse(op, "a function argument the core keeps lazy")),
            Kind::RecSet => {
                return Err(refuse(op, "a set of records"));
            }
            _ => None,
        }
    } else {
        None
    };
    let Some(values) = values else {
        return Err(one_arg_error("IsInjective", "function"));
    };
    for (i, &x) in values.iter().enumerate() {
        for &y in &values[i + 1..] {
            match compare::equals(&m.heap, x, y) {
                Ok(true) => return Ok(Handle::FALSE),
                Ok(false) => {}
                Err(_) => {
                    return Err(refuse(
                        op,
                        "values TLC cannot compare (which comparison fails first depends on how TLC holds the function)",
                    ));
                }
            }
        }
    }
    Ok(Handle::TRUE)
}

/// `AntiFunction(f)`: `[y \in Range(f) |-> x]`, keeping for each value the
/// first argument in the order of a stable sort of `f`'s entries by value
/// (TLC's `Arrays.sort`). Values TLC cannot compare are refused, as
/// `dive-eval` refuses them.
fn anti_function(m: &mut Machine, op: StdOp, f: Handle) -> Result<Handle, Error> {
    let pairs: Vec<(Handle, Handle)> = if f.is_compound() {
        let v = m.heap.view(f);
        match v.kind {
            Kind::IntFcn => {
                let mut out = Vec::with_capacity(v.data.len());
                for (k, &x) in (v.lo..).zip(v.data) {
                    out.push((int(k)?, x));
                }
                out
            }
            Kind::Fcn => v
                .keys()
                .iter()
                .copied()
                .zip(v.values().iter().copied())
                .collect(),
            Kind::Lambda => return Err(refuse(op, "a function argument the core keeps lazy")),
            _ => return Err(one_arg_error("AntiFunction", "functions")),
        }
    } else {
        return Err(one_arg_error("AntiFunction", "functions"));
    };
    let mut order: Vec<usize> = (0..pairs.len()).collect();
    order.sort_by(|&a, &b| m.heap.cmp(pairs[a].1, pairs[b].1));
    let sorted: Vec<Handle> = order.iter().map(|&i| pairs[i].1).collect();
    if compare::check_set(&m.heap, &sorted).is_err() {
        return Err(refuse(
            op,
            "values TLC cannot compare (TLC's error depends on its sort's order of comparisons)",
        ));
    }
    let mut out: Vec<(Handle, Handle)> = Vec::new();
    for i in order {
        let (arg, value) = pairs[i];
        let dup = match out.last() {
            Some(&(prev, _)) => compare::equals(&m.heap, value, prev)?,
            None => false,
        };
        if !dup {
            out.push((value, arg));
        }
    }
    Ok(m.heap.fcn_from_pairs(&mut out))
}

#[cfg(test)]
mod replacement_tests {
    use std::sync::Arc;

    use crate::memo::Memo;
    use crate::{Opts, Store};

    use super::*;

    fn words(alphabet: [Handle; 2], maximum: usize) -> Vec<Vec<Handle>> {
        (0..=maximum)
            .flat_map(|len| {
                (0..1usize << len)
                    .map(move |mask| (0..len).map(|i| alphabet[(mask >> i) & 1]).collect())
            })
            .collect()
    }

    // Every flattening of k bounded components is a weak composition of the
    // text length into k parts. Empty components and repeated values are
    // retained. This enumerates S from the actual definition, not matches.
    fn partitions(text: &[Handle]) -> Vec<Vec<Vec<Handle>>> {
        fn extend(
            text: &[Handle],
            remaining: usize,
            parts: &mut Vec<Vec<Handle>>,
            out: &mut Vec<Vec<Vec<Handle>>>,
        ) {
            if remaining == 0 {
                if text.is_empty() {
                    out.push(parts.clone());
                }
                return;
            }
            for len in 0..=text.len() {
                parts.push(text[..len].to_vec());
                extend(&text[len..], remaining - 1, parts, out);
                parts.pop();
            }
        }
        let mut out = Vec::new();
        for count in 1..=text.len() {
            extend(text, count, &mut Vec::new(), &mut out);
        }
        out
    }

    // TLC TupleValue.compareTo compares length first, then contents; recurse
    // once for partition tuples. The test alphabet uses only small integers.
    fn partition_order(a: &[Vec<Handle>], b: &[Vec<Handle>]) -> std::cmp::Ordering {
        a.len().cmp(&b.len()).then_with(|| {
            a.iter()
                .zip(b)
                .map(|(x, y)| {
                    x.len().cmp(&y.len()).then_with(|| {
                        x.iter()
                            .map(|h| h.as_int())
                            .cmp(y.iter().map(|h| h.as_int()))
                    })
                })
                .find(|&order| !order.is_eq())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    fn is_subsequence(s: &[Handle], t: &[Handle]) -> bool {
        (0..=t.len()).any(|start| (start..=t.len()).any(|end| &t[start..end] == s))
    }

    // IndexFirstSubSeq chooses the least prefix containing the pattern; its
    // match ends there. This is separate from ReplaceAll's partition choice.
    fn definition_first(r: &[Handle], s: &[Handle], t: &[Handle]) -> Vec<Handle> {
        let Some(end) = (0..=t.len()).find(|&end| is_subsequence(s, &t[..end])) else {
            return t.to_vec();
        };
        let start = end - s.len();
        t[..start]
            .iter()
            .chain(r)
            .chain(&t[end..])
            .copied()
            .collect()
    }

    // S, T, comp and normalized CHOOSE from the pinned TLA+ definition.
    // Return the first normalized satisfying partition's result, plus every
    // satisfying result so admitted cases prove choice-independent output.
    fn definition_all(r: &[Handle], s: &[Handle], t: &[Handle]) -> (Vec<Handle>, Vec<Vec<Handle>>) {
        let direct = if s == t {
            Some(r.to_vec())
        } else if r == s || !is_subsequence(s, t) {
            Some(t.to_vec())
        } else if s.is_empty() {
            Some(
                t.iter()
                    .flat_map(|&x| r.iter().copied().chain([x]))
                    .collect(),
            )
        } else {
            None
        };
        if let Some(value) = direct {
            return (value.clone(), vec![value]);
        }
        let mut candidates = partitions(t);
        candidates.sort_by(|a, b| partition_order(a, b));
        let indexed: Vec<_> = candidates
            .into_iter()
            .map(|parts| {
                let matches: Vec<_> = parts
                    .iter()
                    .enumerate()
                    .filter_map(|(i, chunk)| (chunk == s).then_some(i + 1))
                    .collect();
                (parts, matches)
            })
            .collect();
        let maximum = indexed
            .iter()
            .map(|(_, matches)| matches.len())
            .max()
            .unwrap_or(0);
        let maximal: Vec<_> = indexed
            .iter()
            .filter(|(_, matches)| matches.len() == maximum)
            .collect();
        let mut outputs: Vec<Vec<Handle>> = Vec::new();
        for (parts, matches) in &maximal {
            if maximal
                .iter()
                .all(|(_, other)| matches.iter().zip(other).all(|(a, b)| a <= b))
            {
                outputs.push(
                    parts
                        .iter()
                        .flat_map(|chunk| if chunk == s { r } else { chunk })
                        .copied()
                        .collect(),
                );
            }
        }
        assert!(
            !outputs.is_empty(),
            "a dominating component-index partition exists"
        );
        (outputs[0].clone(), outputs)
    }

    #[test]
    fn partition_oracle_reproduces_pinned_overlap_surprise() {
        let h = |value| Handle::int(value).unwrap_or(Handle::FALSE);
        let (chosen, outputs) = definition_all(&[h(2)], &[h(1), h(1)], &[h(0), h(1), h(1), h(1)]);
        assert_eq!(chosen, vec![h(0), h(1), h(2)]);
        assert!(outputs.contains(&vec![h(0), h(2), h(1)]));
    }

    #[test]
    fn scalar_replacements_match_bounded_partition_definition() -> Result<(), Error> {
        let mut m = Machine::new(
            Arc::new(Store::new(false)),
            0,
            Opts::default(),
            Memo::new(&[]),
        );
        let alphabet = [m.heap.int_from_i64(0), m.heap.int_from_i64(1)];
        let replacements = words(alphabet, 2);
        let patterns = words(alphabet, 3);
        let texts = words(alphabet, 5);
        let mut admitted = 0;
        let mut refused = 0;
        for r in &replacements {
            for s in &patterns {
                for t in &texts {
                    for op in [
                        StdOp::SequencesExtReplaceFirstSubSeq,
                        StdOp::SequencesExtReplaceAllSubSeqs,
                    ] {
                        let all = op == StdOp::SequencesExtReplaceAllSubSeqs;
                        let positions: Vec<_> = if s.is_empty() {
                            Vec::new()
                        } else {
                            t.windows(s.len())
                                .enumerate()
                                .filter_map(|(i, values)| (values == s).then_some(i))
                                .collect()
                        };
                        let overlap = all
                            && s != t
                            && r != s
                            && positions.windows(2).any(|p| p[1] < p[0] + s.len());
                        let rh = m.heap.tuple(r);
                        let sh = m.heap.tuple(s);
                        let th = m.heap.tuple(t);
                        let result = replace_sequence(&mut m, op, rh, sh, th);
                        if overlap {
                            assert!(
                                matches!(result, Err(ref error) if error.message().contains("overlapping sequence occurrences"))
                            );
                            refused += 1;
                        } else {
                            let actual = m.heap.view(result?).data.to_vec();
                            if all {
                                let (chosen, outputs) = definition_all(r, s, t);
                                assert_eq!(actual, chosen);
                                assert!(outputs.iter().all(|output| *output == actual));
                            } else {
                                assert_eq!(actual, definition_first(r, s, t));
                            }
                            admitted += 1;
                        }
                        m.heap.reset();
                    }
                }
            }
        }
        assert_eq!((admitted, refused), (13_016, 214));
        Ok(())
    }

    #[test]
    fn sequence_replacement_accounts_for_constant_fold_work() {
        let mut m = Machine::new(
            Arc::new(Store::new(false)),
            0,
            Opts::default(),
            Memo::new(&[]),
        );
        let value = m.heap.int_from_i64(1);
        let sequence = m.heap.tuple(&[value]);
        m.set_fuel(0);
        let result = replace_sequence(
            &mut m,
            StdOp::SequencesExtReplaceFirstSubSeq,
            sequence,
            sequence,
            sequence,
        );
        assert!(
            matches!(result, Err(ref error) if error.message().contains("constant-folding budget"))
        );
    }
}
