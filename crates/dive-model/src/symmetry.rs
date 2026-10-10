//! Pure symmetry declaration preparation, bounded closure and safety keys.
//!
//! This matches `MVPerms.permutationSubgroup` and `MVPerm.compose` at the
//! pinned TLC revision. Preparation handles supported tool-level warnings;
//! activation and concrete trace retention belong to the checker. Liveness and
//! random symmetry interactions still require separate implementations.
//! Indices refer to one caller-supplied table of model values, not intern tokens.

mod prepare;

pub use prepare::{Prepared, Reduction};

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use dive_eval::{ErrorClass, St};
use dive_ir::{Effects, Level};
use dive_value::{SetKind, Value};

use crate::tables::Look;
use crate::{BindError, Ec, Model, Pred};

/// A pure symmetry declaration before warning, model-value and closure checks.
///
/// This is an intermediate startup stage, not permission to reduce states.
/// `Tool.getSymmetryPerms` validates all function representations before its
/// too-small heuristic; domain/range and closure checks follow that heuristic.
#[derive(Clone, Debug)]
pub struct Declaration {
    /// The configuration's name.
    pub name: String,
    /// The original definition body, evaluated rather than its cached value.
    pub predicate: Pred,
    /// Function records in normalized run order, including identities.
    pub functions: Arc<[Value]>,
}

impl Model {
    /// Reads the configured pure symmetry definition from TLC's pre-constant
    /// name-table snapshot and validates its set/function representations.
    /// Call once during startup, before checking. This does not build the
    /// group, issue too-small warnings or lift any checking refusal.
    ///
    /// # Example
    /// ```
    /// # use std::sync::Arc;
    /// # use dive_model::{BindOptions, bind};
    /// let source = "---- MODULE M ----\nEXTENDS TLC\nCONSTANT S\nVARIABLE x\n\
    ///               Init == x = TRUE\nNext == UNCHANGED x\n\
    ///               Sym == Permutations(S)\n====\n";
    /// let analysis = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
    /// let ir = Arc::new(dive_ir::lower(&analysis).unwrap());
    /// let cfg = dive_config::parse("CONSTANT S={a,b}\nINIT Init\nNEXT Next\nSYMMETRY Sym\n")
    ///     .into_config().unwrap();
    /// let model = bind(ir, &cfg, &BindOptions::tlc()).unwrap();
    /// let declaration = model.symmetry_declaration().unwrap().unwrap();
    /// assert_eq!(declaration.functions.len(), 2);
    /// assert!(!model.unsupported.is_empty()); // reduction remains unsupported
    /// ```
    ///
    /// # Errors
    /// TLC's proved name/type diagnostics; explicit refusals for unverified
    /// evaluation/normalization startup errors, or effectful, nonconstant and
    /// parameterized declarations. Domain/range
    /// validation, warning construction and kernel exceptions are later stages.
    pub fn symmetry_declaration(&self) -> Result<Option<Declaration>, BindError> {
        let Some(name) = self.symmetry.as_ref() else {
            return Ok(None);
        };
        let definition = match &self.symmetry_definition {
            None => {
                return Err(BindError::tlc(
                    Ec::TlcConfigSpecifiedNotDefined,
                    &["symmetry function", name],
                ));
            }
            Some(Look::Def(definition)) => *definition,
            Some(_) => {
                return Err(symmetry_runtime_error(&format!(
                    "The symmetry function {name} must specify a set of permutations."
                )));
            }
        };
        let def = self.ir.def(definition);
        let node = self.ir.node(def.body);
        if !def.params.is_empty()
            || node.level != Level::Constant
            || node.effects.intersects(Effects::IMPURE)
        {
            return Err(BindError::unsupported(format!(
                "effectful, nonconstant or parameterized SYMMETRY {name} at {}",
                self.ir.location(&node.origin)
            )));
        }
        let predicate = Pred::new(
            def.body,
            self.evaluator
                .scope(definition)
                .map_err(|e| BindError::unsupported(e.to_string()))?,
        );
        let value =
            self.eval(&predicate, &St::constant())
                .map_err(|error| match error.class() {
                    ErrorClass::Unsupported | ErrorClass::ResourceLimit => {
                        BindError::unsupported(format!("SYMMETRY {name}: {}", error.message()))
                    }
                    ErrorClass::Bug => {
                        BindError::bug(format!("SYMMETRY {name}: {}", error.message()))
                    }
                    ErrorClass::Eval => BindError::unsupported(format!(
                        "SYMMETRY {name} declaration evaluation error ({}): {}",
                        error.ec().code(),
                        error.message()
                    )),
                })?;
        let Value::Set(set) = value else {
            return Err(symmetry_runtime_error(
                "The symmetry operator must specify a set of functions.",
            ));
        };
        if !matches!(set.kind(), SetKind::Enum | SetKind::Raw) {
            return Err(symmetry_runtime_error(
                "The symmetry operator must specify a set of functions.",
            ));
        }
        let functions = set
            .elements(self.evaluator.options().ctx)
            .map_err(|error| match error.kind() {
                dive_value::ErrorKind::Codec => BindError::bug(format!(
                    "SYMMETRY {name} set normalization: {}",
                    error.message()
                )),
                _ => BindError::unsupported(format!(
                    "SYMMETRY {name} set normalization: {}",
                    error.message()
                )),
            })?;
        if functions.iter().any(|value| match value {
            Value::Fcn(function) => function.is_tuple_literal() || function.is_record_literal(),
            _ => true,
        }) {
            return Err(symmetry_runtime_error(
                "The symmetry values must be function records.",
            ));
        }
        Ok(Some(Declaration {
            name: name.clone(),
            predicate,
            functions,
        }))
    }
}

fn symmetry_runtime_error(message: &str) -> BindError {
    BindError::tlc(
        Ec::General,
        &[&format!(
            "TLC threw an unexpected exception.\nThis was probably caused by an error in the spec or model.\nSee the User Output or TLC Console for clues to what happened.\nThe exception was a java.lang.RuntimeException\n: {message}"
        )],
    )
}

/// Validation of a declared symmetry function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Require a bijection after extending omitted model values by identity.
    Checked,
    /// Preserve TLC's acceptance of non-bijective functions.
    Tlc,
}

/// Hard resource bounds. A refusal never returns a partial closure.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Maximum model-value universe size.
    pub model_values: usize,
    /// Maximum retained nonidentity maps.
    pub maps: usize,
    /// Maximum retained dense cells plus one working row.
    pub cells: usize,
    /// Maximum input-pair, dense-row and composition work units.
    pub work: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            model_values: 4096,
            maps: 4096,
            cells: 1_000_000,
            work: 32_000_000,
        }
    }
}

/// An invalid declaration or an exhausted resource bound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// TLC's closure kernel requires at least two supplied functions.
    TooFewGenerators,
    /// A domain or range index is outside the supplied universe.
    UnknownModelValue,
    /// The supplied function has two entries for one domain element.
    DuplicateDomain,
    /// The total map, including omitted identities, is not bijective.
    NonBijective,
    /// The named resource bound was exceeded.
    Limit(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewGenerators => {
                f.write_str("symmetry generator set has fewer than two functions")
            }
            Self::UnknownModelValue => f.write_str("symmetry index outside model-value universe"),
            Self::DuplicateDomain => f.write_str("duplicate symmetry function domain"),
            Self::NonBijective => f.write_str("symmetry function is not bijective"),
            Self::Limit(name) => write!(f, "symmetry resource limit: {name}"),
        }
    }
}

impl std::error::Error for Error {}

/// A total map of the model-value universe, with implicit identities made explicit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Permutation {
    images: Arc<Vec<usize>>,
}

impl Permutation {
    /// The image of a model-value index, including identity images.
    #[must_use]
    pub fn image(&self, index: usize) -> Option<usize> {
        self.images.get(index).copied()
    }

    /// Images in the caller's model-value order.
    #[must_use]
    pub fn images(&self) -> &[usize] {
        self.images.as_slice()
    }
}

/// Nonidentity maps in TLC's insertion order.
///
/// In [`Mode::Tlc`], malformed generators can produce a finite semigroup
/// rather than a group.
#[derive(Clone, Debug)]
pub struct Group {
    maps: Vec<Permutation>,
}

impl Group {
    /// Closes normalized generator functions under TLC's composition order.
    /// Generators are consumed in the supplied order; identities and duplicates
    /// are dropped. Composing `a` with `b` applies `a` first, then `b`.
    ///
    /// Omitted domain indices map to themselves. Each supplied function must
    /// have unique domain entries even in TLC mode: the input represents an
    /// already normalized function. The caller must enumerate the source set
    /// in the run's value order before calling this kernel.
    ///
    /// # Example
    ///
    /// ```
    /// use dive_model::symmetry::{Group, Limits, Mode};
    /// let generators = vec![vec![(0, 1), (1, 0)], vec![(2, 3), (3, 2)]];
    /// let group = Group::from_generators(4, &generators, Mode::Checked, Limits::default())?;
    /// // Two independent swaps generate three nonidentity maps.
    /// assert_eq!(group.maps().len(), 3);
    /// assert_eq!(group.maps()[2].images(), [1, 0, 3, 2]);
    /// # Ok::<(), dive_model::symmetry::Error>(())
    /// ```
    ///
    /// # Errors
    /// Fewer than two supplied functions, invalid indices, duplicate domains,
    /// non-bijections in checked mode, or
    /// a named resource refusal. No partial group is exposed on failure.
    pub fn from_generators(
        universe: usize,
        generators: &[Vec<(usize, usize)>],
        mode: Mode,
        limits: Limits,
    ) -> Result<Self, Error> {
        if generators.len() < 2 {
            return Err(Error::TooFewGenerators);
        }
        if universe > limits.model_values {
            return Err(Error::Limit("model values"));
        }
        let mut builder = Builder {
            universe,
            limits,
            work: 0,
            maps: Vec::new(),
            seen: HashSet::new(),
        };
        for generator in generators {
            builder.charge(1)?;
            builder.scratch()?;
            builder.charge(universe)?;
            let mut row: Vec<usize> = (0..universe).collect();
            // Input-domain validation is temporary O(universe) metadata.
            let mut assigned = HashSet::new();
            for &(domain, range) in generator {
                builder.charge(1)?;
                if domain >= universe || range >= universe {
                    return Err(Error::UnknownModelValue);
                }
                if !assigned.insert(domain) {
                    return Err(Error::DuplicateDomain);
                }
                row[domain] = range;
            }
            if mode == Mode::Checked {
                builder.charge(universe)?;
                let images: HashSet<usize> = row.iter().copied().collect();
                if images.len() != universe {
                    return Err(Error::NonBijective);
                }
            }
            builder.insert(row)?;
        }
        let generators = builder.maps.len();
        let mut begin = 0;
        loop {
            let end = builder.maps.len();
            for i in 0..generators {
                for j in begin..end {
                    builder.charge(1)?;
                    builder.scratch()?;
                    builder.charge(universe)?;
                    let first = &builder.maps[i].images;
                    let second = &builder.maps[j].images;
                    let row = first.iter().map(|&image| second[image]).collect();
                    builder.insert(row)?;
                }
            }
            if end == builder.maps.len() {
                break;
            }
            begin = end;
        }
        Ok(Self { maps: builder.maps })
    }

    /// Nonidentity maps. Identity remains the canonicalization's initial candidate.
    #[must_use]
    pub fn maps(&self) -> &[Permutation] {
        &self.maps
    }
}

struct Builder {
    universe: usize,
    limits: Limits,
    work: usize,
    maps: Vec<Permutation>,
    seen: HashSet<Arc<Vec<usize>>>,
}

impl Builder {
    fn charge(&mut self, amount: usize) -> Result<(), Error> {
        self.work = self.work.checked_add(amount).ok_or(Error::Limit("work"))?;
        if self.work > self.limits.work {
            return Err(Error::Limit("work"));
        }
        Ok(())
    }

    fn scratch(&self) -> Result<(), Error> {
        let cells = self
            .maps
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_mul(self.universe))
            .ok_or(Error::Limit("cells"))?;
        if cells > self.limits.cells {
            return Err(Error::Limit("cells"));
        }
        Ok(())
    }

    fn insert(&mut self, row: Vec<usize>) -> Result<(), Error> {
        self.charge(self.universe)?;
        if row.iter().enumerate().all(|(i, &value)| i == value) || self.seen.contains(&row) {
            return Ok(());
        }
        if self.maps.len() >= self.limits.maps {
            return Err(Error::Limit("maps"));
        }
        // Move the vector into an Arc without copying its dense row.
        let images = Arc::new(row);
        self.seen.insert(images.clone());
        self.maps.push(Permutation { images });
        Ok(())
    }
}
