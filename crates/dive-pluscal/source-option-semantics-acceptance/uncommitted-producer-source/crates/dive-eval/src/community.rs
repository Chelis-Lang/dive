//! TLC's Java overrides of the `Bags` standard module and of the
//! CommunityModules operators that corpus models use.
//!
//! Each operator ports the semantics of the Java method TLC installs for it,
//! including its argument checks, error messages and the order in which it
//! walks its arguments:
//! - `Bags`: `tlc2/module/Bags.java` at tlaplus `94d0c50`;
//! - the CommunityModules: `modules/tlc2/overrides/*.java` at
//!   CommunityModules `78b98ae` (release `202610040242`).
//!
//! Where the Java method fails with a Java exception whose message depends on
//! the JVM (a `ClassCastException` or `NullPointerException` thrown inside
//! the method), or where its result depends on the order of a function that
//! TLC may hold unnormalized (dive keeps functions normalized), the operator
//! is refused by name. Provenance: `docs/provenance.md`, section
//! "CommunityModules overrides".

#![allow(
    clippy::many_single_char_names,
    clippy::unused_self,
    clippy::needless_range_loop,
    reason = "short names and index loops follow the Java methods ported; helpers stay methods of the shared state like their siblings"
)]
#![allow(
    clippy::mutable_key_type,
    reason = "a value's interior mutability is its caches (hash, normalized elements, fingerprint), which never change its equality or hash"
)]

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use dive_ir::{Arg, ExprId, StdOp};
use dive_value::{Int, SetKind, Value, tlc};

use crate::core::{Core, is_enumerable, java_class};
use crate::env::{Env, Ev, OpVal, St};
use crate::error::{Ec, EvalError, Result};
use crate::std::StdArg;

/// A graph's node and edge sets, normalized.
type Graph = (Arc<[Value]>, Arc<[Value]>);

/// A Java exception thrown inside an override's method, as `MethodValue`
/// reports it (`TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE`, with the JVM's
/// message, measured on the pinned JRE 17).
fn java_exception(op: StdOp, message: &str) -> EvalError {
    match crate::std::java_signature(op) {
        Some(sig) => EvalError::eval_exception(
            Ec::JavaMethodOverride,
            format!(
                "Attempted to apply the operator overridden by the Java method\n{sig},\nbut it produced the following error:\n{message}"
            ),
        ),
        None => refuse_case(op, "a Java exception inside the override"),
    }
}

/// A refusal of one operator's case.
fn refuse_case(op: StdOp, why: &str) -> EvalError {
    EvalError::unsupported(format!("{} ({why})", op.qualified_name()))
}

impl Core {
    /// The value overrides of this module; `None` for an operator it does not
    /// implement.
    #[allow(clippy::too_many_lines, reason = "one arm per Java method")]
    pub(crate) fn call_ext(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        op: StdOp,
        args: &[StdArg],
        e: ExprId,
    ) -> Option<Result<Value>> {
        let v = |k: usize| -> Result<&Value> {
            match args.get(k) {
                Some(StdArg::V(v)) => Ok(v),
                Some(StdArg::Op(_)) => {
                    Err(refuse_case(op, "an operator where a value is expected"))
                }
                None => Err(EvalError::bug(format!(
                    "{} argument {k}",
                    op.qualified_name()
                ))),
            }
        };
        let o = |k: usize| -> Result<&OpVal> {
            match args.get(k) {
                Some(StdArg::Op(o)) => Ok(o),
                Some(StdArg::V(x)) => Err(self.cast_error(op, x, "OpValue")),
                None => Err(EvalError::bug(format!(
                    "{} argument {k}",
                    op.qualified_name()
                ))),
            }
        };
        if !implemented(op) {
            return None;
        }
        Some((|| -> Result<Value> {
            match op {
                // ------------------------------------------------------------ Bags
                StdOp::BagsEmptyBag => Ok(Value::fcn(Vec::new(), self.ctx)?),
                StdOp::BagsIsABag => {
                    let Some(pairs) = self.fcn_pairs(v(0)?) else {
                        return Ok(Value::FALSE);
                    };
                    Ok(Value::Bool(pairs.iter().all(|(_, x)| positive_int(x))))
                }
                StdOp::BagsBagCardinality => self.bag_cardinality(op, v(0)?),
                StdOp::BagsBagIn | StdOp::BagsCopiesIn => self.bag_in(op, v(0)?, v(1)?),
                StdOp::BagsBagCup => self.bag_cup(op, v(0)?, v(1)?),
                StdOp::BagsBagDiff => self.bag_diff(op, v(0)?, v(1)?),
                StdOp::BagsBagUnion => self.bag_union(op, v(0)?),
                StdOp::BagsSqSubseteq => self.sq_subseteq(op, v(0)?, v(1)?),
                StdOp::BagsBagOfAll => self.bag_of_all(env, st, op, args, e),
                StdOp::BagsBagToSet => {
                    let b = v(0)?;
                    let Value::Fcn(f) = b else {
                        return Err(self.wrong_value(
                            "BagToSet",
                            "a function with a finite domain",
                            b,
                        ));
                    };
                    Ok(self.fcn_domain(f)?)
                }
                StdOp::BagsSetToBag => {
                    let b = v(0)?;
                    let Some(elems) = self.to_set_enum(b)? else {
                        // TLC names the wrong operator here.
                        return Err(self.wrong_value(
                            "BagToSet",
                            "a function with a finite domain",
                            b,
                        ));
                    };
                    let pairs = elems.iter().map(|x| (x.clone(), Value::Int(1))).collect();
                    Ok(Value::fcn(pairs, self.ctx)?)
                }
                StdOp::BagsExtFoldBag => self.fold_bag(env, st, op, o(0)?, v(1)?, v(2)?, e),

                // ---------------------------------------------------- SequencesExt
                StdOp::SequencesExtSetToSeq => {
                    let s = v(0)?;
                    let Some(elems) = self.to_set_enum(s)? else {
                        return Err(java_exception(
                            op,
                            "Cannot invoke \"tlc2.value.impl.Value.normalize()\" because the return value of \"tlc2.value.impl.Value.toSetEnum()\" is null",
                        ));
                    };
                    Ok(Value::tuple(elems.to_vec()))
                }
                StdOp::SequencesExtSetToSeqs => self.set_to_seqs(op, v(0)?),
                StdOp::SequencesExtContains => {
                    let s = v(0)?;
                    let Some(t) = self.to_tuple(s) else {
                        return Err(self.one_arg_error("Contains", "sequence", s));
                    };
                    let x = v(1)?;
                    for y in t {
                        if self.equals(y, x)? {
                            return Ok(Value::TRUE);
                        }
                    }
                    Ok(Value::FALSE)
                }
                StdOp::SequencesExtLongestCommonPrefix => self.longest_common_prefix(op, v(0)?),
                StdOp::SequencesExtFoldLeft
                | StdOp::SequencesExtFoldLeftDomain
                | StdOp::SequencesExtFoldRight
                | StdOp::SequencesExtFoldRightDomain => {
                    let left = matches!(
                        op,
                        StdOp::SequencesExtFoldLeft | StdOp::SequencesExtFoldLeftDomain
                    );
                    let domain = matches!(
                        op,
                        StdOp::SequencesExtFoldLeftDomain | StdOp::SequencesExtFoldRightDomain
                    );
                    // FoldLeft(op, base, s); FoldRight(op, s, base).
                    let (base, s) = if left { (v(1)?, v(2)?) } else { (v(2)?, v(1)?) };
                    let Some(t) = self.to_tuple(s) else {
                        return Err(self.arg_error(
                            if left { "third" } else { "second" },
                            op.name(),
                            "sequence",
                            s,
                        ));
                    };
                    let f = o(0)?;
                    let item = |i: usize| -> Value {
                        if domain {
                            Value::Int(i64::try_from(i).unwrap_or(i64::MAX) + 1)
                        } else {
                            t[i].clone()
                        }
                    };
                    let mut acc = base.clone();
                    if left {
                        for i in 0..t.len() {
                            acc = self.apply_op_values(env, st, f, &[acc, item(i)], e)?;
                        }
                    } else {
                        for i in (0..t.len()).rev() {
                            acc = self.apply_op_values(env, st, f, &[item(i), acc], e)?;
                        }
                    }
                    Ok(acc)
                }
                StdOp::SequencesExtIsPrefix => self.is_prefix(op, v(0)?, v(1)?),
                StdOp::SequencesExtSelectInSeq | StdOp::SequencesExtSelectLastInSeq => {
                    let s = v(0)?;
                    let Some(t) = self.to_tuple(s) else {
                        return Err(self.arg_error("first", op.name(), "sequence", s));
                    };
                    let n = t.len();
                    let last = op == StdOp::SequencesExtSelectLastInSeq;
                    self.select_in(env, st, op, o(1)?, t, 1, n, last, e)
                }
                StdOp::SequencesExtSelectInSubSeq | StdOp::SequencesExtSelectLastInSubSeq => {
                    let s = v(0)?;
                    let Some(t) = self.to_tuple(s) else {
                        return Err(self.arg_error("first", op.name(), "sequence", s));
                    };
                    let f = v(1)?;
                    let Some(from) = small_int(f) else {
                        return Err(if f.as_int().is_some() {
                            refuse_case(op, "an index outside TLC's 32-bit integers")
                        } else {
                            self.arg_error("second", op.name(), "natural", f)
                        });
                    };
                    let tv = v(2)?;
                    let Some(to) = small_int(tv) else {
                        return Err(if tv.as_int().is_some() {
                            refuse_case(op, "an index outside TLC's 32-bit integers")
                        } else {
                            self.arg_error("third", op.name(), "natural", tv)
                        });
                    };
                    if from > to {
                        return Ok(Value::Int(0));
                    }
                    let len = i64::try_from(t.len()).unwrap_or(i64::MAX);
                    if from < 1 || from > len {
                        return Err(self.arg_not_in_domain("second", op.name(), s, f));
                    }
                    if to < 1 || to > len {
                        return Err(self.arg_not_in_domain("third", op.name(), s, tv));
                    }
                    let last = op == StdOp::SequencesExtSelectLastInSubSeq;
                    let (from, to) = (
                        usize::try_from(from).unwrap_or(0),
                        usize::try_from(to).unwrap_or(0),
                    );
                    self.select_in(env, st, op, o(3)?, t, from, to, last, e)
                }
                StdOp::SequencesExtRemoveFirst => {
                    let s = v(0)?;
                    let Some(t) = self.to_tuple(s) else {
                        return Err(self.arg_error("first", "RemoveFirst", "sequence", s));
                    };
                    let x = v(1)?;
                    let mut out = Vec::with_capacity(t.len());
                    let mut found = false;
                    for y in t {
                        if !found && self.equals(y, x)? {
                            found = true;
                        } else {
                            out.push(y.clone());
                        }
                    }
                    Ok(Value::tuple(out))
                }
                StdOp::SequencesExtRemoveFirstMatch => {
                    let s = v(0)?;
                    let Some(t) = self.to_tuple(s) else {
                        return Err(self.arg_error("first", "RemoveFirstMatch", "sequence", s));
                    };
                    let test = o(1)?;
                    let mut out = Vec::with_capacity(t.len());
                    let mut found = false;
                    for y in t {
                        if !found && self.test_bool(env, st, op, test, y, e)? {
                            found = true;
                            continue;
                        }
                        out.push(y.clone());
                    }
                    Ok(Value::tuple(out))
                }
                StdOp::SequencesExtSuffixes => {
                    let s = v(0)?;
                    let Some(t) = self.to_tuple(s) else {
                        return Err(self.one_arg_error("Suffixes", "sequence", s));
                    };
                    let mut out = vec![Value::tuple(Vec::new())];
                    for i in (0..t.len()).rev() {
                        out.push(Value::tuple(t[i..].to_vec()));
                    }
                    // TLC builds it as already normalized: the suffixes in
                    // order of length, which is TLC's tuple order.
                    Ok(Value::set(out, self.ctx)?)
                }
                StdOp::SequencesExtAllSubSeqs => {
                    let s = v(0)?;
                    let Some(t) = self.to_tuple(s) else {
                        return Err(self.one_arg_error("AllSubSeqs", "sequence", s));
                    };
                    let n = t.len();
                    if n > 24 {
                        return Err(EvalError::resource_limit(format!(
                            "AllSubSeqs of a sequence of {n} elements (2^{n} subsequences)"
                        )));
                    }
                    let mut out = Vec::with_capacity(1 << n);
                    for i in 0u32..(1 << n) {
                        let sub = (0..n)
                            .filter(|j| i & (1 << j) != 0)
                            .map(|j| t[j].clone())
                            .collect();
                        out.push(Value::tuple(sub));
                    }
                    Ok(Value::set_raw(out, self.ctx))
                }

                // ------------------------------------------------------- Functions
                StdOp::FunctionsIsInjective => self.is_injective(op, v(0)?),
                StdOp::FunctionsAntiFunction => self.anti_function(op, v(0)?),
                // `SequencesExt.foldSeq` calls `Functions.foldFunction`.
                StdOp::FunctionsFoldFunction | StdOp::SequencesExtFoldSeq => {
                    self.fold_function(env, st, op, "FoldFunction", o(0)?, v(1)?, v(2)?, e)
                }
                StdOp::FunctionsFoldFunctionOnSet => {
                    let fun = v(2)?;
                    let Value::Fcn(f) = fun else {
                        return Err(self.arg_error("third", "FoldFunctionOnSet", "function", fun));
                    };
                    let sub = v(3)?;
                    let Value::Set(set) = sub else {
                        return Err(self.arg_error("fourth", "FoldFunctionOnSet", "set", sub));
                    };
                    if !is_enumerable(sub) {
                        return Err(self.arg_error("fourth", "FoldFunctionOnSet", "set", sub));
                    }
                    let elems = self.tlc_elements(set, "FoldFunctionOnSet")?;
                    self.fold_over(env, st, o(0)?, v(1)?, f, &elems, e)
                }

                // --------------------------------------------------- FiniteSetsExt
                StdOp::FiniteSetsExtFoldSet => {
                    let set = v(2)?;
                    let Value::Set(s) = set else {
                        return Err(self.cast_error(op, set, "Enumerable"));
                    };
                    if !is_enumerable(set) {
                        return Err(self.cast_error(op, set, "Enumerable"));
                    }
                    let elems = self.tlc_elements(s, "FoldSet")?;
                    let f = o(0)?;
                    let mut acc = v(1)?.clone();
                    for x in elems.iter() {
                        acc = self.apply_op_values(env, st, f, &[x.clone(), acc], e)?;
                    }
                    Ok(acc)
                }
                StdOp::FiniteSetsExtQuantify => {
                    let set = v(0)?;
                    if !is_enumerable(set) {
                        return Err(self.arg_error("first", "Quantify", "set", set));
                    }
                    let Some(elems) = self.to_set_enum(set)? else {
                        return Err(self.arg_error("first", "Quantify", "set", set));
                    };
                    let test = o(1)?;
                    let mut n = 0i64;
                    for x in elems.iter() {
                        if self.test_bool(env, st, op, test, x, e)? {
                            n += 1;
                        }
                    }
                    Ok(Value::Int(n))
                }

                // --------------------------------------------------------- Bitwise
                StdOp::BitwiseAnd | StdOp::BitwiseOr | StdOp::BitwiseXor => {
                    let x = self.java_int(op, v(0)?)?;
                    let y = self.java_int(op, v(1)?)?;
                    self.java_int(op, v(2)?)?;
                    self.java_int(op, v(3)?)?;
                    Ok(Value::Int(i64::from(match op {
                        StdOp::BitwiseAnd => x & y,
                        StdOp::BitwiseOr => x | y,
                        _ => x ^ y,
                    })))
                }
                StdOp::BitwiseNot => {
                    let x = self.java_int(op, v(0)?)?;
                    // `Integer.MAX_VALUE >> Integer.numberOfLeadingZeros(x) - 1`:
                    // Java masks the shift distance to five bits.
                    let shift = (x.leading_zeros().wrapping_sub(1)) & 31;
                    let mask = i32::MAX >> shift;
                    Ok(Value::Int(i64::from(!x & mask)))
                }
                StdOp::BitwiseShiftR => {
                    let n = self.java_int(op, v(0)?)?;
                    let pos = self.java_int(op, v(1)?)?;
                    // `n >>> pos`.
                    let r = (n.cast_unsigned() >> (pos.cast_unsigned() & 31)).cast_signed();
                    Ok(Value::Int(i64::from(r)))
                }

                // ------------------------------------------------- DyadicRationals
                StdOp::DyadicRationalsReduce => self.reduce_dyadic(op, v(0)?),

                // --------------------------------------------------------- IOUtils
                StdOp::IoUtilsAtoi => {
                    let s = v(0)?;
                    if let Value::Str(a) = s {
                        let text = a.as_str();
                        if !text.is_ascii() {
                            return Err(refuse_case(
                                op,
                                "a string with non-ASCII characters (Java's Integer.parseInt accepts Unicode digits)",
                            ));
                        }
                        if let Some(i) = java_parse_int(text) {
                            return Ok(Value::Int(i64::from(i)));
                        }
                    }
                    Err(self.one_arg_error("atoi", "string", s))
                }

                // ---------------------------------------------- Graphs (directed)
                StdOp::GraphsAreConnectedIn | StdOp::UndirectedGraphsAreConnectedIn => {
                    self.are_connected_in(op, v(0)?, v(1)?, v(2)?)
                }
                StdOp::GraphsSimplePath | StdOp::UndirectedGraphsSimplePath => {
                    self.simple_path(op, v(0)?)
                }
                StdOp::GraphsIsStronglyConnected => self.is_strongly_connected(op, v(0)?),
                StdOp::UndirectedGraphsConnectedComponents => self.connected_components(op, v(0)?),
                _ => Err(crate::std::refuse(op)),
            }
        })())
    }

    // -------------------------------------------------------------------------
    // Shared helpers
    // -------------------------------------------------------------------------

    /// The `ClassCastException` the method handle's spreader throws for an
    /// argument of the wrong class, as `MethodValue` reports it.
    pub(crate) fn cast_error(&self, op: StdOp, v: &Value, class: &str) -> EvalError {
        let msg = format!(
            "Cannot cast tlc2.value.impl.{} to tlc2.value.impl.{class}",
            java_class(v)
        );
        match crate::std::java_signature(op) {
            Some(sig) => EvalError::eval_exception(
                Ec::JavaMethodOverride,
                format!(
                    "Attempted to apply the operator overridden by the Java method\n{sig},\nbut it produced the following error:\n{msg}"
                ),
            ),
            None => EvalError::unsupported(format!(
                "a {} argument to {}",
                java_class(v),
                op.qualified_name()
            )),
        }
    }

    /// An argument declared `IntValue`: TLC's `int`.
    fn java_int(&self, op: StdOp, v: &Value) -> Result<i32> {
        match v {
            Value::Int(i) => i32::try_from(*i)
                .map_err(|_| refuse_case(op, "an integer outside TLC's 32-bit range")),
            Value::BigInt(_) => Err(refuse_case(op, "an integer outside TLC's 32-bit range")),
            _ => Err(self.cast_error(op, v, "IntValue")),
        }
    }

    fn wrong_value(&self, op: &str, what: &str, v: &Value) -> EvalError {
        EvalError::eval_exception(
            Ec::ApplyingToWrongValue,
            format!(
                "Applying {op} to the following value,\nwhich is not {what}:\n{}",
                self.ppr(v)
            ),
        )
    }

    fn arg_not_in_domain(&self, which: &str, op: &str, s: &Value, x: &Value) -> EvalError {
        EvalError::eval_exception(
            Ec::ArgumentNotInDomain,
            format!(
                "The {which} argument of {op} must be in the domain of its first argument:\n{}\n, but instead it is\n{}",
                self.ppr(s),
                self.ppr(x)
            ),
        )
    }

    /// `Value.toSetEnum()` normalized: the elements of a finite set in the
    /// run's order, `None` for a non-set or `Nat`, `Int`, `STRING`, `Seq(S)`.
    pub(crate) fn to_set_enum(&self, v: &Value) -> Result<Option<Arc<[Value]>>> {
        match v {
            Value::Set(s) if is_enumerable(v) => {
                self.enum_guard(s)?;
                Ok(Some(s.elements(self.ctx)?))
            }
            _ => Ok(None),
        }
    }

    /// `Value.toFcnRcd()`'s pairs, in the function's normalized order; `None`
    /// for a non-function.
    fn fcn_pairs(&self, v: &Value) -> Option<Vec<(Value, Value)>> {
        match v {
            Value::Fcn(f) => Some(f.iter().map(|(d, x)| (d, x.clone())).collect()),
            _ => None,
        }
    }

    /// Whether TLC walks this function's domain in the order dive holds it:
    /// a tuple (TLC's interval domain) or at most one element. Other
    /// functions TLC may hold unnormalized (`:>`, `@@`, `Bags`' results).
    fn domain_order_known(f: &dive_value::Fcn) -> bool {
        f.as_tuple().is_some() || f.len() <= 1
    }

    /// `FunctionValue.getDomain()`.
    fn fcn_domain(&self, f: &dive_value::Fcn) -> Result<Value> {
        if let Some(t) = f.as_tuple() {
            let n = i64::try_from(t.len()).unwrap_or(i64::MAX);
            return Ok(Value::interval(Int::Small(1), Int::Small(n), self.ctx));
        }
        Ok(Value::set(f.iter().map(|(d, _)| d).collect(), self.ctx)?)
    }

    /// Applies a one-argument predicate; a non-boolean result is TLC's error
    /// naming the operator, which dive does not print, so it is refused.
    fn test_bool(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        op: StdOp,
        test: &OpVal,
        x: &Value,
        e: ExprId,
    ) -> Result<bool> {
        match self.apply_op_values(env, st, test, std::slice::from_ref(x), e)? {
            Value::Bool(b) => Ok(b),
            _ => Err(refuse_case(
                op,
                "a test that is not boolean-valued (TLC's message prints the operator)",
            )),
        }
    }

    // -------------------------------------------------------------------------
    // Bags
    // -------------------------------------------------------------------------

    fn bag_cardinality(&self, op: StdOp, b: &Value) -> Result<Value> {
        let Value::Fcn(f) = b else {
            return Err(self.wrong_value("BagCardinality", "a function with a finite domain", b));
        };
        let mut n = Int::Small(0);
        for (_, x) in f.iter() {
            if !positive_int(x) {
                return Err(self.wrong_value("BagCardinality", "a bag", b));
            }
            let Some(c) = x.as_int() else {
                return Err(EvalError::bug("BagCardinality count"));
            };
            n = n.add(&c, self.ctx.int_mode).map_err(|_| {
                refuse_case(
                    op,
                    "an overflow, which TLC reports with partial sums in its own order",
                )
            })?;
        }
        Ok(Value::int(n))
    }

    fn bag_in(&self, op: StdOp, x: &Value, b: &Value) -> Result<Value> {
        let Value::Fcn(f) = b else {
            return Err(java_exception(
                op,
                "Cannot read field \"values\" because \"fcn\" is null",
            ));
        };
        let known = Self::domain_order_known(f);
        for (d, c) in f.iter() {
            let eq = match self.equals(x, &d) {
                Ok(eq) => eq,
                Err(err) if known => return Err(err),
                Err(_) => {
                    return Err(refuse_case(
                        op,
                        "an element TLC cannot compare with the bag's domain (TLC's error depends on the order it holds the bag in)",
                    ));
                }
            };
            if eq {
                let Some(n) = c.as_int() else {
                    return Err(self.arg_error("second", op.name(), "bag", b));
                };
                return Ok(if op == StdOp::BagsBagIn {
                    Value::Bool(!n.is_negative() && n != Int::Small(0))
                } else {
                    c.clone()
                });
            }
        }
        Ok(if op == StdOp::BagsBagIn {
            Value::FALSE
        } else {
            Value::Int(0)
        })
    }

    /// The counts of a bag argument that TLC casts to `IntValue` without
    /// checking.
    fn counts(&self, op: StdOp, f: &dive_value::Fcn) -> Result<Vec<(Value, Int)>> {
        let mut out = Vec::with_capacity(f.len());
        for (d, x) in f.iter() {
            let Some(n) = x.as_int() else {
                return Err(refuse_case(
                    op,
                    "a bag with a count that is not an integer (TLC fails with a ClassCastException)",
                ));
            };
            out.push((d, n));
        }
        Ok(out)
    }

    fn bag_cup(&self, op: StdOp, b1: &Value, b2: &Value) -> Result<Value> {
        let (Value::Fcn(f1), Value::Fcn(f2)) = (b1, b2) else {
            return Err(refuse_case(
                op,
                "a non-function argument (TLC fails with a NullPointerException)",
            ));
        };
        if !f1.iter().all(|(_, x)| positive_int(x)) {
            return Err(self.arg_error("first", "(+)", "bag", b1));
        }
        if !f2.iter().all(|(_, x)| positive_int(x)) {
            return Err(self.arg_error("second", "(+)", "bag", b2));
        }
        let mut acc = self.counts(op, f1)?;
        for (d, n) in self.counts(op, f2)? {
            let mut found = false;
            for (d1, n1) in &mut acc {
                if self.equals(&d, d1)? {
                    *n1 = n1
                        .add(&n, self.ctx.int_mode)
                        .map_err(|_| refuse_case(op, "an overflow of a count"))?;
                    found = true;
                    break;
                }
            }
            if !found {
                acc.push((d, n));
            }
        }
        let pairs = acc.into_iter().map(|(d, n)| (d, Value::int(n))).collect();
        Ok(Value::fcn(pairs, self.ctx)?)
    }

    fn bag_diff(&self, op: StdOp, b1: &Value, b2: &Value) -> Result<Value> {
        let Value::Fcn(f1) = b1 else {
            return Err(self.arg_error("first", "(-)", "bag", b1));
        };
        let Value::Fcn(f2) = b2 else {
            return Err(self.arg_error("second", "(-)", "bag", b2));
        };
        let c1 = self.counts(op, f1)?;
        let c2 = self.counts(op, f2)?;
        let mut out = Vec::new();
        for (d, n1) in c1 {
            let mut n = n1;
            for (d2, n2) in &c2 {
                if self.equals(&d, d2)? {
                    n = n
                        .sub(n2, self.ctx.int_mode)
                        .map_err(|_| refuse_case(op, "an overflow of a count"))?;
                    break;
                }
            }
            if !n.is_negative() && n != Int::Small(0) {
                out.push((d, Value::int(n)));
            }
        }
        Ok(Value::fcn(out, self.ctx)?)
    }

    fn bag_union(&self, op: StdOp, s: &Value) -> Result<Value> {
        let Some(elems) = self.to_set_enum(s)? else {
            return Err(self.wrong_value("BagUnion", "a finite enumerable set", s));
        };
        match elems.len() {
            0 => return Ok(Value::fcn(Vec::new(), self.ctx)?),
            1 => return Ok(elems[0].clone()),
            _ => {}
        }
        let mut acc: Vec<(Value, Int)> = Vec::new();
        for b in elems.iter() {
            let Value::Fcn(f) = b else {
                return Err(EvalError::eval_exception(
                    Ec::BagUnion1,
                    format!(
                        "Attempted to apply BagUnion to the following set, whose\nelement is not a bag:\n{}",
                        self.ppr(s)
                    ),
                ));
            };
            for (d, n) in self.counts(op, f)? {
                let mut found = false;
                for (d1, n1) in &mut acc {
                    if self.equals(&d, d1)? {
                        *n1 = n1
                            .add(&n, self.ctx.int_mode)
                            .map_err(|_| refuse_case(op, "an overflow of a count"))?;
                        found = true;
                        break;
                    }
                }
                if !found {
                    acc.push((d, n));
                }
            }
        }
        let pairs = acc.into_iter().map(|(d, n)| (d, Value::int(n))).collect();
        Ok(Value::fcn(pairs, self.ctx)?)
    }

    fn sq_subseteq(&self, op: StdOp, b1: &Value, b2: &Value) -> Result<Value> {
        let Value::Fcn(f1) = b1 else {
            return Err(self.wrong_value("\\sqsubseteq", "a function with a finite domain", b1));
        };
        let Value::Fcn(f2) = b2 else {
            return Err(self.wrong_value("\\sqsubseteq", "a function with a finite domain", b2));
        };
        let c1 = self.counts(op, f1)?;
        let c2 = self.counts(op, f2)?;
        let mut le = true;
        for (d, n1) in c1 {
            let mut n = n1;
            for (d2, n2) in &c2 {
                if self.equals(&d, d2)? {
                    n = n
                        .sub(n2, self.ctx.int_mode)
                        .map_err(|_| refuse_case(op, "an overflow of a count"))?;
                    break;
                }
            }
            if !n.is_negative() && n != Int::Small(0) {
                le = false;
                if !Self::domain_order_known(f1) {
                    // TLC returns at the first such element in its own order;
                    // the answer is the same, but later comparisons it skips
                    // could have failed. Every comparison above succeeded.
                }
                break;
            }
        }
        Ok(Value::Bool(le))
    }

    fn bag_of_all(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        op: StdOp,
        args: &[StdArg],
        e: ExprId,
    ) -> Result<Value> {
        let f = match args.first() {
            Some(StdArg::Op(o)) => o.clone(),
            Some(StdArg::V(x)) => {
                return Err(EvalError::eval_exception(
                    Ec::ArgumentErrorAn,
                    format!(
                        "The first argument of BagOfAll should be an operator, but instead it is:\n{}",
                        self.ppr(x)
                    ),
                ));
            }
            None => return Err(EvalError::bug("BagOfAll arity")),
        };
        let Some(StdArg::V(b)) = args.get(1) else {
            return Err(EvalError::bug("BagOfAll arity"));
        };
        let Value::Fcn(bf) = b else {
            return Err(self.arg_error("second", "BagOfAll", "function with a finite domain", b));
        };
        if !Self::domain_order_known(bf) && self.op_impure(&f) {
            return Err(refuse_case(
                op,
                "an operator with side effects over a bag TLC may hold unnormalized (the order of its effects is TLC's internal order)",
            ));
        }
        let counts = self.counts(op, bf)?;
        let mut acc: Vec<(Value, Int)> = Vec::new();
        for (d, n) in counts {
            let x = match self.apply_op_values(env, st, &f, &[d], e) {
                Ok(x) => x,
                Err(err) if Self::domain_order_known(bf) || !err.is_eval() => return Err(err),
                Err(_) => {
                    return Err(refuse_case(
                        op,
                        "an error applying the operator over a bag TLC may hold unnormalized",
                    ));
                }
            };
            let mut found = false;
            for (y, m) in &mut acc {
                if self.equals(&x, y)? {
                    *m = m
                        .add(&n, self.ctx.int_mode)
                        .map_err(|_| refuse_case(op, "an overflow of a count"))?;
                    found = true;
                    break;
                }
            }
            if !found {
                acc.push((x, n));
            }
        }
        let pairs = acc.into_iter().map(|(d, n)| (d, Value::int(n))).collect();
        Ok(Value::fcn(pairs, self.ctx)?)
    }

    /// Whether applying an operator value may have side effects.
    fn op_impure(&self, f: &OpVal) -> bool {
        match f {
            OpVal::Def { def, .. } => {
                let d = self.ir.def(*def);
                d.effects.intersects(dive_ir::Effects::IMPURE)
                    || self
                        .ir
                        .node(d.body)
                        .effects
                        .intersects(dive_ir::Effects::IMPURE)
            }
            OpVal::Std(s) => s.impure(),
            _ => true,
        }
    }

    #[allow(clippy::too_many_arguments, reason = "the Java method's arguments")]
    fn fold_bag(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        op: StdOp,
        f: &OpVal,
        base: &Value,
        bag: &Value,
        e: ExprId,
    ) -> Result<Value> {
        let Value::Fcn(bf) = bag else {
            return Err(self.arg_error("third", "FoldBag", "bag", bag));
        };
        if !Self::domain_order_known(bf) {
            return Err(refuse_case(
                op,
                "a bag of two or more elements: TLC folds in the order it holds the bag in, which may be unnormalized",
            ));
        }
        let mut acc = base.clone();
        for (d, x) in bf.iter() {
            if !positive_int(x) {
                return Err(EvalError::eval_exception(
                    Ec::ApplyingToWrongValue,
                    format!(
                        "Applying FoldBag to the following value,\nwhich is not an element of Nat:\n{} (in: {}:>{})",
                        self.ppr(x),
                        self.ppr(&d),
                        self.ppr(x)
                    ),
                ));
            }
            let n = small_int(x).unwrap_or(0);
            for _ in 0..n {
                acc = self.apply_op_values(env, st, f, &[acc, d.clone()], e)?;
            }
        }
        Ok(acc)
    }

    // -------------------------------------------------------------------------
    // SequencesExt
    // -------------------------------------------------------------------------

    fn set_to_seqs(&self, op: StdOp, s: &Value) -> Result<Value> {
        let Some(elems) = self.to_set_enum(s)? else {
            return Err(self.wrong_value("SetToSeqs", "a finite set", s));
        };
        let len = elems.len();
        if len == 0 {
            return Ok(Value::set(vec![Value::tuple(Vec::new())], self.ctx)?);
        }
        if len > 12 {
            // TLC's `int factorial` overflows from 13 elements on.
            return Err(refuse_case(op, "a set of more than 12 elements"));
        }
        let mut idx: Vec<usize> = (0..len).collect();
        let mut in_use = vec![true; len];
        let mut out = Vec::new();
        'done: loop {
            out.push(Value::tuple(
                idx.iter().map(|&i| elems[i].clone()).collect(),
            ));
            let mut i = len;
            loop {
                if i == 0 {
                    break 'done;
                }
                i -= 1;
                let mut found = false;
                for j in idx[i] + 1..len {
                    if !in_use[j] {
                        in_use[j] = true;
                        in_use[idx[i]] = false;
                        idx[i] = j;
                        found = true;
                        break;
                    }
                }
                if found {
                    break;
                }
                if i == 0 {
                    break 'done;
                }
                in_use[idx[i]] = false;
            }
            for slot in idx.iter_mut().skip(i + 1) {
                if let Some(k) = in_use.iter().position(|used| !used) {
                    in_use[k] = true;
                    *slot = k;
                }
            }
        }
        Ok(Value::set_raw(out, self.ctx))
    }

    fn longest_common_prefix(&self, op: StdOp, s: &Value) -> Result<Value> {
        let bad = || self.one_arg_error("LongestCommonPrefix", "non-empty set", s);
        let Some(elems) = self.to_set_enum(s)? else {
            return Err(bad());
        };
        if elems.is_empty() {
            return Err(bad());
        }
        let not_seq = || self.one_arg_error("LongestCommonPrefix", "sequence", s);
        if let Value::Str(first) = &elems[0] {
            let prefix: Vec<u16> = first.as_str().encode_utf16().collect();
            let mut upper = prefix.len();
            for other in &elems[1..] {
                let Value::Str(o) = other else {
                    return Err(not_seq());
                };
                let other: Vec<u16> = o.as_str().encode_utf16().collect();
                for idx in 0..upper {
                    let Some(c) = other.get(idx) else {
                        // `String.charAt` past the end: TLC's normalized
                        // order is by intern token, not by length, so the
                        // first string need not be the shortest (fixture
                        // `ErCmLcpStringOrder`).
                        return Err(java_exception(
                            op,
                            &format!("String index out of range: {idx}"),
                        ));
                    };
                    if prefix[idx] != *c {
                        upper = idx;
                        break;
                    }
                }
                if upper == 0 {
                    return Ok(Value::string(&self.names, ""));
                }
            }
            return Ok(Value::string(
                &self.names,
                &String::from_utf16_lossy(&prefix[..upper]),
            ));
        }
        let Some(prefix) = self.to_tuple(&elems[0]) else {
            return Err(not_seq());
        };
        let mut upper = prefix.len();
        for other in &elems[1..] {
            let Some(other) = self.to_tuple(other) else {
                return Err(not_seq());
            };
            for idx in 0..upper {
                let Some(y) = other.get(idx) else {
                    return Err(refuse_case(
                        op,
                        "a sequence shorter than the first in the set's order (TLC fails with an ArrayIndexOutOfBoundsException)",
                    ));
                };
                if !self.equals(&prefix[idx], y)? {
                    upper = idx;
                    break;
                }
            }
            if upper == 0 {
                return Ok(Value::tuple(Vec::new()));
            }
        }
        Ok(Value::tuple(prefix[..upper].to_vec()))
    }

    fn is_prefix(&self, op: StdOp, a: &Value, b: &Value) -> Result<Value> {
        if let (Value::Str(x), Value::Str(y)) = (a, b) {
            return Ok(Value::Bool(y.as_str().starts_with(x.as_str())));
        }
        // `s.size() <= t.size()` with `s` or `t` null (locals 2 and 3 of a
        // class compiled without variable names).
        let npe = |local: u8| {
            java_exception(
                op,
                &format!(
                    "Cannot invoke \"tlc2.value.impl.TupleValue.size()\" because \"<local{local}>\" is null"
                ),
            )
        };
        let Some(s) = self.to_tuple(a) else {
            return Err(npe(2));
        };
        let Some(t) = self.to_tuple(b) else {
            return Err(npe(3));
        };
        if s.len() > t.len() {
            return Ok(Value::FALSE);
        }
        for (x, y) in s.iter().zip(t) {
            if !self.equals(x, y)? {
                return Ok(Value::FALSE);
            }
        }
        Ok(Value::TRUE)
    }

    /// `SelectInSeq`, `SelectLastInSeq` and their `SubSeq` forms over the
    /// 1-based positions `from..=to`.
    #[allow(clippy::too_many_arguments, reason = "the Java method's arguments")]
    fn select_in(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        op: StdOp,
        test: &OpVal,
        t: &[Value],
        from: usize,
        to: usize,
        last: bool,
        e: ExprId,
    ) -> Result<Value> {
        let positions: Vec<usize> = if last {
            (from..=to).rev().collect()
        } else {
            (from..=to).collect()
        };
        for i in positions {
            let Some(x) = t.get(i.wrapping_sub(1)) else {
                continue;
            };
            if self.test_bool(env, st, op, test, x, e)? {
                return Ok(Value::Int(i64::try_from(i).unwrap_or(i64::MAX)));
            }
        }
        Ok(Value::Int(0))
    }

    /// `ReplaceFirstSubSeq(r, s, t)` and `ReplaceAllSubSeqs(r, s, t)`: an
    /// evaluating override that handles strings and returns `null` (evaluate
    /// the TLA+ definition, which evaluates the arguments again) otherwise.
    pub(crate) fn replace_sub_seq(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        op: StdOp,
        args: &[Arg],
        e: ExprId,
    ) -> Result<Ev> {
        let mut vals = Vec::with_capacity(3);
        for a in args {
            let Arg::Expr { expr, .. } = a else {
                return Err(EvalError::bug("ReplaceSubSeq argument"));
            };
            vals.push(self.eval_v(env, st, *expr)?);
        }
        if let [Value::Str(r), Value::Str(s), Value::Str(t)] = vals.as_slice() {
            let (r, s, t) = (r.as_str(), s.as_str(), t.as_str());
            let out = if op == StdOp::SequencesExtReplaceFirstSubSeq {
                if s.is_empty() {
                    format!("{r}{t}")
                } else {
                    t.replacen(s, r, 1)
                }
            } else if s.is_empty() {
                if t.chars().any(|c| u32::from(c) > 0xFFFF) {
                    return Err(refuse_case(
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
            return Ok(Ev::V(Value::string(&self.names, &out)));
        }
        // `null`: TLC evaluates the TLA+ definition.
        let Some(def) = self
            .ir
            .defs()
            .iter()
            .position(|d| d.std == Some(op))
            .map(|i| dive_ir::DefId(u32::try_from(i).unwrap_or(u32::MAX)))
        else {
            return Err(EvalError::bug(format!(
                "no definition for {}",
                op.qualified_name()
            )));
        };
        self.call_def(env, st, def, None, args, e)
    }

    // -------------------------------------------------------------------------
    // Functions
    // -------------------------------------------------------------------------

    fn is_injective(&self, op: StdOp, v: &Value) -> Result<Value> {
        let values: Vec<Value> = match v {
            Value::Fcn(f) => f.iter().map(|(_, x)| x.clone()).collect(),
            Value::Set(s) if s.kind() == SetKind::RcdSet => s
                .as_rcd_set()
                .map(|fs| fs.iter().map(|(_, x)| x.clone()).collect())
                .unwrap_or_default(),
            _ => return Err(self.one_arg_error("IsInjective", "function", v)),
        };
        let literal_tuple = matches!(v, Value::Fcn(f) if f.is_tuple_literal());
        for i in 0..values.len() {
            for j in i + 1..values.len() {
                match self.equals(&values[i], &values[j]) {
                    Ok(true) => return Ok(Value::FALSE),
                    Ok(false) => {}
                    Err(err) if literal_tuple || !err.is_eval() => return Err(err),
                    Err(_) => {
                        return Err(refuse_case(
                            op,
                            "values TLC cannot compare (which comparison fails first depends on how TLC holds the function)",
                        ));
                    }
                }
            }
        }
        Ok(Value::TRUE)
    }

    fn anti_function(&self, op: StdOp, f: &Value) -> Result<Value> {
        let Value::Fcn(fun) = f else {
            if let Value::Set(_) = f {
                // `f.normalize()` runs first.
                let _ = self.to_set_enum(f)?;
            }
            return Err(self.one_arg_error("AntiFunction", "functions", f));
        };
        let pairs: Vec<(Value, Value)> = fun.iter().map(|(d, x)| (d, x.clone())).collect();
        let mut order: Vec<usize> = (0..pairs.len()).collect();
        let mut failed = None;
        // A stable sort by value, as `Arrays.sort` on objects.
        order.sort_by(|&a, &b| {
            if failed.is_some() {
                return std::cmp::Ordering::Equal;
            }
            match self.compare(&pairs[a].1, &pairs[b].1) {
                Ok(o) => o,
                Err(err) => {
                    failed = Some(err);
                    std::cmp::Ordering::Equal
                }
            }
        });
        if failed.is_some() {
            return Err(refuse_case(
                op,
                "values TLC cannot compare (TLC's error depends on its sort's order of comparisons)",
            ));
        }
        let mut out: Vec<(Value, Value)> = Vec::new();
        for i in order {
            let (d, x) = &pairs[i];
            let dup = match out.last() {
                Some((t, _)) => self.equals(x, t)?,
                None => false,
            };
            if !dup {
                out.push((x.clone(), d.clone()));
            }
        }
        Ok(Value::fcn(out, self.ctx)?)
    }

    /// TLC's `compareTo` in the run's value order.
    fn compare(&self, a: &Value, b: &Value) -> Result<std::cmp::Ordering> {
        if self.ctx.order == crate::core::TLC_ORDER {
            Ok(tlc::compare(a, b, self.ctx)?)
        } else {
            Ok(a.cmp_with(b, self.ctx.order))
        }
    }

    /// `Functions.foldFunction`: `foldFunctionOnSet` over the function's
    /// domain, in its normalized order.
    #[allow(clippy::too_many_arguments, reason = "the Java method's arguments")]
    fn fold_function(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        _op: StdOp,
        name: &str,
        f: &OpVal,
        base: &Value,
        fun: &Value,
        e: ExprId,
    ) -> Result<Value> {
        let Value::Fcn(fc) = fun else {
            return Err(self.arg_error("third", name, "function", fun));
        };
        let domain: Vec<Value> = match fc.as_tuple() {
            Some(t) => (1..=t.len())
                .map(|i| Value::Int(i64::try_from(i).unwrap_or(i64::MAX)))
                .collect(),
            None => match self.fcn_domain(fc)? {
                Value::Set(s) => s.elements(self.ctx)?.to_vec(),
                _ => Vec::new(),
            },
        };
        self.fold_over(env, st, f, base, fc, &domain, e)
    }

    #[allow(clippy::too_many_arguments, reason = "the Java method's arguments")]
    fn fold_over(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        f: &OpVal,
        base: &Value,
        fun: &dive_value::Fcn,
        domain: &[Value],
        e: ExprId,
    ) -> Result<Value> {
        let mut acc = base.clone();
        for x in domain {
            let y = self.apply_fcn(fun, x)?;
            acc = self.apply_op_values(env, st, f, &[y, acc], e)?;
        }
        Ok(acc)
    }

    // -------------------------------------------------------------------------
    // DyadicRationals
    // -------------------------------------------------------------------------

    fn reduce_dyadic(&self, op: StdOp, val: &Value) -> Result<Value> {
        let rec = match val {
            Value::Fcn(f) if f.is_record_literal() => f,
            // TLC names `Half` here.
            _ => return Err(self.one_arg_error("Half", "record", val)),
        };
        let field = |name: &str| -> Result<Value> {
            let key = Value::string(&self.names, name);
            match rec.apply(&key) {
                Some(x) => Ok(x.clone()),
                None => Err(EvalError::coded(
                    Ec::JavaMethodOverride,
                    format!(
                        "Attempted to apply the operator overridden by the Java method\n{},\nbut it produced the following error:\nAttempted to access nonexistent field '{name}' of record\n{}",
                        crate::std::java_signature(op).unwrap_or(""),
                        self.ppr(val)
                    ),
                )),
            }
        };
        let d = field("den")?;
        let n = field("num")?;
        let (Some(d), Some(n)) = (small_int(&d), small_int(&n)) else {
            return Err(refuse_case(
                op,
                "a numerator or denominator that is not a 32-bit integer (TLC fails with a ClassCastException)",
            ));
        };
        let g = gcd(n, d);
        if g == 0 || n == i64::from(i32::MIN) || d == i64::from(i32::MIN) {
            return Err(refuse_case(
                op,
                "a zero or minimal numerator and denominator (TLC fails with an ArithmeticException)",
            ));
        }
        if g == 1 {
            return Ok(val.clone());
        }
        // Java's truncating division.
        self.record_value(&[("den", Value::Int(d / g)), ("num", Value::Int(n / g))])
    }

    fn record_value(&self, fields: &[(&str, Value)]) -> Result<Value> {
        let fields = fields
            .iter()
            .map(|(n, v)| (self.names.intern(n), v.clone()))
            .collect();
        Ok(Value::record(fields, self.ctx)?)
    }

    // -------------------------------------------------------------------------
    // Graphs and UndirectedGraphs (`AbstractGraphs`)
    // -------------------------------------------------------------------------

    /// `AbstractGraphs.toGraph`: the node and edge sets, normalized.
    fn to_graph(&self, op: StdOp, arg_pos: &str, g: &Value) -> Result<Graph> {
        let kind = if op.module() == "Graphs" {
            "graph"
        } else {
            "undirected graph"
        };
        let err = |detail: &str| {
            EvalError::eval_exception(
                Ec::ArgumentError,
                format!(
                    "The {arg_pos} argument of {} should be a {kind} {detail}, but instead it is:\n{}",
                    op.name(),
                    self.ppr(g)
                ),
            )
        };
        let fields: Vec<(String, Value)> = match g {
            Value::Fcn(f) => {
                if let Some(fs) = f.as_record() {
                    fs.iter()
                        .map(|(a, v)| (a.as_str().to_owned(), v.clone()))
                        .collect()
                } else if f.is_empty() {
                    Vec::new()
                } else {
                    return Err(err("record with a node and an edge field"));
                }
            }
            _ => return Err(err("record with a node and an edge field")),
        };
        let get = |name: &str| {
            fields
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.clone())
        };
        let nodes = match get("node") {
            Some(n) => self.to_set_enum(&n)?,
            None => None,
        };
        let Some(nodes) = nodes else {
            return Err(err("record whose node field is a set"));
        };
        let edges = match get("edge") {
            Some(n) => self.to_set_enum(&n)?,
            None => None,
        };
        let Some(edges) = edges else {
            return Err(err("record whose edge field is a set"));
        };
        Ok((nodes, edges))
    }

    /// An edge's endpoints: a 2-tuple (`Graphs`) or a set of one or two
    /// elements (`UndirectedGraphs`); `None` for anything else.
    fn endpoints(&self, directed: bool, edge: &Value) -> Result<Option<(Value, Value)>> {
        if directed {
            return Ok(match self.to_tuple(edge) {
                Some([a, b]) => Some((a.clone(), b.clone())),
                _ => None,
            });
        }
        Ok(match self.to_set_enum(edge)? {
            Some(e) if e.len() == 1 => Some((e[0].clone(), e[0].clone())),
            Some(e) if e.len() == 2 => Some((e[0].clone(), e[1].clone())),
            _ => None,
        })
    }

    /// `AbstractGraphs.adjacency`: arcs between nodes, in edge order.
    fn adjacency(
        &self,
        directed: bool,
        transpose: bool,
        nodes_set: &Value,
        edges: &[Value],
    ) -> Result<HashMap<Value, Vec<Value>>> {
        let mut adj: HashMap<Value, Vec<Value>> = HashMap::new();
        for edge in edges {
            let Some((mut from, mut to)) = self.endpoints(directed, edge)? else {
                continue;
            };
            if !self.member(&from, nodes_set)? || !self.member(&to, nodes_set)? {
                continue;
            }
            if transpose {
                std::mem::swap(&mut from, &mut to);
            }
            adj.entry(from.clone()).or_default().push(to.clone());
            if !directed && from != to {
                adj.entry(to).or_default().push(from);
            }
        }
        Ok(adj)
    }

    fn are_connected_in(&self, op: StdOp, m: &Value, n: &Value, g: &Value) -> Result<Value> {
        let directed = op.module() == "Graphs";
        let (nodes, edges) = self.to_graph(op, "third", g)?;
        let nodes_set = Value::set(nodes.to_vec(), self.ctx)?;
        if !self.member(m, &nodes_set)? || !self.member(n, &nodes_set)? {
            return Ok(Value::FALSE);
        }
        if self.equals(m, n)? {
            return Ok(Value::TRUE);
        }
        let adj = self.adjacency(directed, false, &nodes_set, &edges)?;
        Ok(Value::Bool(reachable(m, &adj).contains(n)))
    }

    fn simple_path(&self, op: StdOp, g: &Value) -> Result<Value> {
        let directed = op.module() == "Graphs";
        let (nodes, edges) = self.to_graph(op, "first", g)?;
        let nodes_set = Value::set(nodes.to_vec(), self.ctx)?;
        let adj = self.adjacency(directed, false, &nodes_set, &edges)?;
        let mut paths = Vec::new();
        for start in nodes.iter() {
            let mut path = vec![start.clone()];
            let mut visited: HashSet<Value> = HashSet::from([start.clone()]);
            extend_path(start, &adj, &mut path, &mut visited, &mut paths);
        }
        Ok(Value::set_raw(paths, self.ctx))
    }

    fn is_strongly_connected(&self, op: StdOp, g: &Value) -> Result<Value> {
        let (nodes, edges) = self.to_graph(op, "first", g)?;
        if nodes.is_empty() {
            return Ok(Value::TRUE);
        }
        let nodes_set = Value::set(nodes.to_vec(), self.ctx)?;
        let root = &nodes[0];
        let adj = self.adjacency(true, false, &nodes_set, &edges)?;
        if reachable(root, &adj).len() != nodes.len() {
            return Ok(Value::FALSE);
        }
        let radj = self.adjacency(true, true, &nodes_set, &edges)?;
        Ok(Value::Bool(reachable(root, &radj).len() == nodes.len()))
    }

    fn connected_components(&self, op: StdOp, g: &Value) -> Result<Value> {
        let (nodes, edges) = self.to_graph(op, "first", g)?;
        let nodes_set = Value::set(nodes.to_vec(), self.ctx)?;
        let index: HashMap<&Value, usize> = nodes.iter().enumerate().map(|(i, v)| (v, i)).collect();
        let mut parent: Vec<usize> = (0..nodes.len()).collect();
        for edge in edges.iter() {
            let Some((a, b)) = self.endpoints(false, edge)? else {
                continue;
            };
            if self.member(&a, &nodes_set)? && self.member(&b, &nodes_set)? {
                let (Some(&i), Some(&j)) = (index.get(&a), index.get(&b)) else {
                    continue;
                };
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                if ri != rj {
                    parent[rj] = ri;
                }
            }
        }
        let mut comps: Vec<(usize, Vec<Value>)> = Vec::new();
        for (i, v) in nodes.iter().enumerate() {
            let r = find(&mut parent, i);
            match comps.iter_mut().find(|(k, _)| *k == r) {
                Some((_, c)) => c.push(v.clone()),
                None => comps.push((r, vec![v.clone()])),
            }
        }
        let sets = comps
            .into_iter()
            .map(|(_, c)| Value::set(c, self.ctx))
            .collect::<dive_value::Result<Vec<_>>>()?;
        Ok(Value::set(sets, self.ctx)?)
    }
}

/// Union-find's `find`, flattening the path.
fn find(parent: &mut [usize], i: usize) -> usize {
    let mut r = i;
    while parent[r] != r {
        r = parent[r];
    }
    let mut c = i;
    while parent[c] != r {
        let p = parent[c];
        parent[c] = r;
        c = p;
    }
    r
}

/// Breadth-first reachability from `source`, inclusive.
fn reachable(source: &Value, adj: &HashMap<Value, Vec<Value>>) -> HashSet<Value> {
    let mut visited = HashSet::from([source.clone()]);
    let mut frontier = VecDeque::from([source.clone()]);
    while let Some(cur) = frontier.pop_front() {
        for succ in adj.get(&cur).map(Vec::as_slice).unwrap_or_default() {
            if visited.insert(succ.clone()) {
                frontier.push_back(succ.clone());
            }
        }
    }
    visited
}

fn extend_path(
    cur: &Value,
    adj: &HashMap<Value, Vec<Value>>,
    path: &mut Vec<Value>,
    visited: &mut HashSet<Value>,
    paths: &mut Vec<Value>,
) {
    paths.push(Value::tuple(path.clone()));
    for succ in adj.get(cur).map(Vec::as_slice).unwrap_or_default() {
        if visited.insert(succ.clone()) {
            path.push(succ.clone());
            extend_path(succ, adj, path, visited, paths);
            path.pop();
            visited.remove(succ);
        }
    }
}

/// Whether a bag count is valid: an integer greater than 0.
fn positive_int(x: &Value) -> bool {
    x.as_int()
        .is_some_and(|n| !n.is_negative() && n != Int::Small(0))
}

/// An integer that fits TLC's `int`.
fn small_int(v: &Value) -> Option<i64> {
    match v {
        Value::Int(i) if i32::try_from(*i).is_ok() => Some(*i),
        _ => None,
    }
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

/// `Integer.parseInt` on ASCII text.
fn java_parse_int(s: &str) -> Option<i32> {
    let b = s.as_bytes();
    let (neg, digits) = match b.first()? {
        b'-' => (true, &b[1..]),
        b'+' => (false, &b[1..]),
        _ => (false, b),
    };
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let mut n: i64 = 0;
    for d in digits {
        n = n * 10 + i64::from(d - b'0');
        if n > i64::from(i32::MAX) + 1 {
            return None;
        }
    }
    i32::try_from(if neg { -n } else { n }).ok()
}

/// Whether this module implements `op`.
fn implemented(op: StdOp) -> bool {
    matches!(
        op,
        StdOp::BagsEmptyBag
            | StdOp::BagsIsABag
            | StdOp::BagsBagCardinality
            | StdOp::BagsBagIn
            | StdOp::BagsCopiesIn
            | StdOp::BagsBagCup
            | StdOp::BagsBagDiff
            | StdOp::BagsBagUnion
            | StdOp::BagsSqSubseteq
            | StdOp::BagsBagOfAll
            | StdOp::BagsBagToSet
            | StdOp::BagsSetToBag
            | StdOp::BagsExtFoldBag
            | StdOp::SequencesExtSetToSeq
            | StdOp::SequencesExtSetToSeqs
            | StdOp::SequencesExtContains
            | StdOp::SequencesExtLongestCommonPrefix
            | StdOp::SequencesExtFoldSeq
            | StdOp::SequencesExtFoldLeft
            | StdOp::SequencesExtFoldRight
            | StdOp::SequencesExtFoldLeftDomain
            | StdOp::SequencesExtFoldRightDomain
            | StdOp::SequencesExtIsPrefix
            | StdOp::SequencesExtSelectInSeq
            | StdOp::SequencesExtSelectInSubSeq
            | StdOp::SequencesExtSelectLastInSeq
            | StdOp::SequencesExtSelectLastInSubSeq
            | StdOp::SequencesExtRemoveFirst
            | StdOp::SequencesExtRemoveFirstMatch
            | StdOp::SequencesExtSuffixes
            | StdOp::SequencesExtAllSubSeqs
            | StdOp::FunctionsIsInjective
            | StdOp::FunctionsAntiFunction
            | StdOp::FunctionsFoldFunction
            | StdOp::FunctionsFoldFunctionOnSet
            | StdOp::FiniteSetsExtFoldSet
            | StdOp::FiniteSetsExtQuantify
            | StdOp::BitwiseAnd
            | StdOp::BitwiseOr
            | StdOp::BitwiseXor
            | StdOp::BitwiseNot
            | StdOp::BitwiseShiftR
            | StdOp::DyadicRationalsReduce
            | StdOp::IoUtilsAtoi
            | StdOp::GraphsAreConnectedIn
            | StdOp::GraphsSimplePath
            | StdOp::GraphsIsStronglyConnected
            | StdOp::UndirectedGraphsAreConnectedIn
            | StdOp::UndirectedGraphsSimplePath
            | StdOp::UndirectedGraphsConnectedComponents
    )
}

/// `Method.toString()` of the Java methods this module ports.
#[allow(clippy::too_many_lines, reason = "a table")]
pub(crate) fn java_signature(op: StdOp) -> Option<&'static str> {
    Some(match op {
        StdOp::BagsEmptyBag => "public static tlc2.value.impl.Value tlc2.module.Bags.EmptyBag()",
        StdOp::BagsIsABag => {
            "public static tlc2.value.IBoolValue tlc2.module.Bags.IsABag(tlc2.value.impl.Value)"
        }
        StdOp::BagsBagCardinality => {
            "public static tlc2.value.impl.IntValue tlc2.module.Bags.BagCardinality(tlc2.value.impl.Value)"
        }
        StdOp::BagsBagIn => {
            "public static tlc2.value.IBoolValue tlc2.module.Bags.BagIn(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::BagsCopiesIn => {
            "public static tlc2.value.impl.IntValue tlc2.module.Bags.CopiesIn(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::BagsBagCup => {
            "public static tlc2.value.impl.Value tlc2.module.Bags.BagCup(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::BagsBagDiff => {
            "public static tlc2.value.impl.Value tlc2.module.Bags.BagDiff(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::BagsBagUnion => {
            "public static tlc2.value.impl.Value tlc2.module.Bags.BagUnion(tlc2.value.impl.Value)"
        }
        StdOp::BagsSqSubseteq => {
            "public static tlc2.value.IBoolValue tlc2.module.Bags.SqSubseteq(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::BagsBagOfAll => {
            "public static tlc2.value.impl.Value tlc2.module.Bags.BagOfAll(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::BagsBagToSet => {
            "public static tlc2.value.impl.Value tlc2.module.Bags.BagToSet(tlc2.value.impl.Value)"
        }
        StdOp::BagsSetToBag => {
            "public static tlc2.value.impl.Value tlc2.module.Bags.SetToBag(tlc2.value.impl.Value)"
        }
        StdOp::BagsExtFoldBag => {
            "public static tlc2.value.impl.Value tlc2.overrides.BagsExt.foldBag(tlc2.value.impl.OpValue,tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::SequencesExtSetToSeq => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.SetToSeq(tlc2.value.impl.Value)"
        }
        StdOp::SequencesExtSetToSeqs => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.SetToSeqs(tlc2.value.impl.Value)"
        }
        StdOp::SequencesExtContains => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.Contains(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::SequencesExtLongestCommonPrefix => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.longestCommonPrefix(tlc2.value.impl.Value)"
        }
        StdOp::SequencesExtFoldSeq => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.foldSeq(tlc2.value.impl.OpValue,tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::SequencesExtFoldLeft => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.foldLeft(tlc2.value.impl.OpValue,tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::SequencesExtFoldRight => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.foldRight(tlc2.value.impl.OpValue,tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::SequencesExtFoldLeftDomain => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.foldLeftDomain(tlc2.value.impl.OpValue,tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::SequencesExtFoldRightDomain => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.foldRightDomain(tlc2.value.impl.OpValue,tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::SequencesExtIsPrefix => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.isPrefix(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::SequencesExtSelectInSeq => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.selectInSeq(tlc2.value.impl.Value,tlc2.value.impl.OpValue)"
        }
        StdOp::SequencesExtSelectInSubSeq => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.SelectInSubSeq(tlc2.value.impl.Value,tlc2.value.impl.Value,tlc2.value.impl.Value,tlc2.value.impl.OpValue)"
        }
        StdOp::SequencesExtSelectLastInSeq => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.selectLastInSeq(tlc2.value.impl.Value,tlc2.value.impl.OpValue)"
        }
        StdOp::SequencesExtSelectLastInSubSeq => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.SelectLastInSubSeq(tlc2.value.impl.Value,tlc2.value.impl.Value,tlc2.value.impl.Value,tlc2.value.impl.OpValue)"
        }
        StdOp::SequencesExtRemoveFirst => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.removeFirst(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::SequencesExtRemoveFirstMatch => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.removeFirstMatch(tlc2.value.impl.Value,tlc2.value.impl.OpValue)"
        }
        StdOp::SequencesExtSuffixes => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.Suffixes(tlc2.value.impl.Value)"
        }
        StdOp::SequencesExtAllSubSeqs => {
            "public static tlc2.value.impl.Value tlc2.overrides.SequencesExt.AllSubSeqs(tlc2.value.impl.Value)"
        }
        StdOp::FunctionsIsInjective => {
            "public static tlc2.value.impl.BoolValue tlc2.overrides.Functions.IsInjective(tlc2.value.impl.Value)"
        }
        StdOp::FunctionsAntiFunction => {
            "public static tlc2.value.impl.Value tlc2.overrides.Functions.antiFunction(tlc2.value.impl.Value)"
        }
        StdOp::FunctionsFoldFunction => {
            "public static tlc2.value.impl.Value tlc2.overrides.Functions.foldFunction(tlc2.value.impl.OpValue,tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::FunctionsFoldFunctionOnSet => {
            "public static tlc2.value.impl.Value tlc2.overrides.Functions.foldFunctionOnSet(tlc2.value.impl.OpValue,tlc2.value.impl.Value,tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::FiniteSetsExtFoldSet => {
            "public static tlc2.value.impl.Value tlc2.overrides.FiniteSetsExt.foldSet(tlc2.value.impl.OpValue,tlc2.value.impl.Value,tlc2.value.impl.Enumerable)"
        }
        StdOp::FiniteSetsExtQuantify => {
            "public static tlc2.value.impl.Value tlc2.overrides.FiniteSetsExt.quantify(tlc2.value.impl.Value,tlc2.value.impl.OpValue)"
        }
        StdOp::BitwiseAnd => {
            "public static tlc2.value.impl.IntValue tlc2.overrides.Bitwise.And(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue,tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::BitwiseOr => {
            "public static tlc2.value.impl.IntValue tlc2.overrides.Bitwise.Or(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue,tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::BitwiseXor => {
            "public static tlc2.value.impl.IntValue tlc2.overrides.Bitwise.Xor(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue,tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::BitwiseNot => {
            "public static tlc2.value.impl.IntValue tlc2.overrides.Bitwise.NotR(tlc2.value.impl.IntValue)"
        }
        StdOp::BitwiseShiftR => {
            "public static tlc2.value.impl.IntValue tlc2.overrides.Bitwise.shiftR(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::DyadicRationalsReduce => {
            "public static tlc2.value.impl.Value tlc2.overrides.DyadicRationals.reduce(tlc2.value.impl.Value)"
        }
        StdOp::IoUtilsAtoi => {
            "public static tlc2.value.impl.Value tlc2.overrides.IOUtils.atoi(tlc2.value.impl.Value)"
        }
        StdOp::GraphsAreConnectedIn => {
            "public static tlc2.value.impl.Value tlc2.overrides.Graphs.areConnectedIn(tlc2.value.impl.Value,tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::GraphsSimplePath => {
            "public static tlc2.value.impl.Value tlc2.overrides.Graphs.simplePath(tlc2.value.impl.Value)"
        }
        StdOp::GraphsIsStronglyConnected => {
            "public static tlc2.value.impl.Value tlc2.overrides.Graphs.isStronglyConnected(tlc2.value.impl.Value)"
        }
        StdOp::UndirectedGraphsAreConnectedIn => {
            "public static tlc2.value.impl.Value tlc2.overrides.UndirectedGraphs.areConnectedIn(tlc2.value.impl.Value,tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::UndirectedGraphsSimplePath => {
            "public static tlc2.value.impl.Value tlc2.overrides.UndirectedGraphs.simplePath(tlc2.value.impl.Value)"
        }
        StdOp::UndirectedGraphsConnectedComponents => {
            "public static tlc2.value.impl.Value tlc2.overrides.UndirectedGraphs.connectedComponents(tlc2.value.impl.Value)"
        }
        StdOp::TlcTLCSet => {
            "public static tlc2.value.impl.Value tlc2.module.TLCGetSet.TLCSet(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        _ => return None,
    })
}
