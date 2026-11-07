//! Profiles and the expected-divergence registry (ADR 0010).
//!
//! `dive` runs under one of three profiles. The profile is chosen globally
//! with `--compat=tlc`, or `DIVE_COMPAT=tlc`, and every run records it.
//! - [`Profile::Dive`] (the default) may differ from TLC in owner-approved
//!   ways. Each approved divergence is listed in
//!   `conformance/expected-divergences.toml`, with a justification in
//!   `docs/divergences.md`.
//! - [`Profile::Tlc`] selects nominal TLC switches, including defaults for
//!   worker count, seeding and exit codes (ADR 0153: compatibility is
//!   nominal). A compatibility gap is a finding unless it is registered here
//!   with `profile = "tlc"` and documented in `docs/tlc-differences.md`; it
//!   is the only profile whose agreement counts for the strict tier, and a
//!   registered gap never counts as agreement.
//! - [`Profile::TlcWithOverrides`] is TLC behaviour with explicitly listed
//!   per-feature overrides. It is never strict-tier agreement.
//!
//! An entry applies only under the profile it names: `dive` entries (and
//! option entries) under `dive`, `tlc` entries under `tlc`. There, a
//! disagreement that matches an entry, with the entry's expected verdict and
//! with every differing field among the entry's `fields` or its exact
//! `exit_code_pair` or `generated_count_pair`, is reported as
//! `expected-divergence`. It is never counted as `agree`. Any other
//! disagreement, and every difference under `tlc+overrides`, is `disagree`.
//! TLC oracles and external adapters have no profile, and the registry never
//! applies to them.

use crate::compare::{CompareContext, FieldDiff};
use crate::error::{Error, Result};
use crate::tlc::result::Verdict;
use crate::upstream::COMPARED_FIELDS;
use crate::util::glob_match;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// An implementation profile (ADR 0010).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile {
    /// `dive`'s default profile: TLC-compatible except for owner-approved,
    /// registered divergences.
    #[serde(rename = "dive")]
    #[default]
    Dive,
    /// `--compat=tlc`: nominal TLC switches, with every difference still a finding.
    #[serde(rename = "tlc")]
    Tlc,
    /// TLC behaviour with explicit per-feature overrides.
    #[serde(rename = "tlc+overrides")]
    TlcWithOverrides(Vec<String>),
}

impl Profile {
    /// Parses `dive`, `tlc`, or `tlc+overrides=<feature>[,<feature>...]`.
    ///
    /// # Errors
    /// Fails on anything else, or on an empty override list.
    pub fn parse(text: &str) -> Result<Self> {
        match text {
            "dive" => Ok(Self::Dive),
            "tlc" => Ok(Self::Tlc),
            other => {
                let list = other.strip_prefix("tlc+overrides=").ok_or_else(|| {
                    Error::Usage(format!(
                        "unknown profile `{other}` (expected `dive`, `tlc` or `tlc+overrides=<feature>,...`)"
                    ))
                })?;
                let overrides: Vec<String> = list
                    .split(',')
                    .map(str::trim)
                    .filter(|f| !f.is_empty())
                    .map(str::to_owned)
                    .collect();
                if overrides.is_empty() {
                    return Err(Error::Usage(
                        "`tlc+overrides` needs at least one override".to_owned(),
                    ));
                }
                Ok(Self::TlcWithOverrides(overrides))
            }
        }
    }

    /// The profile from `DIVE_COMPAT` (`tlc` selects [`Profile::Tlc`]), if set.
    ///
    /// # Errors
    /// Fails on any other non-empty value.
    pub fn from_env() -> Result<Option<Self>> {
        match std::env::var("DIVE_COMPAT") {
            Ok(v) if v == "tlc" => Ok(Some(Self::Tlc)),
            Ok(v) if v.is_empty() => Ok(None),
            Ok(v) => Err(Error::Usage(format!(
                "DIVE_COMPAT=`{v}`: only `tlc` is defined"
            ))),
            Err(_) => Ok(None),
        }
    }

    /// Whether agreement under this profile counts for the strict tier.
    /// Only pure `tlc` does.
    #[must_use]
    pub fn is_strict(&self) -> bool {
        *self == Self::Tlc
    }
}

/// One declared physical process-exit difference; never a code normalization.
///
/// ```
/// use dive_conformance::divergence::ExitCodePair;
/// let native_safety = ExitCodePair { expected: 12, actual: 10 };
/// assert_ne!(native_safety.expected, native_safety.actual);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExitCodePair {
    /// The pinned TLC process status (0 through 255).
    pub expected: u8,
    /// The documented native process status (0 through 255).
    pub actual: u8,
}

impl ExitCodePair {
    fn matches(self, diff: &FieldDiff) -> bool {
        diff.field == "exit_code"
            && diff.expected.as_u64() == Some(u64::from(self.expected))
            && diff.actual.as_u64() == Some(u64::from(self.actual))
    }
}

/// One declared generated-count difference; never a count normalization.
///
/// ```
/// use dive_conformance::divergence::GeneratedCountPair;
/// let composition = GeneratedCountPair { expected: 1_350_001, actual: 900_001 };
/// assert_ne!(composition.expected, composition.actual);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedCountPair {
    /// The pinned TLC generated-state count.
    pub expected: u64,
    /// The documented native generated-state count.
    pub actual: u64,
}

impl GeneratedCountPair {
    fn matches(self, diff: &FieldDiff) -> bool {
        diff.field == "counts.states_generated"
            && diff.expected.as_u64() == Some(self.expected)
            && diff.actual.as_u64() == Some(self.actual)
    }

    fn observed(
        self,
        expected: &crate::tlc::result::Counts,
        actual: &crate::tlc::result::Counts,
    ) -> bool {
        expected.states_generated == Some(self.expected)
            && actual.states_generated == Some(self.actual)
    }
}

/// Results, worker policy and differences from one native comparison.
///
/// The registry uses the actual count facts when native multi-worker policy
/// intentionally omits a generated-count difference. This never supplies
/// authority for another field or for missing count facts.
#[derive(Debug, Clone, Copy)]
pub struct ObservedComparison<'a> {
    /// The selected pinned oracle result.
    pub expected: &'a crate::tlc::result::OracleResult,
    /// The observed native result, including its actual counts.
    pub actual: &'a crate::tlc::result::OracleResult,
    /// The context used to compare these results.
    pub context: CompareContext,
    /// All remaining differences, including independent word-validation failures.
    pub differences: &'a [FieldDiff],
}

#[derive(Clone, Copy)]
struct ResultFacts<'a> {
    expected: &'a crate::tlc::result::Counts,
    actual: &'a crate::tlc::result::Counts,
    unreduced: bool,
    context: Option<CompareContext>,
}

impl ResultFacts<'_> {
    fn generated_matches(self, pair: GeneratedCountPair, diffs: &[FieldDiff]) -> bool {
        if !pair.observed(self.expected, self.actual) {
            return false;
        }
        if diffs.iter().any(|diff| pair.matches(diff)) {
            return true;
        }
        self.context.is_some_and(|context| {
            context.workers > 1
                && context.counts_comparable
                && !diffs
                    .iter()
                    .any(|diff| diff.field == "counts.states_generated")
        })
    }
}

/// Exact expected/native `(distinct, generated, depth)` counts for D-005.
/// Every count is bound, even when a compared field did not differ. This pair
/// additionally requires an actual native report's bound unreduced mode.
///
/// ```
/// use dive_conformance::divergence::UnreducedSymmetryCounts;
/// let pair = UnreducedSymmetryCounts { expected: [1,2,1], actual: [2,3,2] };
/// assert_ne!(pair.expected, pair.actual);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnreducedSymmetryCounts {
    /// The pinned reduced TLC tuple: distinct, generated, depth.
    pub expected: [u64; 3],
    /// The independently observed unreduced native tuple in that same order.
    pub actual: [u64; 3],
}
impl UnreducedSymmetryCounts {
    fn observed(
        self,
        expected: &crate::tlc::result::Counts,
        actual: &crate::tlc::result::Counts,
    ) -> bool {
        let counts =
            |c: &crate::tlc::result::Counts| [c.distinct_states, c.states_generated, c.depth];
        counts(expected) == self.expected.map(Some) && counts(actual) == self.actual.map(Some)
    }
    fn matches(self, diff: &FieldDiff) -> bool {
        let Some(index) = [
            "counts.distinct_states",
            "counts.states_generated",
            "counts.depth",
        ]
        .iter()
        .position(|field| *field == diff.field) else {
            return false;
        };
        diff.expected.as_u64() == Some(self.expected[index])
            && diff.actual.as_u64() == Some(self.actual[index])
    }
}

/// One owner-approved divergence from TLC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedDivergence {
    /// Stable id, e.g. `big-integers`.
    pub id: String,
    /// Model or module path glob it covers.
    pub pattern: String,
    /// The profile it applies to: `dive`, or `tlc` for a registered nominal
    /// compatibility gap (ADR 0153), whose `link` points into
    /// `docs/tlc-differences.md`.
    #[serde(default)]
    pub profile: Option<String>,
    /// Or the implementation option it applies to (under `dive`), e.g. `int=big`.
    #[serde(default)]
    pub option: Option<String>,
    /// The verdict `dive` is expected to give. When it is the golden's own
    /// verdict, the entry excuses differences in other fields only.
    pub expected_verdict: Verdict,
    /// Globs of the compared fields that may differ (see
    /// [`COMPARED_FIELDS`]). A disagreement is excused only when every
    /// differing field matches one of them, so an entry for differing counts
    /// never hides a differing verdict or property. Never the trace
    /// validation fields: a counterexample must always validate.
    pub fields: Vec<String>,
    /// Optional exact pinned-TLC/native exit pair. When present, the
    /// disagreement must contain that exact numeric pair. Null, signals,
    /// unrelated codes and non-integers cannot match. `fields` may not
    /// authorize exits; all other differences still require their own field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code_pair: Option<ExitCodePair>,
    /// Optional exact pinned-TLC/native generated-state pair. When present,
    /// that exact unsigned integer difference must occur; `fields` may not
    /// cover `counts.states_generated`. Every other difference still needs
    /// its own declared field or exact exit pair. Never strict agreement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_count_pair: Option<GeneratedCountPair>,
    /// Exact D-005 counts, bound to default profile and actual whole-run
    /// unreduced symmetry metadata. No field glob may share count authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unreduced_symmetry_counts: Option<UnreducedSymmetryCounts>,
    /// Why the divergence is approved.
    pub justification: String,
    /// The entry in `docs/divergences.md`, e.g. `docs/divergences.md#integer-overflow`,
    /// or, for a `tlc` entry, in `docs/tlc-differences.md`.
    pub link: String,
}

/// Fields no expected divergence may excuse: every counterexample must
/// validate (charter rule 2, ADR 0016).
const UNEXCUSABLE_FIELDS: &[&str] = &["trace.validated", "trace.invalid"];

/// The parsed `conformance/expected-divergences.toml`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    /// Entries.
    #[serde(default)]
    pub divergence: Vec<ExpectedDivergence>,
}

impl Registry {
    /// Parses and validates the registry.
    ///
    /// # Errors
    /// Fails on:
    /// - malformed TOML or duplicate ids;
    /// - an entry naming any profile other than `dive` or `tlc` (in
    ///   particular `tlc+overrides`, which the registry never excuses);
    /// - an entry with both or neither of `profile` and `option`;
    /// - an empty justification, or a link outside `docs/divergences.md`
    ///   (`docs/tlc-differences.md` for a `tlc` entry);
    /// - an expected verdict that is not an oracle fact;
    /// - no fields or exact pairs, equal pairs, a field glob covering
    ///   `exit_code` (only exact pairs may excuse it), no compared field, or one
    ///   that matches a trace validation field (`trace.validated`,
    ///   `trace.invalid`), which no divergence may excuse.
    /// - a field glob covering `counts.states_generated` alongside an exact
    ///   generated-count pair (the pair must be the sole authority for it).
    pub fn parse(text: &str) -> Result<Self> {
        let reg: Self = toml::from_str(text)
            .map_err(|e| Error::Config(format!("expected-divergences.toml: {e}")))?;
        let mut ids = std::collections::BTreeSet::new();
        for d in &reg.divergence {
            let bad = |why: &str| {
                Err(Error::Config(format!(
                    "expected divergence `{}`: {why}",
                    d.id
                )))
            };
            if !ids.insert(d.id.as_str()) {
                return bad("duplicate id");
            }
            match (d.profile.as_deref(), d.option.as_deref()) {
                (Some("dive" | "tlc"), None) | (None, Some(_)) => {}
                (Some(p), None) if p.starts_with("tlc") || p.contains("compat") => {
                    return bad("the registry never applies under the tlc profiles");
                }
                (Some(p), None) => return bad(&format!("unknown profile `{p}`")),
                (Some(_), Some(_)) => return bad("give `profile` or `option`, not both"),
                (None, None) => return bad("give `profile = \"dive\"` or an `option`"),
            }
            if d.option.as_deref().is_some_and(|o| o.contains("compat")) {
                return bad("the registry never applies under the tlc profiles");
            }
            if d.justification.trim().is_empty() {
                return bad("empty justification");
            }
            if d.profile.as_deref() == Some("tlc") {
                if !d.link.starts_with("docs/tlc-differences.md#") {
                    return bad("a tlc compat gap must link into docs/tlc-differences.md");
                }
            } else if !d.link.starts_with("docs/divergences.md") {
                return bad("link must point into docs/divergences.md");
            }
            if !d.expected_verdict.is_oracle_fact() {
                return bad("expected_verdict must be a real verdict");
            }
            if d.fields.is_empty()
                && d.exit_code_pair.is_none()
                && d.generated_count_pair.is_none()
                && d.unreduced_symmetry_counts.is_none()
            {
                return bad("lists no fields, exit_code_pair or generated_count_pair");
            }
            if d.exit_code_pair.is_some_and(|p| p.expected == p.actual) {
                return bad("exit_code_pair must declare different physical codes");
            }
            if d.generated_count_pair
                .is_some_and(|p| p.expected == p.actual)
            {
                return bad("generated_count_pair must declare different counts");
            }
            if let Some(pair) = d.unreduced_symmetry_counts {
                if d.profile.as_deref() != Some("dive") || d.option.is_some() {
                    return bad("unreduced_symmetry_counts requires the default dive profile");
                }
                if pair.expected == pair.actual || d.generated_count_pair.is_some() {
                    return bad(
                        "unreduced_symmetry_counts requires different exact tuples and sole count authority",
                    );
                }
            }
            for f in &d.fields {
                if d.unreduced_symmetry_counts.is_some()
                    && [
                        "counts.distinct_states",
                        "counts.states_generated",
                        "counts.depth",
                    ]
                    .iter()
                    .any(|field| glob_match(f, field))
                {
                    return bad(
                        "unreduced_symmetry_counts cannot share authority with a count-field glob",
                    );
                }
                if glob_match(f, "exit_code") {
                    return bad("exit_code requires an exact exit_code_pair, never a field glob");
                }
                if d.generated_count_pair.is_some() && glob_match(f, "counts.states_generated") {
                    return bad("generated_count_pair cannot share authority with a field glob");
                }
                if !COMPARED_FIELDS.iter().any(|k| glob_match(f, k)) {
                    return bad(&format!("field `{f}` matches no compared field"));
                }
                if UNEXCUSABLE_FIELDS.iter().any(|k| glob_match(f, k)) {
                    return bad(&format!(
                        "field `{f}` covers trace validation, which a divergence never excuses"
                    ));
                }
            }
        }
        Ok(reg)
    }

    /// Loads `<repo>/conformance/expected-divergences.toml`; a missing file
    /// is an empty registry.
    ///
    /// # Errors
    /// Fails if the file exists but is unreadable or invalid.
    pub fn load(repo_root: &Path) -> Result<Self> {
        let path = repo_root
            .join("conformance")
            .join("expected-divergences.toml");
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(Error::io(&path, &e)),
        }
    }

    /// The entry that excuses a disagreement on `path`, if any. Only under
    /// the `dive` profile, or under `tlc` for an entry registered with
    /// `profile = "tlc"`; never under `tlc+overrides` or without a profile.
    /// An entry matches when its glob matches `path`, it names the run's
    /// profile (an option entry: `dive` with that active option, `name` or `name=value`),
    /// the observed verdict is its expected verdict, and every field in
    /// `diffs` matches one of its `fields` or its exact `exit_code_pair` or
    /// `generated_count_pair`. Each declared pair must occur in `diffs`; it
    /// never authorizes different or absent numeric values or another field.
    #[must_use]
    pub fn excuse(
        &self,
        profile: Option<&Profile>,
        options: &[String],
        path: &str,
        observed: Verdict,
        diffs: &[FieldDiff],
    ) -> Option<&ExpectedDivergence> {
        self.excuse_with_facts(profile, options, path, observed, diffs, None)
    }

    /// Like [`Self::excuse`], also binding the complete count tuples and private
    /// strict-reader mode fact required by the D-005 entry. Legacy callers
    /// cannot authorize that entry through numeric differences alone.
    ///
    /// ```no_run
    /// # fn example(registry: &dive_conformance::divergence::Registry,
    /// # expected: &dive_conformance::tlc::result::OracleResult,
    /// # actual: &dive_conformance::tlc::result::OracleResult) {
    /// let entry = registry.excuse_observed(None, &[], "M.cfg", expected, actual, &[]);
    /// assert!(entry.is_none()); // non-native/no profile never uses the registry
    /// # }
    /// ```
    #[must_use]
    pub fn excuse_observed(
        &self,
        profile: Option<&Profile>,
        options: &[String],
        path: &str,
        expected: &crate::tlc::result::OracleResult,
        actual: &crate::tlc::result::OracleResult,
        diffs: &[FieldDiff],
    ) -> Option<&ExpectedDivergence> {
        self.excuse_with_facts(
            profile,
            options,
            path,
            actual.verdict,
            diffs,
            Some(ResultFacts {
                expected: &expected.counts,
                actual: &actual.counts,
                unreduced: actual.native_unreduced_symmetry,
                context: None,
            }),
        )
    }

    /// Matches a native comparison with its actual worker and count policy.
    ///
    /// At one worker, a declared generated-count pair must still occur in the
    /// differences. At multiple workers with comparable counts, the exact
    /// expected/actual count facts can instead satisfy that pair when policy
    /// omits generated-count comparison. Missing or changed facts never match.
    /// Legacy [`Self::excuse`] and [`Self::excuse_observed`] retain their
    /// mandatory generated-count difference.
    ///
    /// ```no_run
    /// # fn example(registry: &dive_conformance::divergence::Registry,
    /// # expected: &dive_conformance::tlc::result::OracleResult,
    /// # actual: &dive_conformance::tlc::result::OracleResult) {
    /// use dive_conformance::compare::CompareContext;
    /// use dive_conformance::divergence::ObservedComparison;
    /// let comparison = ObservedComparison {
    ///     expected, actual,
    ///     context: CompareContext { workers: 4, counts_comparable: true },
    ///     differences: &[],
    /// };
    /// assert!(registry.excuse_compared(None, &[], "M.cfg", comparison).is_none());
    /// # }
    /// ```
    #[must_use]
    pub fn excuse_compared(
        &self,
        profile: Option<&Profile>,
        options: &[String],
        path: &str,
        comparison: ObservedComparison<'_>,
    ) -> Option<&ExpectedDivergence> {
        if comparison.context.workers == 0 {
            return None;
        }
        self.excuse_with_facts(
            profile,
            options,
            path,
            comparison.actual.verdict,
            comparison.differences,
            Some(ResultFacts {
                expected: &comparison.expected.counts,
                actual: &comparison.actual.counts,
                unreduced: comparison.actual.native_unreduced_symmetry,
                context: Some(comparison.context),
            }),
        )
    }

    fn excuse_with_facts(
        &self,
        profile: Option<&Profile>,
        options: &[String],
        path: &str,
        observed: Verdict,
        diffs: &[FieldDiff],
        facts: Option<ResultFacts<'_>>,
    ) -> Option<&ExpectedDivergence> {
        let name = match profile {
            Some(Profile::Dive) => "dive",
            Some(Profile::Tlc) => "tlc",
            _ => return None,
        };
        if diffs.is_empty() {
            return None;
        }
        self.divergence.iter().find(|d| {
            glob_match(&d.pattern, path)
                && d.expected_verdict == observed
                && d.unreduced_symmetry_counts.is_none_or(|pair| {
                    facts.is_some_and(|facts| {
                        facts.unreduced
                            && pair.observed(facts.expected, facts.actual)
                            && [
                                "counts.distinct_states",
                                "counts.states_generated",
                                "counts.depth",
                            ]
                            .iter()
                            .enumerate()
                            .all(|(index, field)| {
                                pair.expected[index] == pair.actual[index]
                                    || diffs
                                        .iter()
                                        .any(|diff| diff.field == *field && pair.matches(diff))
                            })
                    })
                })
                && d.exit_code_pair
                    .is_none_or(|p| diffs.iter().any(|x| p.matches(x)))
                && d.generated_count_pair.is_none_or(|pair| {
                    facts.map_or_else(
                        || diffs.iter().any(|diff| pair.matches(diff)),
                        |facts| facts.generated_matches(pair, diffs),
                    )
                })
                && diffs.iter().all(|x| {
                    if x.field == "exit_code" {
                        d.exit_code_pair.is_some_and(|p| p.matches(x))
                    } else if d.unreduced_symmetry_counts.is_some()
                        && [
                            "counts.distinct_states",
                            "counts.states_generated",
                            "counts.depth",
                        ]
                        .contains(&x.field.as_str())
                    {
                        d.unreduced_symmetry_counts
                            .is_some_and(|pair| pair.matches(x))
                    } else if x.field == "counts.states_generated"
                        && d.generated_count_pair.is_some()
                    {
                        d.generated_count_pair.is_some_and(|p| p.matches(x))
                    } else {
                        d.fields.iter().any(|g| glob_match(g, &x.field))
                    }
                })
                && match (&d.profile, &d.option) {
                    (Some(p), None) => p == name,
                    (None, Some(o)) => name == "dive" && options.iter().any(|a| a == o),
                    _ => false,
                }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRY: &str = r#"
[[divergence]]
id = "big-integers"
pattern = "specifications/x/*.cfg"
profile = "dive"
expected_verdict = "success"
fields = ["verdict", "error_class", "counts.*"]
justification = "dive uses arbitrary-precision integers by default"
link = "docs/divergences.md#integer-overflow"
"#;

    fn diff(field: &str) -> Vec<FieldDiff> {
        vec![FieldDiff {
            field: field.to_owned(),
            expected: 1.into(),
            actual: 2.into(),
        }]
    }

    #[test]
    fn registry_validates() -> Result<()> {
        let r = Registry::parse(ENTRY)?;
        assert_eq!(r.divergence.len(), 1);
        for (from, to) in [
            // A tlc compat gap must link into docs/tlc-differences.md.
            (r#"profile = "dive""#, r#"profile = "tlc""#),
            (r#"profile = "dive""#, r#"profile = "tlc+overrides""#),
            (r#"profile = "dive""#, r#"profile = "compat=tlc""#),
            (r#"profile = "dive""#, r#"option = "compat=tlc""#),
            (r#"profile = "dive""#, r#"profile = "fast""#),
            (r#"profile = "dive""#, ""),
            (
                "docs/divergences.md#integer-overflow",
                "https://example.com",
            ),
            ("dive uses arbitrary-precision integers by default", " "),
            (
                r#"expected_verdict = "success""#,
                r#"expected_verdict = "timeout""#,
            ),
            (
                r#"fields = ["verdict", "error_class", "counts.*"]"#,
                "fields = []",
            ),
            (r#"fields = ["verdict", "error_class", "counts.*"]"#, ""),
            (r#""counts.*""#, r#""count.depth""#),
            (r#""counts.*""#, r#""trace.validated""#),
            (r#""counts.*""#, r#""trace.*""#),
        ] {
            assert!(Registry::parse(&ENTRY.replace(from, to)).is_err(), "{to}");
        }
        assert!(Registry::parse(&format!("{ENTRY}{ENTRY}")).is_err());
        assert!(Registry::parse(&format!("{ENTRY}\nextra = 1\n")).is_err());
        Ok(())
    }

    const TLC_GAP: &str = r#"
[[divergence]]
id = "gap"
pattern = "specifications/x/X.cfg"
profile = "tlc"
expected_verdict = "success"
fields = []
generated_count_pair = { expected = 10, actual = 4 }
justification = "nominal compat gap (ADR 0153)"
link = "docs/tlc-differences.md#compat-gaps"
"#;

    fn generated(expected: u64, actual: u64) -> Vec<FieldDiff> {
        vec![FieldDiff {
            field: "counts.states_generated".to_owned(),
            expected: expected.into(),
            actual: actual.into(),
        }]
    }

    #[test]
    fn a_registered_tlc_compat_gap_is_an_expected_divergence_only_under_tlc() -> Result<()> {
        let r = Registry::parse(TLC_GAP)?;
        let p = "specifications/x/X.cfg";
        let tlc = Some(&Profile::Tlc);
        let d = generated(10, 4);
        assert!(r.excuse(tlc, &[], p, Verdict::Success, &d).is_some());
        // Never under another profile, nor for other counts, fields, verdicts
        // or models.
        assert!(
            r.excuse(Some(&Profile::Dive), &[], p, Verdict::Success, &d)
                .is_none()
        );
        let over = Profile::TlcWithOverrides(vec!["int".to_owned()]);
        assert!(
            r.excuse(Some(&over), &[], p, Verdict::Success, &d)
                .is_none()
        );
        assert!(r.excuse(None, &[], p, Verdict::Success, &d).is_none());
        assert!(
            r.excuse(tlc, &[], p, Verdict::Success, &generated(10, 5))
                .is_none()
        );
        assert!(
            r.excuse(tlc, &[], p, Verdict::Success, &diff("counts.depth"))
                .is_none()
        );
        let mut both = generated(10, 4);
        both.extend(diff("counts.distinct_states"));
        assert!(r.excuse(tlc, &[], p, Verdict::Success, &both).is_none());
        assert!(r.excuse(tlc, &[], p, Verdict::Overflow, &d).is_none());
        let other = "specifications/y/Y.cfg";
        assert!(r.excuse(tlc, &[], other, Verdict::Success, &d).is_none());
        // A default-profile entry never excuses a tlc disagreement.
        let dive = Registry::parse(ENTRY)?;
        assert!(
            dive.excuse(tlc, &[], p, Verdict::Success, &diff("counts.depth"))
                .is_none()
        );
        // A tlc gap must be documented in docs/tlc-differences.md.
        assert!(
            Registry::parse(&TLC_GAP.replace(
                "docs/tlc-differences.md#compat-gaps",
                "docs/divergences.md#d-029"
            ))
            .is_err()
        );
        assert!(
            Registry::parse(&TLC_GAP.replace(r#"profile = "tlc""#, r#"option = "compat=tlc""#))
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn repository_registry_loads() -> Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let r = Registry::load(&root)?;
        assert!(r.divergence.iter().any(|d| d.id == "d010-acp-nb-wrong-tlc"));
        Ok(())
    }

    #[test]
    fn excuses_only_under_dive() -> Result<()> {
        let r = Registry::parse(ENTRY)?;
        let p = "specifications/x/X.cfg";
        let d = diff("counts.depth");
        let dive = Some(&Profile::Dive);
        assert!(r.excuse(dive, &[], p, Verdict::Success, &d).is_some());
        assert!(
            r.excuse(Some(&Profile::Tlc), &[], p, Verdict::Success, &d)
                .is_none()
        );
        let over = Profile::TlcWithOverrides(vec!["int".to_owned()]);
        assert!(
            r.excuse(Some(&over), &[], p, Verdict::Success, &d)
                .is_none()
        );
        assert!(r.excuse(None, &[], p, Verdict::Success, &d).is_none());
        // Wrong verdict or path: not excused.
        assert!(r.excuse(dive, &[], p, Verdict::Overflow, &d).is_none());
        let other = "specifications/y/Y.cfg";
        assert!(r.excuse(dive, &[], other, Verdict::Success, &d).is_none());
        // A differing field outside `fields`, alone or with listed ones: not
        // excused. Nor is a trace that did not validate, or no difference.
        assert!(
            r.excuse(dive, &[], p, Verdict::Success, &diff("violated_property"))
                .is_none()
        );
        let mut mixed = d.clone();
        mixed.extend(diff("trace.validated"));
        assert!(r.excuse(dive, &[], p, Verdict::Success, &mixed).is_none());
        assert!(r.excuse(dive, &[], p, Verdict::Success, &[]).is_none());
        // Option-scoped entries need the option.
        let opt = Registry::parse(&ENTRY.replace(r#"profile = "dive""#, r#"option = "int=big""#))?;
        assert!(opt.excuse(dive, &[], p, Verdict::Success, &d).is_none());
        let on = vec!["int=big".to_owned()];
        assert!(opt.excuse(dive, &on, p, Verdict::Success, &d).is_some());
        Ok(())
    }

    const EXIT_ENTRY: &str = r#"
[[divergence]]
id = "native-safety-exit"
pattern = "specifications/x/X.cfg"
profile = "dive"
expected_verdict = "safety-violation"
fields = []
exit_code_pair = { expected = 12, actual = 10 }
justification = "established native property status"
link = "docs/divergences.md#d-023-truthful-default-numeric-exit-accounting"
"#;

    #[test]
    fn exit_pair_registry_rejects_wildcards_and_malformed_pairs() -> Result<()> {
        Registry::parse(EXIT_ENTRY)?;
        for fields in ["exit_code", "exit*", "*", "trace.*"] {
            assert!(
                Registry::parse(
                    &EXIT_ENTRY.replace("fields = []", &format!("fields = [\"{fields}\"]"))
                )
                .is_err()
            );
        }
        for pair in [
            "{ expected = 12, actual = 12 }",
            "{ expected = 12, actual = -1 }",
            "{ expected = 12, actual = 256 }",
            "{ expected = 12, actual = true }",
            "{ expected = 12, actual = 10.0 }",
            "{ expected = 12, actual = \"10\" }",
            "{ expected = 12 }",
            "{ expected = 12, actual = 10, other = 0 }",
        ] {
            assert!(
                Registry::parse(&EXIT_ENTRY.replace("{ expected = 12, actual = 10 }", pair))
                    .is_err(),
                "{pair}"
            );
        }
        Ok(())
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one exact-pair negative matrix shares the same registry and diff"
    )]
    fn exact_exit_pair_never_excuses_other_codes_or_fields() -> Result<()> {
        let r = Registry::parse(EXIT_ENTRY)?;
        let p = "specifications/x/X.cfg";
        let exit = |expected, actual| FieldDiff {
            field: "exit_code".into(),
            expected,
            actual,
        };
        let pair = exit(12.into(), 10.into());
        let dive = Some(&Profile::Dive);
        assert!(
            r.excuse(
                dive,
                &[],
                p,
                Verdict::SafetyViolation,
                std::slice::from_ref(&pair)
            )
            .is_some()
        );
        for value in [
            serde_json::Value::Null,
            0.into(),
            77.into(),
            (-1).into(),
            256.into(),
            true.into(),
            serde_json::json!(10.0),
            "10".into(),
        ] {
            for bad in [exit(12.into(), value.clone()), exit(value, 10.into())] {
                assert!(
                    r.excuse(dive, &[], p, Verdict::SafetyViolation, &[bad])
                        .is_none()
                );
            }
        }
        for field in [
            "verdict",
            "error_class",
            "violated_property",
            "counts.depth",
            "counts.states_generated",
            "trace.invalid",
            "trace.validated",
        ] {
            let mut mixed = vec![pair.clone()];
            mixed.extend(diff(field));
            assert!(
                r.excuse(dive, &[], p, Verdict::SafetyViolation, &mixed)
                    .is_none(),
                "{field}"
            );
        }
        let counts = Registry::parse(
            &EXIT_ENTRY.replace("fields = []", "fields = [\"counts.states_generated\"]"),
        )?;
        let mut mixed = vec![pair.clone()];
        mixed.extend(diff("counts.states_generated"));
        assert!(
            counts
                .excuse(dive, &[], p, Verdict::SafetyViolation, &mixed)
                .is_some()
        );
        assert!(
            counts
                .excuse(
                    dive,
                    &[],
                    p,
                    Verdict::SafetyViolation,
                    &diff("counts.states_generated")
                )
                .is_none()
        );
        for profile in [
            None,
            Some(&Profile::Tlc),
            Some(&Profile::TlcWithOverrides(vec!["x".into()])),
        ] {
            assert!(
                r.excuse(
                    profile,
                    &[],
                    p,
                    Verdict::SafetyViolation,
                    std::slice::from_ref(&pair)
                )
                .is_none()
            );
        }
        for verdict in [
            Verdict::Success,
            Verdict::Unclassified,
            Verdict::ResourceLimit,
        ] {
            assert!(
                r.excuse(dive, &[], p, verdict, std::slice::from_ref(&pair))
                    .is_none()
            );
        }
        assert!(
            r.excuse(
                dive,
                &[],
                "specifications/y/Y.cfg",
                Verdict::SafetyViolation,
                &[pair]
            )
            .is_none()
        );
        Ok(())
    }

    #[test]
    fn profiles_parse_and_only_tlc_is_strict() -> Result<()> {
        assert_eq!(Profile::parse("tlc")?, Profile::Tlc);
        assert_eq!(
            Profile::parse("tlc+overrides=int, symmetry")?,
            Profile::TlcWithOverrides(vec!["int".to_owned(), "symmetry".to_owned()])
        );
        assert!(Profile::parse("tlc+overrides=").is_err());
        assert!(Profile::parse("default").is_err());
        assert!(Profile::Tlc.is_strict());
        assert!(!Profile::Dive.is_strict());
        assert!(!Profile::TlcWithOverrides(vec!["int".to_owned()]).is_strict());
        Ok(())
    }

    const GENERATED_ENTRY: &str = r#"
[[divergence]]
id = "composition-generated-count"
pattern = "specifications/x/X.cfg"
profile = "dive"
expected_verdict = "success"
fields = []
generated_count_pair = { expected = 1350001, actual = 900001 }
justification = "documented D-022 composition intermediates"
link = "docs/divergences.md#registry"
"#;

    fn generated_diff(expected: serde_json::Value, actual: serde_json::Value) -> FieldDiff {
        FieldDiff {
            field: "counts.states_generated".into(),
            expected,
            actual,
        }
    }

    #[test]
    fn generated_pair_registry_rejects_shared_authority_and_malformed_pairs() -> Result<()> {
        Registry::parse(GENERATED_ENTRY)?;
        for fields in [
            "counts.states_generated",
            "counts.*",
            "counts.states_*",
            "*",
        ] {
            assert!(
                Registry::parse(
                    &GENERATED_ENTRY.replace("fields = []", &format!("fields = [\"{fields}\"]"))
                )
                .is_err(),
                "{fields}"
            );
        }
        for pair in [
            "{ expected = 1350001, actual = 1350001 }",
            "{ expected = -1, actual = 900001 }",
            "{ expected = 1350001, actual = -1 }",
            "{ expected = true, actual = 900001 }",
            "{ expected = 1350001, actual = false }",
            "{ expected = 1350001, actual = 900001.0 }",
            "{ expected = \"1350001\", actual = 900001 }",
            "{ expected = 1350001, actual = \"900001\" }",
            "{ expected = 1350001, actual = null }",
            "{ expected = 1350001 }",
            "{ actual = 900001 }",
            "{ expected = 1350001, actual = 900001, other = 0 }",
        ] {
            assert!(
                Registry::parse(
                    &GENERATED_ENTRY.replace("{ expected = 1350001, actual = 900001 }", pair)
                )
                .is_err(),
                "{pair}"
            );
        }
        for profile in ["tlc", "tlc+overrides", "adapter", "compat=tlc"] {
            assert!(
                Registry::parse(
                    &GENERATED_ENTRY
                        .replace("profile = \"dive\"", &format!("profile = \"{profile}\""))
                )
                .is_err()
            );
        }
        let plain = Registry::parse(ENTRY)?;
        assert!(plain.divergence[0].generated_count_pair.is_none());
        Ok(())
    }

    #[test]
    fn exact_generated_pair_rejects_other_and_noninteger_counts() -> Result<()> {
        let registry = Registry::parse(GENERATED_ENTRY)?;
        let path = "specifications/x/X.cfg";
        let pair = generated_diff(1_350_001.into(), 900_001.into());
        assert!(
            registry
                .excuse(Some(&Profile::Dive), &[], path, Verdict::Success, &[pair])
                .is_some()
        );
        for value in [
            serde_json::Value::Null,
            true.into(),
            false.into(),
            (-1).into(),
            "900001".into(),
            serde_json::json!(900_001.0),
            0.into(),
            900_000.into(),
            900_002.into(),
            1_350_000.into(),
            1_350_002.into(),
            u64::MAX.into(),
        ] {
            for invalid in [
                generated_diff(1_350_001.into(), value.clone()),
                generated_diff(value, 900_001.into()),
            ] {
                assert!(
                    registry
                        .excuse(
                            Some(&Profile::Dive),
                            &[],
                            path,
                            Verdict::Success,
                            &[invalid]
                        )
                        .is_none()
                );
            }
        }
        for invalid in [
            generated_diff(900_001.into(), 1_350_001.into()),
            generated_diff(1_350_001.into(), 1_350_001.into()),
            generated_diff(900_001.into(), 900_001.into()),
        ] {
            assert!(
                registry
                    .excuse(
                        Some(&Profile::Dive),
                        &[],
                        path,
                        Verdict::Success,
                        &[invalid]
                    )
                    .is_none()
            );
        }
        Ok(())
    }

    #[test]
    fn generated_pair_needs_the_exact_diff_and_no_unlisted_observables() -> Result<()> {
        let registry = Registry::parse(GENERATED_ENTRY)?;
        let path = "specifications/x/X.cfg";
        let pair = generated_diff(1_350_001.into(), 900_001.into());
        let dive = Some(&Profile::Dive);
        assert!(
            registry
                .excuse(dive, &[], path, Verdict::Success, &[])
                .is_none()
        );
        for field in [
            "counts.distinct_states",
            "counts.depth",
            "verdict",
            "error_class",
            "violated_property",
            "trace.length",
            "trace.ending",
            "trace.invalid",
            "trace.validated",
            "exit_code",
        ] {
            let mut mixed = vec![pair.clone()];
            mixed.extend(diff(field));
            assert!(
                registry
                    .excuse(dive, &[], path, Verdict::Success, &mixed)
                    .is_none(),
                "{field}"
            );
        }
        let listed = Registry::parse(
            &GENERATED_ENTRY.replace("fields = []", "fields = [\"counts.depth\"]"),
        )?;
        let mut mixed = vec![pair.clone()];
        mixed.extend(diff("counts.depth"));
        assert!(
            listed
                .excuse(dive, &[], path, Verdict::Success, &mixed)
                .is_some()
        );
        assert!(
            listed
                .excuse(dive, &[], path, Verdict::Success, &diff("counts.depth"))
                .is_none()
        );
        for profile in [
            None,
            Some(&Profile::Tlc),
            Some(&Profile::TlcWithOverrides(vec!["x".into()])),
        ] {
            assert!(
                registry
                    .excuse(
                        profile,
                        &[],
                        path,
                        Verdict::Success,
                        std::slice::from_ref(&pair)
                    )
                    .is_none()
            );
        }
        for verdict in [
            Verdict::SafetyViolation,
            Verdict::Unclassified,
            Verdict::ResourceLimit,
        ] {
            assert!(
                registry
                    .excuse(dive, &[], path, verdict, std::slice::from_ref(&pair))
                    .is_none()
            );
        }
        assert!(
            registry
                .excuse(
                    dive,
                    &[],
                    "specifications/y/X.cfg",
                    Verdict::Success,
                    &[pair]
                )
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn generated_and_exit_pairs_each_require_their_own_exact_difference() -> Result<()> {
        let registry = Registry::parse(&format!(
            "{GENERATED_ENTRY}\nexit_code_pair = {{ expected = 12, actual = 10 }}\n"
        ))?;
        let pair = generated_diff(1_350_001.into(), 900_001.into());
        let exit = FieldDiff {
            field: "exit_code".into(),
            expected: 12.into(),
            actual: 10.into(),
        };
        let path = "specifications/x/X.cfg";
        for absent in [pair.clone(), exit.clone()] {
            assert!(
                registry
                    .excuse(Some(&Profile::Dive), &[], path, Verdict::Success, &[absent])
                    .is_none()
            );
        }
        assert!(
            registry
                .excuse(
                    Some(&Profile::Dive),
                    &[],
                    path,
                    Verdict::Success,
                    &[pair, exit]
                )
                .is_some()
        );
        Ok(())
    }

    #[test]
    fn repository_d022_entries_authorize_only_the_documented_generated_pair() -> Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let registry = Registry::load(&root)?;
        for (model, expected, actual) in [
            ("MCCRDT", 1_350_001_u64, 900_001_u64),
            ("MCReplicatedLog", 17_425, 11_617),
        ] {
            let path = format!("specifications/FiniteMonotonic/{model}.cfg");
            let pair = generated_diff(expected.into(), actual.into());
            let entry = registry
                .excuse(
                    Some(&Profile::Dive),
                    &[],
                    &path,
                    Verdict::Success,
                    std::slice::from_ref(&pair),
                )
                .ok_or_else(|| {
                    Error::Config(format!("missing documented D-022 pair for {model}"))
                })?;
            assert_eq!(entry.fields, [] as [String; 0]);
            assert!(entry.exit_code_pair.is_none());
            assert_eq!(
                entry.generated_count_pair,
                Some(GeneratedCountPair { expected, actual })
            );
            for invalid in [
                generated_diff(expected.into(), (actual + 1).into()),
                generated_diff(actual.into(), expected.into()),
            ] {
                assert!(
                    registry
                        .excuse(
                            Some(&Profile::Dive),
                            &[],
                            &path,
                            Verdict::Success,
                            &[invalid]
                        )
                        .is_none()
                );
            }
            let mut mixed = vec![pair.clone()];
            mixed.extend(diff("counts.distinct_states"));
            assert!(
                registry
                    .excuse(Some(&Profile::Dive), &[], &path, Verdict::Success, &mixed)
                    .is_none()
            );
            for profile in [
                None,
                Some(&Profile::Tlc),
                Some(&Profile::TlcWithOverrides(vec![
                    "action-composition=tla".into(),
                ])),
            ] {
                assert!(
                    registry
                        .excuse(
                            profile,
                            &[],
                            &path,
                            Verdict::Success,
                            std::slice::from_ref(&pair)
                        )
                        .is_none()
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod unreduced_symmetry_tests {
    #![allow(clippy::expect_used, reason = "fixed primary fixtures and assertions")]
    use super::*;
    use crate::tlc::result::OracleResult;
    use serde_json::json;
    const ENTRY: &str = r#"[[divergence]]
id = "symmetry-liveness-unreduced-p4"
pattern = "specifications/P4SymLive/P4SymLive.cfg"
profile = "dive"
expected_verdict = "success"
fields = []
unreduced_symmetry_counts = { expected = [1,2,1], actual = [2,3,2] }
justification = "D-005 exact primary full-state reference"
link = "docs/divergences.md#symmetry-with-liveness"
"#;
    const PATH: &str = "specifications/P4SymLive/P4SymLive.cfg";
    fn facts() -> (Registry, OracleResult, OracleResult, Vec<FieldDiff>) {
        let golden: crate::golden::ModelGolden = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"), "/../../conformance/golden/tlc-2026.10.06.014338/probes/specifications/P4SymLive/P4SymLive.cfg.json"
        ))).expect("original reduced golden");
        let expected = golden.result;
        let mut actual = expected.clone();
        actual.native_unreduced_symmetry = true;
        actual.counts.distinct_states = Some(2);
        actual.counts.states_generated = Some(3);
        actual.counts.depth = Some(2);
        let diffs = [
            "counts.distinct_states",
            "counts.states_generated",
            "counts.depth",
        ]
        .into_iter()
        .zip([(1, 2), (2, 3), (1, 2)])
        .map(|(field, (expected, actual))| FieldDiff {
            field: field.into(),
            expected: json!(expected),
            actual: json!(actual),
        })
        .collect();
        (
            Registry::parse(ENTRY).expect("exact entry"),
            expected,
            actual,
            diffs,
        )
    }
    #[test]
    fn all_three_counts_and_actual_default_unreduced_mode_are_required() {
        let (registry, expected, mut actual, diffs) = facts();
        let profile = Some(&Profile::Dive);
        assert!(
            registry
                .excuse_observed(profile, &[], PATH, &expected, &actual, &diffs)
                .is_some()
        );
        assert!(
            registry
                .excuse(profile, &[], PATH, actual.verdict, &diffs)
                .is_none()
        );
        actual.native_unreduced_symmetry = false;
        assert!(
            registry
                .excuse_observed(profile, &[], PATH, &expected, &actual, &diffs)
                .is_none()
        );
        actual.native_unreduced_symmetry = true;
        // Wire deserialization cannot manufacture the private mode fact.
        let transported: OracleResult =
            serde_json::from_value(serde_json::to_value(&actual).expect("json")).expect("read");
        assert!(!transported.native_unreduced_symmetry);
        for profile in [
            None,
            Some(Profile::Tlc),
            Some(Profile::TlcWithOverrides(vec![
                "symmetry-liveness=unreduced".into(),
            ])),
        ] {
            assert!(
                registry
                    .excuse_observed(profile.as_ref(), &[], PATH, &expected, &actual, &diffs)
                    .is_none()
            );
        }
        assert!(
            registry
                .excuse_observed(
                    Some(&Profile::Dive),
                    &[],
                    "other.cfg",
                    &expected,
                    &actual,
                    &diffs
                )
                .is_none()
        );
    }
    #[test]
    fn exact_unreduced_tuple_rejects_missing_reversed_and_noninteger_facts() {
        let (registry, expected, actual, diffs) = facts();
        let mut reversed = Registry::parse(ENTRY).expect("entry");
        let pair = reversed.divergence[0]
            .unreduced_symmetry_counts
            .as_mut()
            .expect("tuple");
        std::mem::swap(&mut pair.expected, &mut pair.actual);
        assert!(
            reversed
                .excuse_observed(Some(&Profile::Dive), &[], PATH, &expected, &actual, &diffs)
                .is_none()
        );
        for index in 0..3 {
            let mut bad_expected = expected.clone();
            match index {
                0 => bad_expected.counts.distinct_states = None,
                1 => bad_expected.counts.states_generated = Some(3),
                _ => bad_expected.counts.depth = Some(2),
            }
            assert!(
                registry
                    .excuse_observed(
                        Some(&Profile::Dive),
                        &[],
                        PATH,
                        &bad_expected,
                        &actual,
                        &diffs
                    )
                    .is_none()
            );
            let mut bad = actual.clone();
            match index {
                0 => bad.counts.distinct_states = Some(3),
                1 => bad.counts.states_generated = Some(4),
                _ => bad.counts.depth = None,
            }
            assert!(
                registry
                    .excuse_observed(Some(&Profile::Dive), &[], PATH, &expected, &bad, &diffs)
                    .is_none()
            );
            let mut missing = diffs.clone();
            missing.remove(index);
            assert!(
                registry
                    .excuse_observed(
                        Some(&Profile::Dive),
                        &[],
                        PATH,
                        &expected,
                        &actual,
                        &missing
                    )
                    .is_none()
            );
            for value in [json!(null), json!(true), json!("2"), json!(2.5), json!(99)] {
                let mut bad = diffs.clone();
                bad[index].actual = value;
                assert!(
                    registry
                        .excuse_observed(Some(&Profile::Dive), &[], PATH, &expected, &actual, &bad)
                        .is_none()
                );
            }
        }
    }
    #[test]
    fn no_other_observable_can_be_excused_by_the_tuple() {
        let (registry, expected, actual, diffs) = facts();
        for field in [
            "verdict",
            "error_class",
            "violated_property",
            "exit_code",
            "trace.length",
            "trace.ending",
            "trace.validated",
            "trace.invalid",
            "counts.other",
        ] {
            let mut bad = diffs.clone();
            bad.push(FieldDiff {
                field: field.into(),
                expected: json!(1),
                actual: json!(2),
            });
            assert!(
                registry
                    .excuse_observed(Some(&Profile::Dive), &[], PATH, &expected, &actual, &bad)
                    .is_none(),
                "{field}"
            );
        }
        let mut bad = actual.clone();
        bad.verdict = Verdict::LivenessViolation;
        assert!(
            registry
                .excuse_observed(Some(&Profile::Dive), &[], PATH, &expected, &bad, &diffs)
                .is_none()
        );
    }
    #[test]
    fn tuple_schema_rejects_missing_noninteger_equal_and_overlapping_authority() {
        for tuple in [
            "{ expected=[1,2], actual=[2,3,2] }",
            "{ expected=[1,2,1] }",
            "{ expected=[1,2,1], actual=[1,2,1] }",
            "{ expected=[1,2,1], actual=[2,3,2.0] }",
            "{ expected=[1,2,1], actual=[2,3,true] }",
            "{ expected=[1,2,1], actual=[2,3,\"2\"] }",
        ] {
            assert!(
                Registry::parse(&ENTRY.replace("{ expected = [1,2,1], actual = [2,3,2] }", tuple))
                    .is_err()
            );
        }
        for glob in [
            "counts.*",
            "counts.distinct_states",
            "counts.states_generated",
            "counts.depth",
        ] {
            assert!(
                Registry::parse(&ENTRY.replace("fields = []", &format!("fields = [\"{glob}\"]")))
                    .is_err()
            );
        }
        assert!(
            Registry::parse(&ENTRY.replace(
                "profile = \"dive\"",
                "option = \"symmetry-liveness=unreduced\""
            ))
            .is_err()
        );
        assert!(
            Registry::parse(
                &(ENTRY.to_owned() + "generated_count_pair = { expected=2, actual=3 }\n")
            )
            .is_err()
        );
    }
}
