//! Whole-run policy for declarations combined with genuine temporal obligations.

/// How a configured `SYMMETRY` interacts with genuine temporal properties.
/// The TLC-equivalent request is [`Self::Tlc`]; its quotient checking remains
/// unsupported. Safety-only roles retain their existing reduction in every mode.
///
/// ```
/// use dive_model::SymmetryLiveness;
/// assert_eq!(SymmetryLiveness::Unreduced.as_str(), "unreduced");
/// assert_eq!(SymmetryLiveness::Tlc.as_str(), "tlc");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymmetryLiveness {
    /// Disable reduction for the entire run that checks temporal properties.
    Unreduced,
    /// Refuse the combination before graph capture.
    Refuse,
    /// Request TLC's quotient semantics; currently a named refusal.
    Tlc,
}
impl SymmetryLiveness {
    /// Stable CLI/report spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unreduced => "unreduced",
            Self::Refuse => "refuse",
            Self::Tlc => "tlc",
        }
    }
}

/// A policy resolved after successful binding of the actual configured roles.
/// Presence is not evidence of exploration or completion. The declaration is
/// retained even when the whole temporal run ignores its reduction.
///
/// ```
/// # fn example(model: &dive_model::Model) {
/// let policy = model.symmetry_liveness();
/// if policy.unreduced() {
///     assert!(policy.declared().is_some());
///     assert!(policy.genuine_temporal());
///     assert_eq!(policy.effective(), "unreduced");
/// }
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymmetryLivenessResolution {
    requested: SymmetryLiveness,
    declared: Option<String>,
    temporal: bool,
}
impl SymmetryLivenessResolution {
    pub(crate) fn new(
        requested: SymmetryLiveness,
        declared: Option<String>,
        temporal: bool,
    ) -> Self {
        Self {
            requested,
            declared,
            temporal,
        }
    }
    /// The requested policy, independent of whether it applies.
    #[must_use]
    pub const fn requested(&self) -> SymmetryLiveness {
        self.requested
    }
    /// The original configured declaration, never erased by this policy.
    #[must_use]
    pub fn declared(&self) -> Option<&str> {
        self.declared.as_deref()
    }
    /// Whether actual bound roles include at least one temporal property.
    #[must_use]
    pub const fn genuine_temporal(&self) -> bool {
        self.temporal
    }
    /// Whether reduction is disabled for the entire temporal check.
    #[must_use]
    pub fn unreduced(&self) -> bool {
        self.declared.is_some() && self.temporal && self.requested == SymmetryLiveness::Unreduced
    }
    /// Stable effective policy; `not-applicable` says nothing about safety
    /// reduction, which is prepared separately by an ordinary safety checker.
    #[must_use]
    pub fn effective(&self) -> &'static str {
        if self.declared.is_none() || !self.temporal {
            return "not-applicable";
        }
        match self.requested {
            SymmetryLiveness::Unreduced => "unreduced",
            SymmetryLiveness::Refuse => "refused",
            SymmetryLiveness::Tlc => "unsupported-tlc-quotient",
        }
    }
}
