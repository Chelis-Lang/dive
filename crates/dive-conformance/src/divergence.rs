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
//! matches an entry, with the entry's expected verdict, is reported as
//! `expected-divergence`. It is never counted as `agree`. Any other
//! disagreement, and every difference under `tlc` or `tlc+overrides`, is
//! `disagree`. TLC oracles and external adapters have no profile, and the
//! registry never applies to them.

use crate::error::{Error, Result};
use crate::tlc::result::Verdict;
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
    /// The verdict `dive` is expected to give instead of TLC's.
    pub expected_verdict: Verdict,
    /// Why the divergence is approved.
    pub justification: String,
    /// The entry in `docs/divergences.md`, e.g. `docs/divergences.md#integer-overflow`.
    pub link: String,
}

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
    /// - an expected verdict that is not an oracle fact.
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
    /// and the observed verdict is its expected verdict.
    #[must_use]
    pub fn excuse(
        &self,
        profile: Option<&Profile>,
        options: &[String],
        path: &str,
        observed: Verdict,
    ) -> Option<&ExpectedDivergence> {
        if profile != Some(&Profile::Dive) {
            return None;
        }
        self.divergence.iter().find(|d| {
            glob_match(&d.pattern, path)
                && d.expected_verdict == observed
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
justification = "dive uses arbitrary-precision integers by default"
link = "docs/divergences.md#integer-overflow"
"#;

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
        ] {
            assert!(Registry::parse(&ENTRY.replace(from, to)).is_err(), "{to}");
        }
        assert!(Registry::parse(&format!("{ENTRY}{ENTRY}")).is_err());
        assert!(Registry::parse(&format!("{ENTRY}\nextra = 1\n")).is_err());
        Ok(())
    }

    #[test]
    fn excuses_only_under_dive() -> Result<()> {
        let r = Registry::parse(ENTRY)?;
        let p = "specifications/x/X.cfg";
        assert!(
            r.excuse(Some(&Profile::Dive), &[], p, Verdict::Success)
                .is_some()
        );
        assert!(
            r.excuse(Some(&Profile::Tlc), &[], p, Verdict::Success)
                .is_none()
        );
        let over = Profile::TlcWithOverrides(vec!["int".to_owned()]);
        assert!(r.excuse(Some(&over), &[], p, Verdict::Success).is_none());
        assert!(r.excuse(None, &[], p, Verdict::Success).is_none());
        // Wrong verdict or path: not excused.
        assert!(
            r.excuse(Some(&Profile::Dive), &[], p, Verdict::Overflow)
                .is_none()
        );
        let other = "specifications/y/Y.cfg";
        assert!(
            r.excuse(Some(&Profile::Dive), &[], other, Verdict::Success)
                .is_none()
        );
        // Option-scoped entries need the option.
        let opt = Registry::parse(&ENTRY.replace(r#"profile = "dive""#, r#"option = "int=big""#))?;
        assert!(
            opt.excuse(Some(&Profile::Dive), &[], p, Verdict::Success)
                .is_none()
        );
        let on = vec!["int=big".to_owned()];
        assert!(
            opt.excuse(Some(&Profile::Dive), &on, p, Verdict::Success)
                .is_some()
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
