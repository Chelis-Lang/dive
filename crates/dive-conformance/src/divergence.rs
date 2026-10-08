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
//! differing field among the entry's `fields`, is reported as
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
    /// - no `fields`, a field glob that matches no compared field, or one
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
            if d.fields.is_empty() {
                return bad("lists no fields");
            }
            for f in &d.fields {
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
    /// `diffs` matches one of its `fields`.
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
                && diffs
                    .iter()
                    .all(|x| d.fields.iter().any(|g| glob_match(g, &x.field)))
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
