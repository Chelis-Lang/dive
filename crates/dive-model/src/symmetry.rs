//! Bounded closure of model-value symmetry generators (ADR 0019, S1).
//!
//! This matches `MVPerms.permutationSubgroup` and `MVPerm.compose` at the
//! pinned TLC revision. It does not enable reduction: model binding, warnings,
//! canonical state keys and independent symmetric trace checks remain separate.
//! Indices refer to one caller-supplied table of model values, not intern tokens.

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

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
