//! TLC agreement for `RandomElement` and the `Randomization` operators.
//!
//! The specs in `tests/fixtures/random/specs/` were run on the pinned TLC with the
//! harness (`dev capture --seed S`, one directory per seed under
//! `tests/fixtures/random/tlc/`). Each test replays the spec by hand, with no
//! evaluator: the same operator calls in TLC's evaluation order, on the streams
//! [`Streams`] hands out, and with TLC's state fingerprints from `dive-value`.
//! Every value TLC printed must be reproduced exactly, as printed text.

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "test code; short names mirror the TLA+ specs"
)]

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

use dive_stdlib::random::{
    RandomError, RandomMode, RandomSource, Stream, Streams, random_element, random_set_of_subsets,
    random_subset,
};
use dive_value::fp64::{Fingerprinter, TlcFingerprint};
use dive_value::print::print_value;
use dive_value::{Ctx, Int, Interner, Value};

const ORACLE: &str = "tlc-2026.10.06.014338";
const SEEDS: [i64; 6] = [0, 7, -3, 1_234_567_890_123, i64::MAX, i64::MIN];

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/random")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn stdout(dir: &str, name: &str) -> String {
    read(&fixtures().join(format!("tlc/{dir}/{ORACLE}/{name}.stdout")))
}

/// The values TLC printed with `Print`/`PrintT` after start-up: the text outside
/// `-tool` message blocks, one entry per value (continuation lines of a
/// pretty-printed value are indented).
fn printed(out: &str) -> Vec<String> {
    let mut depth = 0usize;
    let mut started = false;
    let mut values: Vec<String> = Vec::new();
    for line in out.lines() {
        if line.starts_with("@!@!@STARTMSG") {
            depth += 1;
            if line.starts_with("@!@!@STARTMSG 2185:") {
                started = true;
            }
            continue;
        }
        if line.starts_with("@!@!@ENDMSG") {
            depth = depth.saturating_sub(1);
            continue;
        }
        if depth > 0 || !started {
            continue;
        }
        if line.starts_with(' ') {
            let last = values.last_mut().expect("continuation after a value");
            last.push('\n');
            last.push_str(line);
        } else {
            values.push(line.to_owned());
        }
    }
    values
}

/// The body of the innermost `-tool` message with code `code`.
fn message(out: &str, code: u32) -> Option<String> {
    let start = format!("@!@!@STARTMSG {code}:");
    let end = format!("@!@!@ENDMSG {code} @!@!@");
    let from = out.find(&start)?;
    let body = &out[from..];
    let body = &body[body.find('\n')? + 1..];
    Some(body[..body.find(&end)?].trim_end().to_owned())
}

/// Builds TLA+ values and the printed text of tuples, under `--compat=tlc`.
struct Tla {
    ctx: Ctx,
    names: Interner,
}

impl Tla {
    /// Interns `idents` first, in the order TLC's lexer meets them in the spec.
    fn new(idents: &[&str]) -> Self {
        let names = Interner::new();
        for i in idents {
            names.intern(i);
        }
        Self {
            ctx: Ctx::tlc(),
            names,
        }
    }
    fn int(i: i64) -> Value {
        Value::from(i)
    }
    fn str(&self, s: &str) -> Value {
        Value::string(&self.names, s)
    }
    fn range(&self, lo: i64, hi: i64) -> Value {
        Value::interval(Int::from(lo), Int::from(hi), self.ctx)
    }
    fn set(&self, elems: Vec<Value>) -> Value {
        Value::set_raw(elems, self.ctx)
    }
    fn fcns(&self, d: Value, r: Value) -> Value {
        Value::fcn_set(d, r, self.ctx)
    }
    fn rcds(&self, fields: &[(&str, Value)]) -> Value {
        Value::record_set(
            fields
                .iter()
                .map(|(n, v)| (self.names.intern(n), v.clone()))
                .collect(),
            self.ctx,
        )
        .unwrap()
    }
    fn product(&self, factors: Vec<Value>) -> Value {
        Value::product(factors, self.ctx)
    }
    fn print(&self, v: &Value) -> String {
        print_value(v, self.ctx).unwrap()
    }
    /// `PrintT(<<tag, values...>>)`.
    fn line(&self, tag: &str, values: Vec<Value>) -> String {
        let mut t = vec![self.str(tag)];
        t.extend(values);
        self.print(&Value::tuple(t))
    }
    fn re(&self, s: &Value, src: &mut dyn RandomSource) -> Value {
        random_element(s, self.ctx, src).unwrap()
    }
    fn rs(&self, k: i64, s: &Value, src: &mut dyn RandomSource) -> Value {
        random_subset(&Self::int(k), s, self.ctx, src).unwrap()
    }
    fn rsos(&self, k: i64, n: i64, s: &Value, src: &mut dyn RandomSource) -> Value {
        random_set_of_subsets(&Self::int(k), &Self::int(n), s, self.ctx, src).unwrap()
    }
    /// The elements of a set in TLC's normalized order.
    fn elems(&self, s: &Value) -> Vec<Value> {
        s.as_set().unwrap().elements(self.ctx).unwrap().to_vec()
    }
}

/// TLC's fingerprint of a state (`-fp 0`); the specs have one variable, or
/// variables listed in TLC's order.
fn state_fp(vars: &[Value]) -> u64 {
    TlcFingerprint::new(0)
        .unwrap()
        .fingerprint_state(vars, Ctx::tlc())
        .unwrap()
}

fn assert_lines(spec: &str, seed: i64, want: &[String]) {
    let got = printed(&stdout(&format!("seed_{seed}"), spec));
    assert_eq!(
        got.len(),
        want.len(),
        "{spec} seed {seed}: TLC printed {} values, dive {}\nTLC:\n{}\ndive:\n{}",
        got.len(),
        want.len(),
        got.join("\n"),
        want.join("\n")
    );
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert_eq!(g, w, "{spec} seed {seed}: value {i} differs");
    }
}

// ---------------------------------------------------------------------------------
// RndStartup: constant definitions, ASSUMEs and Init share one stream.
// ---------------------------------------------------------------------------------

fn replay_startup(seed: i64) -> Vec<String> {
    let t = Tla::new(&[
        "RndStartup",
        "Integers",
        "TLC",
        "Randomization",
        "C1",
        "C2",
        "zq",
        "aq",
    ]);
    let mut st = Streams::new(seed, RandomMode::Tlc).startup();
    let s = &mut st;
    let r = |a, b| t.range(a, b);
    let i = Tla::int;
    let mut out = Vec::new();
    // Constant definitions, evaluated at start-up in definition order.
    let c1 = t.re(&r(1, 1_000_000), s);
    let c2 = t.rs(3, &r(1, 100), s);
    out.push(t.line("c", vec![c1, c2]));
    let v = vec![t.re(&r(1, 10), s), t.re(&r(-5, 5), s), t.re(&r(7, 7), s)];
    out.push(t.line("interval", v));
    let digits = t.set([3, 1, 4, 1, 5, 9, 2, 6].map(i).to_vec());
    let sets = t.set(vec![
        t.set(vec![i(1), i(2)]),
        t.set(vec![i(3)]),
        t.set(vec![]),
    ]);
    let tuples = t.set(vec![
        Value::tuple(vec![i(1), i(2)]),
        Value::tuple(vec![i(2), i(1)]),
        Value::tuple(vec![i(1)]),
    ]);
    let v = vec![t.re(&digits, s), t.re(&sets, s), t.re(&tuples, s)];
    out.push(t.line("enum", v));
    let v = vec![t.re(&Value::subset(r(1, 4), t.ctx), s)];
    out.push(t.line("subsetof", v));
    let v = vec![
        t.re(&t.fcns(r(1, 3), r(1, 5)), s),
        t.re(
            &t.fcns(
                t.set(vec![i(10), i(20)]),
                t.set(vec![Value::TRUE, Value::FALSE]),
            ),
            s,
        ),
        t.re(&t.fcns(t.set(vec![]), r(1, 3)), s),
    ];
    out.push(t.line("fcns", v));
    let v = vec![t.re(
        &t.rcds(&[("zq", r(1, 9)), ("aq", t.set(vec![i(7), i(8)]))]),
        s,
    )];
    out.push(t.line("rcds", v));
    let v = vec![t.re(&t.product(vec![r(1, 3), r(4, 6), t.set(vec![i(0)])]), s)];
    out.push(t.line("tuples", v));
    let v = vec![
        t.rs(3, &r(1, 10), s),
        t.rs(10, &r(1, 10), s),
        t.rs(12, &r(1, 10), s),
        t.rs(0, &r(1, 10), s),
        t.rs(2, &r(1, 0), s),
    ];
    out.push(t.line("rs-interval", v));
    let v = vec![
        t.rs(2, &t.set(vec![i(10), i(20), i(30), i(40)]), s),
        t.rs(2, &Value::subset(r(1, 3), t.ctx), s),
    ];
    out.push(t.line("rs-enum", v));
    let v = vec![
        t.rs(3, &t.fcns(r(1, 3), r(1, 4)), s),
        t.rs(2, &t.rcds(&[("zq", r(1, 3)), ("aq", r(1, 2))]), s),
        t.rs(4, &t.product(vec![r(1, 2), r(1, 3)]), s),
        t.rs(2, &t.fcns(r(1, 2), t.set(vec![])), s),
    ];
    out.push(t.line("rs-product", v));
    let v = vec![
        t.rs(3, &t.fcns(r(1, 3), r(1, 2000)), s),
        t.rs(2, &t.product(vec![r(1, 100_000), r(1, 100_000)]), s),
    ];
    out.push(t.line("rs-big", v));
    let v = vec![
        t.rsos(4, 2, &r(1, 5), s),
        t.rsos(3, 0, &r(1, 3), s),
        t.rsos(3, 3, &r(1, 3), s),
        t.rsos(8, 1, &r(1, 3), s),
    ];
    out.push(t.line("rsos", v));
    let v = vec![t.re(&r(1, 1_000_000), s), t.rs(3, &r(1, 1_000_000), s)];
    out.push(t.line("large", v));
    // Init: the set literal left to right, then one initial state per element in
    // normalized order, each drawing once more.
    let xs = t.set(vec![
        t.re(&r(1, 1000), s),
        t.re(&r(1, 1000), s),
        t.re(&r(1, 1000), s),
    ]);
    for x in t.elems(&xs) {
        let v = vec![x, t.re(&r(1, 1_000_000), s)];
        out.push(t.line("init", v));
    }
    out
}

#[test]
fn startup_stream_matches_tlc() {
    for seed in SEEDS {
        assert_lines("RndStartup", seed, &replay_startup(seed));
    }
}

#[test]
fn probe_p9_seed_7() {
    // docs/internal/survey/competitive.md, P9: `RandomElement(1..1000000)` in Init, -seed 7.
    let t = Tla::new(&[]);
    let mut s = Streams::new(7, RandomMode::Tlc).startup();
    assert_eq!(t.print(&t.re(&t.range(1, 1_000_000), &mut s)), "730700");
}

// ---------------------------------------------------------------------------------
// RndNext: one stream per expanded state, fingerprint XOR seed.
// ---------------------------------------------------------------------------------

fn replay_next(seed: i64) -> Vec<String> {
    let t = Tla::new(&["RndNext", "Naturals", "TLC", "Randomization", "x"]);
    let streams = Streams::new(seed, RandomMode::Tlc);
    let r = |a, b| t.range(a, b);
    let i = Tla::int;
    let mut out = Vec::new();
    let mut seen = HashSet::from([0i64]);
    let mut queue = VecDeque::from([0i64]);
    while let Some(x) = queue.pop_front() {
        let mut st: Stream = streams.state(state_fp(&[i(x)]));
        let s = &mut st;
        let mut succ = Vec::new();
        if x < 3 {
            let v = vec![
                i(x),
                t.re(&r(1, 1_000_000), s),
                t.rs(2, &r(1, 10), s),
                t.re(&t.fcns(r(1, 2), r(1, 3)), s),
            ];
            out.push(t.line("a", v));
            succ.push(x + 1);
            let v = vec![
                i(x),
                t.rsos(2, 1, &r(1, 3), s),
                t.re(&t.set(vec![i(10), i(20), i(30)]), s),
            ];
            out.push(t.line("b", v));
            succ.push(x + 2);
        } else {
            let v = vec![i(x), t.re(&r(1, 1_000_000), s)];
            out.push(t.line("c", v));
            succ.push(x);
        }
        for y in succ {
            if seen.insert(y) {
                queue.push_back(y);
            }
        }
    }
    out
}

#[test]
fn state_streams_match_tlc() {
    for seed in SEEDS {
        assert_lines("RndNext", seed, &replay_next(seed));
    }
}

// ---------------------------------------------------------------------------------
// RndBfs: a random state graph, explored breadth first.
// ---------------------------------------------------------------------------------

fn replay_bfs(seed: i64) -> (Vec<String>, usize) {
    let t = Tla::new(&["RndBfs", "Naturals", "TLC", "x"]);
    let streams = Streams::new(seed, RandomMode::Tlc);
    let i = Tla::int;
    let mut start = streams.startup();
    let inits = t.set(vec![
        t.re(&t.range(0, 60), &mut start),
        t.re(&t.range(0, 60), &mut start),
    ]);
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut queue = VecDeque::new();
    for x in t.elems(&inits) {
        let x = x.as_int().and_then(|v| v.to_i64()).unwrap();
        if seen.insert(x) {
            queue.push_back(x);
        }
    }
    while let Some(x) = queue.pop_front() {
        let mut s = streams.state(state_fp(&[i(x)]));
        for k in 1..=2 {
            // TLC re-evaluates the LET-bound `d` in each conjunct of the action, so
            // the `d` printed and the `d` in `x'` are two draws (`RndLetRedraw`).
            let printed = t.re(&t.range(1, 9), &mut s);
            out.push(t.line("e", vec![i(x), i(k), printed]));
            let d = t.re(&t.range(1, 9), &mut s);
            let dv = d.as_int().and_then(|v| v.to_i64()).unwrap();
            let y = (x * 3 + dv * k) % 61;
            if seen.insert(y) {
                queue.push_back(y);
            }
        }
    }
    (out, seen.len())
}

#[test]
fn breadth_first_random_graph_matches_tlc() {
    for seed in SEEDS {
        let (lines, distinct) = replay_bfs(seed);
        assert_lines("RndBfs", seed, &lines);
        let out = stdout(&format!("seed_{seed}"), "RndBfs");
        assert!(
            out.contains(&format!(" {distinct} distinct states found")),
            "RndBfs seed {seed}: {distinct} distinct states"
        );
    }
}

// ---------------------------------------------------------------------------------
// RndTraceAssume: TLC loses the counterexample when an ASSUME drew first.
// ---------------------------------------------------------------------------------

#[test]
fn tlc_rebuilds_the_initial_state_with_a_reset_stream() {
    // TLC's start-up stream draws r1 (ASSUME), then r2 and r3 (the Init set). To print
    // the trace it re-evaluates Init with a reset stream, which yields r1 and r2.
    // The violation is reached first from the smaller initial x; when that is r3
    // (and r3 is not r1), TLC reports the violation, exits 12, and prints no trace.
    let t = Tla::new(&[]);
    let mut lost = 0;
    for seed in SEEDS {
        let mut s = Streams::new(seed, RandomMode::Tlc).startup();
        let mut draw = || {
            let v = t.re(&t.range(1, 1_000_000), &mut s);
            v.as_int().and_then(|v| v.to_i64()).unwrap()
        };
        let (r1, r2, r3) = (draw(), draw(), draw());
        let first = r2.min(r3);
        let rebuilt = first == r1 || first == r2;
        let out = stdout(&format!("seed_{seed}"), "RndTraceAssume");
        assert_eq!(
            read(&fixtures().join(format!("tlc/seed_{seed}/{ORACLE}/RndTraceAssume.exit"))).trim(),
            "12"
        );
        assert!(out.contains(&format!("<<\"assume\", {r1}>>")));
        let has_trace = out.contains("@!@!@STARTMSG 2121:");
        assert_eq!(has_trace, rebuilt, "seed {seed}: r = {r1}, {r2}, {r3}");
        if has_trace {
            assert!(out.contains(&format!("/\\ x = {first}\n")), "seed {seed}");
        } else {
            lost += 1;
        }
    }
    assert!(
        lost > 0,
        "the fixtures include a seed where TLC loses the trace"
    );
}

// ---------------------------------------------------------------------------------
// Errors: TLC's error classes and messages.
// ---------------------------------------------------------------------------------

#[test]
fn errors_match_tlc() {
    let t = Tla::new(&["a", "b"]);
    let ctx = t.ctx;
    let r = |a, b| t.range(a, b);
    let i = Tla::int;
    let nat = Value::nat(ctx);
    let mut s = Streams::new(0, RandomMode::Tlc).startup();
    let s = &mut s;
    let cases: Vec<(&str, Result<Value, RandomError>)> = vec![
        ("RndErrEmptySet", random_element(&t.set(vec![]), ctx, s)),
        ("RndErrEmptyInterval", random_element(&r(1, 0), ctx, s)),
        (
            "RndErrInfinite",
            random_element(
                &t.rcds(&[("a", t.set(vec![i(1)])), ("b", nat.clone())]),
                ctx,
                s,
            ),
        ),
        ("RndErrNotSet", random_element(&i(3), ctx, s)),
        ("RndErrNegative", random_subset(&i(-1), &r(1, 5), ctx, s)),
        ("RndErrNotFinite", random_subset(&i(2), &nat, ctx, s)),
        ("RndErrNotInt", random_subset(&t.str("a"), &r(1, 5), ctx, s)),
        (
            "RndErrPicks",
            random_set_of_subsets(&i(9), &i(1), &r(1, 3), ctx, s),
        ),
        (
            "RndErrMean",
            random_set_of_subsets(&i(2), &i(4), &r(1, 3), ctx, s),
        ),
        (
            "RndErrThird",
            random_set_of_subsets(&i(2), &i(1), &nat, ctx, s),
        ),
    ];
    for (spec, got) in cases {
        let err = got.expect_err(spec);
        let out = stdout("errors", spec);
        let want = message(&out, err.tlc_code())
            .unwrap_or_else(|| panic!("{spec}: no TLC message {}", err.tlc_code()));
        assert_eq!(err.message(), want, "{spec}");
    }
}

// ---------------------------------------------------------------------------------
// RndLetRedraw: when TLC re-evaluates a LET-bound draw.
// ---------------------------------------------------------------------------------

/// TLC caches a LET-bound value within one expression only partly, and not at all
/// across the conjuncts of `Init` or an action: each such use draws again. The
/// evaluator must reproduce exactly these draw counts under `--compat=tlc`.
fn replay_let_redraw(seed: i64) -> Vec<String> {
    let t = Tla::new(&["RndLetRedraw", "Naturals", "TLC", "Twice", "y", "x", "d"]);
    let streams = Streams::new(seed, RandomMode::Tlc);
    let i = Tla::int;
    let big = t.range(1, 1_000_000);
    let mut s = streams.startup();
    let mut out = Vec::new();
    // ASSUME: `d, d` in one constant-level expression is one draw.
    let d = t.re(&big, &mut s);
    out.push(t.line("assume", vec![d.clone(), d]));
    // Init, conjunct 1: `d` and `Twice(d)` are two draws; `y, y` inside `Twice` one.
    let d = t.re(&big, &mut s);
    let y = t.re(&big, &mut s);
    out.push(t.line("init", vec![d, Value::tuple(vec![y.clone(), y])]));
    // Conjunct 2 (`x = d`) and conjunct 3 each draw again.
    let x0 = t.re(&big, &mut s);
    let d = t.re(&big, &mut s);
    out.push(t.line("init-x", vec![x0.clone(), d]));
    out.push(t.line("state", vec![x0.clone()]));
    let mut x = x0.as_int().and_then(|v| v.to_i64()).unwrap();
    let mut seen = HashSet::from([x]);
    loop {
        let mut st = streams.state(state_fp(&[i(x)]));
        let d = t.re(&big, &mut st);
        out.push(t.line("next", vec![i(x), d]));
        let next = if x < 1_000_001 {
            let d = t.re(&big, &mut st);
            1_000_001 + d.as_int().and_then(|v| v.to_i64()).unwrap()
        } else {
            x
        };
        if !seen.insert(next) {
            break;
        }
        out.push(t.line("state", vec![i(next)]));
        x = next;
    }
    out
}

#[test]
fn let_bound_draws_follow_tlc() {
    for seed in [0, 7] {
        assert_lines("RndLetRedraw", seed, &replay_let_redraw(seed));
    }
}

// ---------------------------------------------------------------------------------
// The fingerprint polynomial: `-seed` alone does not replay state-level draws.
// ---------------------------------------------------------------------------------

/// TLC picks a random fingerprint polynomial unless `-fp` is given, and state-level
/// draws are seeded from the fingerprint. Three runs of `RndNext` with `-seed 7` and
/// no `-fp` on the pinned TLC (outside the harness, which always passes `-fp 0`)
/// printed `<<"a", 0, r, ...>>` with these `r` and polynomials.
#[test]
fn state_draws_depend_on_the_fingerprint_polynomial() {
    let t = Tla::new(&[]);
    let streams = Streams::new(7, RandomMode::Tlc);
    for (fp_index, want) in [(71, 560_481), (55, 618_858), (1, 37_068), (0, 772_259)] {
        let fp = TlcFingerprint::new(fp_index)
            .unwrap()
            .fingerprint_state(&[Tla::int(0)], Ctx::tlc())
            .unwrap();
        let mut s = streams.state(fp);
        assert_eq!(
            t.print(&t.re(&t.range(1, 1_000_000), &mut s)),
            want.to_string(),
            "-fp {fp_index}"
        );
    }
}
