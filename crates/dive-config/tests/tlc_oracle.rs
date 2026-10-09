//! `dive-config` against TLC's own cfg reader.
//!
//! The `.jsonl` files were produced by `acceptance/regenerate.sh`, which runs
//! the pinned `tla2tools.jar`'s `tlc2.tool.impl.ModelConfig.parse()` on each
//! file and records what it built (or the error it threw) in a canonical
//! compact JSON form. These tests render dive's result in the same form and
//! compare the strings exactly. No Java is needed to run them.
//!
//! The captured TLC runs under `tests/fixtures/tlc/` (made with
//! `dive-conformance dev capture`) confirm that `ModelConfig`'s verdict is
//! TLC's verdict: the same `EC` code, exit status and line.

use dive_config::{
    ConfigParse, ConfigValue, ConstantEntry, Diagnostic, ModelConfig, Name, Spanned,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const ORACLE: &str = "tlc-2026.10.06.014338";

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &Path) -> String {
    match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => unreachable!("cannot read {}: {e}", path.display()),
    }
}

// ---------------------------------------------------------------------------
// Rendering, mirroring `acceptance/CfgDump.java` character for character.

/// Java `CfgDump.q`: a JSON string.
fn q(s: &str) -> String {
    let mut b = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => b.push_str("\\\""),
            '\\' => b.push_str("\\\\"),
            '\n' => b.push_str("\\n"),
            '\r' => b.push_str("\\r"),
            '\t' => b.push_str("\\t"),
            c if (c as u32) < 0x20
                || c == '\u{2028}'
                || c == '\u{2029}'
                || ((c as u32) >= 0x7f && (c as u32) < 0xa0) =>
            {
                let _ = write!(b, "\\u{:04x}", c as u32);
            }
            c => b.push(c),
        }
    }
    b.push('"');
    b
}

fn value(v: &ConfigValue) -> String {
    match v {
        ConfigValue::Int(i) => format!("{{\"int\":{i}}}"),
        ConfigValue::String(s) => format!("{{\"string\":{}}}", q(s)),
        ConfigValue::Bool(b) => format!("{{\"bool\":{b}}}"),
        ConfigValue::ModelValue(m) => format!("{{\"model\":{}}}", q(m)),
        ConfigValue::Set(items) => {
            let inner: Vec<String> = items.iter().map(|i| value(&i.value)).collect();
            format!("{{\"set\":[{}]}}", inner.join(","))
        }
    }
}

fn line(name: &Name, args: &[Spanned<ConfigValue>], v: &Spanned<ConfigValue>) -> String {
    let mut s = format!("{{\"lhs\":{}", q(&name.text));
    if !args.is_empty() {
        let a: Vec<String> = args.iter().map(|a| value(&a.value)).collect();
        let _ = write!(s, ",\"args\":[{}]", a.join(","));
    }
    let _ = write!(s, ",\"value\":{}}}", value(&v.value));
    s
}

fn names(list: &[Name]) -> String {
    let v: Vec<String> = list.iter().map(|n| q(&n.text)).collect();
    format!("[{}]", v.join(","))
}

fn str_map(m: &BTreeMap<&str, &Name>) -> String {
    let v: Vec<String> = m
        .iter()
        .map(|(k, n)| format!("{}:{}", q(k), q(&n.text)))
        .collect();
    format!("{{{}}}", v.join(","))
}

fn render_config(cfg: &ModelConfig) -> String {
    let mut s = String::from("{\"ok\":true");
    let singles = [
        ("init", &cfg.init),
        ("next", &cfg.next),
        ("specification", &cfg.specification),
        ("view", &cfg.view),
        ("symmetry", &cfg.symmetry),
        ("alias", &cfg.alias),
        ("periodic", &cfg.periodic),
        ("rl_reward", &cfg.rl_reward),
    ];
    for (k, n) in singles {
        if let Some(n) = n {
            let _ = write!(s, ",{}:{}", q(k), q(&n.text));
        }
    }
    if let Some(d) = &cfg.check_deadlock {
        let _ = write!(s, ",\"check_deadlock\":{}", d.value);
    }
    let lists = [
        ("invariants", &cfg.invariants),
        ("properties", &cfg.properties),
        ("constraints", &cfg.constraints),
        ("action_constraints", &cfg.action_constraints),
        ("postconditions", &cfg.postconditions),
        ("possible", &cfg.possible),
    ];
    for (k, l) in lists {
        if !l.is_empty() {
            let _ = write!(s, ",{}:{}", q(k), names(l));
        }
    }
    let root: Vec<String> = cfg
        .assignments()
        .map(|a| line(a.name, a.args, a.value))
        .collect();
    if !root.is_empty() {
        let _ = write!(s, ",\"constants\":[{}]", root.join(","));
    }
    let module = cfg.module_assignments();
    if !module.is_empty() {
        let v: Vec<String> = module
            .iter()
            .map(|(m, list)| {
                let l: Vec<String> = list.iter().map(|a| line(a.name, a.args, a.value)).collect();
                format!("{}:[{}]", q(m), l.join(","))
            })
            .collect();
        let _ = write!(s, ",\"module_constants\":{{{}}}", v.join(","));
    }
    let subs = cfg.substitutions();
    if !subs.is_empty() {
        let _ = write!(s, ",\"overrides\":{}", str_map(&subs));
    }
    let msubs = cfg.module_substitutions();
    if !msubs.is_empty() {
        let v: Vec<String> = msubs
            .iter()
            .map(|(m, map)| format!("{}:{}", q(m), str_map(map)))
            .collect();
        let _ = write!(s, ",\"module_overrides\":{{{}}}", v.join(","));
    }
    if !cfg.model_values.is_empty() {
        let v: Vec<String> = cfg.model_values.iter().map(|m| q(m)).collect();
        let _ = write!(s, ",\"model_values\":[{}]", v.join(","));
    }
    s.push('}');
    s
}

fn render_error(d: &Diagnostic) -> String {
    let Some(tlc) = d.tlc else {
        return format!("{{\"ok\":false,\"dive_only\":{}}}", q(d.code.as_str()));
    };
    let mut s = format!("{{\"ok\":false,\"error_code\":{}", tlc.code);
    if tlc.code == 1000 {
        let _ = write!(s, ",\"exception\":{}", q(tlc.detail.unwrap_or("")));
    } else if let Some(l) = tlc.line {
        let _ = write!(s, ",\"line\":{l}");
        if let Some(detail) = tlc.detail {
            let _ = write!(s, ",\"detail\":{}", q(detail));
        }
    }
    s.push('}');
    s
}

fn render(p: &ConfigParse) -> String {
    match (p.config(), p.error()) {
        (Some(cfg), _) => render_config(cfg),
        (None, Some(e)) => render_error(e),
        (None, None) => "{\"ok\":false}".to_owned(),
    }
}

/// TLC's lexical-error column is not modelled; drop it before comparing.
fn without_column(tlc: &str) -> String {
    match tlc.find(",\"column\":") {
        Some(i) => {
            let rest = &tlc[i + 10..];
            let end = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            format!("{}{}", &tlc[..i], &rest[end..])
        }
        None => tlc.to_owned(),
    }
}

// ---------------------------------------------------------------------------
// Reading the oracle files.

struct Case {
    path: String,
    bytes: Vec<u8>,
    tlc: String,
}

fn base64(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut acc = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((acc >> bits) & 0xff).unwrap_or(0));
        }
    }
    out
}

/// Each line is `{"path":..,"cfg":..|"cfg_base64":..,"tlc":{..}}`. The
/// `tlc` object is taken verbatim: inside JSON strings every `"` is
/// escaped, so the first `,"tlc":` is the key.
fn cases(file: &str) -> Vec<Case> {
    let text = read(&crate_dir().join(file));
    let mut out = Vec::new();
    for l in text.lines().filter(|l| !l.is_empty()) {
        let Some(i) = l.find(",\"tlc\":") else {
            unreachable!("{file}: malformed line {l}")
        };
        let head: serde_json::Value = match serde_json::from_str(&format!("{}}}", &l[..i])) {
            Ok(v) => v,
            Err(e) => unreachable!("{file}: {e}"),
        };
        let path = head["path"].as_str().unwrap_or_default().to_owned();
        let bytes = match (head["cfg"].as_str(), head["cfg_base64"].as_str()) {
            (Some(s), _) => s.as_bytes().to_vec(),
            (None, Some(b)) => base64(b),
            (None, None) => unreachable!("{file}: {path} has no cfg"),
        };
        let tlc = l[i + 7..l.len() - 1].to_owned();
        out.push(Case { path, bytes, tlc });
    }
    out
}

/// Counts (both accept, both reject, dive-only accept, TLC-only accept)
/// and collects mismatches.
#[derive(Default, Debug)]
struct Matrix {
    both_accept: usize,
    both_reject: usize,
    dive_only: usize,
    tlc_only: usize,
    mismatches: Vec<String>,
}

fn compare(file: &str, filter: impl Fn(&str) -> bool) -> Matrix {
    let mut m = Matrix::default();
    for c in cases(file).into_iter().filter(|c| filter(&c.path)) {
        let (_, parsed) = dive_config::parse_bytes(&c.bytes);
        let tlc_ok = c.tlc.starts_with("{\"ok\":true");
        match (parsed.accepted(), tlc_ok) {
            (true, true) => m.both_accept += 1,
            (false, false) => m.both_reject += 1,
            (true, false) => m.dive_only += 1,
            (false, true) => m.tlc_only += 1,
        }
        let ours = render(&parsed);
        let theirs = without_column(&c.tlc);
        if ours != theirs {
            m.mismatches.push(format!(
                "{file}: {}\n  tlc:  {theirs}\n  dive: {ours}",
                c.path
            ));
        }
    }
    m
}

fn assert_exact(file: &str, m: &Matrix) {
    assert!(
        m.mismatches.is_empty(),
        "{} of the cfg files in {file} differ from TLC:\n{}",
        m.mismatches.len(),
        m.mismatches.join("\n")
    );
    assert_eq!((m.dive_only, m.tlc_only), (0, 0), "{file}: {m:?}");
}

#[test]
fn examples_corpus_matches_tlc() {
    let m = compare("tests/corpus/examples.jsonl", |_| true);
    assert_exact("examples", &m);
    // Pinned Examples commit 6a7b26a: 235 cfg files, of which TLC rejects
    // the two Apalache-only Disruptor configs (`NULL = -1`).
    assert_eq!((m.both_accept, m.both_reject), (233, 2), "{m:?}");
}

#[test]
fn tlaplus_test_models_match_tlc() {
    let m = compare("tests/corpus/tlaplus.jsonl", |p| p.contains("/test-model/"));
    assert_exact("tlaplus test-model", &m);
    assert_eq!((m.both_accept, m.both_reject), (528, 2), "{m:?}");
    let all = compare("tests/corpus/tlaplus.jsonl", |_| true);
    assert_exact("tlaplus", &all);
    assert_eq!((all.both_accept, all.both_reject), (649, 2), "{all:?}");
}

#[test]
fn mutant_specs_match_tlc() {
    let m = compare("tests/fixtures/specs.jsonl", |_| true);
    assert_exact("specs", &m);
}

#[test]
fn parse_cases_match_tlc() {
    let m = compare("tests/fixtures/modelconfig.jsonl", |_| true);
    assert_exact("modelconfig", &m);
}

// ---------------------------------------------------------------------------
// Captured TLC runs.

/// The first cfg-reader message in a `-tool` transcript: `(code, text)`.
fn cfg_message(stdout: &str) -> Option<(u16, String)> {
    let mut lines = stdout.lines();
    while let Some(l) = lines.next() {
        let Some(rest) = l.strip_prefix("@!@!@STARTMSG ") else {
            continue;
        };
        let code: u16 = rest
            .split(':')
            .next()
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        if (5001..=5007).contains(&code) || code == 1000 {
            let body: Vec<&str> = lines
                .by_ref()
                .take_while(|l| !l.starts_with("@!@!@ENDMSG"))
                .collect();
            return Some((code, body.join("\n")));
        }
    }
    None
}

#[test]
fn captured_tlc_runs_agree() {
    let specs = crate_dir().join("tests/fixtures/specs");
    let runs = crate_dir().join("tests/fixtures/tlc").join(ORACLE);
    let mut names: Vec<String> = match std::fs::read_dir(&specs) {
        Ok(d) => d
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(e) => unreachable!("{e}"),
    };
    names.sort();
    assert_eq!(names.len(), 30);
    for name in names {
        let cfg = read(&specs.join(&name).join(format!("{name}.cfg")));
        let stdout = read(&runs.join(format!("{name}.stdout")));
        let exit: u8 = read(&runs.join(format!("{name}.exit")))
            .trim()
            .parse()
            .unwrap_or(0);
        let parsed = dive_config::parse(&cfg);
        match (parsed.error(), cfg_message(&stdout)) {
            (None, None) => {}
            (Some(e), Some((code, text))) => {
                let Some(tlc) = e.tlc else {
                    unreachable!("{name}: dive-only error {}", e.code)
                };
                assert_eq!(tlc.code, code, "{name}");
                assert_eq!(tlc.exit_code, exit, "{name}");
                if let Some(line) = tlc.line {
                    let at = if code == 5007 {
                        format!("Lexical error at line {line},")
                    } else {
                        format!("at line {line}\n")
                    };
                    assert!(text.contains(&at), "{name}: TLC said {text:?}");
                }
                if let Some(detail) = tlc.detail {
                    assert!(text.contains(detail), "{name}: TLC said {text:?}");
                }
            }
            (ours, theirs) => unreachable!("{name}: dive {ours:?}, TLC {theirs:?}"),
        }
    }
}

/// Accepted mutants whose TLC outcome shows how the cfg was interpreted.
#[test]
fn captured_interpretations() {
    let runs = crate_dir().join("tests/fixtures/tlc").join(ORACLE);
    let out = |n: &str| read(&runs.join(format!("{n}.stdout")));
    let cfg = |n: &str| {
        let src = read(
            &crate_dir()
                .join("tests/fixtures/specs")
                .join(n)
                .join(format!("{n}.cfg")),
        );
        dive_config::parse(&src).into_config()
    };
    // Repeated INVARIANT sections append: TLC checks (and violates) the second.
    assert!(out("CfgRepeatedSection").contains("Invariant Inv is violated"));
    let c = cfg("CfgRepeatedSection");
    let invs: Vec<_> = c
        .iter()
        .flat_map(|c| &c.invariants)
        .map(|n| n.text.as_str())
        .collect();
    assert_eq!(invs, ["Ok", "Inv"]);
    // A comma is a name: TLC looks up the invariant `,`.
    assert!(out("CfgCommaList").contains("The invariant , specified in the configuration file"));
    // INIT's value is the keyword NEXT.
    assert!(out("CfgKeywordAsValue").contains("The initial predicate NEXT specified"));
    // Strings are verbatim: `ASSUME N = "a\\nb"` holds, so the run succeeds.
    assert!(out("CfgStringRaw").contains("@!@!@STARTMSG 2193:0"));
    let c = cfg("CfgStringRaw");
    let v = c
        .as_ref()
        .and_then(|c| c.assignments().next().map(|a| a.value.value.clone()));
    assert_eq!(v, Some(ConfigValue::String("a\\nb".to_owned())));
    // The last of two assignments wins in TLC (`ASSUME N = 2` holds); both are kept.
    assert!(out("CfgDuplicateConstant").contains("@!@!@STARTMSG 2193:0"));
    assert_eq!(
        cfg("CfgDuplicateConstant").map(|c| c.assignments().count()),
        Some(2)
    );
    // A trailing `\*` comment without a newline is fine.
    assert!(out("CfgTrailingComment").contains("Invariant Inv is violated"));
    // `{1,}}` is a set with the model value `}`.
    assert_eq!(
        cfg("CfgSetTrailingComma").map(|c| c.model_values),
        Some(vec!["}".to_owned()])
    );
    // Everything in the all-keywords file is accepted and the run succeeds.
    assert!(out("CfgAllKeywords").contains("@!@!@STARTMSG 2193:0"));
    let c = cfg("CfgAllKeywords");
    assert!(c.as_ref().is_some_and(|c| !c.checks_deadlock()));
    assert!(matches!(
        c.as_ref().map(|c| &c.constants[..]),
        Some([.., ConstantEntry::Substitution { .. }])
    ));
}

// ---------------------------------------------------------------------------
// The harness goldens (small tier).

fn json_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.filter_map(Result::ok) {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "sany") {
                json_files(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "json") {
            out.push(p);
        }
    }
}

/// Every model TLC reached a verdict on has a cfg TLC's reader accepted;
/// dive must accept it too, and the invariant or property TLC reports as
/// violated must be one dive read from the cfg.
#[test]
fn small_tier_goldens_agree() {
    let root = crate_dir()
        .join("../../conformance/golden")
        .join(ORACLE)
        .join("examples");
    let mut files = Vec::new();
    json_files(&root, &mut files);
    assert!(
        files.len() >= 140,
        "only {} goldens under {}",
        files.len(),
        root.display()
    );
    let corpus: BTreeMap<String, Vec<u8>> = cases("tests/corpus/examples.jsonl")
        .into_iter()
        .map(|c| (c.path, c.bytes))
        .collect();
    let mut checked_names = 0;
    for f in &files {
        let g: serde_json::Value = match serde_json::from_str(&read(f)) {
            Ok(v) => v,
            Err(e) => unreachable!("{}: {e}", f.display()),
        };
        // Worker-specific goldens (ADR 0004) record TLC at several workers;
        // the configuration facts are the single-worker golden's.
        if g["workers"] != 1 {
            continue;
        }
        let id = g["model"]["id"].as_str().unwrap_or_default();
        let Some(bytes) = corpus.get(id) else {
            unreachable!("golden {id} has no cfg in the corpus file")
        };
        let (_, parsed) = dive_config::parse_bytes(bytes);
        let Some(cfg) = parsed.config() else {
            unreachable!(
                "{id}: TLC ran this model but dive rejects its cfg: {:?}",
                parsed.error()
            )
        };
        let result = &g["result"];
        if let Some(prop) = result["violated_property"].as_str() {
            let class = result["error_class"].as_str().unwrap_or_default();
            // TLC checks a PROPERTY of the form []P as an invariant and then
            // reports an invariant violation (Examples `ACP_NB_WRONG_TLC`).
            let pool: Vec<&Name> = if class.starts_with("TLC_INVARIANT") {
                cfg.invariants.iter().chain(&cfg.properties).collect()
            } else {
                cfg.properties.iter().collect()
            };
            assert!(
                pool.iter().any(|n| n.text == prop),
                "{id}: TLC reports `{prop}` ({class}) violated, but dive read {:?}",
                pool.iter().map(|n| &n.text).collect::<Vec<_>>()
            );
            checked_names += 1;
        }
        if result["error_class"] == "TLC_DEADLOCK_REACHED" {
            assert!(cfg.checks_deadlock(), "{id}: TLC found a deadlock");
        }
    }
    assert_eq!(checked_names, 17);
}
