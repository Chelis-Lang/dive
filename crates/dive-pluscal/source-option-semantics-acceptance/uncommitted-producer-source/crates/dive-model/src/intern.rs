//! The run's name table, fed in TLC's encounter order (ADR 0012).
//!
//! TLC orders strings, model values and record fields by their token in
//! `util.UniqueString`'s table, and fingerprints a model value by its token.
//! Measured on the pinned TLC (`acceptance/ModelDump.java` reads the table
//! after TLC has processed the model), the table is filled in this order:
//!
//! 1. the configuration's values, strings and model values alike, in the order
//!    TLC's `ModelConfig.parse` reads them (it runs before SANY). Token 0 is
//!    never used, so the first value has token 1;
//! 2. a fixed set of names SANY creates before it parses (`PREFIX` in `tlc_names.rs`);
//! 3. the image of every token SANY's lexer reads, file by file in parse
//!    order, with SANY's syntax-node kind names (`NODE_KINDS`) after the
//!    root module's header line ([`dive_ir::Ir::lexical_names`]);
//! 4. names SANY's semantic analysis creates: a few internal names, then the
//!    contents of string literals as it builds their `StringNode`s, module by
//!    module in semantic-analysis order and in source order within a module
//!    ([`semantic_strings`]);
//! 5. strings and names created while TLC runs.
//!
//! Model values come first, so their tokens are TLC's exactly. The relative
//! order of every other name is TLC's as long as SANY does not intern it
//! under a different image (some operator forms) or in a step not listed
//! here; `acceptance/README.md` records how close the feed is on the corpus.

use dive_config::{ConfigValue, ConstantEntry, ModelConfig};
use dive_ir::Ir;
use dive_value::Interner;

use crate::tlc_names::{NODE_KINDS, PREFIX};

/// A name TLC never interns, standing for its unused token 0.
const TOKEN_ZERO: &str = "\u{0}dive: token 0\u{0}";

/// Names SANY's semantic analysis interns after parsing, before TLC
/// processes the constants.
const SEMANTIC: &[&str] = &["$$InAssume", "nullODN", "nullOpArg", "nullLabelNode"];

/// The name table for a run: everything TLC interns before it evaluates
/// anything.
pub(crate) fn feed(ir: &Ir, cfg: &ModelConfig) -> Interner {
    let names = Interner::new();
    names.intern(TOKEN_ZERO);
    for e in &cfg.constants {
        if let ConstantEntry::Assignment { args, value, .. } = e {
            for a in args {
                cfg_value(&names, &a.value);
            }
            cfg_value(&names, &value.value);
        }
    }
    for p in PREFIX {
        names.intern(p);
    }
    let lexical = ir.lexical_names();
    // The root module's header line is its first three tokens (`---- MODULE`,
    // the name, `----`) in TLC's lexer; dive's lexer may split the first.
    let header = header_len(lexical);
    for n in &lexical[..header] {
        names.intern(n);
    }
    for k in NODE_KINDS {
        names.intern(k);
    }
    for n in &lexical[header..] {
        names.intern(n);
    }
    for s in SEMANTIC {
        names.intern(s);
    }
    for s in semantic_strings(ir) {
        names.intern(&s);
    }
    names
}

/// The contents of every string literal, in the order SANY's semantic
/// analysis creates their `StringNode`s (each interns its contents): the
/// file-level modules in semantic-analysis order (the IR's module order),
/// and within each file in source order (inner modules are analysed where
/// they appear).
#[must_use]
pub fn semantic_strings(ir: &Ir) -> Vec<Box<str>> {
    let rank: std::collections::HashMap<dive_ir::FileId, usize> = ir
        .modules()
        .iter()
        .filter(|m| m.nesting == 0)
        .filter_map(|m| m.file)
        .enumerate()
        .map(|(i, f)| (f, i))
        .collect();
    let mut found: Vec<(usize, u32, &str)> = ir
        .nodes()
        .iter()
        .filter_map(|n| match &n.expr {
            dive_ir::Expr::Str(s) => {
                let f = n.origin.file?;
                Some((*rank.get(&f)?, n.origin.range.0, &**s))
            }
            _ => None,
        })
        .collect();
    found.sort_by_key(|&(r, at, _)| (r, at));
    let mut seen = std::collections::HashSet::new();
    found
        .into_iter()
        .filter(|(_, _, s)| seen.insert(*s))
        .map(|(_, _, s)| s.into())
        .collect()
}

/// How many of the first lexical names make up the root module's header.
fn header_len(lexical: &[Box<str>]) -> usize {
    // TLC's lexer reads `---- MODULE` as one token; dive's may read the dashes
    // and `MODULE` apart. The header ends at the second run of dashes.
    let mut dashes = 0;
    for (i, n) in lexical.iter().enumerate() {
        if n.starts_with("----") {
            dashes += 1;
            if dashes == 2 {
                return i + 1;
            }
        }
    }
    0
}

fn cfg_value(names: &Interner, v: &ConfigValue) {
    match v {
        ConfigValue::String(s) | ConfigValue::ModelValue(s) => {
            names.intern(s);
        }
        ConfigValue::Set(items) => {
            for i in items {
                cfg_value(names, &i.value);
            }
        }
        ConfigValue::Int(_) | ConfigValue::Bool(_) => {}
    }
}
