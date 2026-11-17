//! The PlusCal translator proper: a port of the semantics of the pinned
//! tlaplus/tlaplus `pcal.trans` (MIT), ported from tlaplus at `94d0c50`
//! (`ParseAlgorithm`, `PcalSymTab`, `PcalFixIDs`, `PcalTranslate` and
//! `PcalTLAGen`). The upstream MIT copyright and permission notice is retained in this directory's `LICENSE`.
//!
//! Stages, in the reference order:
//! 1. [`build`]: the parsed source becomes the translator's tree; macro
//!    definitions are expanded in terms of earlier macros.
//! 2. [`labels`]: macro calls are expanded, missing labels inserted and
//!    statements grouped into labelled steps.
//! 3. [`symtab`]: names are disambiguated and rewritten.
//! 4. [`explode`]: steps are flattened, with `pc` and `stack` control flow.
//! 5. [`generate`]: TLA+ is generated.

mod ast;
mod build;
mod explode;
mod expr;
mod generate;
mod header;
mod labels;
mod macros;
mod original;

pub(super) use original::{
    debug_header as serialize_debug_header, serialize as serialize_original,
};
mod symtab;

use crate::{Code, Parsed};

/// A translation failure: the reference's message, and where it applies.
#[derive(Clone, Debug)]
pub(crate) struct Fail {
    pub message: String,
    /// Source byte offset, when known.
    pub at: Option<usize>,
    /// Diagnostic class; [`Code::Internal`] marks an invariant failure.
    pub code: Code,
    /// Failure during bounded text generation rather than tree normalization.
    pub in_generation: bool,
}

impl Fail {
    pub(crate) fn new(message: String) -> Self {
        Self {
            message,
            at: None,
            code: Code::Syntax,
            in_generation: false,
        }
    }
    pub(crate) fn at(message: String, at: usize) -> Self {
        Self {
            message,
            at: Some(at),
            code: Code::Syntax,
            in_generation: false,
        }
    }
    pub(crate) fn internal(message: &str) -> Self {
        Self {
            message: message.to_owned(),
            at: None,
            code: Code::Internal,
            in_generation: false,
        }
    }
    /// Reclassifies the failure.
    pub(crate) fn code(mut self, code: Code) -> Self {
        self.code = code;
        self
    }
    pub(crate) fn generation(mut self) -> Self {
        self.in_generation = true;
        self
    }
}

/// Reference translator settings (`PcalParams`).
#[derive(Clone, Debug)]
pub(crate) struct Settings {
    /// `-label`.
    pub label: bool,
    /// `-labelRoot`.
    pub label_root: String,
    /// `""`, `"nof"`, `"wf"`, `"sf"` or `"wfNext"`.
    pub fairness: String,
    /// `-noDoneDisjunct`.
    pub no_done_disjunct: bool,
    /// `-lineWidth` (default 78).
    pub wrap_column: usize,
    /// Input language weight; versions before 1500 disable pc optimizations.
    pub version_weight: i32,
    /// Bound on the nesting of the translated statement tree.
    pub max_depth: usize,
    /// Bound on the number of actions before generation.
    pub max_actions: usize,
    /// Maximum generated text bytes, checked during construction.
    pub max_output_bytes: usize,
    /// Pre-allocation bounds for macro expansion.
    pub max_macro_nodes: usize,
    pub max_macro_tokens: usize,
    pub max_macro_bytes: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            label: false,
            label_root: "Lbl_".into(),
            fairness: String::new(),
            no_done_disjunct: false,
            wrap_column: 78,
            version_weight: 1903,
            max_depth: 128,
            max_actions: 10_000,
            max_output_bytes: 4_194_304,
            max_macro_nodes: 20_000,
            max_macro_tokens: 400_000,
            max_macro_bytes: 4_194_304,
        }
    }
}

/// An inserted label with its statement location.
#[derive(Clone, Debug)]
pub(crate) struct AddedLabel {
    pub name: String,
    pub line: usize,
    pub column: usize,
    pub at: usize,
    pub macro_call: Option<(usize, usize)>,
}

/// One translated process.
#[derive(Clone, Debug)]
pub(crate) struct ProcessInfo {
    pub name: String,
    pub local_variables: Vec<String>,
}

/// A generated action's exact byte range paired with its retained source owner.
#[derive(Clone, Debug)]
pub(crate) struct EmissionMap {
    pub generated: crate::ast::Span,
    pub original: ast::Loc,
}

/// The translation body and the facts callers report.
#[derive(Clone, Debug)]
pub(crate) struct Output {
    pub lines: Vec<String>,
    /// Labelled actions, in generation order.
    pub actions: Vec<String>,
    pub mappings: Vec<EmissionMap>,
    pub program_counter: bool,
    pub default_init: bool,
    pub added_labels: Vec<AddedLabel>,
    pub processes: Vec<ProcessInfo>,
    /// Comment lines reporting renamed symbols.
    pub renamed: Vec<String>,
}

/// Translates a parsed algorithm.
#[allow(
    clippy::too_many_lines,
    reason = "ordered translation stages and output metadata"
)]
pub(crate) fn translate(parsed: &Parsed, settings: &Settings) -> Result<Output, Fail> {
    let mut budget = macros::Budget::new(
        settings.max_macro_nodes,
        settings.max_macro_tokens,
        settings.max_macro_bytes,
        settings.max_depth,
    );
    let mut alg = build::Builder::new(parsed, settings.max_depth, &mut budget).build()?;
    if settings.version_weight < 1500 {
        alg.facts.omit_pc = false;
        alg.facts.omit_stuttering_when_done = false;
    }
    let mut fairness = settings.fairness.clone();
    match &mut alg.body {
        ast::Body::Multi(processes) => {
            for process in processes {
                if process.fairness == ast::Fairness::Unfair {
                    process.fairness = match fairness.as_str() {
                        "wf" => ast::Fairness::Weak,
                        "sf" => ast::Fairness::Strong,
                        _ => ast::Fairness::Unfair,
                    };
                }
            }
        }
        ast::Body::Uni { .. } => {
            if alg.fair {
                fairness = "wf".into();
            }
        }
    }
    let fair_algorithm = alg.fair;
    labels::normalize(
        &mut alg,
        &labels::LabelOptions {
            label: settings.label,
            root: &settings.label_root,
        },
        &mut budget,
    )?;
    check_normalized_depth(&alg, settings.max_depth)?;
    let mut st = symtab::SymTab::new(&alg)?;
    st.disambiguate();
    symtab::fix(&mut alg, &mut st)?;
    let procs: Vec<explode::ProcInfo> = alg
        .prcds
        .iter()
        .map(|p| explode::ProcInfo {
            name: p.name.clone(),
            params: p.params.clone(),
            decls: p.decls.clone(),
            initial_pc: st
                .find_proc(&p.name)
                .map(|e| e.initial_pc.clone())
                .unwrap_or_default(),
        })
        .collect();
    let facts = alg.facts.clone();
    explode::Exploder::new(&procs, facts.omit_pc).explode(&mut alg)?;
    check_generated_names(&alg)?;
    let options = generate::GenOptions {
        fairness,
        fair_algorithm,
        no_done_disjunct: settings.no_done_disjunct,
        wrap_column: settings.wrap_column,
        omit_pc: facts.omit_pc,
        omit_stuttering_when_done: facts.omit_stuttering_when_done,
        has_default_initialization: facts.has_default_initialization,
        max_output_bytes: settings.max_output_bytes,
        source_at: parsed.algorithm.span.start,
    };
    if facts.has_default_initialization {
        st.check_default_init_value()?;
    }
    let mut actions = Vec::new();
    for procedure in &alg.prcds {
        actions.extend(procedure.body.iter().map(|l| l.label.clone()));
    }
    let mut processes = Vec::new();
    match &alg.body {
        ast::Body::Uni { body, .. } => {
            actions.extend(body.iter().map(|l| l.label.clone()));
        }
        ast::Body::Multi(list) => {
            for process in list {
                actions.extend(process.body.iter().map(|l| l.label.clone()));
                processes.push(ProcessInfo {
                    name: process.name.clone(),
                    local_variables: process.decls.iter().map(|d| d.var.clone()).collect(),
                });
            }
        }
    }
    if actions.len() > settings.max_actions {
        return Err(Fail::at(
            "PlusCal labelled-action limit reached".into(),
            parsed.algorithm.span.start,
        )
        .code(Code::Resource));
    }
    let generated = generate::Gen::new(&options, &st).generate(&alg)?;
    Ok(Output {
        lines: generated.lines,
        actions,
        mappings: generated.mappings,
        program_counter: !facts.omit_pc,
        default_init: facts.has_default_initialization,
        added_labels: facts
            .added_labels
            .iter()
            .map(|(name, loc, at)| AddedLabel {
                name: name.clone(),
                line: loc.line,
                column: loc.col,
                at: *at,
                macro_call: loc.macro_call,
            })
            .collect(),
        processes,
        renamed: st.report.clone(),
    })
}

/// Names the translation itself defines.
const GENERATED: &[&str] = &[
    "Init",
    "Next",
    "Spec",
    "vars",
    "ProcSet",
    "Termination",
    "Terminating",
];

/// Checks the expanded, grouped tree without recursing beyond the user bound.
fn check_normalized_depth(alg: &ast::Algorithm, limit: usize) -> Result<(), Fail> {
    let mut pending = Vec::new();
    for mac in &alg.macros {
        pending.extend(mac.body.iter().map(|s| (s, 1usize)));
    }
    for procedure in &alg.prcds {
        for labeled in &procedure.body {
            pending.extend(labeled.stmts.iter().map(|s| (s, 1usize)));
        }
    }
    match &alg.body {
        ast::Body::Uni { body, .. } => {
            for labeled in body {
                pending.extend(labeled.stmts.iter().map(|s| (s, 1usize)));
            }
        }
        ast::Body::Multi(processes) => {
            for process in processes {
                for labeled in &process.body {
                    pending.extend(labeled.stmts.iter().map(|s| (s, 1usize)));
                }
            }
        }
    }
    while let Some((stmt, depth)) = pending.pop() {
        if depth > limit {
            return Err(Fail::at(
                "PlusCal normalized control-flow depth limit reached".into(),
                stmt.at,
            )
            .code(Code::Resource));
        }
        let next = depth.saturating_add(1);
        match &stmt.kind {
            ast::Kind::If { then, els, .. } => {
                pending.extend(then.iter().chain(els).map(|s| (s, next)));
            }
            ast::Kind::LabelIf {
                unlab_then,
                lab_then,
                unlab_else,
                lab_else,
                ..
            } => {
                pending.extend(unlab_then.iter().chain(unlab_else).map(|s| (s, next)));
                pending.extend(
                    lab_then
                        .iter()
                        .chain(lab_else)
                        .flat_map(|l| &l.stmts)
                        .map(|s| (s, next)),
                );
            }
            ast::Kind::Either(choices) => {
                pending.extend(choices.iter().flatten().map(|s| (s, next)));
            }
            ast::Kind::LabelEither(clauses) => {
                for clause in clauses {
                    pending.extend(clause.unlab.iter().map(|s| (s, next)));
                    pending.extend(clause.lab.iter().flat_map(|l| &l.stmts).map(|s| (s, next)));
                }
            }
            ast::Kind::With { body, .. } => pending.extend(body.iter().map(|s| (s, next))),
            ast::Kind::While { unlab, lab, .. } => {
                pending.extend(unlab.iter().map(|s| (s, next)));
                pending.extend(lab.iter().flat_map(|l| &l.stmts).map(|s| (s, next)));
            }
            _ => {}
        }
    }
    Ok(())
}

/// pcal.trans translates an algorithm that declares a variable, label,
/// procedure or process with a name the translation also defines, and SANY
/// then rejects the module. dive refuses the algorithm instead, naming the
/// clashing name.
fn check_generated_names(alg: &ast::Algorithm) -> Result<(), Fail> {
    let mut names: Vec<&str> = alg.decls.iter().map(|d| d.var.as_str()).collect();
    for procedure in &alg.prcds {
        names.push(&procedure.name);
        names.extend(
            procedure
                .params
                .iter()
                .chain(&procedure.decls)
                .map(|d| d.var.as_str()),
        );
        names.extend(procedure.body.iter().map(|l| l.label.as_str()));
    }
    match &alg.body {
        ast::Body::Uni { body, .. } => names.extend(body.iter().map(|l| l.label.as_str())),
        ast::Body::Multi(processes) => {
            for process in processes {
                names.push(&process.name);
                names.extend(process.decls.iter().map(|d| d.var.as_str()));
                names.extend(process.body.iter().map(|l| l.label.as_str()));
            }
        }
    }
    match names.iter().find(|n| GENERATED.contains(n)) {
        Some(name) => Err(Fail::new(format!(
            "unsupported PlusCal translation: `{name}' is also defined by the translation (pcal.trans emits a module SANY rejects)"
        ))
        .code(Code::Unsupported)),
        None => Ok(()),
    }
}
