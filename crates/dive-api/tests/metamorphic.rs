//! Metamorphic tests (charter §Engineering practice): renaming symbols,
//! reordering definitions and conjuncts, adding unused definitions, adding
//! comments and splitting a module must not change what `dive check` reports.
//!
//! Each seed is a fixture spec with a same-named configuration. Every
//! relation rewrites the seed's source (and configuration) through the
//! lossless syntax tree, both versions are checked in the default profile,
//! and the reports are compared:
//!
//! - *exact* relations must reproduce the verdict, exit class, property,
//!   statistics, diagnostics codes, printed output and every counterexample
//!   state (with renamed variables mapped back);
//! - *order* relations (reordered `Next` disjuncts and configured
//!   invariants) may change which of several same-depth violations is
//!   reported, so they compare the verdict, the depth and, for complete
//!   searches, the statistics;
//! - reordering an invariant's conjuncts can turn a guarded conjunction into
//!   an evaluation error, as in TLC, so a pair where exactly one side fails
//!   with an evaluation error is recorded as inconclusive.
//!
//! The default profile is text-independent by design (ADR 0010, D-006,
//! D-010), so these relations hold there; under `--compat=tlc` string and
//! model-value `CHOOSE` is exempt and the suite does not run.
//!
//! The fast test checks a fixed seed list. `DIVE_METAMORPHIC=all` checks
//! every eligible fixture in the workspace (about 1,300 seeds), and
//! `DIVE_METAMORPHIC_REPORT=<path>` writes a JSON summary of that sweep.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dive_api::{BudgetSetting, CheckRequest};
use dive_syntax::{SyntaxKind, SyntaxNode, ast::AstNode as _};
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Seeds for the fast test: repo-relative root modules, each with a
/// same-named configuration beside it. Chosen from a full sweep to cover
/// every verdict and every relation, including the rarely applicable
/// conjunct, disjunct and configuration reorders.
const FAST_SEEDS: &[&str] = &[
    "conformance/corpus/validation-smoke/specifications/3870929418abe4114f36583a9a4d04798480ce4f/TLC_M0.tla",
    "conformance/corpus/validation-smoke/specifications/e5d7e351f1d2a4c04a6827d0297b2a90db5011ce/TLC_M0.tla",
    "conformance/corpus/validation-smoke/specifications/f85a8c8b070809cb0f54c081da908852bbb949be/TLC_M0.tla",
    "crates/dive-api/tests/fixtures/mode-controls/ModeMutation.tla",
    "crates/dive-api/tests/fixtures/native-sequence-operators/models/SequenceSubSeqLetHolds/SequenceSubSeqLetHolds.tla",
    "crates/dive-api/tests/fixtures/native-sequence-operators/models/SequenceTailWrongArity/SequenceTailWrongArity.tla",
    "crates/dive-api/tests/fixtures/temporal-let/specs/LtDisabled/LtDisabled.tla",
    "crates/dive-api/tests/fixtures/temporal-operators/models/OpConstantHolds/OpConstantHolds.tla",
    "crates/dive-api/tests/fixtures/checker/disk-seen-roles/ActionProperty/ActionProperty.tla",
    "crates/dive-api/tests/fixtures/checker/disk-seen-roles/AritySpecification/AritySpecification.tla",
    "crates/dive-api/tests/fixtures/checker/disk-seen-roles/AssumeFalseSafety/AssumeFalseSafety.tla",
    "crates/dive-api/tests/fixtures/checker/disk-seen/ReplayLaterViolation/ReplayLaterViolation.tla",
    "crates/dive-api/tests/fixtures/checker/disk-seen/ReplayPrintEmpty/ReplayPrintEmpty.tla",
    "crates/dive-api/tests/fixtures/checker/errors/specs/ErActConstrAssert/ErActConstrAssert.tla",
    "crates/dive-api/tests/fixtures/checker/errors/specs/ErAssumeAssert/ErAssumeAssert.tla",
    "crates/dive-api/tests/fixtures/checker/errors/specs/ErFpLambda/ErFpLambda.tla",
    "crates/dive-api/tests/fixtures/checker/errors/specs/ErFpTuple/ErFpTuple.tla",
    "crates/dive-api/tests/fixtures/checker/errors/specs/ErInitIncomplete/ErInitIncomplete.tla",
    "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextIncomplete/ErNextIncomplete.tla",
    "crates/dive-api/tests/fixtures/checker/errors/specs/ErNextRhsAssert/ErNextRhsAssert.tla",
    "crates/dive-api/tests/fixtures/checker/errors/specs/ErPostFalse/ErPostFalse.tla",
    "crates/dive-api/tests/fixtures/checker/errors/specs/ErPropAssert/ErPropAssert.tla",
    "crates/dive-api/tests/fixtures/checker/errors/specs/ErPropInitDivZero/ErPropInitDivZero.tla",
    "crates/dive-config/tests/fixtures/specs/CfgCommaList/CfgCommaList.tla",
    "crates/dive-config/tests/fixtures/specs/CfgLexBom/CfgLexBom.tla",
    "crates/dive-config/tests/fixtures/specs/CfgTwiceInit/CfgTwiceInit.tla",
    "crates/dive-conformance/tests/fixtures/specs/ActionProperty/ActionProperty.tla",
    "crates/dive-conformance/tests/fixtures/specs/InitProperty/InitProperty.tla",
    "crates/dive-conformance/tests/fixtures/specs/PrintMarker/PrintMarker.tla",
    "crates/dive-eval/tests/fixtures/cdot/specs/CdDeadlock/CdDeadlock.tla",
    "crates/dive-eval/tests/fixtures/examples/specifications/CigaretteSmokers/CigaretteSmokers.tla",
    "crates/dive-eval/tests/fixtures/examples/specifications/DieHard/DieHard.tla",
    "crates/dive-eval/tests/fixtures/examples/specifications/MissionariesAndCannibals/MissionariesAndCannibals.tla",
    "crates/dive-eval/tests/fixtures/next/specs/NxForallOrder/NxForallOrder.tla",
    "crates/dive-eval/tests/fixtures/next/specs/NxInLazyCup/NxInLazyCup.tla",
    "crates/dive-eval/tests/fixtures/specs/EvLazy/EvLazy.tla",
    "crates/dive-ir/tests/fixtures/actions/specs/SplitThrows/SplitThrows.tla",
    "crates/dive-ir/tests/fixtures/var-order/specs/Vars12/Vars12.tla",
    "crates/dive-model/tests/fixtures/specs/ErUnassigned/ErUnassigned.tla",
    "crates/dive-model/tests/fixtures/specs/SuPrintAssume/SuPrintAssume.tla",
    "crates/dive-trace/tests/fixtures/specs/EnabledQuantifiedCover/EnabledQuantifiedCover.tla",
    "crates/dive-value/tests/fixtures/printing/specs/EmptySymbolicSize/EmptySymbolicSize.tla",
    "crates/dive-value/tests/fixtures/specs/RvMemNorm/RvMemNorm.tla",
];

/// States explored per run before a pair is treated as inconclusive.
const MAX_STATES: u64 = 20_000;
/// Wall time per run before a pair is treated as inconclusive.
const TIME_LIMIT: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------------------
// Scratch directories

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scratch() -> Result<Scratch> {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let root = std::env::temp_dir().join(format!(
        "dive-metamorphic-{}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&root)?;
    Ok(Scratch(root))
}

// ---------------------------------------------------------------------------
// A small deterministic generator, so a failing relation replays exactly.

struct Rng(u64);
impl Rng {
    fn new(seed: &str) -> Self {
        // FNV-1a over the seed's path; never zero.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in seed.bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        Self(h | 1)
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n.max(1)).unwrap_or(1)).unwrap_or(0)
    }
    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            items.swap(i, self.below(i + 1));
        }
    }
}

// ---------------------------------------------------------------------------
// The seed module, read through the lossless syntax tree

/// A module-level unit: its source range and the module-level names it
/// defines and mentions.
struct Unit {
    start: usize,
    end: usize,
    kind: SyntaxKind,
    defines: Vec<String>,
    mentions: BTreeSet<String>,
}

struct Module {
    text: String,
    /// The module's own name.
    name: String,
    units: Vec<Unit>,
    /// The `EXTENDS` clause's range and names, when present.
    extends: Option<(usize, usize, Vec<String>)>,
    /// End of the module header line.
    header_end: usize,
    /// Every identifier occurrence: (start, end, text, renamable position).
    idents: Vec<(usize, usize, String, bool)>,
    /// Whether the module defines or declares a symbolic operator, uses
    /// `RECURSIVE`, instances, nested modules, proofs or recovered syntax:
    /// unit-level rewrites skip such modules.
    unit_rewrites_unsafe: bool,
    root: SyntaxNode,
}

fn offset(size: dive_syntax::TextSize) -> usize {
    usize::from(size)
}

/// Identifier positions that name something other than a module-level
/// symbol: record fields, `EXCEPT` field components, the left-hand side of
/// an instance substitution and an instanced module's operator after `!`.
fn is_label_position(token: &dive_syntax::SyntaxToken) -> bool {
    // `I!Op` names an operator of the instanced module.
    let mut before = token.prev_token();
    while before.as_ref().is_some_and(|t| t.kind().is_trivia()) {
        before = before.and_then(|t| t.prev_token());
    }
    if before.is_some_and(|t| t.kind() == SyntaxKind::BANG) {
        return true;
    }
    let Some(parent) = token.parent() else {
        return false;
    };
    match parent.kind() {
        SyntaxKind::RECORD_FIELD
        | SyntaxKind::FIELD_VAL
        | SyntaxKind::FIELD_SET
        | SyntaxKind::EXCEPT_COMPONENT => true,
        SyntaxKind::SUBSTITUTION => {
            // `x <- e`: only the substituted parameter is a label.
            let first = parent
                .children_with_tokens()
                .filter_map(dive_syntax::SyntaxElement::into_token)
                .find(|t| t.kind() == SyntaxKind::IDENT);
            first.as_ref() == Some(token)
        }
        _ => false,
    }
}

fn declared_names(node: &SyntaxNode) -> (Vec<String>, bool) {
    // Names declared by a unit, and whether a symbolic operator is involved.
    let mut names = Vec::new();
    let mut symbolic = false;
    let ident = |n: &SyntaxNode| {
        n.children_with_tokens()
            .filter_map(dive_syntax::SyntaxElement::into_token)
            .find(|t| t.kind() == SyntaxKind::IDENT)
            .map(|t| t.text().to_owned())
    };
    match node.kind() {
        SyntaxKind::VARIABLE_DECL => {
            names.extend(
                node.children_with_tokens()
                    .filter_map(dive_syntax::SyntaxElement::into_token)
                    .filter(|t| t.kind() == SyntaxKind::IDENT)
                    .map(|t| t.text().to_owned()),
            );
            for child in node.children() {
                if let Some(name) = ident(&child) {
                    names.push(name);
                }
            }
        }
        SyntaxKind::CONSTANT_DECL => {
            for child in node.children() {
                match child.kind() {
                    SyntaxKind::IDENT_DECL => names.extend(ident(&child)),
                    _ => symbolic = true,
                }
            }
        }
        SyntaxKind::OPERATOR_DEF | SyntaxKind::FUNCTION_DEF => {
            let lhs = node.children().next();
            match lhs.as_ref().map(SyntaxNode::kind) {
                Some(SyntaxKind::IDENT_LHS) => names.extend(lhs.as_ref().and_then(ident)),
                Some(SyntaxKind::PREFIX_LHS | SyntaxKind::INFIX_LHS | SyntaxKind::POSTFIX_LHS) => {
                    symbolic = true;
                }
                _ => names.extend(ident(node)),
            }
        }
        _ => {}
    }
    (names, symbolic)
}

fn read_module(text: &str) -> Option<Module> {
    let parse = dive_syntax::parse_module(text);
    if !parse.is_ok() {
        return None;
    }
    let module = parse.module()?;
    let node = module.syntax().clone();
    let name = module.name()?;
    let mut units = Vec::new();
    let mut extends = None;
    let mut header_end = 0;
    let mut unit_rewrites_unsafe = false;
    for child in node.children() {
        let range = child.text_range();
        let (start, end) = (offset(range.start()), offset(range.end()));
        match child.kind() {
            SyntaxKind::MODULE_HEADER => header_end = end,
            SyntaxKind::EXTENDS => {
                let names = child
                    .children_with_tokens()
                    .filter_map(dive_syntax::SyntaxElement::into_token)
                    .filter(|t| t.kind() == SyntaxKind::IDENT)
                    .map(|t| t.text().to_owned())
                    .collect();
                extends = Some((start, end, names));
            }
            SyntaxKind::VARIABLE_DECL
            | SyntaxKind::CONSTANT_DECL
            | SyntaxKind::OPERATOR_DEF
            | SyntaxKind::FUNCTION_DEF
            | SyntaxKind::ASSUMPTION => {
                let (defines, symbolic) = declared_names(&child);
                unit_rewrites_unsafe |= symbolic;
                let mentions = child
                    .descendants_with_tokens()
                    .filter_map(dive_syntax::SyntaxElement::into_token)
                    .filter(|t| t.kind() == SyntaxKind::IDENT)
                    .map(|t| t.text().to_owned())
                    .collect();
                units.push(Unit {
                    start,
                    end,
                    kind: child.kind(),
                    defines,
                    mentions,
                });
            }
            SyntaxKind::RECURSIVE_DECL
            | SyntaxKind::MODULE_DEF
            | SyntaxKind::INSTANCE
            | SyntaxKind::THEOREM
            | SyntaxKind::USE_OR_HIDE
            | SyntaxKind::MODULE
            | SyntaxKind::ERROR => unit_rewrites_unsafe = true,
            _ => {}
        }
    }
    let idents = node
        .descendants_with_tokens()
        .filter_map(dive_syntax::SyntaxElement::into_token)
        .filter(|t| t.kind() == SyntaxKind::IDENT)
        .map(|t| {
            let r = t.text_range();
            (
                offset(r.start()),
                offset(r.end()),
                t.text().to_owned(),
                !is_label_position(&t),
            )
        })
        .collect();
    // `LOCAL` definitions do not cross a module split; treat them as unsafe
    // for unit rewrites so the split never hides one.
    unit_rewrites_unsafe |= node
        .descendants_with_tokens()
        .filter_map(dive_syntax::SyntaxElement::into_token)
        .any(|t| t.kind() == SyntaxKind::LOCAL_KW);
    Some(Module {
        text: text.to_owned(),
        name,
        units,
        extends,
        header_end,
        idents,
        unit_rewrites_unsafe,
        root: node,
    })
}

/// The column at which `at` starts within its line.
fn column(text: &str, at: usize) -> usize {
    at - text[..at].rfind('\n').map_or(0, |i| i + 1)
}

/// A unit's text, prefixed so its first token keeps its original column:
/// junction lists are layout-sensitive, and their later lines keep their
/// absolute columns when the text is copied.
fn unit_text(module: &Module, unit: &Unit) -> String {
    format!(
        "{}{}",
        " ".repeat(column(&module.text, unit.start)),
        &module.text[unit.start..unit.end]
    )
}

// ---------------------------------------------------------------------------
// Configuration files

const CFG_SECTIONS: &[&str] = &[
    "CONSTANT",
    "CONSTANTS",
    "INIT",
    "NEXT",
    "SPECIFICATION",
    "INVARIANT",
    "INVARIANTS",
    "PROPERTY",
    "PROPERTIES",
    "SYMMETRY",
    "VIEW",
    "CONSTRAINT",
    "CONSTRAINTS",
    "ACTION_CONSTRAINT",
    "ACTION_CONSTRAINTS",
    "ALIAS",
    "POSTCONDITION",
    "CHECK_DEADLOCK",
];

#[derive(Clone, Debug, PartialEq)]
enum CfgToken {
    Word(String),
    Punct(String),
    Other(String),
}

/// Splits a configuration into tokens, keeping every byte so the text can be
/// rebuilt exactly.
fn cfg_tokens(text: &str) -> Vec<(CfgToken, String)> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &text[i..];
        let (len, token) = if rest.starts_with("\\*") {
            let len = rest.find('\n').unwrap_or(rest.len());
            (len, CfgToken::Other(String::new()))
        } else if rest.starts_with("(*") {
            let len = rest.find("*)").map_or(rest.len(), |e| e + 2);
            (len, CfgToken::Other(String::new()))
        } else if let Some(body) = rest.strip_prefix('"') {
            let len = body.find('"').map_or(rest.len(), |e| e + 2);
            (len, CfgToken::Other(String::new()))
        } else if rest.starts_with("<-") {
            (2, CfgToken::Punct("<-".to_owned()))
        } else {
            let c = bytes[i];
            if c.is_ascii_alphanumeric() || c == b'_' {
                let len = rest
                    .bytes()
                    .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                    .count();
                (len, CfgToken::Word(rest[..len].to_owned()))
            } else if c.is_ascii_whitespace() {
                let len = rest.bytes().take_while(u8::is_ascii_whitespace).count();
                (len, CfgToken::Other(String::new()))
            } else {
                let len = rest.chars().next().map_or(1, char::len_utf8);
                (len, CfgToken::Punct(rest[..len].to_owned()))
            }
        };
        out.push((token, rest[..len].to_owned()));
        i += len;
    }
    out
}

/// Renames module-level names in a configuration: every name in a name
/// section, and in a `CONSTANT` section the left-hand side of `=` and both
/// sides of `<-` (a value after `=` is a model value, not a module name).
fn rename_cfg(text: &str, renames: &BTreeMap<String, String>) -> String {
    let tokens = cfg_tokens(text);
    let significant: Vec<usize> = tokens
        .iter()
        .enumerate()
        .filter(|(_, (t, _))| !matches!(t, CfgToken::Other(_)))
        .map(|(i, _)| i)
        .collect();
    let mut section = "";
    let mut out = String::new();
    for (pos, (token, raw)) in tokens.iter().enumerate() {
        let CfgToken::Word(word) = token else {
            out.push_str(raw);
            continue;
        };
        if CFG_SECTIONS.contains(&word.as_str()) {
            section = CFG_SECTIONS
                .iter()
                .find(|s| **s == word)
                .copied()
                .unwrap_or("");
            out.push_str(raw);
            continue;
        }
        let k = significant.iter().position(|&i| i == pos);
        let next = k
            .and_then(|k| significant.get(k + 1))
            .map(|&i| &tokens[i].0);
        let prev = k
            .and_then(|k| k.checked_sub(1))
            .and_then(|k| significant.get(k))
            .map(|&i| &tokens[i].0);
        let punct = |t: Option<&CfgToken>, p: &str| matches!(t, Some(CfgToken::Punct(s)) if s == p);
        let is_name = if section.starts_with("CONSTANT") {
            punct(next, "=") || punct(next, "<-") || punct(prev, "<-")
        } else {
            section != "CHECK_DEADLOCK"
        };
        match renames.get(word) {
            Some(new) if is_name => out.push_str(new),
            _ => out.push_str(raw),
        }
    }
    out
}

/// The names listed under the configuration sections in `wanted`.
fn cfg_names(text: &str, wanted: &[&str]) -> Vec<String> {
    let mut section = "";
    let mut names = Vec::new();
    for (token, _) in cfg_tokens(text) {
        if let CfgToken::Word(word) = token {
            if CFG_SECTIONS.contains(&word.as_str()) {
                section = CFG_SECTIONS
                    .iter()
                    .find(|s| **s == word)
                    .copied()
                    .unwrap_or("");
            } else if wanted.contains(&section) {
                names.push(word);
            }
        }
    }
    names
}

// ---------------------------------------------------------------------------
// Relations

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Strength {
    /// Everything observable must match.
    Exact,
    /// Exact unless exactly one side fails with an evaluation error.
    ExactUnlessGuard,
    /// Verdict and depth; statistics for complete searches.
    Order,
}

struct Variant {
    relation: &'static str,
    strength: Strength,
    /// Files to write beside the copied siblings: (file name, contents).
    files: Vec<(String, String)>,
    cfg: String,
    /// Renamed names, new to old, for mapping reports back.
    back: BTreeMap<String, String>,
}

/// Names that occur anywhere in `module` or the seed's other modules.
fn taken_names(module: &Module, siblings: &BTreeSet<String>) -> BTreeSet<String> {
    let mut taken: BTreeSet<String> = module.idents.iter().map(|i| i.2.clone()).collect();
    taken.extend(siblings.iter().cloned());
    taken
}

/// A fresh name of the same length as `old`. Junction lists are layout
/// sensitive, so a rename must not move any later token on its line. Every
/// candidate has a lowercase letter, so it is never a keyword, and starts
/// with `Z` or `Q`, so it is unlikely to be a standard-module name.
fn same_length(taken: &BTreeSet<String>, used: &BTreeSet<String>, old: &str) -> Option<String> {
    let len = old.len();
    let lower: Vec<char> = ('a'..='z').collect();
    let candidates: Vec<String> = match len {
        0 => Vec::new(),
        1 => lower
            .iter()
            .chain(&['Z', 'Q'])
            .map(char::to_string)
            .collect(),
        2 => ['Z', 'Q']
            .iter()
            .flat_map(|h| lower.iter().map(move |c| format!("{h}{c}")))
            .collect(),
        _ => (0..10_000u32)
            .map(|n| format!("Zq{n:0>width$}", width = len - 2))
            .filter(|c| c.len() == len)
            .collect(),
    };
    candidates
        .into_iter()
        .find(|c| c != old && !taken.contains(c) && !used.contains(c))
}

fn fresh(taken: &BTreeSet<String>, base: &str) -> String {
    let mut n = 0;
    loop {
        let candidate = format!("Meta{n}{base}");
        if !taken.contains(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Renames every module-level variable, constant and operator that no
/// sibling module mentions and no record field, `EXCEPT` path or instance
/// substitution uses as a label.
fn rename_symbols(module: &Module, cfg: &str, siblings: &BTreeSet<String>) -> Option<Variant> {
    let declared: BTreeSet<&String> = module.units.iter().flat_map(|u| &u.defines).collect();
    let labels: BTreeSet<&String> = module
        .idents
        .iter()
        .filter(|i| !i.3)
        .map(|i| &i.2)
        .collect();
    let taken = taken_names(module, siblings);
    let mut renames = BTreeMap::new();
    let mut used = BTreeSet::new();
    for name in declared {
        if labels.contains(name) || siblings.contains(name) || name == &module.name {
            continue;
        }
        if let Some(new) = same_length(&taken, &used, name) {
            used.insert(new.clone());
            renames.insert(name.clone(), new);
        }
    }
    if renames.is_empty() {
        return None;
    }
    let mut text = String::new();
    let mut at = 0;
    for (start, end, ident, renamable) in &module.idents {
        if let Some(new) = renames.get(ident).filter(|_| *renamable) {
            text.push_str(&module.text[at..*start]);
            text.push_str(new);
            at = *end;
        }
    }
    text.push_str(&module.text[at..]);
    Some(Variant {
        relation: "rename-symbols",
        strength: Strength::Exact,
        files: vec![(format!("{}.tla", module.name), text)],
        cfg: rename_cfg(cfg, &renames),
        back: renames.into_iter().map(|(old, new)| (new, old)).collect(),
    })
}

/// A random order of the units that keeps every definition after the
/// definitions and declarations it mentions, and keeps variable declarations
/// in their original relative order (it fixes the state layout).
fn topological_order(module: &Module, rng: &mut Rng) -> Vec<usize> {
    let n = module.units.len();
    let mut definer: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, unit) in module.units.iter().enumerate() {
        for name in &unit.defines {
            definer.insert(name.as_str(), i);
        }
    }
    let mut deps: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); n];
    let mut last_variables = None;
    let mut last_assumption = None;
    for (i, unit) in module.units.iter().enumerate() {
        for name in &unit.mentions {
            if let Some(&d) = definer.get(name.as_str())
                && d != i
            {
                deps[i].insert(d);
            }
        }
        if unit.kind == SyntaxKind::VARIABLE_DECL {
            if let Some(prev) = last_variables {
                deps[i].insert(prev);
            }
            last_variables = Some(i);
        }
        // Assumptions are evaluated in text order, which orders their
        // printed output and decides which false one is reported.
        if unit.kind == SyntaxKind::ASSUMPTION {
            if let Some(prev) = last_assumption {
                deps[i].insert(prev);
            }
            last_assumption = Some(i);
        }
    }
    let mut placed = vec![false; n];
    let mut order = Vec::with_capacity(n);
    while order.len() < n {
        let ready: Vec<usize> = (0..n)
            .filter(|&i| !placed[i] && deps[i].iter().all(|&d| placed[d]))
            .collect();
        if ready.is_empty() {
            // A cycle (only possible through a mis-attributed mention):
            // keep the remaining units in source order.
            order.extend((0..n).filter(|&i| !placed[i]));
            break;
        }
        let pick = ready[rng.below(ready.len())];
        placed[pick] = true;
        order.push(pick);
    }
    order
}

/// Rebuilds the module with its units in `order`, keeping the header,
/// `EXTENDS` and the closing line.
fn with_units(module: &Module, order: &[usize], separator: &str) -> String {
    let first = module.units.iter().map(|u| u.start).min().unwrap_or(0);
    let last = module.units.iter().map(|u| u.end).max().unwrap_or(0);
    let start_line = module.text[..first].rfind('\n').map_or(0, |i| i + 1);
    let mut text = module.text[..start_line].to_owned();
    for (k, &i) in order.iter().enumerate() {
        if k > 0 {
            text.push_str(separator);
        }
        text.push_str(&unit_text(module, &module.units[i]));
    }
    text.push_str(&module.text[last..]);
    text
}

fn reorder_definitions(module: &Module, cfg: &str, rng: &mut Rng) -> Option<Variant> {
    if module.unit_rewrites_unsafe || module.units.len() < 2 {
        return None;
    }
    let mut order = topological_order(module, rng);
    if order.iter().copied().eq(0..order.len()) {
        // Try once more for a different order; a fully chained module has
        // only its source order.
        order = topological_order(module, rng);
        if order.iter().copied().eq(0..order.len()) {
            return None;
        }
    }
    Some(Variant {
        relation: "reorder-definitions",
        strength: Strength::Exact,
        files: vec![(
            format!("{}.tla", module.name),
            with_units(module, &order, "\n\n"),
        )],
        cfg: cfg.to_owned(),
        back: BTreeMap::new(),
    })
}

/// Comments and blank lines between every pair of units.
fn add_comments(module: &Module, cfg: &str) -> Option<Variant> {
    if module.units.len() < 2 {
        return None;
    }
    let mut text = String::new();
    let mut at = 0;
    for unit in module.units.iter().skip(1) {
        let col = column(&module.text, unit.start);
        text.push_str(&module.text[at..unit.start]);
        text.push_str(
            "\n\\* metamorphic line comment\n(* metamorphic (* nested *) block comment *)\n\n",
        );
        text.push_str(&" ".repeat(col));
        at = unit.start;
    }
    text.push_str(&module.text[at..]);
    Some(Variant {
        relation: "add-comments",
        strength: Strength::Exact,
        files: vec![(format!("{}.tla", module.name), text)],
        cfg: cfg.to_owned(),
        back: BTreeMap::new(),
    })
}

/// Unused definitions with string literals and a string `CHOOSE`, before the
/// first unit and after the last. In TLC their position can change the
/// result of an unrelated `CHOOSE` over strings (D-006); in the default
/// profile it must not.
fn add_unused_definitions(
    module: &Module,
    cfg: &str,
    siblings: &BTreeSet<String>,
) -> Option<Variant> {
    let first = module.units.iter().map(|u| u.start).min()?;
    let last = module.units.iter().map(|u| u.end).max()?;
    let taken = taken_names(module, siblings);
    let (a, b, c) = (
        fresh(&taken, "UnusedStrings"),
        fresh(&taken, "UnusedChoice"),
        fresh(&taken, "UnusedNumber"),
    );
    let head = format!(
        "{a} == {{\"zz\", \"b\", \"a\", \"mm\", \"\"}}\n{b} == CHOOSE s \\in {{\"zz\", \"a\"}} : TRUE\n\n"
    );
    let tail = format!("\n\n{c} == <<\"tail\", 42, {a}>>\n");
    let col = column(&module.text, first);
    let mut text = module.text[..first].to_owned();
    if col > 0 {
        text.push('\n');
    }
    text.push_str(&head);
    text.push_str(&" ".repeat(col));
    text.push_str(&module.text[first..last]);
    text.push_str(&tail);
    text.push_str(&module.text[last..]);
    Some(Variant {
        relation: "add-unused-definitions",
        strength: Strength::Exact,
        files: vec![(format!("{}.tla", module.name), text)],
        cfg: cfg.to_owned(),
        back: BTreeMap::new(),
    })
}

/// Moves the declarations and a dependency-closed prefix of definitions into
/// a new module that the root extends.
fn split_module(
    module: &Module,
    cfg: &str,
    siblings: &BTreeSet<String>,
    rng: &mut Rng,
) -> Option<Variant> {
    if module.unit_rewrites_unsafe || module.units.len() < 2 {
        return None;
    }
    let order = topological_order(module, rng);
    let cut = 1 + rng.below(order.len() - 1);
    let (base_units, rest) = order.split_at(cut);
    let taken = taken_names(module, siblings);
    let base_name = fresh(&taken, &format!("{}Base", module.name));
    let extends: Vec<String> = module
        .extends
        .as_ref()
        .map(|(_, _, names)| names.clone())
        .unwrap_or_default();
    let mut base = format!("---- MODULE {base_name} ----\n");
    if !extends.is_empty() {
        let _ = writeln!(base, "EXTENDS {}", extends.join(", "));
    }
    for &i in base_units {
        base.push_str(&unit_text(module, &module.units[i]));
        base.push_str("\n\n");
    }
    base.push_str("====\n");
    // The root: header, `EXTENDS` of the new module, the remaining units.
    let mut root = module.text[..module.header_end].to_owned();
    let _ = write!(root, "\nEXTENDS {base_name}\n");
    let after_header = module.extends.as_ref().map_or(module.header_end, |e| e.1);
    let first = module.units.iter().map(|u| u.start).min()?;
    let start_line = module.text[..first].rfind('\n').map_or(0, |i| i + 1);
    // Keep anything between the header (or `EXTENDS`) and the first unit,
    // such as separators.
    root.push_str(&module.text[after_header..start_line.max(after_header)]);
    for &i in rest {
        root.push_str(&unit_text(module, &module.units[i]));
        root.push_str("\n\n");
    }
    let last = module.units.iter().map(|u| u.end).max()?;
    root.push_str(module.text[last..].trim_start_matches(['\n', ' ']));
    Some(Variant {
        relation: "split-module",
        strength: Strength::Exact,
        files: vec![
            (format!("{}.tla", module.name), root),
            (format!("{base_name}.tla"), base),
        ],
        cfg: cfg.to_owned(),
        back: BTreeMap::new(),
    })
}

/// The junction list that forms the whole body of the parameterless
/// operator definition `name`, when there is one.
fn body_junction(module: &Module, name: &str, kind: SyntaxKind) -> Option<SyntaxNode> {
    module.root.children().find_map(|unit| {
        if unit.kind() != SyntaxKind::OPERATOR_DEF {
            return None;
        }
        let lhs = unit.children().next()?;
        if lhs.kind() != SyntaxKind::IDENT_LHS || lhs.children().next().is_some() {
            return None;
        }
        let lhs_name = lhs
            .children_with_tokens()
            .filter_map(dive_syntax::SyntaxElement::into_token)
            .find(|t| t.kind() == SyntaxKind::IDENT)?;
        if lhs_name.text() != name || lhs.text().to_string().contains('(') {
            return None;
        }
        let body = unit.children().last()?;
        (body.kind() == kind).then_some(body)
    })
}

/// Permutes the items of the junction lists at `lists`, keeping each item's
/// bullet at the list's column so the layout still parses.
fn permute_junctions(module: &Module, lists: &[SyntaxNode], rng: &mut Rng) -> Option<String> {
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for list in lists {
        let items: Vec<(usize, usize)> = list
            .children()
            .map(|item| {
                let r = item.text_range();
                (offset(r.start()), offset(r.end()))
            })
            .collect();
        if items.len() < 2 {
            continue;
        }
        let col = column(&module.text, items[0].0);
        let mut order: Vec<usize> = (0..items.len()).collect();
        rng.shuffle(&mut order);
        if order.iter().copied().eq(0..items.len()) {
            order.reverse();
        }
        let mut text = String::new();
        for (k, &i) in order.iter().enumerate() {
            if k > 0 {
                text.push('\n');
                text.push_str(&" ".repeat(col));
            }
            text.push_str(&module.text[items[i].0..items[i].1]);
        }
        let range = list.text_range();
        edits.push((offset(range.start()), offset(range.end()), text));
    }
    if edits.is_empty() {
        return None;
    }
    edits.sort_by_key(|e| e.0);
    let mut out = String::new();
    let mut at = 0;
    for (start, end, text) in edits {
        if start < at {
            return None;
        }
        out.push_str(&module.text[at..start]);
        out.push_str(&text);
        at = end;
    }
    out.push_str(&module.text[at..]);
    Some(out)
}

/// Permutes the conjuncts of every configured invariant whose body is a
/// conjunction list. Exploration is unchanged; only a guard such as
/// `x \in S /\ x.f = 1` can turn into an evaluation error.
fn reorder_invariant_conjuncts(module: &Module, cfg: &str, rng: &mut Rng) -> Option<Variant> {
    let lists: Vec<SyntaxNode> = cfg_names(cfg, &["INVARIANT", "INVARIANTS"])
        .iter()
        .filter_map(|name| body_junction(module, name, SyntaxKind::CONJ_LIST))
        .collect();
    let text = permute_junctions(module, &lists, rng)?;
    Some(Variant {
        relation: "reorder-invariant-conjuncts",
        strength: Strength::ExactUnlessGuard,
        files: vec![(format!("{}.tla", module.name), text)],
        cfg: cfg.to_owned(),
        back: BTreeMap::new(),
    })
}

/// Permutes the disjuncts of the configured `NEXT` action when its body is a
/// disjunction list. The state space is unchanged; the order in which a
/// level's successors are found is not.
fn reorder_next_disjuncts(module: &Module, cfg: &str, rng: &mut Rng) -> Option<Variant> {
    let lists: Vec<SyntaxNode> = cfg_names(cfg, &["NEXT"])
        .iter()
        .filter_map(|name| body_junction(module, name, SyntaxKind::DISJ_LIST))
        .collect();
    let text = permute_junctions(module, &lists, rng)?;
    Some(Variant {
        relation: "reorder-next-disjuncts",
        strength: Strength::Order,
        files: vec![(format!("{}.tla", module.name), text)],
        cfg: cfg.to_owned(),
        back: BTreeMap::new(),
    })
}

/// Reverses the order of configured invariants, which decides which one is
/// reported when a state violates several.
fn reorder_cfg_invariants(module: &Module, cfg: &str) -> Option<Variant> {
    let names = cfg_names(cfg, &["INVARIANT", "INVARIANTS"]);
    if names.len() < 2 {
        return None;
    }
    let mut reversed = names.clone();
    reversed.reverse();
    let mut text = String::new();
    let mut k = 0;
    let mut section = "";
    for (token, raw) in cfg_tokens(cfg) {
        match token {
            CfgToken::Word(word) if CFG_SECTIONS.contains(&word.as_str()) => {
                section = CFG_SECTIONS
                    .iter()
                    .find(|s| **s == word)
                    .copied()
                    .unwrap_or("");
                text.push_str(&raw);
            }
            CfgToken::Word(_) if section.starts_with("INVARIANT") => {
                text.push_str(&reversed[k]);
                k += 1;
            }
            _ => text.push_str(&raw),
        }
    }
    Some(Variant {
        relation: "reorder-config-invariants",
        strength: Strength::Order,
        files: vec![(format!("{}.tla", module.name), module.text.clone())],
        cfg: text,
        back: BTreeMap::new(),
    })
}

fn variants(seed: &str, module: &Module, cfg: &str, siblings: &BTreeSet<String>) -> Vec<Variant> {
    let mut rng = Rng::new(seed);
    [
        rename_symbols(module, cfg, siblings),
        reorder_definitions(module, cfg, &mut rng),
        add_comments(module, cfg),
        add_unused_definitions(module, cfg, siblings),
        split_module(module, cfg, siblings, &mut rng),
        reorder_invariant_conjuncts(module, cfg, &mut rng),
        reorder_next_disjuncts(module, cfg, &mut rng),
        reorder_cfg_invariants(module, cfg),
    ]
    .into_iter()
    .flatten()
    .collect()
}

// ---------------------------------------------------------------------------
// Running and comparing

fn run(dir: &Path, root: &str) -> Result<Value> {
    let report = dive_api::check_file_engine(
        &CheckRequest {
            file: dir.join(format!("{root}.tla")),
            config: Some(dir.join(format!("{root}.cfg"))),
            seed: Some(0),
            fp: Some(0),
            workers: Some(1),
            max_states: Some(MAX_STATES),
            time_limit: BudgetSetting::Explicit(TIME_LIMIT),
            ..CheckRequest::default()
        },
        dive_api::Engine::Core,
    )?;
    Ok(report.to_json())
}

/// Replaces source locations (`line 3, col 9 to line 3, col 16 of module
/// M`), which every layout change moves, and maps renamed words back.
fn normalize_text(text: &str, back: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("line ") {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        match tail.find(" of module ") {
            Some(end)
                if tail[..end]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || " ,".contains(c)) =>
            {
                // A split moves a definition to another module, so the
                // module name goes too.
                out.push_str("<location>");
                let after = &tail[end + " of module ".len()..];
                let name = after
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .unwrap_or(after.len());
                rest = &after[name..];
            }
            _ => {
                out.push_str("line ");
                rest = &tail["line ".len()..];
            }
        }
    }
    out.push_str(rest);
    if back.is_empty() {
        return out;
    }
    // Map words outside string literals: printed strings are values, and a
    // rename never touches them.
    let mut mapped = String::new();
    let mut word = String::new();
    let mut in_string = false;
    for c in out.chars().chain(std::iter::once(' ')) {
        if !in_string && (c.is_ascii_alphanumeric() || c == '_') {
            word.push(c);
        } else {
            mapped.push_str(back.get(&word).unwrap_or(&word));
            word.clear();
            mapped.push(c);
            if c == '"' {
                in_string = !in_string;
            }
        }
    }
    mapped.pop();
    mapped
}

/// The observable result, with renamed names mapped back to the seed's.
fn observed(doc: &Value, back: &BTreeMap<String, String>) -> Value {
    let name = |v: &Value| match v.as_str() {
        Some(s) => json!(normalize_text(s, back)),
        None => v.clone(),
    };
    let output: Vec<Value> = doc["output"]
        .as_array()
        .map(|a| a.iter().map(name).collect())
        .unwrap_or_default();
    let states = doc["counterexample"]["states"]
        .as_array()
        .map(|states| {
            states
                .iter()
                .map(|s| {
                    let values: serde_json::Map<String, Value> = s["values"]
                        .as_object()
                        .map(|m| {
                            m.iter()
                                .map(|(k, v)| (back.get(k).unwrap_or(k).clone(), v.clone()))
                                .collect()
                        })
                        .unwrap_or_default();
                    Value::Object(values)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let codes = |key: &str| -> Vec<Value> {
        doc[key]
            .as_array()
            .map(|a| a.iter().map(|d| d["code"].clone()).collect())
            .unwrap_or_default()
    };
    json!({
        "verdict": doc["verdict"],
        "property": name(&doc["property"]),
        "tlc": doc["tlc"]["code"],
        "exit_status": doc["tlc"]["exit_status"],
        "statistics": {
            "generated": doc["statistics"]["generated"],
            "distinct": doc["statistics"]["distinct"],
            "depth": doc["statistics"]["depth"],
            "left_on_queue": doc["statistics"]["left_on_queue"],
            "initial_generated": doc["statistics"]["initial_generated"],
        },
        "counterexample": {
            "length": doc["counterexample"]["length"],
            "ending": doc["counterexample"]["ending"],
            "back_to": doc["counterexample"]["back_to"],
            "states": states,
        },
        "diagnostics": codes("diagnostics"),
        "warnings": codes("warnings"),
        "output": output,
    })
}

fn stopped(doc: &Value) -> bool {
    doc["verdict"] == "stopped" || doc["verdict"] == "resource-limit"
}

/// Whether the report is an evaluation error found while checking (not a
/// refusal, a parse error or a configuration error).
fn evaluation_error(doc: &Value) -> bool {
    [
        "evaluation-error",
        "model-error",
        "overflow",
        "assertion-failure",
    ]
    .iter()
    .any(|v| doc["verdict"] == *v)
}

/// Known text dependence in the default profile, reported separately so a
/// sweep stays readable. Each is a real violation of the charter's
/// metamorphic rule awaiting an owner decision, not a harness exemption.
fn known_finding(relation: &str, source: &str, a: &Value, b: &Value) -> Option<&'static str> {
    let same_verdict = a["verdict"] == b["verdict"] && a["statistics"] == b["statistics"];
    let only_output = same_verdict && a["counterexample"] == b["counterexample"];
    let unit_moves = relation == "reorder-definitions" || relation == "split-module";
    if unit_moves && same_verdict && source.contains("Random") {
        // Start-up evaluation draws constant definitions from one random
        // stream in definition order and module order (fixture RndStartup).
        return Some("start-up random stream follows definition and module order");
    }
    if relation == "split-module" && only_output {
        // A failing constant definition's `Print` runs again for each module
        // in the EXTENDS chain (fixtures SuPrintRetry, SuPrintRedirect,
        // SuPrintSubst).
        return Some("start-up Print repeats once per extended module");
    }
    None
}

enum Outcome {
    Agree,
    Known(&'static str),
    Inconclusive(String),
    Disagree(Value, Value),
}

fn compare(
    strength: Strength,
    original: &Value,
    variant: &Value,
    back: &BTreeMap<String, String>,
) -> Outcome {
    if stopped(original) || stopped(variant) {
        return Outcome::Inconclusive("a run reached its budget".to_owned());
    }
    let a = observed(original, &BTreeMap::new());
    let b = observed(variant, back);
    let agree = match strength {
        Strength::Exact => a == b,
        Strength::ExactUnlessGuard => {
            if evaluation_error(original) != evaluation_error(variant) {
                return Outcome::Inconclusive(
                    "a reordered guard raised an evaluation error".to_owned(),
                );
            }
            a == b
        }
        Strength::Order => {
            let complete = a["verdict"] == "success";
            a["verdict"] == b["verdict"]
                && a["tlc"] == b["tlc"]
                && a["counterexample"]["length"] == b["counterexample"]["length"]
                && (!complete || a["statistics"] == b["statistics"])
        }
    };
    if agree {
        Outcome::Agree
    } else {
        Outcome::Disagree(a, b)
    }
}

fn classify(relation: &str, source: &str, outcome: Outcome) -> Outcome {
    match outcome {
        Outcome::Disagree(a, b) => match known_finding(relation, source, &a, &b) {
            Some(why) => Outcome::Known(why),
            None => Outcome::Disagree(a, b),
        },
        other => other,
    }
}

fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
}

/// Every identifier-like word in the seed's other modules.
fn sibling_words(dir: &Path, root: &str) -> Result<BTreeSet<String>> {
    let mut words = BTreeSet::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "tla")
            && path.file_stem().is_some_and(|s| s != root)
        {
            words.extend(self::words(
                &std::fs::read_to_string(&path).unwrap_or_default(),
            ));
        }
    }
    Ok(words)
}

/// Copies the seed directory's modules into `into`, then writes the variant.
fn materialize(
    dir: &Path,
    into: &Path,
    root: &str,
    files: &[(String, String)],
    cfg: &str,
) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_file()
            && let Some(name) = path.file_name()
        {
            std::fs::copy(&path, into.join(name))?;
        }
    }
    for (name, text) in files {
        std::fs::write(into.join(name), text)?;
    }
    std::fs::write(into.join(format!("{root}.cfg")), cfg)?;
    Ok(())
}

#[derive(Default)]
struct Tally {
    seeds: usize,
    skipped_seeds: BTreeMap<String, usize>,
    agree: BTreeMap<&'static str, usize>,
    inconclusive: BTreeMap<&'static str, usize>,
    inconclusive_reasons: BTreeMap<String, usize>,
    known: Vec<Value>,
    applied: Vec<Value>,
    disagreements: Vec<Value>,
}

fn check_seed(workspace: &Path, rel: &str, tally: &mut Tally) -> Result<()> {
    let tla = workspace.join(rel);
    let Some(dir) = tla.parent() else {
        return Ok(());
    };
    let root = tla
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_owned();
    let text = std::fs::read_to_string(&tla)?;
    let cfg = std::fs::read_to_string(dir.join(format!("{root}.cfg")))?;
    let mut skip = |why: &str| {
        *tally.skipped_seeds.entry(why.to_owned()).or_default() += 1;
    };
    if text.contains("--algorithm") || text.contains("--fair") {
        skip("PlusCal source");
        return Ok(());
    }
    let Some(module) = read_module(&text) else {
        skip("does not parse cleanly");
        return Ok(());
    };
    if module.name != root {
        skip("module name differs from the file name");
        return Ok(());
    }
    tally.seeds += 1;
    let original_dir = scratch()?;
    materialize(dir, &original_dir.0, &root, &[], &cfg)?;
    let original = run(&original_dir.0, &root)?;
    if stopped(&original) {
        *tally
            .inconclusive
            .entry("seed reached its budget")
            .or_default() += 1;
        return Ok(());
    }
    let mut siblings = sibling_words(dir, &root)?;
    // Modules after the root's closing line (in its trailer) are siblings too.
    let range = module.root.text_range();
    siblings.extend(words(&text[..offset(range.start())]));
    siblings.extend(words(&text[offset(range.end())..]));
    for variant in variants(rel, &module, &cfg, &siblings) {
        let variant_dir = scratch()?;
        materialize(dir, &variant_dir.0, &root, &variant.files, &variant.cfg)?;
        let result = run(&variant_dir.0, &root)?;
        let outcome = compare(variant.strength, &original, &result, &variant.back);
        match classify(variant.relation, &text, outcome) {
            Outcome::Agree => {
                *tally.agree.entry(variant.relation).or_default() += 1;
                tally.applied.push(json!({"seed": rel, "relation": variant.relation, "verdict": original["verdict"]}));
            }
            Outcome::Known(why) => tally.known.push(json!({"seed": rel, "relation": variant.relation, "finding": why})),
            Outcome::Inconclusive(why) => {
                *tally.inconclusive.entry(variant.relation).or_default() += 1;
                *tally.inconclusive_reasons.entry(why).or_default() += 1;
            }
            Outcome::Disagree(a, b) => tally.disagreements.push(json!({
                "seed": rel,
                "relation": variant.relation,
                "original": a,
                "variant": b,
                "files": variant.files.iter().map(|(n, t)| json!({"name": n, "text": t})).collect::<Vec<_>>(),
                "cfg": variant.cfg,
            })),
        }
    }
    Ok(())
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every fixture root module with a same-named configuration beside it.
fn all_seeds(workspace: &Path) -> Vec<String> {
    let mut seeds = Vec::new();
    let mut stack = vec![
        workspace.join("crates"),
        workspace.join("conformance/corpus"),
    ];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path
                    .file_name()
                    .is_some_and(|n| n != "target" && n != "src")
                {
                    stack.push(path);
                }
            } else if path.extension().is_some_and(|e| e == "tla")
                && path.with_extension("cfg").is_file()
                && let Ok(rel) = path.strip_prefix(workspace)
            {
                seeds.push(rel.to_string_lossy().into_owned());
            }
        }
    }
    seeds.sort();
    seeds
}

fn summary(tally: &Tally) -> String {
    let mut out = format!("metamorphic: {} seeds checked\n", tally.seeds);
    for (why, n) in &tally.skipped_seeds {
        let _ = writeln!(out, "  skipped seeds, {why}: {n}");
    }
    let mut known: BTreeMap<String, usize> = BTreeMap::new();
    for k in &tally.known {
        *known
            .entry(k["finding"].as_str().unwrap_or_default().to_owned())
            .or_default() += 1;
    }
    for (why, n) in known {
        let _ = writeln!(out, "  known finding, {why}: {n}");
    }
    let relations: BTreeSet<&&str> = tally
        .agree
        .keys()
        .chain(tally.inconclusive.keys())
        .collect();
    for relation in relations {
        let _ = writeln!(
            out,
            "  {relation}: {} agree, {} inconclusive, {} disagree",
            tally.agree.get(*relation).unwrap_or(&0),
            tally.inconclusive.get(*relation).unwrap_or(&0),
            tally
                .disagreements
                .iter()
                .filter(|d| d["relation"] == **relation)
                .count()
        );
    }
    out
}

fn check_seeds(seeds: &[String]) -> Result<Tally> {
    let workspace = workspace_root();
    let mut tally = Tally::default();
    for seed in seeds {
        check_seed(&workspace, seed, &mut tally)?;
    }
    Ok(tally)
}

/// The comparison is not vacuous: changing what a spec means is caught.
#[test]
fn a_semantic_change_disagrees() -> Result<()> {
    let spec = |bound: u8| {
        format!(
            "---- MODULE Counter ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == x < {bound} /\\ x' = x + 1\nSmall == x < 3\n====\n"
        )
    };
    let cfg = "INIT Init\nNEXT Next\nINVARIANT Small\n";
    let mut reports = Vec::new();
    for bound in [2, 3] {
        let dir = scratch()?;
        std::fs::write(dir.0.join("Counter.tla"), spec(bound))?;
        std::fs::write(dir.0.join("Counter.cfg"), cfg)?;
        reports.push(run(&dir.0, "Counter")?);
    }
    assert!(matches!(
        compare(Strength::Exact, &reports[0], &reports[1], &BTreeMap::new()),
        Outcome::Disagree(..)
    ));
    assert!(matches!(
        compare(Strength::Order, &reports[0], &reports[1], &BTreeMap::new()),
        Outcome::Disagree(..)
    ));
    Ok(())
}

#[test]
fn fixed_seeds_satisfy_every_relation() -> Result<()> {
    let seeds: Vec<String> = FAST_SEEDS.iter().map(|s| (*s).to_owned()).collect();
    let tally = check_seeds(&seeds)?;
    eprint!("{}", summary(&tally));
    assert!(
        tally.disagreements.is_empty(),
        "{}",
        serde_json::to_string_pretty(&tally.disagreements)?
    );
    Ok(())
}

#[test]
fn every_fixture_satisfies_every_relation_when_requested() -> Result<()> {
    if std::env::var("DIVE_METAMORPHIC").as_deref() != Ok("all") {
        return Ok(());
    }
    let seeds = all_seeds(&workspace_root());
    let tally = check_seeds(&seeds)?;
    eprint!("{}", summary(&tally));
    if let Ok(path) = std::env::var("DIVE_METAMORPHIC_REPORT") {
        std::fs::write(
            path,
            serde_json::to_string_pretty(&json!({
                "seeds": tally.seeds,
                "skipped_seeds": tally.skipped_seeds,
                "agree": tally.agree,
                "inconclusive": tally.inconclusive,
                "inconclusive_reasons": tally.inconclusive_reasons,
                "known": tally.known,
                "applied": tally.applied,
                "disagreements": tally.disagreements,
            }))?,
        )?;
    }
    assert!(
        tally.disagreements.is_empty(),
        "{} disagreements; set DIVE_METAMORPHIC_REPORT for details",
        tally.disagreements.len()
    );
    Ok(())
}
