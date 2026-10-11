//! Known upstream changes between pinned TLC builds
//! (`conformance/oracle-changes.toml`; ADR 0002, ADR 0003).
//!
//! When one TLC oracle runs against another's goldens, many differences are
//! not dive's concern at all: they are upstream bug fixes or features that one
//! build has and the other lacks. `docs/survey/oracle-history.md` catalogues
//! them. This registry makes the catalogue machine-readable. A cross-oracle
//! difference that a registered change fully explains is reported as
//! [`Outcome::KnownUpstreamChange`](crate::compare::Outcome::KnownUpstreamChange),
//! never as `agree`, and is not a finding. Any difference that no entry
//! explains stays `disagree`.
//!
//! The registry applies only when the implementation under test is itself a
//! TLC oracle. It never applies to `dive` or to external adapters, whose
//! differences from the goldens are always theirs to explain.
//!
//! An entry explains a difference when all of these hold:
//! - the two oracles involved are one from `absent_in` (builds without the
//!   change) and one from `present_in` (builds with it), in either role;
//! - the model (or SANY module) path matches one of `models` (or `modules`);
//! - every differing field matches one of `fields` (globs such as
//!   `counts.*`), possibly together with other entries that also apply;
//! - the optional `absent_result` / `present_result` conditions hold for
//!   the result of the build without / with the change.

use crate::compare::FieldDiff;
use crate::config::OraclesFile;
use crate::error::{Error, Result};
use crate::sany::SanyResult;
use crate::tlc::result::OracleResult;
use crate::util::glob_match;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

/// Every field a model or module comparison can report as differing. An
/// entry's `fields` globs must each match at least one of these.
pub const COMPARED_FIELDS: &[&str] = &[
    "verdict",
    "error_class",
    "violated_property",
    "counts.distinct_states",
    "counts.states_generated",
    "counts.states_left_on_queue",
    "counts.depth",
    "trace.length",
    "trace.ending",
    "trace.validated",
    "accepted",
    "unresolved_modules",
];

/// A condition on one oracle's result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultCondition {
    /// The verdict must be one of these (TLC or SANY verdict names, e.g.
    /// `parse-error`).
    #[serde(default)]
    pub verdict: Vec<String>,
    /// The `EC` error class must be one of these.
    #[serde(default)]
    pub error_class: Vec<String>,
    /// The result must name at least one unresolved module listed here, and
    /// every one it names must be listed here or by another entry that also
    /// applies.
    #[serde(default)]
    pub unresolved_modules: Vec<String>,
    /// The distinct-state count must be exactly this.
    #[serde(default)]
    pub distinct_states: Option<u64>,
}

/// One registered upstream change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleChange {
    /// Stable id, e.g. `alias-keyword`.
    pub id: String,
    /// The upstream commit that made the change (abbreviated or full SHA),
    /// or `<from>..<to>` when it lies somewhere in that range and is not
    /// attributed to one commit.
    pub upstream_commit: String,
    /// What changed, for a reader of the report.
    pub description: String,
    /// The entry in `docs/survey/oracle-history.md` or the ADR that records it.
    pub link: String,
    /// Pinned oracle ids whose builds lack the change.
    pub absent_in: Vec<String>,
    /// Pinned oracle ids whose builds have it.
    pub present_in: Vec<String>,
    /// Model path globs it covers.
    #[serde(default)]
    pub models: Vec<String>,
    /// SANY module path globs it covers.
    #[serde(default)]
    pub modules: Vec<String>,
    /// Globs of the compared fields it may change (see [`COMPARED_FIELDS`]).
    pub fields: Vec<String>,
    /// Condition on the result of the build without the change.
    #[serde(default)]
    pub absent_result: ResultCondition,
    /// Condition on the result of the build with the change.
    #[serde(default)]
    pub present_result: ResultCondition,
}

/// What a condition is checked against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultFacts {
    /// Verdict name.
    pub verdict: String,
    /// `EC` error class, for TLC results.
    pub error_class: Option<String>,
    /// Modules the oracle could not find.
    pub unresolved_modules: Vec<String>,
    /// Distinct states, for TLC results.
    pub distinct_states: Option<u64>,
}

fn name_of<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

impl From<&OracleResult> for ResultFacts {
    fn from(r: &OracleResult) -> Self {
        Self {
            verdict: r.verdict.as_str().to_owned(),
            error_class: r.error_class.clone(),
            unresolved_modules: r.unresolved_modules.clone(),
            distinct_states: r.counts.distinct_states,
        }
    }
}

impl From<&SanyResult> for ResultFacts {
    fn from(r: &SanyResult) -> Self {
        Self {
            verdict: name_of(&r.verdict),
            error_class: None,
            unresolved_modules: r.unresolved_modules.clone(),
            distinct_states: None,
        }
    }
}

/// Whether a comparison concerns a TLC model or a SANY module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
    /// A model (`.cfg`) compared on TLC results.
    Model,
    /// A module compared on SANY results.
    Module,
}

/// The oracles and results of one cross-oracle comparison.
#[derive(Debug, Clone, Copy)]
pub struct Comparison<'a> {
    /// Model or module.
    pub subject: Subject,
    /// Its corpus path.
    pub path: &'a str,
    /// The oracle that produced the golden.
    pub golden_oracle: &'a str,
    /// The oracle under test; `None` for any implementation that is not a
    /// TLC oracle, to which the registry never applies.
    pub implementation_oracle: Option<&'a str>,
    /// The golden's result.
    pub golden: &'a ResultFacts,
    /// The observed result.
    pub observed: &'a ResultFacts,
    /// The differing fields.
    pub diffs: &'a [FieldDiff],
}

impl ResultCondition {
    /// The condition apart from `unresolved_modules`, which needs the union
    /// over applicable entries.
    fn holds_locally(&self, r: &ResultFacts) -> bool {
        (self.verdict.is_empty() || self.verdict.contains(&r.verdict))
            && (self.error_class.is_empty()
                || r.error_class
                    .as_ref()
                    .is_some_and(|c| self.error_class.contains(c)))
            && self
                .distinct_states
                .is_none_or(|n| r.distinct_states == Some(n))
    }
}

/// The parsed `conformance/oracle-changes.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    /// Entries.
    #[serde(default)]
    pub change: Vec<OracleChange>,
}

fn is_commitish(s: &str) -> bool {
    (7..=40).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Registry {
    /// Parses and validates the registry against the pinned oracles.
    ///
    /// # Errors
    /// Fails on malformed TOML, a duplicate id, and on an entry that:
    /// - has an empty description, or a link outside `docs/`;
    /// - has an `upstream_commit` that is not a commit or `<from>..<to>`;
    /// - names an unpinned oracle, or the same oracle in `absent_in` and
    ///   `present_in`, or leaves either empty;
    /// - covers no model and no module;
    /// - lists no fields, or a field glob that matches no compared field.
    pub fn parse(text: &str, oracles: &OraclesFile) -> Result<Self> {
        let reg: Self =
            toml::from_str(text).map_err(|e| Error::Config(format!("oracle-changes.toml: {e}")))?;
        let pinned: BTreeSet<&str> = oracles.tlc.iter().map(|o| o.id.as_str()).collect();
        let mut ids = BTreeSet::new();
        for c in &reg.change {
            let bad = |why: String| Err(Error::Config(format!("oracle change `{}`: {why}", c.id)));
            if !ids.insert(c.id.as_str()) {
                return bad("duplicate id".to_owned());
            }
            if c.description.trim().is_empty() {
                return bad("empty description".to_owned());
            }
            if !c.link.starts_with("docs/") {
                return bad("link must point into docs/".to_owned());
            }
            let commit_ok = match c.upstream_commit.split_once("..") {
                Some((a, b)) => is_commitish(a) && is_commitish(b),
                None => is_commitish(&c.upstream_commit),
            };
            if !commit_ok {
                return bad(format!(
                    "upstream_commit `{}` is not a commit or <from>..<to>",
                    c.upstream_commit
                ));
            }
            if c.absent_in.is_empty() || c.present_in.is_empty() {
                return bad("absent_in and present_in must both name oracles".to_owned());
            }
            for id in c.absent_in.iter().chain(&c.present_in) {
                if !pinned.contains(id.as_str()) {
                    return bad(format!("`{id}` is not a pinned oracle"));
                }
            }
            if c.absent_in.iter().any(|a| c.present_in.contains(a)) {
                return bad("an oracle cannot both lack and have the change".to_owned());
            }
            if c.models.is_empty() && c.modules.is_empty() {
                return bad("covers no model and no module".to_owned());
            }
            if c.fields.is_empty() {
                return bad("lists no fields".to_owned());
            }
            for f in &c.fields {
                if !COMPARED_FIELDS.iter().any(|k| glob_match(f, k)) {
                    return bad(format!("field `{f}` matches no compared field"));
                }
            }
        }
        Ok(reg)
    }

    /// Loads `<repo>/conformance/oracle-changes.toml`; a missing file is an
    /// empty registry.
    ///
    /// # Errors
    /// Fails if the file exists but is unreadable or invalid.
    pub fn load(repo_root: &Path, oracles: &OraclesFile) -> Result<Self> {
        let path = repo_root.join("conformance").join("oracle-changes.toml");
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text, oracles),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(Error::io(&path, &e)),
        }
    }

    /// The entries that together explain every difference in `cmp`, or an
    /// empty list when the difference is not fully explained (it then stays
    /// `disagree`). Always empty for an implementation that is not a TLC
    /// oracle, and when nothing differs.
    #[must_use]
    pub fn explain(&self, cmp: &Comparison<'_>) -> Vec<&OracleChange> {
        let Some(imp) = cmp.implementation_oracle else {
            return Vec::new();
        };
        if cmp.diffs.is_empty() {
            return Vec::new();
        }
        // Entries that apply to these oracles, this path and these results,
        // with the result of the build lacking the change.
        let mut candidates: Vec<(&OracleChange, &ResultFacts)> = Vec::new();
        for c in &self.change {
            let has = |list: &[String], id: &str| list.iter().any(|x| x == id);
            let (absent, present) =
                if has(&c.absent_in, imp) && has(&c.present_in, cmp.golden_oracle) {
                    (cmp.observed, cmp.golden)
                } else if has(&c.present_in, imp) && has(&c.absent_in, cmp.golden_oracle) {
                    (cmp.golden, cmp.observed)
                } else {
                    continue;
                };
            let globs = match cmp.subject {
                Subject::Model => &c.models,
                Subject::Module => &c.modules,
            };
            if !globs.iter().any(|g| glob_match(g, cmp.path)) {
                continue;
            }
            if !c.absent_result.holds_locally(absent) || !c.present_result.holds_locally(present) {
                continue;
            }
            candidates.push((c, absent));
        }
        // `unresolved_modules` conditions: the absent build's unresolved
        // modules must all be explained by the applicable entries together.
        let allowed: BTreeSet<&str> = candidates
            .iter()
            .flat_map(|(c, _)| {
                c.absent_result
                    .unresolved_modules
                    .iter()
                    .map(String::as_str)
            })
            .collect();
        candidates.retain(|(c, absent)| {
            c.absent_result.unresolved_modules.is_empty()
                || (absent
                    .unresolved_modules
                    .iter()
                    .any(|m| c.absent_result.unresolved_modules.contains(m))
                    && absent
                        .unresolved_modules
                        .iter()
                        .all(|m| allowed.contains(m.as_str())))
        });
        let covers = |c: &OracleChange, field: &str| c.fields.iter().any(|g| glob_match(g, field));
        let all_covered = cmp
            .diffs
            .iter()
            .all(|d| candidates.iter().any(|(c, _)| covers(c, &d.field)));
        if !all_covered {
            return Vec::new();
        }
        candidates
            .into_iter()
            .map(|(c, _)| c)
            .filter(|c| cmp.diffs.iter().any(|d| covers(c, &d.field)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    const PIN: &str = "tlc-2026.10.06.014338";
    const OLD: &str = "tlc-1.7.4";
    const NONE: [String; 0] = [];

    fn ids(found: &[&OracleChange]) -> Vec<String> {
        found.iter().map(|c| c.id.clone()).collect()
    }

    fn oracles() -> Result<OraclesFile> {
        Ok(Config::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))?.oracles)
    }

    const ENTRY: &str = r#"
[[change]]
id = "depth-zero-states"
upstream_commit = "508aaf05f"
description = "depth 0 when no state was generated"
link = "docs/survey/oracle-history.md"
absent_in = ["tlc-1.7.4"]
present_in = ["tlc-2026.10.06.014338"]
models = ["specifications/**"]
fields = ["counts.depth"]
present_result = { distinct_states = 0 }
"#;

    fn facts(verdict: &str, distinct: Option<u64>, unresolved: &[&str]) -> ResultFacts {
        ResultFacts {
            verdict: verdict.to_owned(),
            error_class: None,
            unresolved_modules: unresolved.iter().map(|s| (*s).to_owned()).collect(),
            distinct_states: distinct,
        }
    }

    fn diff(field: &str) -> FieldDiff {
        FieldDiff {
            field: field.to_owned(),
            expected: serde_json::Value::Null,
            actual: serde_json::Value::Null,
        }
    }

    fn cmp<'a>(
        imp: Option<&'a str>,
        golden: &'a ResultFacts,
        observed: &'a ResultFacts,
        diffs: &'a [FieldDiff],
    ) -> Comparison<'a> {
        Comparison {
            subject: Subject::Model,
            path: "specifications/Stones/Stones.cfg",
            golden_oracle: PIN,
            implementation_oracle: imp,
            golden,
            observed,
            diffs,
        }
    }

    #[test]
    fn registered_change_is_explained() -> Result<()> {
        let r = Registry::parse(ENTRY, &oracles()?)?;
        let g = facts("success", Some(0), &[]);
        let o = facts("success", Some(0), &[]);
        let d = [diff("counts.depth")];
        let ids: Vec<&str> = r
            .explain(&cmp(Some(OLD), &g, &o, &d))
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        assert_eq!(ids, ["depth-zero-states"]);
        // Either direction: the pin run against goldens of the old build.
        let mut rev = cmp(Some(PIN), &o, &g, &d);
        rev.golden_oracle = OLD;
        assert_eq!(r.explain(&rev).len(), 1);
        Ok(())
    }

    #[test]
    fn unregistered_difference_stays_disagree() -> Result<()> {
        let r = Registry::parse(ENTRY, &oracles()?)?;
        let g = facts("success", Some(0), &[]);
        let o = facts("success", Some(0), &[]);
        // A field the entry does not cover, alone or with a covered one.
        assert_eq!(
            ids(&r.explain(&cmp(Some(OLD), &g, &o, &[diff("counts.distinct_states")]))),
            NONE
        );
        let both = [diff("counts.depth"), diff("verdict")];
        assert_eq!(ids(&r.explain(&cmp(Some(OLD), &g, &o, &both))), NONE);
        // The condition fails: states were generated.
        let g216 = facts("success", Some(216), &[]);
        assert_eq!(
            ids(&r.explain(&cmp(Some(OLD), &g216, &o, &[diff("counts.depth")]))),
            NONE
        );
        // Another path.
        let mut other = cmp(Some(OLD), &g, &o, &[]);
        let d = [diff("counts.depth")];
        other.diffs = &d;
        other.path = "elsewhere/X.cfg";
        assert_eq!(ids(&r.explain(&other)), NONE);
        // The same oracle on both sides is not a cross-oracle change.
        assert_eq!(ids(&r.explain(&cmp(Some(PIN), &g, &o, &d))), NONE);
        // Nothing differs: nothing to explain.
        assert_eq!(ids(&r.explain(&cmp(Some(OLD), &g, &o, &[]))), NONE);
        Ok(())
    }

    #[test]
    fn never_applies_to_non_oracle_implementations() -> Result<()> {
        let r = Registry::parse(ENTRY, &oracles()?)?;
        let g = facts("success", Some(0), &[]);
        let o = facts("success", Some(0), &[]);
        let d = [diff("counts.depth")];
        assert_eq!(ids(&r.explain(&cmp(None, &g, &o, &d))), NONE);
        Ok(())
    }

    #[test]
    fn unresolved_modules_are_explained_together() -> Result<()> {
        let text = r#"
[[change]]
id = "a"
upstream_commit = "856398af6"
description = "TLCExt is standard"
link = "docs/survey/oracle-history.md"
absent_in = ["tlc-1.7.4"]
present_in = ["tlc-2026.10.06.014338"]
modules = ["**"]
fields = ["accepted", "verdict", "unresolved_modules"]
absent_result = { verdict = ["parse-error"], unresolved_modules = ["TLCExt"] }

[[change]]
id = "b"
upstream_commit = "eb3ff99d16..94d0c50"
description = "CommunityModules need a newer TLC"
link = "docs/decisions/0003-golden-format-and-provenance.md"
absent_in = ["tlc-1.7.4"]
present_in = ["tlc-2026.10.06.014338"]
modules = ["**"]
fields = ["accepted", "verdict", "unresolved_modules"]
absent_result = { verdict = ["parse-error"], unresolved_modules = ["Functions"] }
"#;
        let r = Registry::parse(text, &oracles()?)?;
        let g = facts("accept", None, &[]);
        let d = [
            diff("accepted"),
            diff("verdict"),
            diff("unresolved_modules"),
        ];
        let module = |o: &'_ ResultFacts| -> Vec<String> {
            let mut c = cmp(Some(OLD), &g, o, &d);
            c.subject = Subject::Module;
            r.explain(&c).iter().map(|c| c.id.clone()).collect()
        };
        assert_eq!(module(&facts("parse-error", None, &["TLCExt"])), ["a"]);
        assert_eq!(
            module(&facts("parse-error", None, &["TLCExt", "Functions"])),
            ["a", "b"]
        );
        // A module no entry lists, or none at all: not explained.
        assert_eq!(module(&facts("parse-error", None, &["Common"])), NONE);
        assert_eq!(
            module(&facts("parse-error", None, &["TLCExt", "Common"])),
            NONE
        );
        assert_eq!(module(&facts("parse-error", None, &[])), NONE);
        Ok(())
    }

    #[test]
    fn registry_validates() -> Result<()> {
        let o = oracles()?;
        assert!(Registry::parse(ENTRY, &o).is_ok());
        for (from, to) in [
            (
                "upstream_commit = \"508aaf05f\"",
                "upstream_commit = \"HEAD\"",
            ),
            (
                "upstream_commit = \"508aaf05f\"",
                "upstream_commit = \"508aaf05f..\"",
            ),
            ("absent_in = [\"tlc-1.7.4\"]", "absent_in = [\"tlc-9\"]"),
            ("absent_in = [\"tlc-1.7.4\"]", "absent_in = []"),
            (
                "absent_in = [\"tlc-1.7.4\"]",
                "absent_in = [\"tlc-2026.10.06.014338\"]",
            ),
            ("models = [\"specifications/**\"]", ""),
            ("fields = [\"counts.depth\"]", "fields = [\"count.depth\"]"),
            ("fields = [\"counts.depth\"]", "fields = []"),
            (
                "description = \"depth 0 when no state was generated\"",
                "description = \" \"",
            ),
            (
                "link = \"docs/survey/oracle-history.md\"",
                "link = \"https://example.com\"",
            ),
        ] {
            assert!(
                Registry::parse(&ENTRY.replace(from, to), &o).is_err(),
                "{to}"
            );
        }
        assert!(Registry::parse(&format!("{ENTRY}{ENTRY}"), &o).is_err());
        assert!(Registry::parse(&format!("{ENTRY}\nextra = 1\n"), &o).is_err());
        Ok(())
    }

    #[test]
    fn repository_registry_is_valid() -> Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let r = Registry::load(&root, &oracles()?)?;
        assert!(r.change.len() >= 4);
        Ok(())
    }
}
