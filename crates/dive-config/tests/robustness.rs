//! No input makes the reader panic, and its answer is a function of the input.

use dive_config::{Code, MAX_SET_NESTING, parse, parse_bytes};
use proptest::prelude::*;

/// Fragments that steer random input into the reader's interesting paths.
const VOCAB: &[&str] = &[
    "CONSTANT",
    "CONSTANTS",
    "INIT",
    "NEXT",
    "SPECIFICATION",
    "INVARIANT",
    "PROPERTY",
    "CONSTRAINT",
    "ACTION_CONSTRAINT",
    "VIEW",
    "SYMMETRY",
    "ALIAS",
    "POSTCONDITION",
    "CHECK_DEADLOCK",
    "_PERIODIC",
    "_RL_REWARD",
    "_POSSIBLE",
    "TRUE",
    "FALSE",
    "x",
    "M",
    "=",
    "<-",
    "[",
    "]",
    "{",
    "}",
    "(",
    ")",
    ",",
    "!",
    "1",
    "2147483648",
    "\\h1F",
    "\"s\"",
    "\"open",
    "(*",
    "*)",
    "\\*",
    "\n",
    "\r",
    " ",
    ";",
    "-",
    "<<",
    "←",
];

fn soup() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(VOCAB), 0..60).prop_map(|v| v.join(" "))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn token_soup_never_panics(src in soup()) {
        let a = parse(&src);
        let b = parse(&src);
        prop_assert_eq!(&a, &b);
        // Exactly one error when rejected, none when accepted.
        let errors = a.diagnostics().iter().filter(|d| d.is_error()).count();
        prop_assert_eq!(errors, usize::from(!a.accepted()));
        for d in a.diagnostics() {
            prop_assert!(usize::from(d.range.end()) <= src.len());
        }
    }

    #[test]
    fn arbitrary_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..200)) {
        let (text, parsed) = parse_bytes(&bytes);
        for d in parsed.diagnostics() {
            prop_assert!(usize::from(d.range.end()) <= text.len());
        }
    }
}

#[test]
fn nesting_limit_is_explicit() {
    let ok = format!(
        "CONSTANT S = {}{}",
        "{".repeat(MAX_SET_NESTING),
        "}".repeat(MAX_SET_NESTING)
    );
    assert!(parse(&ok).accepted());
    let deep = MAX_SET_NESTING + 1;
    let too_deep = format!("CONSTANT S = {}{}", "{".repeat(deep), "}".repeat(deep));
    let p = parse(&too_deep);
    let e = p.error().map(|e| (e.code, e.tlc));
    assert_eq!(e, Some((Code::NestingTooDeep, None)));
}

#[test]
fn very_deep_input_does_not_overflow() {
    let src = format!("CONSTANT S = {}", "{".repeat(1_000_000));
    assert_eq!(
        parse(&src).error().map(|e| e.code),
        Some(Code::NestingTooDeep)
    );
}
