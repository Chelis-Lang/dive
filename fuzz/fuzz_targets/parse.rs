//! Fuzz `dive_syntax::parse_module`: it must never panic, the CST's text
//! must equal the input, and diagnostics must point inside the input.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let src = String::from_utf8_lossy(data);
    let parse = dive_syntax::parse_module(&src);
    assert_eq!(parse.syntax().to_string(), src);
    let len = u32::try_from(src.len()).unwrap_or(u32::MAX);
    for d in parse.diagnostics() {
        assert!(u32::from(d.range.end()) <= len);
    }
});
