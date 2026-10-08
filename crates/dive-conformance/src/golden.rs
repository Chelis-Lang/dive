//! Golden files (ADR 0003).
//!
//! A golden is pretty-printed JSON with a fixed key order. Everything except
//! the trailing `provenance` object is the *payload*: deterministic, free of
//! timestamps, timings and machine paths, and the only part ever compared.
//!
//! ```text
//! conformance/golden/<oracle-id>/<corpus-id>/<model-path>.json       TLC model
//! conformance/golden/<oracle-id>/<corpus-id>/sany/<module-path>.json SANY module
//! ```
//!
//! Path components are sanitised to `[A-Za-z0-9._-]`. Existing goldens are
//! never overwritten without a `--reason`.

use crate::corpus::{ModelEntry, ModelMode};
use crate::error::{Error, Result};
use crate::oracle::OracleProvenance;
use crate::sany::SanyResult;
use crate::tlc::result::OracleResult;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Version of the golden format. Bump on any incompatible change.
pub const GOLDEN_SCHEMA: u32 = 1;

/// How a golden was produced. Never compared.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// The oracle, its pins, capabilities and Java.
    pub oracle: OracleProvenance,
    /// The corpus commit the input came from.
    pub corpus_commit: String,
    /// The exact command line, with machine paths as placeholders
    /// (`$DIVE_CACHE_DIR`, `$TMP`, `$CWD`).
    pub command: Vec<String>,
    /// The harness timeout in seconds.
    pub timeout_seconds: u64,
    /// The recorded reason for (re)generation, if given.
    pub reason: Option<String>,
    /// `dive-conformance <version>`.
    pub generator: String,
    /// When the golden was generated (RFC 3339 UTC).
    pub generated_at: String,
    /// Wall-clock time of the oracle run in milliseconds.
    pub elapsed_ms: u64,
    /// State counts observed but kept out of the payload because they are
    /// not reproducible: in simulation TLC's generated-state count varies
    /// between runs even with `-seed 0` and one worker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncompared_counts: Option<crate::tlc::result::Counts>,
}

/// The model a TLC golden is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldenModel {
    /// Model id (corpus-relative `.cfg` path).
    pub id: String,
    /// Corpus-relative module path.
    pub module: String,
    /// Mode.
    pub mode: ModelMode,
    /// Module features.
    pub features: Vec<String>,
    /// Whether state counts are comparable.
    pub counts_comparable: bool,
}

impl From<&ModelEntry> for GoldenModel {
    fn from(m: &ModelEntry) -> Self {
        Self {
            id: m.id.clone(),
            module: m.module.clone(),
            mode: m.mode,
            features: m.features.clone(),
            counts_comparable: m.counts_comparable,
        }
    }
}

/// A TLC golden.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelGolden {
    /// [`GOLDEN_SCHEMA`].
    pub schema: u32,
    /// Always `tlc-model`.
    pub kind: String,
    /// Oracle id.
    pub oracle: String,
    /// Corpus id.
    pub corpus: String,
    /// The model.
    pub model: GoldenModel,
    /// `-workers` (always 1 for goldens, ADR 0004).
    pub workers: u32,
    /// The oracle's result.
    pub result: OracleResult,
    /// Provenance; not compared.
    pub provenance: Provenance,
}

/// A SANY golden.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SanyGolden {
    /// [`GOLDEN_SCHEMA`].
    pub schema: u32,
    /// Always `sany-module`.
    pub kind: String,
    /// Oracle id.
    pub oracle: String,
    /// Corpus id.
    pub corpus: String,
    /// Corpus-relative module path.
    pub module: String,
    /// SANY's result.
    pub result: SanyResult,
    /// Provenance; not compared.
    pub provenance: Provenance,
}

/// Encodes a corpus-relative path for use under the golden root, keeping
/// its directory structure. Bytes outside `[A-Za-z0-9._-]` (including `%`)
/// are percent-encoded per UTF-8 byte, so the mapping is injective:
/// `a b/M.cfg` and `a_b/M.cfg` stay distinct. Empty, `.` and `..`
/// components are dropped; corpus enumeration rejects such paths anyway.
#[must_use]
pub fn sanitise(path: &str) -> String {
    use std::fmt::Write as _;
    path.split('/')
        .filter(|c| !c.is_empty() && *c != "." && *c != "..")
        .map(|c| {
            let mut out = String::with_capacity(c.len());
            for b in c.bytes() {
                if b.is_ascii_alphanumeric() || b"._-".contains(&b) {
                    out.push(char::from(b));
                } else {
                    let _ = write!(out, "%{b:02X}");
                }
            }
            out
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// The golden root, `<repo>/conformance/golden`.
#[must_use]
pub fn golden_root(repo_root: &Path) -> PathBuf {
    repo_root.join("conformance").join("golden")
}

/// Path of the TLC golden for `model_id`.
#[must_use]
pub fn model_golden_path(root: &Path, oracle: &str, corpus: &str, model_id: &str) -> PathBuf {
    root.join(oracle)
        .join(corpus)
        .join(format!("{}.json", sanitise(model_id)))
}

/// Path of the SANY golden for `module`.
#[must_use]
pub fn sany_golden_path(root: &Path, oracle: &str, corpus: &str, module: &str) -> PathBuf {
    root.join(oracle)
        .join(corpus)
        .join("sany")
        .join(format!("{}.json", sanitise(module)))
}

/// Serialises a golden as pretty JSON with a trailing newline.
///
/// # Errors
/// Fails only if serialisation fails.
pub fn to_pretty<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string_pretty(value)
        .map(|s| s + "\n")
        .map_err(|e| Error::Golden(format!("cannot serialise golden: {e}")))
}

/// The payload of a serialised golden: everything except `provenance`, as
/// canonical JSON.
///
/// # Errors
/// Fails if `json` is not a JSON object.
pub fn payload(json: &str) -> Result<String> {
    let mut v: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| Error::Golden(format!("golden is not JSON: {e}")))?;
    let obj = v
        .as_object_mut()
        .ok_or_else(|| Error::Golden("golden is not a JSON object".to_owned()))?;
    obj.remove("provenance");
    Ok(v.to_string())
}

/// What writing a golden did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteStatus {
    /// No golden existed.
    Created,
    /// A golden existed and its payload changed.
    Replaced,
    /// A golden existed and only its provenance changed.
    ReplacedSamePayload,
}

/// Writes `text` to `path`, refusing to overwrite unless `reason` is given.
///
/// # Errors
/// Fails if a golden exists and `reason` is `None`, or on I/O failure.
pub fn write_golden(path: &Path, text: &str, reason: Option<&str>) -> Result<WriteStatus> {
    let status = if path.exists() {
        if reason.is_none_or(|r| r.trim().is_empty()) {
            return Err(Error::Golden(format!(
                "{} exists; regenerating a golden requires --reason \"<why>\"",
                path.display()
            )));
        }
        let old = std::fs::read_to_string(path).map_err(|e| Error::io(path, &e))?;
        if payload(&old).ok() == payload(text).ok() {
            WriteStatus::ReplacedSamePayload
        } else {
            WriteStatus::Replaced
        }
    } else {
        WriteStatus::Created
    };
    crate::util::write_atomic(path, text.as_bytes())?;
    Ok(status)
}

/// Reads a TLC golden.
///
/// # Errors
/// Fails if the file is unreadable, malformed, or of another schema.
pub fn read_model_golden(path: &Path) -> Result<ModelGolden> {
    let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, &e))?;
    let g: ModelGolden = serde_json::from_str(&text)
        .map_err(|e| Error::Golden(format!("{}: {e}", path.display())))?;
    if g.schema != GOLDEN_SCHEMA || g.kind != "tlc-model" {
        return Err(Error::Golden(format!(
            "{}: unsupported golden schema {} / kind {}",
            path.display(),
            g.schema,
            g.kind
        )));
    }
    Ok(g)
}

/// Reads a SANY golden.
///
/// # Errors
/// Fails if the file is unreadable, malformed, or of another schema.
pub fn read_sany_golden(path: &Path) -> Result<SanyGolden> {
    let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, &e))?;
    let g: SanyGolden = serde_json::from_str(&text)
        .map_err(|e| Error::Golden(format!("{}: {e}", path.display())))?;
    if g.schema != GOLDEN_SCHEMA || g.kind != "sany-module" {
        return Err(Error::Golden(format!(
            "{}: unsupported golden schema {} / kind {}",
            path.display(),
            g.schema,
            g.kind
        )));
    }
    Ok(g)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitising_paths() {
        assert_eq!(
            sanitise("specifications/N-Queens/Queens.toolbox/FourQueens/MC.cfg"),
            "specifications/N-Queens/Queens.toolbox/FourQueens/MC.cfg"
        );
        assert_eq!(sanitise("a b/../c$d"), "a%20b/c%24d");
        assert_ne!(sanitise("a b/M.cfg"), sanitise("a_b/M.cfg"));
        assert_ne!(sanitise("a%20b"), sanitise("a b"));
        let root = Path::new("/g");
        assert_eq!(
            model_golden_path(root, "o", "examples", "specifications/x/X.cfg"),
            PathBuf::from("/g/o/examples/specifications/x/X.cfg.json")
        );
        assert_eq!(
            sany_golden_path(root, "o", "examples", "specifications/x/X.tla"),
            PathBuf::from("/g/o/examples/sany/specifications/x/X.tla.json")
        );
    }

    #[test]
    fn overwrite_requires_reason() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = crate::util::TempDir::new("golden")?;
        let p = dir.path().join("a/b.json");
        let a = "{\"x\":1,\"provenance\":{\"t\":1}}";
        let b = "{\"x\":1,\"provenance\":{\"t\":2}}";
        let c = "{\"x\":2,\"provenance\":{\"t\":3}}";
        assert_eq!(write_golden(&p, a, None)?, WriteStatus::Created);
        assert!(matches!(write_golden(&p, b, None), Err(Error::Golden(_))));
        assert!(matches!(
            write_golden(&p, b, Some("  ")),
            Err(Error::Golden(_))
        ));
        assert_eq!(
            write_golden(&p, b, Some("r"))?,
            WriteStatus::ReplacedSamePayload
        );
        assert_eq!(write_golden(&p, c, Some("r"))?, WriteStatus::Replaced);
        assert_eq!(std::fs::read_to_string(&p)?, c);
        Ok(())
    }
}
