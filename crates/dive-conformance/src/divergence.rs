//! Profiles and the expected-divergence registry (ADR 0010).
//!
//! `dive` runs under one of three profiles. The profile is chosen globally
//! with `--compat=tlc`, or `DIVE_COMPAT=tlc`, and every run records it.
//! - [`Profile::Dive`] (the default) may differ from TLC in owner-approved
//!   ways. Each approved divergence is listed in
//!   `conformance/expected-divergences.toml`, with a justification in
//!   `docs/divergences.md`.
//! - [`Profile::Tlc`] is pure TLC behaviour, including TLC's defaults for
//!   the worker count, seeding and exit codes. It must match TLC on the
//!   whole corpus. It is the only profile whose agreement counts for the
//!   strict tier.
//! - [`Profile::TlcWithOverrides`] is TLC behaviour with explicitly listed
//!   per-feature overrides. It is never strict-tier agreement.
//!
//! The registry applies only under `dive`. There, a disagreement that
//! matches an entry, with the entry's expected verdict and with every
//! differing field among the entry's `fields` or its exact `exit_code_pair`,
//! is reported as
//! `expected-divergence`. It is never counted as `agree`. Any other
//! disagreement, and every difference under `tlc` or `tlc+overrides`, is
//! `disagree`. TLC oracles and external adapters have no profile, and the
//! registry never applies to them.

use crate::compare::FieldDiff;
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
    /// `--compat=tlc`: pure TLC behaviour.
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

/// One owner-approved divergence from TLC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectedDivergence {
    /// Stable id, e.g. `big-integers`.
    pub id: String,
    /// Model or module path glob it covers.
    pub pattern: String,
    /// The profile it applies to. Only `dive` is allowed.
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
    /// Why the divergence is approved.
    pub justification: String,
    /// The entry in `docs/divergences.md`, e.g. `docs/divergences.md#integer-overflow`.
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
    /// - an entry naming any profile other than `dive` (in particular `tlc`
    ///   or `tlc+overrides`: the registry never applies under the TLC
    ///   profiles);
    /// - an entry with both or neither of `profile` and `option`;
    /// - an empty justification, or a link outside `docs/divergences.md`;
    /// - an expected verdict that is not an oracle fact;
    /// - no fields or exit pair, equal exit pairs, a field glob covering
    ///   `exit_code` (only exact pairs may excuse it), no compared field, or one
    ///   that matches a trace validation field (`trace.validated`,
    ///   `trace.invalid`), which no divergence may excuse.
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
                (Some("dive"), None) | (None, Some(_)) => {}
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
            if !d.link.starts_with("docs/divergences.md") {
                return bad("link must point into docs/divergences.md");
            }
            if !d.expected_verdict.is_oracle_fact() {
                return bad("expected_verdict must be a real verdict");
            }
            if d.fields.is_empty() && d.exit_code_pair.is_none() {
                return bad("lists no fields or exit_code_pair");
            }
            if d.exit_code_pair.is_some_and(|p| p.expected == p.actual) {
                return bad("exit_code_pair must declare different physical codes");
            }
            for f in &d.fields {
                if glob_match(f, "exit_code") {
                    return bad("exit_code requires an exact exit_code_pair, never a field glob");
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
    /// the `dive` profile; never under `tlc`, `tlc+overrides` or without a
    /// profile. An entry matches when its glob matches `path`, it applies to
    /// the `dive` profile or to an active option (`name` or `name=value`),
    /// the observed verdict is its expected verdict, and every field in
    /// `diffs` matches one of its `fields` or its exact `exit_code_pair`.
    /// A declared pair must occur in `diffs`; it never authorizes a different
    /// or absent numeric exit or excuses another field.
    #[must_use]
    pub fn excuse(
        &self,
        profile: Option<&Profile>,
        options: &[String],
        path: &str,
        observed: Verdict,
        diffs: &[FieldDiff],
    ) -> Option<&ExpectedDivergence> {
        if profile != Some(&Profile::Dive) || diffs.is_empty() {
            return None;
        }
        self.divergence.iter().find(|d| {
            glob_match(&d.pattern, path)
                && d.expected_verdict == observed
                && d.exit_code_pair
                    .is_none_or(|p| diffs.iter().any(|x| p.matches(x)))
                && diffs.iter().all(|x| {
                    if x.field == "exit_code" {
                        d.exit_code_pair.is_some_and(|p| p.matches(x))
                    } else {
                        d.fields.iter().any(|g| glob_match(g, &x.field))
                    }
                })
                && match (&d.profile, &d.option) {
                    (Some(_), None) => true,
                    (None, Some(o)) => options.iter().any(|a| a == o),
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
}
