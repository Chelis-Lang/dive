//! TLC's state fingerprints, end to end from the specification text.
//!
//! For each specification in `dive-ir`'s `tests/fixtures/var-order/specs/`, TLC's
//! `-fp 0 -dump dot` output (`tests/fixtures/tlc-var-order/<Name>.dot`) gives every
//! reachable state with its full 64-bit `TLCState.fingerPrint()` as the node id.
//! The test analyses and lowers the specification with `dive-sema` and `dive-ir`,
//! takes TLC's variable order from `Ir::tlc_var_order`, rebuilds each state from
//! its printed values in the IR's variable order, and requires
//! `StateFingerprinter::tlc` to reproduce the node id exactly. The specifications
//! hold integers, booleans, strings and tuples, whose fingerprints do not depend
//! on intern tokens.
//!
//! dive has no evaluator yet, so the states come from TLC's dump, not from dive's
//! own exploration: what is verified end to end is the variable order (from the
//! source) and the fingerprint of a given state.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed expectation is a test failure"
)]

use std::path::PathBuf;

use dive_store::state::StateFingerprinter;
use dive_value::{Ctx, Interner, Value};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn specs_dir() -> PathBuf {
    manifest_dir().join("../dive-ir/tests/fixtures/var-order/specs")
}

/// Undoes the dot label's escaping (`\\` and `\"`; `\n` separates lines).
fn unescape(label: &str) -> String {
    let mut out = String::new();
    let mut chars = label.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The states of a dot dump: node id and the label's `name = value` lines.
fn states(dot: &str) -> Vec<(i64, Vec<(String, String)>)> {
    let mut out = Vec::new();
    for line in dot.lines() {
        let Some((id, rest)) = line.split_once(" [label=\"") else {
            continue;
        };
        let Ok(id) = id.parse::<i64>() else {
            continue;
        };
        // The label ends at the first unescaped quote.
        let mut end = None;
        let mut escaped = false;
        for (i, c) in rest.char_indices() {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => {
                    end = Some(i);
                    break;
                }
                _ => {}
            }
        }
        let label = unescape(&rest[..end.expect("a closed label")]);
        let vars = label
            .lines()
            .map(|l| {
                let l = l.strip_prefix("/\\ ").unwrap_or(l);
                let (n, v) = l.split_once(" = ").expect("name = value");
                (n.to_owned(), v.to_owned())
            })
            .collect();
        out.push((id, vars));
    }
    out
}

/// Parses TLC's printing of an integer, boolean, string or tuple of these.
fn value(names: &Interner, src: &str) -> Value {
    fn parse(names: &Interner, s: &mut &str) -> Value {
        *s = s.trim_start();
        if let Some(rest) = s.strip_prefix("<<") {
            *s = rest;
            let mut elems = Vec::new();
            loop {
                *s = s.trim_start();
                if let Some(rest) = s.strip_prefix(">>") {
                    *s = rest;
                    return Value::tuple(elems);
                }
                elems.push(parse(names, s));
                *s = s.trim_start();
                if let Some(rest) = s.strip_prefix(',') {
                    *s = rest;
                }
            }
        }
        if let Some(rest) = s.strip_prefix('"') {
            let end = rest.find('"').expect("a closed string");
            let v = Value::string(names, &rest[..end]);
            *s = &rest[end + 1..];
            return v;
        }
        for (word, v) in [("TRUE", Value::TRUE), ("FALSE", Value::FALSE)] {
            if let Some(rest) = s.strip_prefix(word) {
                *s = rest;
                return v;
            }
        }
        let end = s
            .char_indices()
            .find(|&(i, c)| !(c.is_ascii_digit() || (i == 0 && c == '-')))
            .map_or(s.len(), |(i, _)| i);
        let n: i64 = s[..end].parse().expect("an integer");
        *s = &s[end..];
        Value::from(n)
    }
    let mut s = src;
    let v = parse(names, &mut s);
    assert!(s.trim().is_empty(), "trailing input in {src:?}");
    v
}

struct Spec {
    /// The IR's variable names, by `VarId`.
    vars: Vec<String>,
    /// TLC's order as IR indices.
    tlc_order: Vec<usize>,
}

fn lower(name: &str) -> Spec {
    let path = specs_dir().join(name).join(format!("{name}.tla"));
    let a = dive_sema::analyze_file(&path, &dive_sema::Options::default()).expect("readable");
    assert!(a.accepted(), "{name}: rejected");
    let ir = dive_ir::lower(&a).expect("lowers");
    Spec {
        vars: ir.vars().iter().map(|&d| ir.decl(d).name.clone()).collect(),
        tlc_order: ir.tlc_var_order().iter().map(|v| v.index()).collect(),
    }
}

fn spec_names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(specs_dir())
        .expect("the specs")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .collect();
    names.sort();
    names
}

fn dump(name: &str) -> String {
    let p = manifest_dir()
        .join("tests/fixtures/tlc-var-order")
        .join(format!("{name}.dot"));
    std::fs::read_to_string(&p).unwrap_or_else(|_| panic!("{}", p.display()))
}

#[test]
fn every_state_fingerprint_equals_tlcs() {
    let names = Interner::new();
    let ctx = Ctx::tlc();
    let mut checked = 0;
    let mut reordered = 0;
    let mut specs = 0;
    for name in spec_names() {
        let spec = lower(&name);
        let tlc = StateFingerprinter::tlc(0, &spec.tlc_order).expect("a permutation");
        assert!(tlc.is_tlc());
        // The IR's order, as a control: it must miss wherever the orders differ.
        let identity: Vec<usize> = (0..spec.vars.len()).collect();
        let ir_order = StateFingerprinter::tlc(0, &identity).expect("a permutation");
        let differs = spec.tlc_order != identity;
        let dot = dump(&name);
        let states = states(&dot);
        assert!(!states.is_empty(), "{name}: no states");
        for (id, printed) in states {
            // TLC prints a state in its own variable order.
            let printed_order: Vec<&str> = printed.iter().map(|(n, _)| n.as_str()).collect();
            let tlc_names: Vec<&str> = spec
                .tlc_order
                .iter()
                .map(|&i| spec.vars[i].as_str())
                .collect();
            assert_eq!(printed_order, tlc_names, "{name}: TLC's printing order");
            let state: Vec<Value> = spec
                .vars
                .iter()
                .map(|v| {
                    let (_, src) = printed
                        .iter()
                        .find(|(n, _)| n == v)
                        .unwrap_or_else(|| panic!("{name}: {v} not printed"));
                    value(&names, src)
                })
                .collect();
            let got = tlc.fingerprint(&state, ctx).expect("fingerprint");
            assert_eq!(
                got,
                u64::from_ne_bytes(id.to_ne_bytes()),
                "{name}: state {printed:?}"
            );
            assert_eq!(tlc.tlc_fingerprint(&state, ctx).expect("fingerprint"), got);
            if differs {
                assert_ne!(ir_order.fingerprint(&state, ctx).expect("fingerprint"), got);
            }
            checked += 1;
        }
        specs += 1;
        if differs {
            reordered += 1;
        }
    }
    assert_eq!(specs, 29);
    assert_eq!(checked, 116);
    // The IR's order is not TLC's in most of these specifications.
    assert_eq!(reordered, 24, "specifications whose two orders differ");
}

#[test]
fn the_default_profile_keeps_its_own_fingerprint() {
    let ctx = Ctx::tlc();
    let state = [Value::from(1), Value::TRUE];
    let dive = StateFingerprinter::new(0).expect("index 0");
    assert!(!dive.is_tlc());
    let tlc = StateFingerprinter::tlc(0, &[1, 0]).expect("a permutation");
    assert_ne!(
        dive.fingerprint(&state, ctx).expect("fp"),
        tlc.fingerprint(&state, ctx).expect("fp")
    );
    // A state with the wrong number of values is a caller bug, not a fingerprint.
    assert!(tlc.fingerprint(&state[..1], ctx).is_err());
    assert!(StateFingerprinter::tlc(0, &[0, 2]).is_none());
    assert!(StateFingerprinter::tlc(131, &[0]).is_none());
}
