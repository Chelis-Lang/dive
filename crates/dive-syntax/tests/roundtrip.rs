//! Property tests: for any input the parser does not panic, the CST's text
//! equals the input, and diagnostics point inside the input.
//!
//! The nightly `cargo-fuzz` target in `fuzz/` checks the same properties at
//! much higher volume; these run on stable in the normal test suite.

use dive_syntax::{SyntaxKind, parse_module};
use proptest::prelude::*;

fn check(src: &str) -> Result<(), TestCaseError> {
    let p = parse_module(src);
    let root = p.syntax();
    prop_assert_eq!(root.to_string(), src);
    prop_assert_eq!(root.kind(), SyntaxKind::SOURCE_FILE);
    let len = u32::try_from(src.len()).unwrap_or(u32::MAX);
    for d in p.diagnostics() {
        prop_assert!(u32::from(d.range.end()) <= len, "{:?} out of bounds", d);
        prop_assert!(d.code.as_str().starts_with("DIVE-S"));
    }
    // Every token is a leaf whose kind is a token kind.
    for t in root
        .descendants_with_tokens()
        .filter_map(rowan::NodeOrToken::into_token)
    {
        prop_assert!(!t.kind().is_node(), "{:?} used as a token", t.kind());
    }
    Ok(())
}

/// Fragments that exercise the grammar when glued together.
const VOCAB: &[&str] = &[
    "x",
    "y",
    "Foo",
    "1",
    "2",
    "\"s\"",
    "TRUE",
    "@",
    "==",
    "(",
    ")",
    "[",
    "]",
    "{",
    "}",
    "<<",
    ">>",
    ">>_",
    "]_",
    ",",
    ":",
    "::",
    ".",
    "!",
    "'",
    "+",
    "-",
    "*",
    "^",
    "=",
    "#",
    "\\in",
    "\\X",
    "\\cup",
    "/\\",
    "\\/",
    "~",
    "[]",
    "<>",
    "=>",
    "<=>",
    "~>",
    "-+->",
    "..",
    "|->",
    "->",
    "<-",
    "_",
    "-.",
    "ENABLED",
    "UNCHANGED",
    "SUBSET",
    "DOMAIN",
    "IF",
    "THEN",
    "ELSE",
    "CASE",
    "OTHER",
    "LET",
    "IN",
    "\\A",
    "\\E",
    "\\AA",
    "\\EE",
    "CHOOSE",
    "LAMBDA",
    "EXCEPT",
    "WF_",
    "SF_",
    "LOCAL",
    "INSTANCE",
    "WITH",
    "VARIABLE",
    "CONSTANT",
    "RECURSIVE",
    "ASSUME",
    "PROVE",
    "THEOREM",
    "LEMMA",
    "PROOF",
    "BY",
    "OBVIOUS",
    "OMITTED",
    "QED",
    "DEF",
    "USE",
    "HIDE",
    "NEW",
    "SUFFICES",
    "PICK",
    "TAKE",
    "WITNESS",
    "HAVE",
    "DEFINE",
    "<1>1.",
    "<1>",
    "<2>1.",
    "<*>",
    "<+>",
    "<1>2",
    "----",
    "---- MODULE Inner ----",
    "====",
    "(* c *)",
    "(*",
    "*)",
    "\\* c\n",
    "\n",
    "\n  ",
    " ",
    "\t",
    "∧",
    "∨",
    "∈",
    "≜",
    "⟨",
    "⟩",
    "□",
    "◇",
    "ℕ",
    "\\b101",
    "\\h1F",
    "1.5",
    "-|-",
    "x./\\",
    "é",
];

fn soup() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(VOCAB), 0..60).prop_map(|parts| {
        let mut s = String::from("---- MODULE M ----\n");
        for p in parts {
            s.push_str(p);
            s.push(' ');
        }
        s.push_str("\n====\n");
        s
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, ..ProptestConfig::default() })]

    #[test]
    fn arbitrary_text_round_trips(s in any::<String>()) {
        check(&s)?;
    }

    #[test]
    fn arbitrary_bytes_round_trip(b in prop::collection::vec(any::<u8>(), 0..512)) {
        check(&String::from_utf8_lossy(&b))?;
    }

    #[test]
    fn token_soup_round_trips(s in soup()) {
        check(&s)?;
    }

    #[test]
    fn module_with_arbitrary_body_round_trips(body in "[ -~\\n\\t]{0,200}") {
        check(&format!("---- MODULE M ----\n{body}\n===="))?;
    }
}

/// Runs `f` on a thread with a 2 MiB stack, like the default test thread.
fn on_small_stack(f: impl FnOnce() + Send + 'static) {
    let h = std::thread::Builder::new().stack_size(2 << 20).spawn(f);
    match h.map(std::thread::JoinHandle::join) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => std::panic::resume_unwind(e),
        Err(e) => unreachable!("cannot spawn: {e}"),
    }
}

#[test]
fn deep_nesting_is_refused_without_overflow() {
    on_small_stack(|| {
        let depth = 5000;
        let src = format!(
            "---- MODULE M ----\nx == {}1{}\n====\n",
            "(".repeat(depth),
            ")".repeat(depth)
        );
        let p = parse_module(&src);
        assert_eq!(p.syntax().to_string(), src);
        assert!(
            p.diagnostics()
                .iter()
                .any(|d| d.code.as_str() == "DIVE-S0040")
        );
    });
}

#[test]
fn moderate_nesting_is_accepted() {
    on_small_stack(|| {
        let depth = 200;
        let src = format!(
            "---- MODULE M ----\nx == {}1{}\n====\n",
            "(".repeat(depth),
            ")".repeat(depth)
        );
        assert!(parse_module(&src).is_ok());
    });
}

fn chain(n: usize) -> String {
    let mut src = String::from("---- MODULE M ----\nx == 1");
    for _ in 0..n {
        src.push_str(" + 1");
    }
    src.push_str("\nY == 2\n====\n");
    src
}

#[test]
fn long_operator_chains_are_accepted() {
    on_small_stack(|| {
        let src = chain(1500);
        let p = parse_module(&src);
        assert!(p.is_ok(), "{:?}", p.diagnostics().first());
        assert_eq!(p.syntax().to_string(), src);
    });
}

#[test]
fn very_long_operator_chains_are_refused_without_overflow() {
    on_small_stack(|| {
        let src = chain(20_000);
        let p = parse_module(&src);
        assert_eq!(p.syntax().to_string(), src);
        let codes: Vec<&str> = p.diagnostics().iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, ["DIVE-S0040"]);
        // The rest of the module is still parsed.
        let module = p.module();
        let names: Vec<String> = module
            .iter()
            .flat_map(dive_syntax::ast::Module::operator_definitions)
            .filter_map(|d| d.name())
            .collect();
        assert_eq!(names, ["Y"]);
    });
}
