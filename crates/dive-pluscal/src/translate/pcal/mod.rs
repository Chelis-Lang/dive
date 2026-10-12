//! The PlusCal translator proper: a port of the semantics of the pinned
//! `pcal.trans` (`ParseAlgorithm`, `PcalSymTab`, `PcalFixIDs`,
//! `PcalTranslate` and `PcalTLAGen`). See `docs/provenance.md`.
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
mod labels;
mod macros;
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
}

impl Fail {
    pub(crate) fn new(message: String) -> Self {
        Self {
            message,
            at: None,
            code: Code::Syntax,
        }
    }
    pub(crate) fn at(message: String, at: usize) -> Self {
        Self {
            message,
            at: Some(at),
            code: Code::Syntax,
        }
    }
    pub(crate) fn internal(message: &str) -> Self {
        Self {
            message: message.to_owned(),
            at: None,
            code: Code::Internal,
        }
    }
    /// Reclassifies the failure.
    pub(crate) fn code(mut self, code: Code) -> Self {
        self.code = code;
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
    /// Bound on the nesting of the translated statement tree.
    pub max_depth: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            label: false,
            label_root: "Lbl_".into(),
            fairness: String::new(),
            no_done_disjunct: false,
            wrap_column: 78,
            max_depth: 128,
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
}

/// One translated process.
#[derive(Clone, Debug)]
pub(crate) struct ProcessInfo {
    pub name: String,
    pub is_eq: bool,
    pub local_variables: Vec<String>,
}

/// The translation body and the facts callers report.
#[derive(Clone, Debug)]
pub(crate) struct Output {
    pub lines: Vec<String>,
    /// Labelled actions, in generation order.
    pub actions: Vec<String>,
    pub program_counter: bool,
    pub default_init: bool,
    pub added_labels: Vec<AddedLabel>,
    pub processes: Vec<ProcessInfo>,
    /// Comment lines reporting renamed symbols.
    pub renamed: Vec<String>,
}

/// Translates a parsed algorithm.
pub(crate) fn translate(parsed: &Parsed, settings: &Settings) -> Result<Output, Fail> {
    let mut alg = build::Builder::new(parsed, settings.max_depth).build()?;
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
    )?;
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
    };
    let lines = generate::Gen::new(&options, &st).generate(&alg)?;
    if facts.has_default_initialization {
        st.check_default_init_value()?;
    }
    let mut actions = Vec::new();
    for procedure in &alg.prcds {
        actions.extend(procedure.body.iter().map(|l| l.label.clone()));
    }
    let mut processes = Vec::new();
    match &alg.body {
        ast::Body::Uni { body, .. } => actions.extend(body.iter().map(|l| l.label.clone())),
        ast::Body::Multi(list) => {
            for process in list {
                actions.extend(process.body.iter().map(|l| l.label.clone()));
                processes.push(ProcessInfo {
                    name: process.name.clone(),
                    is_eq: process.is_eq,
                    local_variables: process.decls.iter().map(|d| d.var.clone()).collect(),
                });
            }
        }
    }
    Ok(Output {
        lines,
        actions,
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

/// pcal.trans translates an algorithm that declares a variable, label,
/// procedure or process with a name the translation also defines, and SANY
/// then rejects the module. dive refuses instead (D-008).
fn check_generated_names(alg: &ast::Algorithm) -> Result<(), Fail> {
    let mut names: Vec<&str> = alg.decls.iter().map(|d| d.var.as_str()).collect();
    for procedure in &alg.prcds {
        names.push(&procedure.name);
        names.extend(procedure.params.iter().chain(&procedure.decls).map(|d| d.var.as_str()));
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
