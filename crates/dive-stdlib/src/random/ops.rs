//! `RandomElement` (module `TLC`) and the `Randomization` operators over values.

use std::fmt;

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use dive_value::print::{print_value, to_tlc_string_lenient};
use dive_value::{Atom, Ctx, ErrorKind, Int, Set, SetKind, Value, ValueError, tlc};

use super::lcg::{BigIndices, SubsetIndices, big_digit};
use super::source::RandomSource;

/// The Java method TLC names when `RandomElement` throws a non-TLC exception.
pub const RANDOM_ELEMENT_METHOD: &str =
    "public static tlc2.value.impl.Value tlc2.module.TLC.RandomElement(tlc2.value.impl.Value)";
/// The Java method TLC names when `RandomSubset` throws a non-TLC exception.
pub const RANDOM_SUBSET_METHOD: &str = "public static tlc2.value.impl.Value \
     tlc2.module.Randomization.RandomSubset(tlc2.value.impl.Value,tlc2.value.impl.Value)";
/// The Java method TLC names when `RandomSetOfSubsets` throws a non-TLC exception.
pub const RANDOM_SET_OF_SUBSETS_METHOD: &str = "public static tlc2.value.impl.Value \
     tlc2.module.Randomization.RandomSetOfSubsets(tlc2.value.impl.Value,\
     tlc2.value.impl.Value,tlc2.value.impl.Value)";

/// An error from a random operator, with TLC's error class and message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RandomError {
    /// `EC.TLC_MODULE_ARGUMENT_ERROR` (2169): `The <position> argument of <op> should
    /// be a <expected>, but instead it is:\n<value>`.
    Argument {
        /// `first`, `second` or `third`.
        position: &'static str,
        /// The operator.
        op: &'static str,
        /// What TLC expected (TLC's wording, including its `a a finite set`).
        expected: String,
        /// The argument as TLC prints it.
        value: String,
    },
    /// `EC.TLC_MODULE_APPLYING_TO_WRONG_VALUE` (2176): `Applying <op> to the following
    /// value,\nwhich is not <expected>:\n<value>`.
    WrongValue {
        /// The operator.
        op: &'static str,
        /// What TLC expected.
        expected: &'static str,
        /// The value as TLC prints it.
        value: String,
    },
    /// `EC.TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE` (2154): a Java exception other than
    /// TLC's own evaluation errors escaped the override (an index out of bounds, a
    /// negative array size, or a `GENERAL` error such as an unenumerable set).
    JavaOverride {
        /// The Java method's signature, as TLC prints it.
        method: &'static str,
        /// The exception's message.
        message: String,
    },
    /// A value-layer error that TLC reports with its own class.
    Value(ValueError),
}

impl RandomError {
    /// TLC's `EC` code.
    #[must_use]
    pub const fn tlc_code(&self) -> u32 {
        match self {
            Self::Argument { .. } => 2169,
            Self::WrongValue { .. } => 2176,
            Self::JavaOverride { .. } => 2154,
            Self::Value(e) => e.kind().tlc_code(),
        }
    }

    /// The message TLC prints.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Argument {
                position,
                op,
                expected,
                value,
            } => format!(
                "The {position} argument of {op} should be a {expected}, but instead it is:\n{value}"
            ),
            Self::WrongValue {
                op,
                expected,
                value,
            } => {
                format!("Applying {op} to the following value,\nwhich is not {expected}:\n{value}")
            }
            Self::JavaOverride { method, message } => format!(
                "Attempted to apply the operator overridden by the Java method\n{method},\n\
                 but it produced the following error:\n{message}"
            ),
            Self::Value(e) => e.message().to_owned(),
        }
    }
}

impl fmt::Display for RandomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TLC error {}: {}", self.tlc_code(), self.message())
    }
}

impl std::error::Error for RandomError {}

type Result<T> = std::result::Result<T, RandomError>;

/// `Values.ppr(v.toString())`.
fn ppr(v: &Value, ctx: Ctx) -> String {
    print_value(v, ctx).unwrap_or_else(|_| to_tlc_string_lenient(v, ctx))
}

/// A value-layer error raised inside an override. TLC's `GENERAL` errors are
/// `Assert.fail`s, not `EvalException`s, so the override reports them as
/// `TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE`; errors with their own class pass through.
fn in_override(method: &'static str) -> impl Fn(ValueError) -> RandomError {
    move |e| match e.kind() {
        ErrorKind::DiveLimit | ErrorKind::Codec | ErrorKind::Json => RandomError::Value(e),
        k if k.tlc_code() == 1000 => RandomError::JavaOverride {
            method,
            message: e.message().to_owned(),
        },
        _ => RandomError::Value(e),
    }
}

/// Infinite sets: TLC's `toSetEnum` gives `null` for them, and `isFinite` is false.
fn is_infinite_set(s: &Set) -> bool {
    matches!(s.kind(), SetKind::Nat | SetKind::Int | SetKind::Strings) || s.is_infinite()
}

fn dive_limit(what: &str) -> RandomError {
    RandomError::Value(ValueError::new(
        ErrorKind::DiveLimit,
        format!("{what} is outside the 32-bit range that TLC's random operators support"),
    ))
}

fn to_i32_size(n: u64) -> Result<i32> {
    i32::try_from(n).map_err(|_| dive_limit(&format!("a set of {n} elements")))
}

// ---------------------------------------------------------------------------------
// RandomElement
// ---------------------------------------------------------------------------------

/// `RandomElement(S)`: a pseudo-random element of the finite set `S`
/// (`tlc2.module.TLC.RandomElement`).
///
/// It draws exactly as TLC does, so with a [`JavaRandom`](super::JavaRandom) stream
/// seeded as TLC seeds it, the result is TLC's:
/// - `[D -> R]`: one `RandomElement(R)` per element of `D`, in normalized order;
/// - `[f1 : S1, ...]`: one per field, in TLC's field order;
/// - `S1 \X ... \X Sn`: one per component;
/// - `a..b`: `a + floor(nextDouble() * (b - a + 1))`;
/// - any other finite set: element `floor(nextDouble() * |S|)` of its normalized
///   enumeration.
///
/// # Errors
/// TLC's: `TLC_MODULE_APPLYING_TO_WRONG_VALUE` for a non-set or an infinite set, and
/// `TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE` for an empty set (after a draw, as in
/// TLC) and for enumeration errors.
pub fn random_element(set: &Value, ctx: Ctx, src: &mut dyn RandomSource) -> Result<Value> {
    let wrap = in_override(RANDOM_ELEMENT_METHOD);
    let wrong = |v: &Value| RandomError::WrongValue {
        op: "RandomElement",
        expected: "a finite set",
        value: ppr(v, ctx),
    };
    let Value::Set(s) = set else {
        return Err(wrong(set));
    };
    if let Some((dom, range)) = s.as_fcn_set() {
        let dom_elems = match dom {
            Value::Set(d) if !is_infinite_set(d) => d.elements(ctx).map_err(&wrap)?,
            _ => return Err(wrong(set)),
        };
        let mut pairs = Vec::with_capacity(dom_elems.len());
        for d in dom_elems.iter() {
            pairs.push((d.clone(), random_element(range, ctx, src)?));
        }
        return Value::fcn(pairs, ctx).map_err(&wrap);
    }
    if let Some(fields) = s.as_rcd_set() {
        let mut out: Vec<(Atom, Value)> = Vec::with_capacity(fields.len());
        for (name, field_set) in fields {
            out.push((name.clone(), random_element(field_set, ctx, src)?));
        }
        return Value::record(out, ctx).map_err(&wrap);
    }
    if let Some(factors) = s.as_product() {
        let mut out = Vec::with_capacity(factors.len());
        for f in factors {
            out.push(random_element(f, ctx, src)?);
        }
        return Ok(Value::tuple(out));
    }
    if is_infinite_set(s) {
        return Err(wrong(set));
    }
    if let Some((lo, _)) = s.as_interval() {
        let size = s.size(ctx).map_err(&wrap)?;
        let idx = src.next_index(size);
        if idx >= size {
            return Err(RandomError::JavaOverride {
                method: RANDOM_ELEMENT_METHOD,
                message: format!(
                    "Attempted to retrieve out-of-bounds element from the interval value {}.",
                    ppr(set, ctx)
                ),
            });
        }
        let v = lo
            .add(&Int::from(idx.cast_signed()), ctx.int_mode)
            .map_err(&wrap)?;
        return Ok(Value::int(v));
    }
    let elems = s.elements(ctx).map_err(&wrap)?;
    let len = elems.len() as u64;
    let idx = src.next_index(len);
    usize::try_from(idx)
        .ok()
        .and_then(|i| elems.get(i))
        .cloned()
        .ok_or_else(|| RandomError::JavaOverride {
            method: RANDOM_ELEMENT_METHOD,
            // Java's ArrayIndexOutOfBoundsException text. TLC's length is the
            // backing array's capacity, 0 for literal and most other empty sets.
            message: format!("Index {idx} out of bounds for length {len}"),
        })
}

// ---------------------------------------------------------------------------------
// RandomSubset
// ---------------------------------------------------------------------------------

/// `RandomSubset(k, S)`: `k` pseudo-random elements of the finite set `S`, without
/// repetition while `k <= |S|` (`tlc2.module.Randomization.RandomSubset`, through
/// `getRandomSubset` of each set class). For `k > |S|` the indices wrap around and the
/// result is all of `S`, as in TLC.
///
/// The result is unnormalized, as TLC builds it (`new SetEnumValue(vec, false)`).
///
/// # Errors
/// TLC's: `TLC_MODULE_ARGUMENT_ERROR` when `k` is not an integer or `S` not a finite
/// set; `TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE` for `k < 0` (Java's
/// `NegativeArraySizeException`) and enumeration errors.
pub fn random_subset(
    k: &Value,
    set: &Value,
    ctx: Ctx,
    src: &mut dyn RandomSource,
) -> Result<Value> {
    let wrap = in_override(RANDOM_SUBSET_METHOD);
    let Some(k_int) = k.as_int() else {
        return Err(RandomError::Argument {
            position: "first",
            op: "RandomSubset",
            expected: "nonnegative integer".to_owned(),
            value: ppr(k, ctx),
        });
    };
    let not_finite = || RandomError::Argument {
        position: "second",
        op: "RandomSubset",
        expected: "a finite set".to_owned(),
        value: ppr(set, ctx),
    };
    let Value::Set(s) = set else {
        return Err(not_finite());
    };
    if is_infinite_set(s) {
        return Err(not_finite());
    }
    let k = k_int
        .to_i32()
        .ok_or_else(|| dive_limit(&format!("RandomSubset's count {k_int}")))?;
    let negative = || RandomError::JavaOverride {
        method: RANDOM_SUBSET_METHOD,
        message: k.to_string(),
    };

    if s.as_fcn_set().is_some() || s.as_rcd_set().is_some() || s.as_product().is_some() {
        if k < 0 {
            return Err(negative());
        }
        return product_subset(k, set, s, ctx, src);
    }
    if let Some((lo, _)) = s.as_interval() {
        if k < 0 {
            return Err(negative());
        }
        let n = to_i32_size(s.size(ctx).map_err(&wrap)?)?;
        let mut indices = SubsetIndices::new(k, n, src);
        let mut out = Vec::new();
        while let Some(i) = indices.next_index() {
            let v = lo
                .add(
                    &Int::from(i64::try_from(i).unwrap_or(i64::MAX)),
                    ctx.int_mode,
                )
                .map_err(&wrap)?;
            out.push(Value::int(v));
        }
        return Ok(Value::set_raw(out, ctx));
    }
    // SetEnumValue, and every other enumerable set through `toSetEnum` (which
    // enumerates before the negative-size error).
    let elems = s.elements(ctx).map_err(&wrap)?;
    if k < 0 {
        return Err(negative());
    }
    let n = to_i32_size(elems.len() as u64)?;
    let mut indices = SubsetIndices::new(k, n, src);
    let mut out = Vec::new();
    while let Some(i) = indices.next_index() {
        if let Some(v) = elems.get(i) {
            out.push(v.clone());
        }
    }
    Ok(Value::set_raw(out, ctx))
}

/// The constituents of `[D -> R]`, `[f : S, ...]` or `S1 \X ... \X Sn` as TLC's
/// `SetOfFcnsOrRcdsValue.Product` reports them, most significant first, and how to
/// build an element from one value per constituent.
enum Product {
    Fcn(std::sync::Arc<[Value]>),
    Rcd(Vec<Atom>),
    Tuple,
}

fn product_subset(
    count: i32,
    whole: &Value,
    set: &Set,
    ctx: Ctx,
    src: &mut dyn RandomSource,
) -> Result<Value> {
    let wrap = in_override(RANDOM_SUBSET_METHOD);
    if tlc::is_empty(whole, ctx).map_err(&wrap)? {
        return Ok(Value::set_raw(Vec::new(), ctx));
    }
    let elems_of = |v: &Value| -> Result<std::sync::Arc<[Value]>> {
        match v {
            Value::Set(c) => c.elements(ctx).map_err(&wrap),
            _ => Err(RandomError::Argument {
                position: "second",
                op: "RandomSubset",
                expected: "a finite set".to_owned(),
                value: ppr(whole, ctx),
            }),
        }
    };
    let (shape, constituents): (Product, Vec<std::sync::Arc<[Value]>>) =
        if let Some((dom, range)) = set.as_fcn_set() {
            let domain = elems_of(dom)?;
            let range = elems_of(range)?;
            let c = vec![range; domain.len()];
            (Product::Fcn(domain), c)
        } else if let Some(fields) = set.as_rcd_set() {
            let names = fields.iter().map(|(n, _)| n.clone()).collect();
            let c = fields
                .iter()
                .map(|(_, f)| elems_of(f))
                .collect::<Result<Vec<_>>>()?;
            (Product::Rcd(names), c)
        } else {
            let factors = set.as_product().unwrap_or(&[]);
            let c = factors.iter().map(elems_of).collect::<Result<Vec<_>>>()?;
            (Product::Tuple, c)
        };
    let build = |values: Vec<Value>| -> Result<Value> {
        match &shape {
            Product::Fcn(d) => {
                Value::fcn(d.iter().cloned().zip(values).collect(), ctx).map_err(&wrap)
            }
            Product::Rcd(names) => {
                Value::record(names.iter().cloned().zip(values).collect(), ctx).map_err(&wrap)
            }
            Product::Tuple => Ok(Value::tuple(values)),
        }
    };
    // The size, and TLC's `needBigInteger`: whether it exceeds an int.
    let mut size = BigInt::from(1);
    for c in &constituents {
        size *= BigInt::from(c.len());
    }
    let mut out = Vec::new();
    if let Some(n) = size.to_i32() {
        // `SubsetEnumerator`: indices in mixed radix, rescaled by the product of the
        // sizes of the less significant constituents.
        let mut rescale = vec![1usize; constituents.len()];
        let mut acc = 1usize;
        for i in (0..constituents.len()).rev() {
            rescale[i] = acc;
            acc = acc.saturating_mul(constituents[i].len());
        }
        let mut indices = SubsetIndices::new(count, n, src);
        while let Some(idx) = indices.next_index() {
            let values = constituents
                .iter()
                .zip(&rescale)
                .map(|(c, r)| c[(idx / r) % c.len()].clone())
                .collect();
            out.push(build(values)?);
        }
    } else {
        let mut rescale = vec![BigInt::from(1); constituents.len()];
        let mut acc = BigInt::from(1);
        for i in (0..constituents.len()).rev() {
            rescale[i].clone_from(&acc);
            acc *= BigInt::from(constituents[i].len());
        }
        let mut indices = BigIndices::new(count, size, src);
        while let Some(idx) = indices.next_index() {
            let values = constituents
                .iter()
                .zip(&rescale)
                .map(|(c, r)| c[big_digit(&idx, r, c.len())].clone())
                .collect();
            out.push(build(values)?);
        }
    }
    Ok(Value::set_raw(out, ctx))
}

// ---------------------------------------------------------------------------------
// RandomSetOfSubsets
// ---------------------------------------------------------------------------------

/// `RandomSetOfSubsets(k, n, S)`: `k` pseudo-random subsets of `S`, each containing
/// each element of `S` with probability `n / |S|`, duplicates removed
/// (`tlc2.module.Randomization.RandomSetOfSubsets`, `SubsetValue.getRandomSetOfSubsets`
/// with `CoinTossingSubsetEnumerator`). Draws `k * |S|` doubles, in pick order and the
/// normalized order of `S`.
///
/// The result is unnormalized, as in TLC. TLC orders it by a Java `HashSet`; dive keeps
/// the first occurrence of each subset in pick order. No TLA+ operation observes the
/// difference: the elements are mutually comparable subsets of `S`, and TLC
/// normalizes before printing, choosing or comparing elementwise.
///
/// # Errors
/// TLC's `TLC_MODULE_ARGUMENT_ERROR`s, in TLC's order of checks.
pub fn random_set_of_subsets(
    k: &Value,
    n: &Value,
    set: &Value,
    ctx: Ctx,
    src: &mut dyn RandomSource,
) -> Result<Value> {
    const OP: &str = "RandomSetOfSubsets";
    let wrap = in_override(RANDOM_SET_OF_SUBSETS_METHOD);
    let arg = |position: &'static str, expected: &str, value: String| RandomError::Argument {
        position,
        op: OP,
        expected: expected.to_owned(),
        value,
    };
    let nonneg = |position: &'static str, v: &Value| -> Result<i32> {
        match v.as_int() {
            Some(i) if !i.is_negative() => i
                .to_i32()
                .ok_or_else(|| dive_limit(&format!("{OP}'s {position} argument {i}"))),
            _ => Err(arg(position, "nonnegative integer", ppr(v, ctx))),
        }
    };
    let picks = nonneg("first", k)?;
    let mean = nonneg("second", n)?;
    let s = match set {
        Value::Set(s) if !is_infinite_set(s) => s,
        _ => return Err(arg("third", "finite set", ppr(set, ctx))),
    };
    let size = s.size(ctx).map_err(&wrap)?;
    // `31 - numberOfLeadingZeros(k) + 1 > size && k > (1 << size)`.
    let bits = u64::from(32 - picks.leading_zeros());
    if bits > size && i64::from(picks) > (1i64 << size) {
        return Err(arg(
            "first",
            &format!("nonnegative integer that is smaller than the subset's size of 2^{size}"),
            picks.to_string(),
        ));
    }
    if size < u64::from(mean.unsigned_abs()) {
        return Err(arg(
            "second",
            "nonnegative integer in range 0..Cardinality(S)",
            ppr(n, ctx),
        ));
    }
    #[allow(
        clippy::cast_precision_loss,
        reason = "TLC: (1d * n) / ev.size(), int size"
    )]
    let probability = f64::from(mean) / size as f64;
    let elems = s.elements(ctx).map_err(&wrap)?;
    let mut subsets: Vec<Value> = Vec::new();
    for _ in 0..picks {
        let mut chosen = Vec::new();
        for e in elems.iter() {
            if src.next_double() < probability {
                chosen.push(e.clone());
            }
        }
        let subset = Value::set_raw(chosen, ctx);
        if !subsets.contains(&subset) {
            subsets.push(subset);
        }
    }
    Ok(Value::set_raw(subsets, ctx))
}
