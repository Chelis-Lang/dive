//! A tiny evaluator for the constant TLA+ expressions used in the fixture specs.
//!
//! It covers only what the fixtures use: literals, set/tuple/record/function
//! constructors, `..`, `\X`, `SUBSET`, `[S -> T]`, `[a : S]`, `Nat`, `Int`, `STRING`,
//! `BOOLEAN`, `Seq`, arithmetic, `=`, `\in`, `:>`, `@@`, `CHOOSE x \in S : TRUE`,
//! `Cardinality` and `TLCFP`. Evaluation follows TLC where it matters to the value
//! layer: set enumerations stay unnormalized (`Value::set_raw`), and `=`/`\in` use
//! TLC's checked operations.

#![allow(dead_code, reason = "shared by several test binaries")]
#![allow(
    clippy::panic,
    clippy::expect_used,
    reason = "test helper: a malformed fixture is a test failure"
)]

use dive_value::fp64::{Fingerprinter, TlcFingerprint};
use dive_value::{Ctx, Int, Interner, Value, ValueError, ops, tlc};

/// Interns the names of a module in SANY/TLC's order: identifiers as the lexer meets
/// them, then the contents of string literals in order of appearance.
pub fn seed_interner(module: &str) -> Interner {
    let names = Interner::new();
    let mut strings = Vec::new();
    let b = module.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'"' {
            let (s, end) = read_string(module, i);
            strings.push(s);
            i = end;
        } else if c == b'\\' {
            // `\in`, `\X`, `\div`: operator tokens, not identifiers.
            i += 1;
            while i < b.len() && b[i].is_ascii_alphanumeric() {
                i += 1;
            }
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            names.intern(&module[start..i]);
        } else if c.is_ascii_digit() {
            while i < b.len() && b[i].is_ascii_alphanumeric() {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    for s in strings {
        names.intern(&s);
    }
    names
}

/// Reads a string literal starting at the quote at `start`; returns its contents and
/// the index after the closing quote.
fn read_string(src: &str, start: usize) -> (String, usize) {
    let mut out = String::new();
    let mut chars = src[start + 1..].char_indices();
    while let Some((k, c)) = chars.next() {
        match c {
            '"' => return (out, start + 1 + k + 1),
            '\\' => {
                if let Some((_, e)) = chars.next() {
                    out.push(match e {
                        't' => '\t',
                        'n' => '\n',
                        'r' => '\r',
                        'f' => '\u{000C}',
                        other => other,
                    });
                }
            }
            c => out.push(c),
        }
    }
    (out, src.len())
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(String),
    Str(String),
    Id(String),
    Op(String),
}

fn lex(src: &str) -> Vec<Tok> {
    const OPS: [&str; 22] = [
        "|->", "<<", ">>", "..", ":>", "@@", "->", "\\in", "\\X", "\\div", "{", "}", "(", ")", "[",
        "]", ",", ":", "=", "+", "-", "*",
    ];
    let mut out = Vec::new();
    let b = src.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'"' {
            let (s, end) = read_string(src, i);
            out.push(Tok::Str(s));
            i = end;
        } else if c.is_ascii_digit() {
            let s = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            out.push(Tok::Num(src[s..i].to_owned()));
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let s = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(Tok::Id(src[s..i].to_owned()));
        } else if c == b'%' || c == b'^' {
            out.push(Tok::Op((c as char).to_string()));
            i += 1;
        } else {
            let op = OPS
                .iter()
                .find(|op| src[i..].starts_with(**op))
                .unwrap_or_else(|| panic!("cannot lex {:?}", &src[i..]));
            out.push(Tok::Op((*op).to_owned()));
            i += op.len();
        }
    }
    out
}

/// The result of evaluating a fixture expression.
pub type Eval = Result<Value, ValueError>;

pub struct Evaluator<'a> {
    pub names: &'a Interner,
    pub ctx: Ctx,
    pub constants: Vec<String>,
}

struct P<'a, 'b> {
    ev: &'b Evaluator<'a>,
    toks: Vec<Tok>,
    pos: usize,
}

impl Evaluator<'_> {
    /// Evaluates a constant expression.
    pub fn eval(&self, src: &str) -> Eval {
        let mut p = P {
            ev: self,
            toks: lex(src),
            pos: 0,
        };
        let v = p.expr()?;
        assert_eq!(p.pos, p.toks.len(), "trailing tokens in {src:?}");
        Ok(v)
    }
}

impl P<'_, '_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn is_op(&self, op: &str) -> bool {
        matches!(self.peek(), Some(Tok::Op(o)) if o == op)
    }
    fn is_id(&self, id: &str) -> bool {
        matches!(self.peek(), Some(Tok::Id(o)) if o == id)
    }
    fn eat(&mut self, op: &str) {
        assert!(
            self.is_op(op) || self.is_id(op),
            "expected {op} at {:?}",
            self.peek()
        );
        self.pos += 1;
    }
    fn ctx(&self) -> Ctx {
        self.ev.ctx
    }

    fn expr(&mut self) -> Eval {
        if self.is_id("CHOOSE") {
            self.pos += 1;
            self.pos += 1; // bound variable
            self.eat("\\in");
            let s = self.expr_no_choose()?;
            self.eat(":");
            self.eat("TRUE");
            let r = ops::choose(&s, self.ctx(), |_| Ok(true))?;
            return Ok(r.expect("CHOOSE over an empty set"));
        }
        self.expr_no_choose()
    }

    fn expr_no_choose(&mut self) -> Eval {
        let a = self.atfcn()?;
        if self.is_op("=") {
            self.pos += 1;
            let b = self.atfcn()?;
            return Ok(Value::Bool(tlc::equals(&a, &b, self.ctx())?));
        }
        if self.is_op("\\in") {
            self.pos += 1;
            let b = self.atfcn()?;
            return Ok(Value::Bool(tlc::member(&a, &b, self.ctx())?));
        }
        Ok(a)
    }

    fn atfcn(&mut self) -> Eval {
        let mut f = self.colon()?;
        while self.is_op("@@") {
            self.pos += 1;
            let g = self.colon()?;
            f = combine(&f, &g, self.ctx())?;
        }
        Ok(f)
    }

    fn colon(&mut self) -> Eval {
        let d = self.range()?;
        if self.is_op(":>") {
            self.pos += 1;
            let v = self.range()?;
            return Value::fcn(vec![(d, v)], self.ctx());
        }
        Ok(d)
    }

    fn range(&mut self) -> Eval {
        let lo = self.prod()?;
        if self.is_op("..") {
            self.pos += 1;
            let hi = self.prod()?;
            let (Some(l), Some(h)) = (lo.as_int(), hi.as_int()) else {
                panic!("non-integer interval bounds");
            };
            return Ok(Value::interval(l, h, self.ctx()));
        }
        Ok(lo)
    }

    fn prod(&mut self) -> Eval {
        let first = self.add()?;
        if !self.is_op("\\X") {
            return Ok(first);
        }
        let mut fs = vec![first];
        while self.is_op("\\X") {
            self.pos += 1;
            fs.push(self.add()?);
        }
        Ok(Value::product(fs, self.ctx()))
    }

    fn add(&mut self) -> Eval {
        let mut a = self.mul()?;
        loop {
            let m = self.ctx().int_mode;
            if self.is_op("+") {
                self.pos += 1;
                let b = self.mul()?;
                a = Value::int(int(&a).add(&int(&b), m)?);
            } else if self.is_op("-") {
                self.pos += 1;
                let b = self.mul()?;
                a = Value::int(int(&a).sub(&int(&b), m)?);
            } else {
                return Ok(a);
            }
        }
    }

    fn mul(&mut self) -> Eval {
        let mut a = self.unary()?;
        loop {
            let m = self.ctx().int_mode;
            let op = match self.peek() {
                Some(Tok::Op(o)) if ["*", "\\div", "%"].contains(&o.as_str()) => o.clone(),
                _ => return Ok(a),
            };
            self.pos += 1;
            let b = self.unary()?;
            a = Value::int(match op.as_str() {
                "*" => int(&a).mul(&int(&b), m)?,
                "\\div" => int(&a).div(&int(&b), m)?,
                _ => int(&a).rem(&int(&b), m)?,
            });
        }
    }

    fn unary(&mut self) -> Eval {
        if self.is_op("-") {
            self.pos += 1;
            let a = self.unary()?;
            return Ok(Value::int(int(&a).neg(self.ctx().int_mode)?));
        }
        if self.is_id("SUBSET") {
            self.pos += 1;
            let a = self.unary()?;
            return Ok(Value::subset(a, self.ctx()));
        }
        let a = self.primary()?;
        if self.is_op("^") {
            self.pos += 1;
            let b = self.primary()?;
            return Ok(Value::int(int(&a).pow(&int(&b), self.ctx().int_mode)?));
        }
        Ok(a)
    }

    fn list(&mut self, close: &str) -> Result<Vec<Value>, ValueError> {
        let mut out = Vec::new();
        while !self.is_op(close) {
            out.push(self.expr()?);
            if self.is_op(",") {
                self.pos += 1;
            }
        }
        self.eat(close);
        Ok(out)
    }

    fn primary(&mut self) -> Eval {
        let ctx = self.ctx();
        let tok = self.peek().cloned().expect("unexpected end of expression");
        self.pos += 1;
        match tok {
            Tok::Num(n) => Ok(Value::int(Int::from_literal(&n, ctx.int_mode)?)),
            Tok::Str(s) => Ok(Value::string(self.ev.names, &s)),
            Tok::Op(o) if o == "(" => {
                let v = self.expr()?;
                self.eat(")");
                Ok(v)
            }
            Tok::Op(o) if o == "{" => Ok(Value::set_raw(self.list("}")?, ctx)),
            Tok::Op(o) if o == "<<" => Ok(Value::tuple(self.list(">>")?)),
            Tok::Op(o) if o == "[" => self.bracket(),
            Tok::Id(id) => match id.as_str() {
                "TRUE" => Ok(Value::TRUE),
                "FALSE" => Ok(Value::FALSE),
                "BOOLEAN" => Ok(Value::boolean_set(ctx)),
                "Nat" => Ok(Value::nat(ctx)),
                "Int" => Ok(Value::int_set(ctx)),
                "STRING" => Ok(Value::string_set(ctx)),
                "Seq" | "Cardinality" | "TLCFP" => {
                    self.eat("(");
                    let a = self.expr()?;
                    self.eat(")");
                    match id.as_str() {
                        "Seq" => Ok(Value::seq_set(a, ctx)),
                        "Cardinality" => {
                            let n = a.as_set().expect("Cardinality of a non-set").size(ctx)?;
                            Ok(Value::Int(i64::try_from(n).unwrap_or(i64::MAX)))
                        }
                        _ => {
                            let fp = TlcFingerprint::new(0).expect("poly 0");
                            let x = fp.fingerprint(&a, ctx)?;
                            #[allow(
                                clippy::cast_possible_truncation,
                                reason = "FP64.Hash is (int)fp"
                            )]
                            Ok(Value::Int(i64::from(x as i32)))
                        }
                    }
                }
                _ if self.ev.constants.contains(&id) => Ok(Value::model(self.ev.names, &id)),
                _ => panic!("unknown identifier {id}"),
            },
            t @ Tok::Op(_) => panic!("unexpected token {t:?}"),
        }
    }

    /// `[f |-> e, ...]`, `[f : S, ...]` or `[S -> T]`.
    fn bracket(&mut self) -> Eval {
        let ctx = self.ctx();
        let is_field = matches!(self.peek(), Some(Tok::Id(_)))
            && matches!(self.toks.get(self.pos + 1), Some(Tok::Op(o)) if o == "|->" || o == ":");
        if is_field {
            let record = matches!(self.toks.get(self.pos + 1), Some(Tok::Op(o)) if o == "|->");
            let mut fields = Vec::new();
            while !self.is_op("]") {
                let Some(Tok::Id(name)) = self.peek().cloned() else {
                    panic!("expected a field name");
                };
                self.pos += 2;
                let v = self.expr()?;
                fields.push((self.ev.names.intern(&name), v));
                if self.is_op(",") {
                    self.pos += 1;
                }
            }
            self.eat("]");
            return if record {
                Value::record(fields, ctx)
            } else {
                Value::record_set(fields, ctx)
            };
        }
        let d = self.expr()?;
        self.eat("->");
        let r = self.expr()?;
        self.eat("]");
        Ok(Value::fcn_set(d, r, ctx))
    }
}

fn int(v: &Value) -> Int {
    v.as_int()
        .unwrap_or_else(|| panic!("not an integer: {v:?}"))
}

/// `f @@ g` (TLC's `CombineFcn`): `f`'s pairs, then `g`'s pairs whose argument is not
/// in `f`'s domain.
fn combine(f: &Value, g: &Value, ctx: Ctx) -> Eval {
    let (Some(f), Some(g)) = (f.as_fcn(), g.as_fcn()) else {
        panic!("@@ of non-functions");
    };
    let mut pairs: Vec<(Value, Value)> = f.iter().map(|(d, v)| (d, v.clone())).collect();
    for (d, v) in g.iter() {
        let mut found = false;
        for (e, _) in f.iter() {
            if tlc::equals(&e, &d, ctx)? {
                found = true;
                break;
            }
        }
        if !found {
            pairs.push((d, v.clone()));
        }
    }
    Value::fcn(pairs, ctx)
}

/// The `CONSTANTS` declared in a module.
pub fn constants(module: &str) -> Vec<String> {
    module
        .lines()
        .filter_map(|l| l.strip_prefix("CONSTANTS "))
        .flat_map(|l| l.split(',').map(|s| s.trim().to_owned()))
        .collect()
}
