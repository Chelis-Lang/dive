//! The typed model configuration.

use crate::value::{ConfigValue, Spanned};
use dive_syntax::TextRange;
use std::collections::BTreeMap;

/// A name as written in the cfg: the token's exact text and its span.
///
/// TLC looks names up by their text, so a name is not necessarily an
/// identifier: `INVARIANT A, B` names `A`, `,` and `B`. For a constant
/// written `A!B!C`, the text is `A!B!C` and the span covers all of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    /// The text TLC uses.
    pub text: String,
    /// Its bytes in the cfg file.
    pub span: TextRange,
}

/// One entry of a `CONSTANT`/`CONSTANTS` section, in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstantEntry {
    /// `C = v`, `F(a1, ..., an) = v`, or the module-scoped `C = [M] v`.
    Assignment {
        /// The constant (or constant operator) being assigned.
        name: Name,
        /// Arguments of a constant-operator entry such as `F(1, 2) = 3`.
        args: Vec<Spanned<ConfigValue>>,
        /// `M` in `C = [M] v`: the assignment applies in module `M` only.
        module: Option<Name>,
        /// The value.
        value: Spanned<ConfigValue>,
    },
    /// `C <- D` (replace `C` by definition `D`), or `C <- [M] D`, which
    /// replaces `C` only in module `M`.
    Substitution {
        /// The constant or definition being replaced.
        name: Name,
        /// `M` in `C <- [M] D`.
        module: Option<Name>,
        /// The definition that replaces it.
        target: Name,
    },
}

/// A parsed TLC model configuration.
///
/// Fields mirror TLC's `ModelConfig` tables. Section keywords that TLC
/// treats as synonyms share one field (`INVARIANT`/`INVARIANTS`), and
/// repeated list sections append. Single-valued keywords may appear once.
///
/// Whether the names exist in the specification, have the right arity and
/// level, and whether `SPECIFICATION` conflicts with `INIT`/`NEXT`, is
/// checked later by TLC's `SpecProcessor`, not by the cfg reader. Those
/// checks belong to semantic analysis and the checker.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ModelConfig {
    /// `INIT`.
    pub init: Option<Name>,
    /// `NEXT`.
    pub next: Option<Name>,
    /// `SPECIFICATION`.
    pub specification: Option<Name>,
    /// `VIEW`.
    pub view: Option<Name>,
    /// `SYMMETRY`.
    pub symmetry: Option<Name>,
    /// `ALIAS`.
    pub alias: Option<Name>,
    /// `_PERIODIC`: TLC's undocumented periodic-evaluation hook.
    pub periodic: Option<Name>,
    /// `_RL_REWARD`: TLC's undocumented reinforcement-learning reward hook.
    pub rl_reward: Option<Name>,
    /// `CHECK_DEADLOCK TRUE|FALSE`. Absent means TLC's default (check).
    pub check_deadlock: Option<Spanned<bool>>,
    /// `INVARIANT` and `INVARIANTS`.
    pub invariants: Vec<Name>,
    /// `PROPERTY` and `PROPERTIES`.
    pub properties: Vec<Name>,
    /// `CONSTRAINT` and `CONSTRAINTS`.
    pub constraints: Vec<Name>,
    /// `ACTION_CONSTRAINT` and `ACTION_CONSTRAINTS`.
    pub action_constraints: Vec<Name>,
    /// `POSTCONDITION` and `POSTCONDITIONS`.
    pub postconditions: Vec<Name>,
    /// `_POSSIBLE`: TLC's provisional reachability assertions.
    pub possible: Vec<Name>,
    /// Every `CONSTANT`/`CONSTANTS` entry, in source order.
    pub constants: Vec<ConstantEntry>,
    /// Model value names in the order TLC creates them (first occurrence
    /// while parsing values, including operator arguments and set
    /// elements). TLC numbers model values in this order.
    pub model_values: Vec<String>,
}

/// An assignment entry, as returned by [`ModelConfig::assignments`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Assignment<'a> {
    /// The constant.
    pub name: &'a Name,
    /// Operator arguments, empty for a plain constant.
    pub args: &'a [Spanned<ConfigValue>],
    /// The value.
    pub value: &'a Spanned<ConfigValue>,
}

impl ModelConfig {
    /// Whether TLC checks for deadlock under this configuration, before
    /// command-line flags: `CHECK_DEADLOCK FALSE` turns it off, and TLC's
    /// `-deadlock` flag also turns it off.
    #[must_use]
    pub fn checks_deadlock(&self) -> bool {
        self.check_deadlock.as_ref().is_none_or(|s| s.value)
    }

    /// Assignments to the root module's constants, in source order,
    /// duplicates included (TLC's `getConstants()`).
    pub fn assignments(&self) -> impl Iterator<Item = Assignment<'_>> {
        self.constants.iter().filter_map(|e| match e {
            ConstantEntry::Assignment {
                name,
                args,
                module: None,
                value,
            } => Some(Assignment { name, args, value }),
            _ => None,
        })
    }

    /// Module-scoped assignments `C = [M] v`, grouped by module, each group
    /// in source order (TLC's `getModConstants()`).
    #[must_use]
    pub fn module_assignments(&self) -> BTreeMap<&str, Vec<Assignment<'_>>> {
        let mut out: BTreeMap<&str, Vec<Assignment<'_>>> = BTreeMap::new();
        for e in &self.constants {
            if let ConstantEntry::Assignment {
                name,
                args,
                module: Some(m),
                value,
            } = e
            {
                out.entry(m.text.as_str())
                    .or_default()
                    .push(Assignment { name, args, value });
            }
        }
        out
    }

    /// Root-module substitutions `C <- D`, keyed by `C`. A later entry for
    /// the same name replaces an earlier one, as in TLC's `getOverrides()`.
    #[must_use]
    pub fn substitutions(&self) -> BTreeMap<&str, &Name> {
        let mut out = BTreeMap::new();
        for e in &self.constants {
            if let ConstantEntry::Substitution {
                name,
                module: None,
                target,
            } = e
            {
                out.insert(name.text.as_str(), target);
            }
        }
        out
    }

    /// Module-scoped substitutions `C <- [M] D`, by module and then by `C`,
    /// later entries replacing earlier ones (TLC's `getModOverrides()`).
    #[must_use]
    pub fn module_substitutions(&self) -> BTreeMap<&str, BTreeMap<&str, &Name>> {
        let mut out: BTreeMap<&str, BTreeMap<&str, &Name>> = BTreeMap::new();
        for e in &self.constants {
            if let ConstantEntry::Substitution {
                name,
                module: Some(m),
                target,
            } = e
            {
                out.entry(m.text.as_str())
                    .or_default()
                    .insert(name.text.as_str(), target);
            }
        }
        out
    }
}
