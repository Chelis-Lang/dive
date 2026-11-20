//! Conservative identity of everything a cached transition prefix can observe.
use super::{Miss, miss};
use dive_config::ModelConfig;
use dive_sema::{Analysis, Origin};
use dive_syntax::{SyntaxKind, ast::Unit};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub(crate) struct Identity {
    config: ModelConfig,
    properties: Vec<String>,
    modules: Vec<Module>,
}
#[derive(Debug, PartialEq, Eq)]
struct Module {
    name: String,
    origin: Source,
    fixed: Vec<String>,
    leaves: Vec<(String, String)>,
}
#[derive(Debug, PartialEq, Eq)]
enum Source {
    File(PathBuf),
    Text(String),
    Standard,
}
impl Identity {
    pub(crate) fn new(
        analysis: &Analysis,
        cfg: &ModelConfig,
        temporal: bool,
    ) -> Result<Self, Miss> {
        let mut config = cfg.clone();
        let properties = std::mem::take(if temporal {
            &mut config.properties
        } else {
            &mut config.invariants
        })
        .into_iter()
        .map(|n| n.text)
        .collect();
        let mut allowed = ModelConfig::default();
        allowed.specification.clone_from(&config.specification);
        allowed.init.clone_from(&config.init);
        allowed.next.clone_from(&config.next);
        allowed.constants.clone_from(&config.constants);
        allowed.model_values.clone_from(&config.model_values);
        allowed.check_deadlock.clone_from(&config.check_deadlock);
        if config != allowed
            || !matches!(
                (&config.specification, &config.init, &config.next),
                (Some(_), None, None) | (None, Some(_), Some(_))
            )
            || config
                .constants
                .iter()
                .any(|c| matches!(c, dive_config::ConstantEntry::Substitution { .. }))
        {
            return Err(miss(
                "requires a specification or INIT/NEXT, literal constants and the requested property role",
            ));
        }
        // Count resolved-source identifier spellings across the *entire* closure.
        // A zero-argument root definition whose declaration is its sole occurrence
        // cannot be reached from another TLA+ expression, even through a helper.
        // CFG references are handled separately. An ambiguous occurrence is a miss.
        let mut uses = BTreeMap::<String, usize>::new();
        for file in analysis.files() {
            for token in file
                .parse
                .syntax()
                .descendants_with_tokens()
                .filter_map(dive_syntax::SyntaxElement::into_token)
            {
                if token.kind() == SyntaxKind::IDENT {
                    *uses.entry(token.text().to_owned()).or_default() += 1;
                }
            }
        }
        let root_dir = analysis
            .model()
            .root()
            .and_then(|id| analysis.model().node(id).span)
            .and_then(|span| analysis.file(span.file))
            .and_then(|file| match &file.origin {
                Origin::File(p) => p.parent(),
                _ => None,
            });
        let modules = analysis
            .files()
            .iter()
            .map(|file| Module::new(file, cfg, &uses, root_dir))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            config,
            properties,
            modules,
        })
    }

    pub(crate) fn compatible(&self, other: &Self) -> bool {
        self.config == other.config
            && self.modules.len() == other.modules.len()
            && self.modules.iter().zip(&other.modules).all(|(a, b)| {
                a.name == b.name
                    && a.origin == b.origin
                    && a.fixed == b.fixed
                    && a.leaves.len() == b.leaves.len()
                    && a.leaves.iter().zip(&b.leaves).all(|((n, x), (m, y))| {
                        n == m
                            && (x == y
                                || (self.properties.contains(n) && other.properties.contains(n)))
                    })
            })
    }
}

impl Module {
    fn new(
        file: &dive_sema::SourceFile,
        cfg: &ModelConfig,
        uses: &BTreeMap<String, usize>,
        root_dir: Option<&Path>,
    ) -> Result<Self, Miss> {
        let module = file.parse.module().ok_or_else(|| miss("missing module"))?;
        let name = module.name().ok_or_else(|| miss("unnamed module"))?;
        let origin = match &file.origin {
            Origin::File(p) => Source::File(
                root_dir
                    .and_then(|d| p.strip_prefix(d).ok())
                    .unwrap_or(p)
                    .to_owned(),
            ),
            Origin::Text => Source::Text(file.display.clone()),
            Origin::Standard
                if matches!(
                    name.as_str(),
                    "Naturals" | "Integers" | "FiniteSets" | "Sequences"
                ) =>
            {
                Source::Standard
            }
            _ => return Err(miss("unsupported module origin or standard module")),
        };
        let mut entry = Module {
            name,
            origin,
            fixed: Vec::new(),
            leaves: Vec::new(),
        };
        let mut start = 0;
        if !matches!(entry.origin, Source::Standard) {
            for unit in module.units() {
                let def = match unit {
                    Unit::OperatorDef(def) => def,
                    Unit::ConstantDecl(_)
                    | Unit::VariableDecl(_)
                    | Unit::FunctionDef(_)
                    | Unit::RecursiveDecl(_) => continue,
                    _ => {
                        return Err(miss(
                            "unsupported declaration, assumption or module instantiation",
                        ));
                    }
                };
                let name = def.name().ok_or_else(|| miss("unnamed operator"))?;
                if uses.get(&name) != Some(&1)
                    || !def.parameters().is_empty()
                    || [
                        cfg.init.as_ref(),
                        cfg.next.as_ref(),
                        cfg.specification.as_ref(),
                    ]
                    .into_iter()
                    .flatten()
                    .any(|n| n.text == name)
                    || cfg.constants.iter().any(|c| match c {
                        dive_config::ConstantEntry::Assignment { name: n, .. } => n.text == name,
                        dive_config::ConstantEntry::Substitution { .. } => true,
                    })
                {
                    continue;
                }
                let range = def
                    .body()
                    .ok_or_else(|| miss("missing operator body"))?
                    .syntax()
                    .text_range();
                let (a, b) = (usize::from(range.start()), usize::from(range.end()));
                entry.fixed.push(
                    file.text
                        .get(start..a)
                        .ok_or_else(|| miss("operator range"))?
                        .to_owned(),
                );
                entry.leaves.push((
                    name,
                    file.text
                        .get(a..b)
                        .ok_or_else(|| miss("operator range"))?
                        .to_owned(),
                ));
                start = b;
            }
        }
        entry.fixed.push(
            file.text
                .get(start..)
                .ok_or_else(|| miss("module range"))?
                .to_owned(),
        );
        Ok(entry)
    }
}
