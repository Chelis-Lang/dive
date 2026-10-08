//! The tlaplus/tree-sitter syntax corpus (`tests/corpus/tla2sany`, MIT).
//!
//! For every test case:
//!
//! 1. dive's verdict (accept or reject) equals the pinned SANY parser's
//!    verdict, recorded in `tests/corpus/sany-verdicts.txt`;
//! 2. the CST's text equals the input;
//! 3. for accepted cases, the *structure* of the CST matches the corpus's
//!    expected tree, compared on a projection that keeps only
//!    expression-structure nodes (operator applications, junction lists,
//!    brackets, binders and definitions). That checks precedence,
//!    associativity and junction-list grouping without depending on how
//!    either tree models names and tokens.

use dive_syntax::{SyntaxKind, SyntaxNode, parse_module};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One corpus test case.
struct Case {
    file: String,
    name: String,
    error_attr: bool,
    input: String,
    expected: String,
}

fn is_rule(line: &str, c: char) -> bool {
    let t = line.trim_end_matches(['\n', '\r']);
    t.ends_with("|||")
        && t.len() > 6
        && t[..t.len() - 3].chars().all(|x| x == c)
        && t[..t.len() - 3].len() >= 3
}

/// Splits a corpus file as `SyntaxCorpusFileParser.java` does.
fn cases_in(file: &str, content: &str) -> Vec<Case> {
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if !is_rule(lines[i], '=') {
            i += 1;
            continue;
        }
        // Header: === / name lines / ===
        let mut j = i + 1;
        let mut header = Vec::new();
        while j < lines.len() && !is_rule(lines[j], '=') {
            header.push(lines[j].trim_end_matches(['\n', '\r']));
            j += 1;
        }
        let name = header
            .iter()
            .take_while(|l| !l.starts_with(':'))
            .copied()
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_owned();
        let error_attr = header.iter().any(|l| l.trim() == ":error");
        // Input until ---|||
        let mut k = j + 1;
        let mut input = String::new();
        while k < lines.len() && !is_rule(lines[k], '-') {
            input.push_str(lines[k]);
            k += 1;
        }
        // Expected until the next ===|||
        let mut m = k + 1;
        let mut expected = String::new();
        while m < lines.len() && !is_rule(lines[m], '=') {
            expected.push_str(lines[m]);
            m += 1;
        }
        out.push(Case {
            file: file.to_owned(),
            name,
            error_attr,
            input,
            expected,
        });
        i = m;
    }
    out
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/tla2sany")
}

fn read(p: &Path) -> String {
    match std::fs::read_to_string(p) {
        Ok(s) => s,
        Err(e) => unreachable!("cannot read {}: {e}", p.display()),
    }
}

fn all_cases() -> Vec<Case> {
    let root = corpus_dir();
    let mut files = Vec::new();
    for dir in [root.clone(), root.join("unicode")] {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            unreachable!("missing corpus dir {}", dir.display())
        };
        let mut names: Vec<PathBuf> = rd
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "txt"))
            .collect();
        names.sort();
        files.extend(names);
    }
    let mut cases = Vec::new();
    for f in files {
        let rel = f
            .strip_prefix(&root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        cases.extend(cases_in(&rel, &read(&f)));
    }
    cases
}

/// `(file, name, occurrence) -> accept`.
fn sany_verdicts() -> BTreeMap<(String, String, usize), bool> {
    let text = read(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus/sany-verdicts.txt"));
    let mut seen: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut out = BTreeMap::new();
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let parts: Vec<&str> = line.split('\t').collect();
        let [file, name, verdict] = parts.as_slice() else {
            unreachable!("bad verdict line {line:?}")
        };
        let n = seen
            .entry(((*file).to_owned(), (*name).to_owned()))
            .or_insert(0);
        out.insert(
            ((*file).to_owned(), (*name).to_owned(), *n),
            *verdict == "accept",
        );
        *n += 1;
    }
    out
}

// ---- Projection -------------------------------------------------------------

/// A projected tree: a label and children.
#[derive(Debug, Clone, PartialEq, Eq)]
struct P(&'static str, Vec<P>);

impl std::fmt::Display for P {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}", self.0)?;
        for c in &self.1 {
            write!(f, " {c}")?;
        }
        write!(f, ")")
    }
}

fn ts_label(name: &str) -> Option<&'static str> {
    Some(match name {
        "bound_infix_op" => "infix",
        "bound_prefix_op" => "prefix",
        "bound_postfix_op" => "postfix",
        "conj_list" => "conj",
        "conj_item" | "disj_item" => "item",
        "disj_list" => "disj",
        "parentheses" => "paren",
        "if_then_else" => "if",
        "case" => "case",
        "case_arm" => "arm",
        "other_arm" => "other",
        "let_in" => "let",
        "bounded_quantification" | "unbounded_quantification" => "quant",
        "choose" => "choose",
        "set_filter" => "filter",
        "set_map" => "map",
        "function_literal" => "fcn",
        "set_of_functions" => "fcnset",
        "record_literal" => "record",
        "set_of_records" => "recset",
        "except" => "except",
        "tuple_literal" => "tuple",
        "finite_set_literal" => "set",
        "function_evaluation" => "apply",
        "fairness" => "fair",
        "lambda" => "lambda",
        "operator_definition" => "def",
        "function_definition" => "fdef",
        _ => return None,
    })
}

fn cst_label(k: SyntaxKind) -> Option<&'static str> {
    use SyntaxKind::*;
    Some(match k {
        INFIX_EXPR => "infix",
        PREFIX_EXPR => "prefix",
        POSTFIX_EXPR => "postfix",
        CONJ_LIST => "conj",
        DISJ_LIST => "disj",
        CONJ_ITEM | DISJ_ITEM => "item",
        PAREN_EXPR => "paren",
        IF_EXPR => "if",
        CASE_EXPR => "case",
        CASE_ARM => "arm",
        OTHER_ARM => "other",
        LET_EXPR => "let",
        BOUNDED_QUANT | UNBOUNDED_QUANT | TEMPORAL_QUANT => "quant",
        CHOOSE => "choose",
        SET_FILTER => "filter",
        SET_MAP => "map",
        FCN_CONSTRUCTOR => "fcn",
        SET_OF_FCNS => "fcnset",
        RECORD => "record",
        SET_OF_RECORDS => "recset",
        EXCEPT => "except",
        TUPLE => "tuple",
        SET_ENUM => "set",
        FCN_APPL => "apply",
        FAIRNESS_EXPR => "fair",
        LAMBDA => "lambda",
        OPERATOR_DEF => "def",
        FUNCTION_DEF => "fdef",
        _ => return None,
    })
}

#[derive(Debug)]
struct S(String, Vec<S>);

fn sexpr(text: &str) -> Option<S> {
    let mut toks = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() || c == '_' {
            cur.push(c);
        } else {
            if !cur.is_empty() {
                toks.push(std::mem::take(&mut cur));
            }
            if c == '(' || c == ')' || c == ':' {
                toks.push(c.to_string());
            }
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    let mut pos = 0;
    let r = sexpr_node(&toks, &mut pos)?;
    (pos == toks.len()).then_some(r)
}

fn sexpr_node(toks: &[String], pos: &mut usize) -> Option<S> {
    if toks.get(*pos)? != "(" {
        return None;
    }
    *pos += 1;
    let name = toks.get(*pos)?.clone();
    *pos += 1;
    let mut children = Vec::new();
    loop {
        match toks.get(*pos)?.as_str() {
            ")" => {
                *pos += 1;
                return Some(S(name, children));
            }
            "(" => children.push(sexpr_node(toks, pos)?),
            _ => {
                // `field: (node)`
                *pos += 1;
                if toks.get(*pos)? != ":" {
                    return None;
                }
                *pos += 1;
                children.push(sexpr_node(toks, pos)?);
            }
        }
    }
}

/// Projects the expected tree. `\X` chains become left-nested infix nodes in
/// both trees (dive keeps them as one n-ary node).
fn project_ts(s: &S) -> Vec<P> {
    let kids: Vec<P> = s.1.iter().flat_map(project_ts).collect();
    match ts_label(&s.0) {
        Some(l) => vec![P(l, kids)],
        None => kids,
    }
}

fn project_cst(n: &SyntaxNode) -> Vec<P> {
    if n.kind() == SyntaxKind::CARTESIAN_PRODUCT {
        // a \X b \X c  ==>  infix(infix(a, b), c)
        let mut operands = n.children().map(|c| project_cst(&c));
        let mut acc = operands.next().unwrap_or_default();
        for mut operand in operands {
            acc.append(&mut operand);
            acc = vec![P("infix", acc)];
        }
        return acc;
    }
    let kids: Vec<P> = n.children().flat_map(|c| project_cst(&c)).collect();
    match cst_label(n.kind()) {
        Some(l) => vec![P(l, kids)],
        None => kids,
    }
}

/// Cases where SANY's own corpus runner expects its tree to differ from the
/// corpus (`TlaPlusSyntaxCorpusTests.java`); structure is not compared.
const SANY_KNOWN_DIFFERENT: &[&str] = &[
    "Cartesian Product as Parameter",
    "Invalid Use of LOCAL in LET/IN",
    "Invalid Use of LOCAL in Proof",
    "Step Expression Requiring Lookahead",
    "String with comment start",
    "Nonfix Submodule Excl (GH tlaplus/tlaplus #GH884)",
    "Nonfix Double Exclamation Operator (GH TSTLA #GH97, GH tlaplus/tlaplus #884)",
    "Label with Subexpression Prefix (GH tlaplus/tlaplus #885)",
    "Empty Tuple Quantification (GH tlaplus/tlaplus #888)",
    "Negative Prefix Op on RHS of Infix (GH tlaplus/tlaplus #893)",
    "Mistaken Set Filter Tuples Test",
];

#[test]
fn corpus_matches_sany_verdicts_and_round_trips() {
    let cases = all_cases();
    assert_eq!(cases.len(), 355, "corpus case count changed");
    let verdicts = sany_verdicts();
    let mut seen: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut failures = Vec::new();
    for c in &cases {
        // The verdict file stores multi-line test names on one line.
        let name = c.name.replace(['\n', '\t'], " ");
        let n = seen.entry((c.file.clone(), name.clone())).or_insert(0);
        let key = (c.file.clone(), name, *n);
        *n += 1;
        let Some(&sany_accepts) = verdicts.get(&key) else {
            unreachable!("no SANY verdict for {key:?}")
        };
        let parse = parse_module(&c.input);
        assert_eq!(
            parse.syntax().to_string(),
            c.input,
            "not lossless: {}",
            c.name
        );
        if parse.is_ok() != sany_accepts {
            failures.push(format!(
                "{} / {}: SANY {} but dive {} ({:?})",
                c.file,
                c.name,
                if sany_accepts { "accepts" } else { "rejects" },
                if parse.is_ok() { "accepts" } else { "rejects" },
                parse.diagnostics().first().map(|d| d.message.clone())
            ));
        }
        // SANY's verdict only disagrees with the corpus's `:error` attribute
        // on the known cases.
        if c.error_attr == sany_accepts {
            assert!(
                SANY_KNOWN_DIFFERENT.contains(&c.name.as_str()),
                "unexpected SANY/corpus disagreement: {}",
                c.name
            );
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn corpus_structure_matches_expected_trees() {
    let mut compared = 0;
    let mut failures = Vec::new();
    for c in all_cases() {
        if c.error_attr || SANY_KNOWN_DIFFERENT.contains(&c.name.as_str()) {
            continue;
        }
        let parse = parse_module(&c.input);
        if !parse.is_ok() {
            continue;
        }
        let Some(expected) = sexpr(&c.expected) else {
            unreachable!("cannot read expected tree of {}", c.name)
        };
        let want: Vec<String> = project_ts(&expected)
            .iter()
            .map(ToString::to_string)
            .collect();
        let got: Vec<String> = project_cst(&parse.syntax())
            .iter()
            .map(ToString::to_string)
            .collect();
        compared += 1;
        if want != got {
            failures.push(format!(
                "{} / {}\n  want {}\n  got  {}",
                c.file,
                c.name,
                want.join(" "),
                got.join(" ")
            ));
        }
    }
    assert!(compared > 300, "only {compared} cases compared");
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
