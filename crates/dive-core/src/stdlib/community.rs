//! CommunityModules operators that take only values: `SequencesExt`
//! (`Contains`, `RemoveFirst`, `Suffixes`, `AllSubSeqs`, `SetToSeq`,
//! `IsPrefix`, `LongestCommonPrefix`, and `ReplaceFirstSubSeq` and
//! `ReplaceAllSubSeqs` on strings and pure scalar sequences), `Functions` (`IsInjective`) and
//! `Bitwise`. The reference is
//! `crates/dive-eval/src/community.rs` (`call_ext`), which follows the Java
//! overrides of `tlc2.overrides` (`docs/provenance.md`).

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
            | StdOp::BitwiseAnd
            | StdOp::BitwiseOr
            | StdOp::BitwiseXor
            | StdOp::BitwiseNot
            | StdOp::BitwiseShiftR
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
        _ => Err(Error::internal("not a CommunityModules operator")),
    }
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
    // test scans an unnormalized set (C1). D-009 answers it in the default
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

/// The fallback's maximum number of non-overlapping fixed-length matches
/// is obtained by choosing the earliest match each time. Exchanging a
/// later first match for the earliest cannot remove a later match. Repeating
/// that exchange also gives the componentwise-leftmost choice in the TLA+
/// definition. KMP finds those positions without constructing `SubSeqs(t)`.
/// Scalar classes are kept uniform so its normalized set cannot raise a
/// comparison error that this direct scan would skip.
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
    let mut failure = vec![0; s.len()];
    let mut matched = 0;
    for i in 1..s.len() {
        while matched > 0 && !m.heap.eq(s[i], s[matched]) {
            m.charge(1)?;
            matched = failure[matched - 1];
        }
        m.charge(1)?;
        if m.heap.eq(s[i], s[matched]) {
            matched += 1;
        }
        failure[i] = matched;
    }
    let mut copied = 0;
    matched = 0;
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
            replacement_extend(m, op, &mut out, &t[copied..i + 1 - s.len()])?;
            replacement_extend(m, op, &mut out, &r)?;
            copied = i + 1;
            if !all {
                break;
            }
            matched = 0; // A match consumes the whole pattern; overlaps lose.
        }
    }
    if copied == 0 {
        return Ok(text);
    }
    replacement_extend(m, op, &mut out, &t[copied..])?;
    Ok(m.heap.tuple(&out))
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
/// (TLC's `Arrays.sort`). Values TLC cannot compare are refused, as the
/// reference engine refuses them.
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

    // Directly enumerate the TLA+ choice's non-overlapping match positions,
    // maximize their count and choose the lexicographically earliest list.
    // This oracle does not use the implementation's prefix table or scan.
    fn expected(r: &[Handle], s: &[Handle], t: &[Handle], all: bool) -> Vec<Handle> {
        if all && s == t {
            return r.to_vec();
        }
        if all && r == s {
            return t.to_vec();
        }
        if s.is_empty() {
            return if all {
                t.iter()
                    .flat_map(|&x| r.iter().copied().chain([x]))
                    .collect()
            } else {
                r.iter().chain(t).copied().collect()
            };
        }
        let positions: Vec<usize> = t
            .windows(s.len())
            .enumerate()
            .filter_map(|(i, values)| (values == s).then_some(i))
            .collect();
        let mut best = Vec::new();
        for mask in 0..1usize << positions.len() {
            let selected: Vec<usize> = positions
                .iter()
                .enumerate()
                .filter_map(|(i, &p)| (mask & (1 << i) != 0).then_some(p))
                .collect();
            if selected.windows(2).any(|p| p[1] < p[0] + s.len()) {
                continue;
            }
            if selected.len() > best.len() || (selected.len() == best.len() && selected < best) {
                best = selected;
            }
        }
        if !all {
            best = positions.into_iter().take(1).collect();
        }
        let mut result = Vec::new();
        let mut copied = 0;
        for p in best {
            result.extend_from_slice(&t[copied..p]);
            result.extend_from_slice(r);
            copied = p + s.len();
        }
        result.extend_from_slice(&t[copied..]);
        result
    }

    #[test]
    fn scalar_replacements_equal_exhaustive_leftmost_maximum_choice() -> Result<(), Error> {
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
        let mut compared = 0;
        for r in &replacements {
            for s in &patterns {
                for t in &texts {
                    for op in [
                        StdOp::SequencesExtReplaceFirstSubSeq,
                        StdOp::SequencesExtReplaceAllSubSeqs,
                    ] {
                        let rh = m.heap.tuple(r);
                        let sh = m.heap.tuple(s);
                        let th = m.heap.tuple(t);
                        let result = replace_sequence(&mut m, op, rh, sh, th)?;
                        let actual = m.heap.view(result).data.to_vec();
                        assert_eq!(
                            actual,
                            expected(r, s, t, op == StdOp::SequencesExtReplaceAllSubSeqs)
                        );
                        compared += 1;
                        m.heap.reset();
                    }
                }
            }
        }
        assert_eq!(compared, 13_230);
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
        m.fuel = 0;
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
