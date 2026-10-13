//! TLC's Java overrides of the standard modules that expression evaluation
//! needs: `Naturals`, `Integers`, `Sequences`, `FiniteSets`, `Strings` and the
//! `TLC` module's basic operators (`tlc2/module/*.java` at tlaplus `94d0c50`).
//!
//! TLC calls an override in one of two ways (`SpecProcessor.processModuleOverrides`):
//! - a **value** override (`MethodValue`) receives its arguments evaluated, left
//!   to right. An exception it raises that is not an `EvalException` is wrapped
//!   in `TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE` with the Java method's signature
//!   (`MethodValue.eval`); [`Core::wrap_override`] reproduces that;
//! - an **evaluating** override (`EvaluatingValue`) receives the argument
//!   expressions.
//!
//! Operators outside this set are refused as `unsupported`, naming the
//! operator, unless the model installs an implementation (`dive-stdlib`).

#![allow(
    clippy::many_single_char_names,
    clippy::unused_self,
    reason = "short names follow the Java methods ported; helpers stay methods of the shared state like their siblings"
)]

use std::sync::Arc;

use dive_ir::{Arg, ExprId, StdKind, StdOp, StdSupport};
use dive_value::{Int, SetKind, Value, print, tlc};

use crate::core::{Core, is_enumerable, java_class};
use crate::env::{Env, Ev, OpVal, St};
use crate::error::{Ec, EvalError, Result};

/// An evaluated argument of a value override.
pub(crate) enum StdArg {
    V(Value),
    Op(OpVal),
}

impl Core {
    /// Evaluates an application of a Java-overridden operator.
    pub(crate) fn eval_std(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        op: StdOp,
        args: &[Arg],
        e: ExprId,
    ) -> Result<Ev> {
        if let Some(&target) = self.std_redirects.get(&op) {
            let def = OpVal::Def {
                def: target,
                parent: None,
                dynamic: None,
            };
            return self.apply_op(&def, env, st, args, e);
        }
        if let Some(host) = self.host.get() {
            host.check_native_access(op)
                .map_err(crate::host::host_error)?;
        }
        match op.kind() {
            StdKind::Value => {
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(match a {
                        Arg::Expr { expr, .. } => StdArg::V(self.eval_v(env, st, *expr)?),
                        Arg::Op { op: o, .. } => StdArg::Op(self.op_value(env, o)?),
                    });
                }
                let _g = self.enter(e)?;
                self.call_value_op(env, st, op, &vals, e)
                    .map(Ev::V)
                    .map_err(|err| self.wrap_override(op, err))
            }
            StdKind::Evaluating => self
                .call_evaluating_op(env, st, op, args, e)
                .map_err(|err| self.wrap_evaluating(op, err)),
            StdKind::Callable => Err(refuse(op)),
        }
    }

    /// `MethodValue.eval`: an error that is not an `EvalException` becomes
    /// `TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE`.
    pub(crate) fn wrap_override(&self, op: StdOp, err: EvalError) -> EvalError {
        // int_arg models the spreader's ClassCastException and has already
        // wrapped it; MethodValue must wrap each error only once.
        if err.passes_override() || err.ec() == Ec::JavaMethodOverride {
            return err;
        }
        let Some(sig) = java_signature(op) else {
            return EvalError::unsupported(format!(
                "an error inside {} (its Java signature is not recorded)",
                op.qualified_name()
            ));
        };
        EvalError::coded(
            Ec::JavaMethodOverride,
            format!(
                "Attempted to apply the operator overridden by the Java method\n{sig},\nbut it produced the following error:\n{}",
                err.tool_text()
            ),
        )
    }

    /// `EvaluatingValue.eval`: every exception, an `EvalException` too, and
    /// including one raised while evaluating the arguments or the TLA+
    /// definition it falls back to, becomes
    /// `TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE` (fixtures `ErCmTlcGet*`).
    fn wrap_evaluating(&self, op: StdOp, err: EvalError) -> EvalError {
        if !err.is_eval() {
            return err;
        }
        let Some(method) = evaluating_method(op) else {
            return EvalError::unsupported(format!(
                "an error inside {} (its Java signature is not recorded)",
                op.qualified_name()
            ));
        };
        EvalError::coded(
            Ec::JavaMethodOverride,
            format!(
                "Attempted to apply the operator overridden by the Java method\npublic static tlc2.value.impl.Value {method}(tlc2.tool.impl.Tool,tla2sany.semantic.ExprOrOpArgNode[],tlc2.util.Context,tlc2.tool.TLCState,tlc2.tool.TLCState,int,tlc2.tool.coverage.CostModel),\nbut it produced the following error:\n{}",
                err.tool_text()
            ),
        )
    }

    #[allow(clippy::too_many_lines, reason = "one arm per Java method")]
    fn call_value_op(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        op: StdOp,
        args: &[StdArg],
        e: ExprId,
    ) -> Result<Value> {
        let v = |k: usize| -> Result<&Value> {
            match args.get(k) {
                Some(StdArg::V(v)) => Ok(v),
                _ => Err(EvalError::bug(format!(
                    "{} argument {k}",
                    op.qualified_name()
                ))),
            }
        };
        let mode = self.ctx.int_mode;
        match op {
            StdOp::NaturalsNat | StdOp::IntegersNat => Ok(Value::nat(self.ctx)),
            StdOp::IntegersInt => Ok(Value::int_set(self.ctx)),
            StdOp::StringsSTRING => Ok(Value::string_set(self.ctx)),
            StdOp::NaturalsPlus | StdOp::IntegersPlus => {
                let (a, b) = self.int_args(op, v(0)?, v(1)?)?;
                Ok(Value::int(a.add(&b, mode)?))
            }
            StdOp::NaturalsMinus | StdOp::IntegersMinus => {
                let (a, b) = self.int_args(op, v(0)?, v(1)?)?;
                Ok(Value::int(a.sub(&b, mode)?))
            }
            StdOp::NaturalsTimes | StdOp::IntegersTimes => {
                let (a, b) = self.int_args(op, v(0)?, v(1)?)?;
                Ok(Value::int(a.mul(&b, mode)?))
            }
            StdOp::NaturalsDiv => {
                // `Naturals.Divide` rounds a negative quotient that truncates to
                // 0 up to 0, and wraps `-2147483648 \div -1` (fixture `RvNatDiv`).
                let (a, b) = self.int_args(op, v(0)?, v(1)?)?;
                Ok(Value::int(a.div_naturals(&b, mode)?))
            }
            StdOp::IntegersDiv => {
                let (a, b) = self.int_args(op, v(0)?, v(1)?)?;
                Ok(Value::int(a.div(&b, mode)?))
            }
            StdOp::NaturalsMod | StdOp::IntegersMod => {
                let (a, b) = self.int_args(op, v(0)?, v(1)?)?;
                Ok(Value::int(a.rem(&b, mode)?))
            }
            StdOp::NaturalsExpt | StdOp::IntegersExpt => {
                let (a, b) = self.int_args(op, v(0)?, v(1)?)?;
                Ok(Value::int(a.pow(&b, mode)?))
            }
            StdOp::IntegersNeg => {
                let a = self.int_arg(op, v(0)?, 0)?;
                Ok(Value::int(a.neg(mode)?))
            }
            StdOp::NaturalsDotDot | StdOp::IntegersDotDot => {
                let (a, b) = self.int_args(op, v(0)?, v(1)?)?;
                Ok(Value::interval(a, b, self.ctx))
            }
            StdOp::NaturalsLt
            | StdOp::NaturalsLeq
            | StdOp::NaturalsGt
            | StdOp::NaturalsGeq
            | StdOp::IntegersLt
            | StdOp::IntegersLeq
            | StdOp::IntegersGt
            | StdOp::IntegersGeq => {
                let sym = match op {
                    StdOp::NaturalsLt | StdOp::IntegersLt => "<",
                    StdOp::NaturalsLeq | StdOp::IntegersLeq => "<=",
                    // `Naturals.GEQ` reports its operator as `>`.
                    StdOp::NaturalsGt | StdOp::IntegersGt | StdOp::NaturalsGeq => ">",
                    _ => ">=",
                };
                let check = |x: &Value, which: &str| -> Result<Int> {
                    x.as_int().ok_or_else(|| {
                        EvalError::eval_exception(
                            Ec::ArgumentErrorAn,
                            format!(
                                "The {which} argument of {sym} should be an integer, but instead it is:\n{}",
                                self.ppr(x)
                            ),
                        )
                    })
                };
                let a = check(v(0)?, "first")?;
                let b = check(v(1)?, "second")?;
                Ok(Value::Bool(match op {
                    StdOp::NaturalsLt | StdOp::IntegersLt => a < b,
                    StdOp::NaturalsLeq | StdOp::IntegersLeq => a <= b,
                    StdOp::NaturalsGt | StdOp::IntegersGt => a > b,
                    _ => a >= b,
                }))
            }
            StdOp::FiniteSetsCardinality => {
                let s = v(0)?;
                if !is_enumerable(s) {
                    return Err(EvalError::eval_exception(
                        Ec::ComputingCardinality,
                        format!(
                            "Attempted to compute cardinality of the value\n{}",
                            self.ppr(s)
                        ),
                    ));
                }
                let Value::Set(set) = s else {
                    return Err(EvalError::bug("enumerable non-set"));
                };
                self.size_guard(set)?;
                let n = set.size(self.ctx)?;
                Ok(Value::int(Int::Small(i64::try_from(n).unwrap_or(i64::MAX))))
            }
            StdOp::FiniteSetsIsFiniteSet => self.is_finite_set(v(0)?).map(Value::Bool),
            StdOp::SequencesSeq => Ok(Value::seq_set(v(0)?.clone(), self.ctx)),
            StdOp::SequencesLen => {
                let s = v(0)?;
                if let Value::Str(a) = s {
                    return Ok(Value::Int(i64::from(a.utf16_len())));
                }
                match self.to_tuple(s) {
                    Some(t) => Ok(Value::Int(i64::try_from(t.len()).unwrap_or(i64::MAX))),
                    None => Err(self.one_arg_error("Len", "sequence", s)),
                }
            }
            StdOp::SequencesHead => {
                let s = v(0)?;
                match self.to_tuple(s) {
                    Some([]) => Err(EvalError::eval_exception(
                        Ec::ApplyEmptySeq,
                        "Attempted to apply Head to the empty sequence.",
                    )),
                    Some(t) => Ok(t[0].clone()),
                    None => Err(self.one_arg_error("Head", "sequence", s)),
                }
            }
            StdOp::SequencesTail => {
                let s = v(0)?;
                if let Value::Str(a) = s {
                    let text = a.as_str();
                    if text.is_empty() {
                        return Err(EvalError::eval_exception(
                            Ec::ApplyEmptySeq,
                            "Attempted to apply Tail to the empty sequence.",
                        ));
                    }
                    return Ok(Value::string(&self.names, &java_substring(text, 1, None)));
                }
                match self.to_tuple(s) {
                    Some([]) => Err(EvalError::eval_exception(
                        Ec::ApplyEmptySeq,
                        "Attempted to apply Tail to the empty sequence.",
                    )),
                    Some(t) => Ok(Value::tuple(t[1..].to_vec())),
                    None => Err(self.one_arg_error("Tail", "sequence", s)),
                }
            }
            StdOp::SequencesCons => {
                let (x, s) = (v(0)?, v(1)?);
                match self.to_tuple(s) {
                    Some(t) => {
                        let mut out = Vec::with_capacity(t.len() + 1);
                        out.push(x.clone());
                        out.extend_from_slice(t);
                        Ok(Value::tuple(out))
                    }
                    None => Err(self.evaluating_error("Cons(v, s)", "sequence", s)),
                }
            }
            StdOp::SequencesAppend => {
                let (s, x) = (v(0)?, v(1)?);
                match self.to_tuple(s) {
                    Some(t) => {
                        let mut out = t.to_vec();
                        out.push(x.clone());
                        Ok(Value::tuple(out))
                    }
                    None => Err(self.evaluating_error("Append(s, v)", "sequence", s)),
                }
            }
            StdOp::SequencesConcat => {
                let (a, b) = (v(0)?, v(1)?);
                if let Value::Str(x) = a {
                    let Value::Str(y) = b else {
                        return Err(self.evaluating_error("t \\o s", "string", b));
                    };
                    return Ok(Value::string(
                        &self.names,
                        &format!("{}{}", x.as_str(), y.as_str()),
                    ));
                }
                let Some(t1) = self.to_tuple(a) else {
                    return Err(self.evaluating_error("s \\o t", "sequence", a));
                };
                let Some(t2) = self.to_tuple(b) else {
                    return Err(self.evaluating_error("t \\o s", "sequence", b));
                };
                if t1.is_empty() {
                    return Ok(self.tuple_value(b));
                }
                if t2.is_empty() {
                    return Ok(self.tuple_value(a));
                }
                let mut out = t1.to_vec();
                out.extend_from_slice(t2);
                Ok(Value::tuple(out))
            }
            StdOp::SequencesSubSeq => self.sub_seq(v(0)?, v(1)?, v(2)?),
            StdOp::SequencesSelectSeq => {
                let s = v(0)?;
                let Some(t) = self.to_tuple(s) else {
                    return Err(self.arg_error("first", "SelectSeq", "sequence", s));
                };
                if t.is_empty() {
                    return Ok(Value::tuple(Vec::new()));
                }
                let Some(StdArg::Op(test)) = args.get(1) else {
                    return Err(match args.get(1) {
                        Some(StdArg::V(x)) => self.arg_error("second", "SelectSeq", "operator", x),
                        _ => EvalError::bug("SelectSeq arity"),
                    });
                };
                let mut out = Vec::new();
                for x in t {
                    match self.apply_op_values(env, st, test, std::slice::from_ref(x), e)? {
                        Value::Bool(true) => out.push(x.clone()),
                        Value::Bool(false) => {}
                        r => {
                            // The pinned TLC reports "operator" and the result here.
                            return Err(self.arg_error("second", "SelectSeq", "operator", &r));
                        }
                    }
                }
                Ok(Value::tuple(out))
            }
            StdOp::TlcPrint => {
                let (a, b) = (v(0)?, v(1)?);
                let text = format!(
                    "{}  {}",
                    self.print_normalized(a)?,
                    self.print_normalized(b)?
                );
                self.print(text);
                Ok(b.clone())
            }
            StdOp::TlcPrintT => {
                let a = v(0)?;
                let text = self.print_normalized(a)?;
                self.print(text);
                Ok(Value::TRUE)
            }
            StdOp::TlcToString => {
                let a = v(0)?;
                let text = print::to_tlc_string(a, self.ctx)?;
                Ok(Value::string(&self.names, &text))
            }
            StdOp::TlcAssert => {
                let (a, b) = (v(0)?, v(1)?);
                if matches!(a, Value::Bool(true)) {
                    return Ok(a.clone());
                }
                Err(EvalError::eval_exception(
                    Ec::AssertFailed,
                    format!(
                        "The first argument of Assert evaluated to FALSE; the second argument was:\n{}",
                        self.ppr(b)
                    ),
                ))
            }
            StdOp::TlcMakeFcn => Ok(Value::fcn(vec![(v(0)?.clone(), v(1)?.clone())], self.ctx)?),
            StdOp::TlcCombineFcn => self.combine_fcn(v(0)?, v(1)?),
            StdOp::TlcSortSeq => {
                let s = v(0)?;
                let Some(t) = self.to_tuple(s) else {
                    return Err(self.arg_error("first", "SortSeq", "natural number", s));
                };
                let Some(StdArg::Op(cmp)) = args.get(1) else {
                    return Err(match args.get(1) {
                        Some(StdArg::V(x)) => self.arg_error("second", "SortSeq", "operator", x),
                        _ => EvalError::bug("SortSeq arity"),
                    });
                };
                if t.is_empty() {
                    return Ok(self.tuple_value(s));
                }
                // TLC's insertion sort (`TLC.SortSeq`).
                let mut out: Vec<Value> = vec![t[0].clone()];
                for x in &t[1..] {
                    let mut j = out.len();
                    out.push(x.clone());
                    loop {
                        let r = self.apply_op_values(
                            env,
                            st,
                            cmp,
                            &[x.clone(), out[j - 1].clone()],
                            e,
                        )?;
                        let Value::Bool(less) = r else {
                            // The pinned TLC reports "operator" and the result here.
                            return Err(self.arg_error("second", "SortSeq", "operator", &r));
                        };
                        if !less {
                            break;
                        }
                        out[j] = out[j - 1].clone();
                        j -= 1;
                        if j == 0 {
                            break;
                        }
                    }
                    out[j] = x.clone();
                }
                Ok(Value::tuple(out))
            }
            StdOp::TlcPermutations => self.permutations(v(0)?),
            StdOp::TlcTLCSet => self.tlc_set(v(0)?, v(1)?),
            StdOp::TlcRandomElement
            | StdOp::RandomizationRandomSubset
            | StdOp::RandomizationRandomSetOfSubsets => {
                let vals: Vec<Value> = (0..args.len())
                    .map(|k| v(k).cloned())
                    .collect::<Result<_>>()?;
                self.call_random(op, &vals)
            }
            _ => match self.call_ext(env, st, op, args, e) {
                Some(r) => r,
                None => Err(refuse(op)),
            },
        }
    }

    fn call_evaluating_op(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        op: StdOp,
        args: &[Arg],
        e: ExprId,
    ) -> Result<Ev> {
        match op {
            StdOp::TlcTLCEval => {
                // `TLCEval.tlcEval`: evaluates the argument and converts lazy sets
                // and functions to their explicit forms.
                let Some(Arg::Expr { expr, .. }) = args.first() else {
                    return Err(EvalError::bug("TLCEval arity"));
                };
                let v = self.eval_v(env, st, *expr)?;
                Ok(Ev::V(self.tlc_eval_convert(v)?))
            }
            StdOp::TlcTLCGet => self.tlc_get(env, st, args, e),
            StdOp::SequencesExtReplaceFirstSubSeq | StdOp::SequencesExtReplaceAllSubSeqs => {
                self.replace_sub_seq(env, st, op, args, e)
            }
            _ => Err(refuse(op)),
        }
    }

    /// `TLCEval.convert`: `toSetEnum`, else `toFcnRcd`, else the value.
    fn tlc_eval_convert(&self, v: Value) -> Result<Value> {
        if let Value::Set(s) = &v {
            if matches!(
                s.kind(),
                SetKind::Nat | SetKind::Int | SetKind::Strings | SetKind::Seq
            ) {
                return Ok(v);
            }
            if matches!(s.kind(), SetKind::Enum | SetKind::Raw) {
                return Ok(v);
            }
            let elems = s.elements(self.ctx)?;
            return Ok(Value::set(elems.to_vec(), self.ctx)?);
        }
        Ok(v)
    }

    /// Applies an operator value to already evaluated arguments (`OpValue.eval`
    /// from a Java override).
    pub(crate) fn apply_op_values(
        self: &Arc<Self>,
        env: &mut Env,
        st: &St,
        op: &OpVal,
        vals: &[Value],
        e: ExprId,
    ) -> Result<Value> {
        match op {
            OpVal::Def { def, parent, .. } => {
                let d = self.ir.def(*def);
                if let Some(s) = d.std {
                    let args: Vec<StdArg> = vals.iter().cloned().map(StdArg::V).collect();
                    return self
                        .call_value_op(env, st, s, &args, e)
                        .map_err(|err| self.wrap_override(s, err));
                }
                if vals.len() != d.params.len() {
                    return Err(EvalError::unsupported(format!(
                        "operator {} applied by a Java override to {} arguments",
                        d.name,
                        vals.len()
                    )));
                }
                let parent = match d.scope {
                    dive_ir::DefScope::Local { .. } | dive_ir::DefScope::Inline { .. } => {
                        parent.clone()
                    }
                    _ => None,
                };
                let mut callee =
                    Env::new(*def, d.depth, d.frame.len(), parent, env.dynamic.clone());
                for (i, x) in vals.iter().enumerate() {
                    callee.bind(i, crate::env::Sv::Val(x.clone()));
                }
                let _g = self.enter(e)?;
                let r = self.eval(
                    &mut callee,
                    &St {
                        control: crate::env::control::CLEAR,
                        ..st.clone()
                    },
                    d.body,
                )?;
                self.force_fcn(r)
            }
            OpVal::Std(s) => {
                let args: Vec<StdArg> = vals.iter().cloned().map(StdArg::V).collect();
                self.call_value_op(env, st, *s, &args, e)
                    .map_err(|err| self.wrap_override(*s, err))
            }
            _ => Err(EvalError::unsupported(
                "a built-in or constant operator passed to a Java override",
            )),
        }
    }

    // -----------------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------------

    /// The integer arguments of an override declared `(IntValue, IntValue)`;
    /// a non-integer makes the method handle's spreader throw a
    /// `ClassCastException`, which `MethodValue` wraps.
    fn int_args(&self, op: StdOp, a: &Value, b: &Value) -> Result<(Int, Int)> {
        Ok((self.int_arg(op, a, 0)?, self.int_arg(op, b, 1)?))
    }

    fn int_arg(&self, op: StdOp, a: &Value, _pos: usize) -> Result<Int> {
        a.as_int().ok_or_else(|| {
            let class = java_class(a);
            let msg = format!(
                "Cannot cast tlc2.value.impl.{class} to tlc2.value.impl.IntValue"
            );
            match java_signature(op) {
                Some(sig) => EvalError::coded(
                    Ec::JavaMethodOverride,
                    format!(
                        "Attempted to apply the operator overridden by the Java method\n{sig},\nbut it produced the following error:\n{msg}"
                    ),
                ),
                None => EvalError::unsupported(format!("a non-integer argument to {}", op.qualified_name())),
            }
        })
    }

    /// `Value.toTuple()`: a tuple, or a function with domain `1..n`.
    pub(crate) fn to_tuple<'a>(&self, v: &'a Value) -> Option<&'a [Value]> {
        match v {
            Value::Fcn(f) => f.as_tuple(),
            _ => None,
        }
    }

    /// The `TupleValue` TLC returns for a value it converted with `toTuple`.
    fn tuple_value(&self, v: &Value) -> Value {
        match v {
            Value::Fcn(f) if !f.is_tuple_literal() => {
                Value::tuple(f.as_tuple().map(<[Value]>::to_vec).unwrap_or_default())
            }
            other => other.clone(),
        }
    }

    pub(crate) fn one_arg_error(&self, op: &str, what: &str, v: &Value) -> EvalError {
        EvalError::eval_exception(
            Ec::OneArgumentError,
            format!(
                "The argument of {op} should be a {what}, but instead it is:\n{}",
                self.ppr(v)
            ),
        )
    }

    pub(crate) fn arg_error(&self, which: &str, op: &str, what: &str, v: &Value) -> EvalError {
        EvalError::eval_exception(
            Ec::ArgumentError,
            format!(
                "The {which} argument of {op} should be a {what}, but instead it is:\n{}",
                self.ppr(v)
            ),
        )
    }

    fn evaluating_error(&self, form: &str, what: &str, v: &Value) -> EvalError {
        EvalError::eval_exception(
            Ec::ModuleEvaluating,
            format!(
                "Evaluating an expression of the form {form} when s is not a {what}:\n{}",
                self.ppr(v)
            ),
        )
    }

    /// `Print`'s rendering: a deep-normalized copy, pretty-printed.
    fn print_normalized(&self, v: &Value) -> Result<String> {
        Ok(print::print_value(v, self.ctx)?)
    }

    /// `Sequences.SubSeq`.
    fn sub_seq(&self, s: &Value, m: &Value, n: &Value) -> Result<Value> {
        let string = match s {
            Value::Str(a) => Some(a.as_str().to_owned()),
            _ => None,
        };
        let tuple = if string.is_some() {
            None
        } else {
            self.to_tuple(s)
        };
        if string.is_none() && tuple.is_none() {
            return Err(self.arg_error("first", "SubSeq", "sequence", s));
        }
        let Some(beg) = m.as_int() else {
            return Err(self.arg_error("second", "SubSeq", "natural number", m));
        };
        let Some(end) = n.as_int() else {
            return Err(self.arg_error("third", "SubSeq", "natural number", n));
        };
        if beg > end {
            return Ok(match string {
                Some(_) => Value::string(&self.names, ""),
                None => Value::tuple(Vec::new()),
            });
        }
        let len = match (&string, tuple) {
            (Some(text), _) => i64::try_from(text.encode_utf16().count()).unwrap_or(i64::MAX),
            (None, Some(t)) => i64::try_from(t.len()).unwrap_or(i64::MAX),
            _ => 0,
        };
        let not_in_domain = |which: &str, x: &Value| {
            EvalError::eval_exception(
                Ec::ArgumentNotInDomain,
                format!(
                    "The {which} argument of SubSeq must be in the domain of its first argument:\n{}\n, but instead it is\n{}",
                    self.ppr(s),
                    self.ppr(x)
                ),
            )
        };
        let (Some(b), Some(e)) = (beg.to_i64(), end.to_i64()) else {
            return Err(not_in_domain("second", m));
        };
        if b < 1 || b > len {
            return Err(not_in_domain("second", m));
        }
        if e < 1 || e > len {
            return Err(not_in_domain("third", n));
        }
        let (bi, ei) = (
            usize::try_from(b - 1).unwrap_or(0),
            usize::try_from(e).unwrap_or(0),
        );
        match (string, tuple) {
            (Some(text), _) => Ok(Value::string(
                &self.names,
                &java_substring(&text, bi, Some(ei)),
            )),
            (None, Some(t)) => Ok(Value::tuple(t[bi..ei].to_vec())),
            _ => Err(EvalError::bug("SubSeq")),
        }
    }

    /// `TLC.CombineFcn` (`@@`).
    fn combine_fcn(&self, f1: &Value, f2: &Value) -> Result<Value> {
        let Value::Fcn(a) = f1 else {
            return Err(self.arg_error("first", "@@", "function", f1));
        };
        let Value::Fcn(b) = f2 else {
            return Err(self.arg_error("second", "@@", "function", f2));
        };
        let mut pairs: Vec<(Value, Value)> = a.iter().map(|(d, x)| (d, x.clone())).collect();
        let n1 = pairs.len();
        for (d, x) in b.iter() {
            let mut found = false;
            for (d1, _) in &pairs[..n1] {
                if tlc::equals(&d, d1, self.ctx)? {
                    found = true;
                    break;
                }
            }
            if !found {
                pairs.push((d, x.clone()));
            }
        }
        match Value::fcn(pairs, self.ctx) {
            Ok(v) => Ok(v),
            Err(err) if err.kind() == dive_value::ErrorKind::Incomparable => {
                Err(EvalError::unsupported(
                    "a function built with @@ whose domain mixes values TLC cannot compare (TLC builds it unnormalized and fails only later)",
                ))
            }
            Err(err) => Err(err.into()),
        }
    }

    /// `TLC.Permutations`.
    fn permutations(&self, s: &Value) -> Result<Value> {
        let elems = match s {
            Value::Set(set) if is_enumerable(s) => set.elements(self.ctx)?,
            _ => {
                return Err(EvalError::eval_exception(
                    Ec::ApplyingToWrongValue,
                    format!(
                        "Applying Permutations to the following value,\nwhich is not a finite set:\n{}",
                        self.ppr(s)
                    ),
                ));
            }
        };
        let mut domain = elems.to_vec();
        if self.ctx.order != crate::core::TLC_ORDER {
            domain.sort_by(|a, b| a.cmp_with(b, crate::core::TLC_ORDER));
        }
        let len = domain.len();
        if len == 0 {
            return Ok(Value::set(vec![Value::tuple_fcn(Vec::new())], self.ctx)?);
        }
        let mut idx: Vec<usize> = (0..len).collect();
        let mut in_use = vec![true; len];
        let mut fcns = Vec::new();
        'done: loop {
            let pairs: Vec<(Value, Value)> = (0..len)
                .map(|i| (domain[i].clone(), domain[idx[i]].clone()))
                .collect();
            fcns.push(Value::fcn(pairs, self.ctx)?);
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
        Ok(Value::set_raw(fcns, self.ctx))
    }

    /// `FiniteSets.IsFiniteSet`: `Value.isFinite()`, class by class.
    fn is_finite_set(&self, v: &Value) -> Result<bool> {
        let non_set = |what: &str| {
            Err(EvalError::general(format!(
                "Attempted to check if the {what} {} is a finite set.",
                self.ppr(v)
            )))
        };
        let Value::Set(s) = v else {
            return match v {
                Value::Bool(_) => non_set("boolean"),
                Value::Int(_) | Value::BigInt(_) => non_set("integer"),
                Value::Str(_) => non_set("string"),
                Value::Model(_) => non_set("model value"),
                _ => Ok(true),
            };
        };
        let comps = s.components();
        let finite_empty = |x: &Value| -> Result<(bool, bool)> {
            let f = self.is_finite_set(x)?;
            let e = if f {
                tlc::is_empty(x, self.ctx)?
            } else {
                false
            };
            Ok((f, e))
        };
        Ok(match s.kind() {
            SetKind::Enum | SetKind::Raw | SetKind::Interval => true,
            SetKind::Nat | SetKind::Int | SetKind::Strings => false,
            SetKind::Seq => match comps.first() {
                Some((_, r)) => self.is_finite_set(r)? && tlc::is_empty(r, self.ctx)?,
                None => false,
            },
            SetKind::Subset => match comps.first() {
                Some((_, b)) => self.is_finite_set(b)?,
                None => true,
            },
            SetKind::FcnSet => {
                let (Some((_, d)), Some((_, r))) = (comps.first(), comps.get(1)) else {
                    return Ok(true);
                };
                let (fd, ed) = finite_empty(d)?;
                if fd && ed {
                    return Ok(true);
                }
                let (fr, er) = finite_empty(r)?;
                if fr && (er || fd) {
                    return Ok(true);
                }
                if !fr {
                    return Ok(false);
                }
                let Value::Set(rs) = r else { return Ok(false) };
                rs.size(self.ctx)? == 1
            }
            SetKind::RcdSet | SetKind::Product => {
                let mut all = true;
                for (_, c) in &comps {
                    let (f, e) = finite_empty(c)?;
                    if f {
                        if e {
                            return Ok(true);
                        }
                    } else {
                        all = false;
                    }
                }
                all
            }
            SetKind::Deferred => {
                return Err(EvalError::unsupported(
                    "IsFiniteSet of a lazily evaluated set",
                ));
            }
        })
    }
}

/// `String.substring` on UTF-16 code units, as Java counts them.
fn java_substring(text: &str, begin: usize, end: Option<usize>) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let end = end.unwrap_or(units.len()).min(units.len());
    let begin = begin.min(end);
    String::from_utf16_lossy(&units[begin..end])
}

/// The class and name of an evaluating override's Java method.
fn evaluating_method(op: StdOp) -> Option<&'static str> {
    Some(match op {
        StdOp::TlcTLCGet => "tlc2.module.TLCGetSet.TLCGetEval",
        StdOp::TlcTLCEval => "tlc2.module.TLCEval.tlcEval",
        StdOp::SequencesExtReplaceFirstSubSeq => "tlc2.overrides.SequencesExt.replaceFirstSubSeq",
        StdOp::SequencesExtReplaceAllSubSeqs => "tlc2.overrides.SequencesExt.replaceAllSubSeq",
        _ => return None,
    })
}

pub(crate) fn refuse(op: StdOp) -> EvalError {
    let why = match op.support() {
        StdSupport::Core => "not implemented in dive's evaluator yet",
        other => other.describe(),
    };
    EvalError::unsupported(format!("{} ({why})", op.qualified_name()))
}

/// `Method.toString()` of the Java method TLC installs, for
/// `TLC_MODULE_VALUE_JAVA_METHOD_OVERRIDE` messages.
#[allow(clippy::too_many_lines, reason = "a table")]
pub(crate) fn java_signature(op: StdOp) -> Option<&'static str> {
    Some(match op {
        StdOp::NaturalsPlus => {
            "public static tlc2.value.impl.IntValue tlc2.module.Naturals.Plus(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::NaturalsMinus => {
            "public static tlc2.value.impl.IntValue tlc2.module.Naturals.Minus(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::NaturalsTimes => {
            "public static tlc2.value.impl.IntValue tlc2.module.Naturals.Times(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::NaturalsDotDot => {
            "public static tlc2.value.impl.IntervalValue tlc2.module.Naturals.DotDot(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::NaturalsDiv => {
            "public static tlc2.value.impl.IntValue tlc2.module.Naturals.Divide(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::NaturalsMod => {
            "public static tlc2.value.impl.IntValue tlc2.module.Naturals.Mod(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::NaturalsExpt => {
            "public static tlc2.value.impl.IntValue tlc2.module.Naturals.Expt(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::IntegersPlus => {
            "public static tlc2.value.impl.IntValue tlc2.module.Integers.Plus(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::IntegersMinus => {
            "public static tlc2.value.impl.IntValue tlc2.module.Integers.Minus(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::IntegersTimes => {
            "public static tlc2.value.impl.IntValue tlc2.module.Integers.Times(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::IntegersDotDot => {
            "public static tlc2.value.impl.IntervalValue tlc2.module.Integers.DotDot(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::IntegersNeg => {
            "public static tlc2.value.impl.IntValue tlc2.module.Integers.Neg(tlc2.value.impl.IntValue)"
        }
        StdOp::IntegersDiv => {
            "public static tlc2.value.impl.IntValue tlc2.module.Integers.Divide(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::IntegersMod => {
            "public static tlc2.value.impl.IntValue tlc2.module.Integers.Mod(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::IntegersExpt => {
            "public static tlc2.value.impl.IntValue tlc2.module.Integers.Expt(tlc2.value.impl.IntValue,tlc2.value.impl.IntValue)"
        }
        StdOp::NaturalsLt => {
            "public static tlc2.value.IBoolValue tlc2.module.Naturals.LT(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::NaturalsLeq => {
            "public static tlc2.value.IBoolValue tlc2.module.Naturals.LE(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::NaturalsGt => {
            "public static tlc2.value.IBoolValue tlc2.module.Naturals.GT(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::NaturalsGeq => {
            "public static tlc2.value.IBoolValue tlc2.module.Naturals.GEQ(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::IntegersLt => {
            "public static tlc2.value.IBoolValue tlc2.module.Integers.LT(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::IntegersLeq => {
            "public static tlc2.value.IBoolValue tlc2.module.Integers.LE(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::IntegersGt => {
            "public static tlc2.value.impl.BoolValue tlc2.module.Integers.GT(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::IntegersGeq => {
            "public static tlc2.value.IBoolValue tlc2.module.Integers.GEQ(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::FiniteSetsIsFiniteSet => {
            "public static tlc2.value.IBoolValue tlc2.module.FiniteSets.IsFiniteSet(tlc2.value.impl.Value)"
        }
        StdOp::FiniteSetsCardinality => {
            "public static tlc2.value.impl.IntValue tlc2.module.FiniteSets.Cardinality(tlc2.value.impl.Value)"
        }
        StdOp::SequencesSeq => {
            "public static tlc2.value.impl.Value tlc2.module.Sequences.Seq(tlc2.value.impl.Value)"
        }
        StdOp::SequencesLen => {
            "public static tlc2.value.impl.IntValue tlc2.module.Sequences.Len(tlc2.value.impl.Value)"
        }
        StdOp::SequencesHead => {
            "public static tlc2.value.impl.Value tlc2.module.Sequences.Head(tlc2.value.impl.Value)"
        }
        StdOp::SequencesTail => {
            "public static tlc2.value.impl.Value tlc2.module.Sequences.Tail(tlc2.value.impl.Value)"
        }
        StdOp::SequencesCons => {
            "public static tlc2.value.impl.Value tlc2.module.Sequences.Cons(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::SequencesAppend => {
            "public static tlc2.value.impl.Value tlc2.module.Sequences.Append(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::SequencesConcat => {
            "public static tlc2.value.impl.Value tlc2.module.Sequences.Concat(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::SequencesSubSeq => {
            "public static tlc2.value.impl.Value tlc2.module.Sequences.SubSeq(tlc2.value.impl.Value,tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::SequencesSelectSeq => {
            "public static tlc2.value.impl.Value tlc2.module.Sequences.SelectSeq(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::TlcPrint => {
            "public static tlc2.value.impl.Value tlc2.module.TLC.Print(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::TlcPrintT => {
            "public static tlc2.value.impl.Value tlc2.module.TLC.PrintT(tlc2.value.impl.Value)"
        }
        StdOp::TlcToString => {
            "public static tlc2.value.impl.Value tlc2.module.TLC.ToString(tlc2.value.impl.Value)"
        }
        StdOp::TlcAssert => {
            "public static tlc2.value.impl.Value tlc2.module.TLC.Assert(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::TlcMakeFcn => {
            "public static tlc2.value.impl.Value tlc2.module.TLC.MakeFcn(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::TlcCombineFcn => {
            "public static tlc2.value.impl.Value tlc2.module.TLC.CombineFcn(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::TlcSortSeq => {
            "public static tlc2.value.impl.Value tlc2.module.TLC.SortSeq(tlc2.value.impl.Value,tlc2.value.impl.Value)"
        }
        StdOp::TlcPermutations => {
            "public static tlc2.value.impl.Value tlc2.module.TLC.Permutations(tlc2.value.impl.Value)"
        }
        _ => return crate::community::java_signature(op),
    })
}
