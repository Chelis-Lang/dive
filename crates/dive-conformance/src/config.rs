//! `conformance/oracles.toml` and `conformance/corpora.toml`.
//!
//! Both files are parsed strictly: unknown keys are errors, ids must be
//! unique, digests must be lower-case SHA-256 hex and commits full 40-hex
//! SHA-1s, so a typo can never silently select a different artifact.

use crate::error::{Error, Result};
use crate::hash::is_sha256_hex;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// What an oracle is for (ADR 0002, revision "the 1.8.x line is the oracle").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// Its goldens and runs decide conformance; disagreements are findings.
    Gating,
    /// Kept for comparison only. It never gates: the harness touches it only
    /// when it is named explicitly (`--oracle`, `--golden-oracle`,
    /// `--impl tlc:<id>`), its runs never produce failing findings, and
    /// reports mark it as reference.
    Reference,
}

impl Role {
    /// The kebab-case name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gating => "gating",
            Self::Reference => "reference",
        }
    }
}

/// One pinned TLC oracle (`[[tlc]]`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TlcOracle {
    /// Stable oracle id, e.g. `tlc-1.7.4`. Goldens live under this id.
    pub id: String,
    /// Whether goldens are generated from this oracle by default. Only a
    /// gating oracle may be the default.
    #[serde(default)]
    pub default: bool,
    /// `gating` or `reference`; required, so no oracle gates by accident.
    pub role: Role,
    /// Human description.
    pub description: String,
    /// Upstream commit or tag the jar was built from.
    pub upstream_commit: String,
    /// Download URL of `tla2tools.jar`.
    pub url: String,
    /// Pinned SHA-256 of the jar.
    pub sha256: String,
    /// Ids of `[[classpath]]` jars put on `-cp` after the oracle jar, in order.
    pub classpath: Vec<String>,
}

/// A pinned downloadable artifact (`[[classpath]]`, or a JRE platform).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    /// Stable id. JRE platforms take their id from the table key.
    #[serde(default)]
    pub id: String,
    /// Download URL.
    pub url: String,
    /// Pinned SHA-256.
    pub sha256: String,
}

/// A TLA+ module library fetched with git at a pinned commit
/// (`[[module_library]]`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleLibrary {
    /// Stable id.
    pub id: String,
    /// Git repository URL.
    pub repo: String,
    /// Full commit SHA.
    pub commit: String,
    /// Directory inside the checkout that holds the `.tla` files.
    pub subdir: String,
    /// SPDX licence of the library.
    pub license: String,
}

/// The pinned Java runtime (`[jre]`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JreConfig {
    /// Temurin version, e.g. `17.0.20.1+1`.
    pub version: String,
    /// Per-platform archives keyed by `<os>-<arch>` (e.g. `macos-aarch64`).
    pub platforms: BTreeMap<String, Artifact>,
}

/// The parsed `oracles.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OraclesFile {
    /// TLC oracles.
    pub tlc: Vec<TlcOracle>,
    /// Classpath jars.
    #[serde(default)]
    pub classpath: Vec<Artifact>,
    /// Module libraries.
    #[serde(default)]
    pub module_library: Vec<ModuleLibrary>,
    /// The Java runtime.
    pub jre: JreConfig,
}

/// How a corpus is laid out, which decides how it is enumerated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layout {
    /// tlaplus/Examples: `specifications/*/manifest.json` (ADR 0005).
    ExamplesManifest,
    /// Every `.tla` file is a module to parse; there are no models.
    ModulesOnly,
    /// Not yet supported; enumeration reports this explicitly.
    Tbd,
}

/// One pinned corpus (`[[corpus]]`): either an upstream git repository
/// pinned by commit (`repo` and `commit`), or a dive-owned corpus kept in
/// this repository (`path`), identified by the digest of its files
/// ([`crate::corpus::tree_digest`]).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// Stable id, e.g. `examples`.
    pub id: String,
    /// Git repository URL. Empty for an in-repository corpus.
    #[serde(default)]
    pub repo: String,
    /// Full commit SHA. Empty for an in-repository corpus.
    #[serde(default)]
    pub commit: String,
    /// In-repository corpus: its directory, relative to the repository root
    /// (under `conformance/`). Mutually exclusive with `repo` and `commit`.
    #[serde(default)]
    pub path: Option<String>,
    /// SPDX licence.
    pub license: String,
    /// Layout.
    pub layout: Layout,
    /// `modules-only`: globs over corpus-relative paths selecting the
    /// modules; empty means every `.tla` file.
    #[serde(default)]
    pub include: Vec<String>,
    /// `modules-only`: modules that are enumerated but never run, each with
    /// its reason. They are reported as `out-of-scope`, never dropped.
    #[serde(default)]
    pub exclude: Vec<Exclusion>,
}

/// A module excluded from a corpus (`[[corpus.exclude]]`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exclusion {
    /// Corpus-relative path of the excluded `.tla` file.
    pub path: String,
    /// Why it is excluded, with evidence (e.g. the oracle's crash).
    pub reason: String,
}

/// The parsed `corpora.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorporaFile {
    /// Corpora.
    pub corpus: Vec<Corpus>,
}

/// Both configuration files plus the repository root they came from.
#[derive(Debug, Clone)]
pub struct Config {
    /// Repository root (the directory containing `conformance/`).
    pub repo_root: PathBuf,
    /// Oracles.
    pub oracles: OraclesFile,
    /// Corpora.
    pub corpora: CorporaFile,
}

fn is_commit(s: &str) -> bool {
    s.len() == 40
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn check_unique<'a>(kind: &str, ids: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if id.is_empty()
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(Error::Config(format!(
                "{kind} id `{id}` must be non-empty [A-Za-z0-9._-]"
            )));
        }
        if !seen.insert(id) {
            return Err(Error::Config(format!("duplicate {kind} id `{id}`")));
        }
    }
    Ok(())
}

fn check_digest(what: &str, sha: &str) -> Result<()> {
    if is_sha256_hex(sha) {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "{what}: `{sha}` is not a lower-case SHA-256"
        )))
    }
}

impl OraclesFile {
    /// Parses and validates `oracles.toml` text.
    ///
    /// # Errors
    /// Fails on malformed TOML or any inconsistency.
    pub fn parse(text: &str) -> Result<Self> {
        let mut file: Self =
            toml::from_str(text).map_err(|e| Error::Config(format!("oracles.toml: {e}")))?;
        for (key, artifact) in &mut file.jre.platforms {
            if !artifact.id.is_empty() {
                return Err(Error::Config(format!(
                    "jre platform `{key}` must not set `id`"
                )));
            }
            artifact.id = format!("jre-{key}");
        }
        file.validate()?;
        Ok(file)
    }

    fn validate(&self) -> Result<()> {
        check_unique("tlc", self.tlc.iter().map(|o| o.id.as_str()))?;
        check_unique("classpath", self.classpath.iter().map(|a| a.id.as_str()))?;
        check_unique(
            "module_library",
            self.module_library.iter().map(|l| l.id.as_str()),
        )?;
        let defaults = self.tlc.iter().filter(|o| o.default).count();
        if defaults != 1 {
            return Err(Error::Config(format!(
                "exactly one [[tlc]] must have default = true (found {defaults})"
            )));
        }
        if let Some(o) = self
            .tlc
            .iter()
            .find(|o| o.default && o.role != Role::Gating)
        {
            return Err(Error::Config(format!(
                "tlc `{}`: the default oracle must have role = \"gating\"",
                o.id
            )));
        }
        let classpath_ids: BTreeSet<&str> = self.classpath.iter().map(|a| a.id.as_str()).collect();
        for o in &self.tlc {
            check_digest(&format!("tlc `{}`", o.id), &o.sha256)?;
            for c in &o.classpath {
                if !classpath_ids.contains(c.as_str()) {
                    return Err(Error::Config(format!(
                        "tlc `{}` names unknown classpath id `{c}`",
                        o.id
                    )));
                }
            }
        }
        for a in &self.classpath {
            check_digest(&format!("classpath `{}`", a.id), &a.sha256)?;
        }
        for (k, a) in &self.jre.platforms {
            check_digest(&format!("jre platform `{k}`"), &a.sha256)?;
        }
        for l in &self.module_library {
            if !is_commit(&l.commit) {
                return Err(Error::Config(format!(
                    "module_library `{}`: commit must be a full SHA",
                    l.id
                )));
            }
            if l.subdir.split('/').any(|c| c == ".." || c.is_empty()) {
                return Err(Error::Config(format!(
                    "module_library `{}`: bad subdir",
                    l.id
                )));
            }
        }
        Ok(())
    }

    /// The default oracle.
    ///
    /// # Errors
    /// Never in a validated file; kept fallible for robustness.
    pub fn default_oracle(&self) -> Result<&TlcOracle> {
        self.tlc
            .iter()
            .find(|o| o.default)
            .ok_or_else(|| Error::Config("no default oracle".to_owned()))
    }

    /// The oracles a command touches when none is named: the gating ones,
    /// or exactly `selected` when given (a reference oracle runs only when
    /// selected explicitly).
    ///
    /// # Errors
    /// Fails if `selected` names no pinned oracle.
    pub fn selected_or_gating(&self, selected: Option<&str>) -> Result<Vec<&TlcOracle>> {
        match selected {
            Some(id) => Ok(vec![self.oracle(id)?]),
            None => Ok(self.tlc.iter().filter(|o| o.role == Role::Gating).collect()),
        }
    }

    /// The oracle with id `id`.
    ///
    /// # Errors
    /// Fails if no such oracle is pinned.
    pub fn oracle(&self, id: &str) -> Result<&TlcOracle> {
        self.tlc.iter().find(|o| o.id == id).ok_or_else(|| {
            Error::Config(format!(
                "unknown oracle `{id}`; pinned: {}",
                self.tlc
                    .iter()
                    .map(|o| o.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })
    }

    /// The classpath artifact with id `id`.
    ///
    /// # Errors
    /// Fails if no such artifact is pinned.
    pub fn classpath_artifact(&self, id: &str) -> Result<&Artifact> {
        self.classpath
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| Error::Config(format!("unknown classpath id `{id}`")))
    }

    /// The JRE artifact for the host platform.
    ///
    /// # Errors
    /// Fails if the host platform has no pinned JRE.
    pub fn host_jre(&self) -> Result<&Artifact> {
        let key = host_platform_key();
        self.jre.platforms.get(&key).ok_or_else(|| {
            Error::Config(format!(
                "no JRE pinned for host platform `{key}`; use --java <path> to opt into a system Java"
            ))
        })
    }
}

/// The `<os>-<arch>` key of the host, as used in `[jre.platforms]`.
#[must_use]
pub fn host_platform_key() -> String {
    let os = std::env::consts::OS;
    format!("{os}-{}", std::env::consts::ARCH)
}

impl CorporaFile {
    /// Parses and validates `corpora.toml` text.
    ///
    /// # Errors
    /// Fails on malformed TOML or any inconsistency.
    pub fn parse(text: &str) -> Result<Self> {
        let file: Self =
            toml::from_str(text).map_err(|e| Error::Config(format!("corpora.toml: {e}")))?;
        check_unique("corpus", file.corpus.iter().map(|c| c.id.as_str()))?;
        for c in &file.corpus {
            match &c.path {
                None if !is_commit(&c.commit) => {
                    return Err(Error::Config(format!(
                        "corpus `{}`: commit must be a full SHA",
                        c.id
                    )));
                }
                Some(_) if !c.repo.is_empty() || !c.commit.is_empty() => {
                    return Err(Error::Config(format!(
                        "corpus `{}`: give `path` (in this repository) or `repo` and `commit`, not both",
                        c.id
                    )));
                }
                Some(p)
                    if !p.starts_with("conformance/")
                        || p.split('/').any(|c| c == ".." || c == "." || c.is_empty()) =>
                {
                    return Err(Error::Config(format!(
                        "corpus `{}`: path `{p}` must be a plain relative path under conformance/",
                        c.id
                    )));
                }
                None | Some(_) => {}
            }
            if c.layout != Layout::ModulesOnly && (!c.include.is_empty() || !c.exclude.is_empty()) {
                return Err(Error::Config(format!(
                    "corpus `{}`: `include` and `exclude` apply to the modules-only layout only",
                    c.id
                )));
            }
            if let Some(x) = c.exclude.iter().find(|x| x.reason.trim().is_empty()) {
                return Err(Error::Config(format!(
                    "corpus `{}`: exclusion `{}` needs a reason",
                    c.id, x.path
                )));
            }
        }
        Ok(file)
    }

    /// The corpus with id `id`.
    ///
    /// # Errors
    /// Fails if no such corpus is pinned.
    pub fn corpus(&self, id: &str) -> Result<&Corpus> {
        self.corpus
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| Error::Config(format!("unknown corpus `{id}`")))
    }
}

impl Config {
    /// Loads both files from `<repo_root>/conformance/`.
    ///
    /// # Errors
    /// Fails if either file is missing or invalid.
    pub fn load(repo_root: &Path) -> Result<Self> {
        let dir = repo_root.join("conformance");
        let read = |name: &str| {
            let p = dir.join(name);
            std::fs::read_to_string(&p).map_err(|e| Error::io(&p, &e))
        };
        Ok(Self {
            repo_root: repo_root.to_path_buf(),
            oracles: OraclesFile::parse(&read("oracles.toml")?)?,
            corpora: CorporaFile::parse(&read("corpora.toml")?)?,
        })
    }

    /// Finds the repository root by walking up from `start` to the first
    /// directory containing `conformance/oracles.toml`.
    ///
    /// # Errors
    /// Fails if no such directory exists.
    pub fn find_repo_root(start: &Path) -> Result<PathBuf> {
        let mut dir = Some(start);
        while let Some(d) = dir {
            if d.join("conformance").join("oracles.toml").is_file() {
                return Ok(d.to_path_buf());
            }
            dir = d.parent();
        }
        Err(Error::Config(format!(
            "no conformance/oracles.toml above {}; pass --repo-root",
            start.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn repository_pins_parse() -> Result<()> {
        let cfg = Config::load(&repo_root())?;
        assert_eq!(cfg.oracles.default_oracle()?.id, "tlc-2026.10.06.014338");
        assert_eq!(
            cfg.oracles.oracle("tlc-1.7.4")?.classpath,
            Vec::<String>::new()
        );
        assert!(cfg.oracles.jre.platforms.contains_key("linux-x86_64"));
        assert_eq!(cfg.oracles.oracle("tlc-1.7.4")?.role, Role::Reference);
        assert_eq!(
            cfg.oracles.default_oracle()?.role,
            Role::Gating,
            "the default oracle gates"
        );
        let implicit: Vec<&str> = cfg
            .oracles
            .selected_or_gating(None)?
            .iter()
            .map(|o| o.id.as_str())
            .collect();
        assert_eq!(implicit, ["tlc-2026.10.06.014338"]);
        let named = cfg.oracles.selected_or_gating(Some("tlc-1.7.4"))?;
        assert_eq!(named[0].role, Role::Reference);
        assert_eq!(
            cfg.corpora.corpus("examples")?.layout,
            Layout::ExamplesManifest
        );
        assert_eq!(
            cfg.corpora.corpus("probes")?.path.as_deref(),
            Some("conformance/corpus/probes")
        );
        Ok(())
    }

    #[test]
    fn in_repository_corpora() {
        let ok = "[[corpus]]\nid = \"p\"\npath = \"conformance/corpus/p\"\nlicense = \"MIT\"\nlayout = \"examples-manifest\"\n";
        let file = CorporaFile::parse(ok);
        assert!(file.is_ok_and(|f| f.corpus[0].path.is_some()));
        let sha = "0".repeat(40);
        for bad in [
            ok.replace("path = ", &format!("commit = \"{sha}\"\npath = ")),
            ok.replace("path = ", "repo = \"https://x\"\npath = "),
            ok.replace("conformance/corpus/p", "conformance/../p"),
            ok.replace("conformance/corpus/p", "/abs/conformance/p"),
            ok.replace("conformance/corpus/p", "elsewhere/p"),
            ok.replace("path = \"conformance/corpus/p\"\n", ""),
        ] {
            assert!(CorporaFile::parse(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn rejects_unknown_keys_and_bad_digests() {
        let base = r#"
[[tlc]]
id = "a"
default = true
role = "gating"
description = "d"
upstream_commit = "x"
url = "https://example.invalid/a.jar"
sha256 = "SHA"
classpath = []
[jre]
version = "1"
[jre.platforms]
"#;
        assert!(matches!(OraclesFile::parse(base), Err(Error::Config(_))));
        let good = base.replace("\"SHA\"", &format!("\"{}\"", "0".repeat(64)));
        assert!(OraclesFile::parse(&good).is_ok());
        let unknown = format!("{good}\n[extra]\nx = 1\n");
        assert!(OraclesFile::parse(&unknown).is_err());
        let bad_cp = good.replace("classpath = []", "classpath = [\"nope\"]");
        assert!(OraclesFile::parse(&bad_cp).is_err());
        let no_role = good.replace("role = \"gating\"\n", "");
        assert!(OraclesFile::parse(&no_role).is_err(), "role is required");
        let bad_role = good.replace("role = \"gating\"", "role = \"advisory\"");
        assert!(OraclesFile::parse(&bad_role).is_err());
        let reference_default = good.replace("role = \"gating\"", "role = \"reference\"");
        assert!(
            OraclesFile::parse(&reference_default).is_err(),
            "a reference oracle cannot be the default"
        );
    }
}
