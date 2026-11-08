//! Results Core and the retired reference engine agreed on (#791).
//!
//! A test that compared Core with the reference engine now compares Core with
//! the result both engines produced when they still ran side by side. Each
//! comparison site has a key made of the running test's name and the site; a
//! site reached more than once gets one entry per pass, in order. A record is
//! the compared value's `Debug` rendering with the workspace root and
//! per-run temporary directories replaced by placeholders; a rendering longer
//! than [`LITERAL_LIMIT`] bytes is recorded as its length and 64-bit FNV-1a
//! digest. The records live in `tests/fixtures/agreed/<test file>.json`,
//! grouped by test.

#![allow(dead_code, reason = "each test binary uses a subset")]

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;

/// Longest rendering recorded literally.
pub const LITERAL_LIMIT: usize = 256;

thread_local! {
    static PASSES: RefCell<HashMap<String, usize>> = RefCell::new(HashMap::new());
}

fn next_key(site: &str) -> String {
    let test = std::thread::current().name().unwrap_or("main").to_owned();
    PASSES.with(|passes| {
        let mut passes = passes.borrow_mut();
        let site = format!("{test}::{site}");
        let pass = passes.entry(site.clone()).or_insert(0);
        let key = format!("{site}#{pass}");
        *pass += 1;
        key
    })
}

/// Replaces every occurrence of `prefix` followed by one path component.
fn replace_component(text: &str, prefix: &str, placeholder: &str) -> String {
    if prefix.len() < 2 {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(prefix) {
        out.push_str(&rest[..at]);
        out.push_str(placeholder);
        let after = &rest[at + prefix.len()..];
        let end = after
            .find(|c: char| c == '/' || c == '"' || c == '\\' || c.is_whitespace())
            .unwrap_or(after.len());
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// Replaces the default memory budget, half of this machine's physical
/// memory, with a placeholder. An explicitly requested budget stays.
fn replace_default_memory(text: &str) -> String {
    const FIELD: &str = "\"max_memory_bytes\": Number(";
    const DEFAULT: &str = "), \"max_memory_requested_source\": String(\"default\")";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(FIELD) {
        let after = &rest[at + FIELD.len()..];
        let digits = after.len() - after.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        out.push_str(&rest[..at]);
        if digits > 0 && after[digits..].starts_with(DEFAULT) {
            out.push_str("\"max_memory_bytes\": String(\"<default>\"");
            rest = &after[digits..];
        } else {
            out.push_str(FIELD);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// A rendering without machine-specific paths or the machine's default
/// memory budget.
#[must_use]
pub fn normalize(rendering: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map(|root| root.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut text = if root.len() > 1 {
        rendering.replace(&root, "<root>")
    } else {
        rendering.to_owned()
    };
    let temp = std::env::temp_dir();
    for temp in [
        temp.to_string_lossy().into_owned(),
        temp.canonicalize()
            .map(|t| t.to_string_lossy().into_owned())
            .unwrap_or_default(),
    ] {
        let temp = temp.trim_end_matches('/');
        if temp.len() > 1 {
            text = replace_component(&text, &format!("{temp}/"), "<tmp>");
        }
    }
    replace_default_memory(&text)
}

/// The recorded form of a rendering: itself when short, else its digest.
#[must_use]
pub fn record(rendering: &str) -> String {
    let rendering = normalize(rendering);
    if rendering.len() <= LITERAL_LIMIT {
        return rendering;
    }
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in rendering.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}:{}", rendering.len())
}

/// Asserts that `actual` renders as the agreed record for this pass of
/// `site` in `records` (the test file's agreed JSON).
///
/// # Panics
///
/// Panics when the record is missing or differs.
#[allow(
    clippy::panic,
    clippy::expect_used,
    reason = "a test assertion reports a differing record by panicking"
)]
pub fn check<T: Debug + ?Sized>(records: &str, site: &str, actual: &T) {
    let key = next_key(site);
    let records: HashMap<String, HashMap<String, String>> =
        serde_json::from_str(records).expect("agreed records map each test to its records");
    let rendering = format!("{actual:?}");
    let (test, pass) = key.rsplit_once("::").unwrap_or(("main", &key));
    let Some(agreed) = records.get(test).and_then(|test| test.get(pass)) else {
        panic!("no agreed record for {key}: {rendering}");
    };
    assert_eq!(&record(&rendering), agreed, "{key}: {rendering}");
}

/// Asserts that `actual` equals `other` and renders as the agreed record.
///
/// `other` is the value the test compared with when the reference engine
/// still ran. It is now computed by Core, or derived from the same run, so
/// the equality keeps any relation within Core while the record keeps the
/// agreed result.
///
/// # Panics
///
/// Panics when the values differ or the record is missing or differs.
pub fn check_eq<A: Debug + PartialEq<B> + ?Sized, B: Debug + ?Sized>(
    records: &str,
    site: &str,
    actual: &A,
    other: &B,
) {
    assert_eq!(actual, other, "{site}");
    check(records, site, actual);
}
