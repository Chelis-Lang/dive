//! Explicit publication of admitted PlusCal artifacts (ADR 0144).
use crate::{ApiError, RewriteReport, RewriteRequest, rewrite_source};
use serde_json::json;
use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_BYTES: u64 = 4_194_304;
const CFG_DELIMITER: &str = "\\* Add statements after this line.";
static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

fn io(path: &Path, error: std::io::Error) -> ApiError {
    ApiError::Io {
        path: path.display().to_string(),
        error,
    }
}
fn rejected(message: &str) -> ApiError {
    ApiError::Rejected {
        message: format!("DIVE-P0100: {message}"),
    }
}

#[derive(Debug)]
struct Snapshot {
    bytes: Option<Vec<u8>>,
    permissions: Option<fs::Permissions>,
}
impl Snapshot {
    fn read(path: &Path) -> Result<Self, ApiError> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self {
                    bytes: None,
                    permissions: None,
                });
            }
            Err(error) => return Err(io(path, error)),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(rejected(&format!(
                "publication target must be a regular file: {}",
                path.display()
            )));
        }
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(|e| io(path, e))?
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| io(path, e))?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(rejected("publication input exceeds 4 MiB"));
        }
        Ok(Self {
            bytes: Some(bytes),
            permissions: Some(metadata.permissions()),
        })
    }
    fn unchanged(&self, path: &Path) -> Result<bool, ApiError> {
        Ok(self.bytes == Self::read(path)?.bytes)
    }
}
#[derive(Debug)]
struct Output {
    path: PathBuf,
    previous: Snapshot,
    bytes: Vec<u8>,
    permissions: Option<fs::Permissions>,
}

/// An admitted translation and exact filesystem inputs awaiting explicit publication.
///
/// Preparation writes nothing. Publication checks all input bytes again, stages
/// every output beside its destination, and atomically replaces each file. An
/// ordinary commit failure restores earlier destinations. This is not a
/// cross-file crash-atomic transaction or a lock against concurrent editors.
/// The input and existing cfg/backup are bounded to 4 MiB and must be regular files.
#[derive(Debug)]
pub struct TranslationPublication {
    source: PathBuf,
    original: Snapshot,
    outputs: Vec<Output>,
    report: RewriteReport,
}
impl TranslationPublication {
    /// The in-memory admission result; no writes have been performed.
    #[must_use]
    pub const fn report(&self) -> &RewriteReport {
        &self.report
    }

    /// Publishes all prepared outputs after checking that the source and existing
    /// destinations have not changed. Rejected translations perform no writes.
    ///
    /// # Errors
    /// An I/O error or `DIVE-P0100` rejection, including a concurrent edit. A
    /// rollback failure retains the original staging file and reports its path;
    /// it never succeeds.
    pub fn publish(mut self) -> Result<RewriteReport, ApiError> {
        if !self.report.accepted() {
            return Ok(self.report);
        }
        self.verify_inputs()?;
        let mut staged = Vec::new();
        let mut restores = Vec::new();
        for output in &self.outputs {
            staged.push(Staged::new(
                &output.path,
                &output.bytes,
                output.permissions.as_ref(),
            )?);
            restores.push(
                output
                    .previous
                    .bytes
                    .as_deref()
                    .map(|bytes| {
                        Staged::new(&output.path, bytes, output.previous.permissions.as_ref())
                    })
                    .transpose()?,
            );
        }
        self.verify_inputs()?;
        commit(&self.outputs, &staged, &restores)?;
        self.report.document["filesystem_plan"]["writes_performed"] = json!(true);
        self.report.document["filesystem_plan"]["written_files"] = json!(
            self.outputs
                .iter()
                .map(|o| o.path.display().to_string())
                .collect::<Vec<_>>()
        );
        Ok(self.report)
    }
    fn verify_inputs(&self) -> Result<(), ApiError> {
        if !self.original.unchanged(&self.source)?
            || self
                .outputs
                .iter()
                .map(|o| o.previous.unchanged(&o.path))
                .collect::<Result<Vec<_>, _>>()?
                .contains(&false)
        {
            return Err(rejected(
                "source, configuration or backup changed after translation preparation; no publication performed",
            ));
        }
        Ok(())
    }
}

fn commit(
    outputs: &[Output],
    staged: &[Staged],
    restores: &[Option<Staged>],
) -> Result<(), ApiError> {
    for (index, output) in outputs.iter().enumerate() {
        if let Err(error) = fs::rename(&staged[index].path, &output.path) {
            let rollback_errors = rollback(outputs, restores, index);
            if !rollback_errors.is_empty() {
                return Err(rejected(&format!(
                    "commit failed: {error}; rollback failed: {}",
                    rollback_errors.join("; ")
                )));
            }
            return Err(io(&output.path, error));
        }
    }
    Ok(())
}

fn rollback(outputs: &[Output], restores: &[Option<Staged>], committed: usize) -> Vec<String> {
    let mut failures = Vec::new();
    for previous in (0..committed).rev() {
        let destination = &outputs[previous].path;
        let outcome = if let Some(restore) = &restores[previous] {
            fs::rename(&restore.path, destination)
        } else {
            fs::remove_file(destination)
        };
        if let Err(error) = outcome {
            let recovery = if let Some(restore) = &restores[previous] {
                restore.preserve.set(true);
                format!("; original retained at {}", restore.path.display())
            } else {
                String::new()
            };
            failures.push(format!("{}: {error}{recovery}", destination.display()));
        }
    }
    failures
}

#[derive(Debug)]
struct Staged {
    path: PathBuf,
    preserve: std::cell::Cell<bool>,
}
impl Staged {
    fn new(
        target: &Path,
        bytes: &[u8],
        permissions: Option<&fs::Permissions>,
    ) -> Result<Self, ApiError> {
        let parent = target
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        for _ in 0..64 {
            let path = parent.join(format!(
                ".dive-pcal-{}-{}.tmp",
                std::process::id(),
                NEXT_FILE.fetch_add(1, Ordering::Relaxed)
            ));
            let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io(&path, error)),
            };
            let staged = Self {
                path,
                preserve: std::cell::Cell::new(false),
            };
            if let Some(permissions) = permissions {
                file.set_permissions(permissions.clone())
                    .map_err(|e| io(&staged.path, e))?;
            }
            file.write_all(bytes).map_err(|e| io(&staged.path, e))?;
            file.sync_all().map_err(|e| io(&staged.path, e))?;
            return Ok(staged);
        }
        Err(rejected("cannot reserve a publication staging file"))
    }
}
impl Drop for Staged {
    fn drop(&mut self) {
        if !self.preserve.get() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

// Pinned trans.run: discard the previous generated prefix; retain the delimiter
// and user suffix, including an existing uncommented SPECIFICATION directive.
fn configuration(
    previous: Option<&[u8]>,
    requirements: &str,
    newline: &str,
) -> Result<Vec<u8>, ApiError> {
    let previous = std::str::from_utf8(previous.unwrap_or_default())
        .map_err(|e| rejected(&format!("configuration is not UTF-8: {e}")))?;
    let normalized = previous.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<&str> = normalized.lines().collect();
    if let Some(delimiter) = lines.iter().position(|line| line.contains(CFG_DELIMITER)) {
        lines.drain(..delimiter);
    } else {
        lines.insert(0, CFG_DELIMITER);
    }
    let has_spec = lines.iter().any(|line| {
        line.find("SPECIFICATION")
            .is_some_and(|at| line.find("\\*").is_none_or(|comment| comment > at))
    });
    let mut result = String::new();
    if !has_spec {
        result.push_str("SPECIFICATION Spec");
        result.push_str(newline);
    }
    for line in requirements.lines().rev() {
        result.push_str(line);
        result.push_str(newline);
    }
    for line in lines {
        result.push_str(line);
        result.push_str(newline);
    }
    Ok(result.into_bytes())
}

/// Prepares a bounded PlusCal rewrite and cfg/backup updates without writing files.
/// Source-option `-noold`/`-nocfg` settings are effective, and the existing default
/// modified-body protection and TLC overwrite policy are retained.
///
/// ```no_run
/// use dive_api::{prepare_translation_file, RewriteRequest};
/// let pending = prepare_translation_file(std::path::Path::new("Algorithm.tla"), &RewriteRequest::default())?;
/// if pending.report().accepted() { let written = pending.publish()?; assert!(written.accepted()); }
/// # Ok::<(), dive_api::ApiError>(())
/// ```
///
/// # Errors
/// I/O or bounded/regular-file input errors. Translation failures remain reports.
pub fn prepare_translation_file(
    path: &Path,
    request: &RewriteRequest,
) -> Result<TranslationPublication, ApiError> {
    let absolute = std::path::absolute(path).map_err(|error| io(path, error))?;
    let path = absolute.as_path();
    if path.extension().is_none_or(|extension| extension != "tla") {
        return Err(rejected("publication requires a .tla module"));
    }
    let original = Snapshot::read(path)?;
    let bytes = original
        .bytes
        .as_deref()
        .ok_or_else(|| rejected("translation source does not exist"))?;
    let source = std::str::from_utf8(bytes)
        .map_err(|e| rejected(&format!("translation source is not UTF-8: {e}")))?;
    let mut request = request.clone();
    if let Some(parent) = path.parent() {
        request
            .translation
            .analyze
            .after_standard
            .push(parent.to_path_buf());
    }
    let report = rewrite_source(&path.display().to_string(), source, &request);
    let mut outputs = Vec::new();
    if report.accepted() {
        let doc = report.to_json();
        let module = doc["replacement"]["module_source"]
            .as_str()
            .ok_or_else(|| rejected("admitted rewrite omitted module bytes"))?;
        if doc["filesystem_plan"]["backup_requested"] == true {
            let backup = path.with_extension("old");
            outputs.push(Output {
                previous: Snapshot::read(&backup)?,
                path: backup,
                bytes: bytes.to_vec(),
                permissions: original.permissions.clone(),
            });
        }
        if doc["filesystem_plan"]["configuration_requested"] == true {
            let cfg = path.with_extension("cfg");
            let previous = Snapshot::read(&cfg)?;
            let requirements = doc["artifact"]["configuration"]
                .as_str()
                .ok_or_else(|| rejected("admitted rewrite omitted configuration requirements"))?;
            let newline = match doc["filesystem_plan"]["body_newlines"].as_str() {
                Some("crlf") => "\r\n",
                Some("cr") => "\r",
                _ => "\n",
            };
            let bytes = configuration(previous.bytes.as_deref(), requirements, newline)?;
            outputs.push(Output {
                path: cfg,
                permissions: previous.permissions.clone(),
                previous,
                bytes,
            });
        }
        outputs.push(Output {
            path: path.to_path_buf(),
            previous: Snapshot::read(path)?,
            bytes: module.as_bytes().to_vec(),
            permissions: original.permissions.clone(),
        });
    }
    Ok(TranslationPublication {
        source: path.to_path_buf(),
        original,
        outputs,
        report,
    })
}

/// Translates and publishes an admitted PlusCal module, backup and configuration.
/// This is the explicit write counterpart of [`crate::rewrite_file`].
///
/// # Errors
/// The same preparation, concurrent-edit and publication errors as
/// [`prepare_translation_file`] and [`TranslationPublication::publish`].
pub fn write_translation_file(
    path: &Path,
    request: &RewriteRequest,
) -> Result<RewriteReport, ApiError> {
    prepare_translation_file(path, request)?.publish()
}

/// Original-AST publication result. Available AST bytes are not a translated
/// module or model proof. Successful writes retain pinned `-writeAST` exit 255.
#[derive(Clone, Debug)]
pub struct OriginalAstWriteReport {
    document: serde_json::Value,
    status: u8,
}
impl OriginalAstWriteReport {
    /// Whether `AST.tla` bytes were written successfully.
    #[must_use]
    pub fn written(&self) -> bool {
        self.document["writes_performed"] == true
    }
    /// Pinned successful-write/source-rejection exit 255; unsupported and bug exits stay native.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        self.status
    }
    /// Source-stage information and actual write disposition, never admission.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        self.document.clone()
    }
}

/// Writes the bounded pre-generation original AST to an explicit destination.
/// CLI `-writeAST` passes `AST.tla` in the current directory. The input module,
/// its backup and configuration are unchanged; no translation is generated.
///
/// # Errors
/// I/O, nonregular target, concurrent-edit or bounded-input errors.
pub fn write_original_ast_file(
    input: &Path,
    target: &Path,
    request: &crate::TranslationRequest,
) -> Result<OriginalAstWriteReport, ApiError> {
    let absolute_input = std::path::absolute(input).map_err(|error| io(input, error))?;
    let absolute_target = std::path::absolute(target).map_err(|error| io(target, error))?;
    let input = absolute_input.as_path();
    let target = absolute_target.as_path();
    let original = Snapshot::read(input)?;
    let bytes = original
        .bytes
        .as_deref()
        .ok_or_else(|| rejected("AST source does not exist"))?;
    let source = std::str::from_utf8(bytes)
        .map_err(|e| rejected(&format!("AST source is not UTF-8: {e}")))?;
    let requested_limits = &request.limits;
    let mut parsing = request.clone();
    parsing.limits.write_ast = false;
    let report = crate::original_ast_source(&input.display().to_string(), source, &parsing);
    let mut document = report.to_json();
    document["requested_translation_options"] = json!(requested_limits);
    if !report.artifact_available() {
        let status = if report.exit_code() == 1 {
            255
        } else {
            report.exit_code()
        };
        document["exit_code"] = json!(status);
        return Ok(OriginalAstWriteReport { document, status });
    }
    let module = document["ast_artifact"]["module_source"]
        .as_str()
        .ok_or_else(|| rejected("available AST omitted module bytes"))?;
    if input == target
        || fs::canonicalize(target)
            .is_ok_and(|destination| fs::canonicalize(input).ok().as_ref() == Some(&destination))
    {
        return Err(rejected("AST destination must differ from input"));
    }
    let previous = Snapshot::read(target)?;
    let staged = Staged::new(target, module.as_bytes(), previous.permissions.as_ref())?;
    if !original.unchanged(input)? || !previous.unchanged(target)? {
        return Err(rejected(
            "source or AST destination changed during preparation",
        ));
    }
    fs::rename(&staged.path, target).map_err(|e| io(target, e))?;
    document["writes_performed"] = json!(true);
    document["written_file"] = json!(target.display().to_string());
    document["exit_code"] = json!(255);
    Ok(OriginalAstWriteReport {
        document,
        status: 255,
    })
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "real filesystem rollback failure controls"
    )]
    use super::*;
    #[test]
    fn commit_failure_restores_replaced_and_removes_new_destinations() {
        for existing in [false, true] {
            let parent = std::env::temp_dir().join(format!(
                "dive-pcal-rollback-{}-{existing}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&parent);
            fs::create_dir_all(&parent).unwrap();
            let first = parent.join("first");
            let failing = parent.join("failing");
            if existing {
                fs::write(&first, b"original").unwrap();
            }
            let outputs = [
                Output {
                    previous: Snapshot::read(&first).unwrap(),
                    path: first.clone(),
                    bytes: b"new".to_vec(),
                    permissions: None,
                },
                Output {
                    previous: Snapshot::read(&failing).unwrap(),
                    path: failing.clone(),
                    bytes: b"fail".to_vec(),
                    permissions: None,
                },
            ];
            let staged = outputs
                .iter()
                .map(|o| Staged::new(&o.path, &o.bytes, None).unwrap())
                .collect::<Vec<_>>();
            let restores = outputs
                .iter()
                .map(|o| {
                    o.previous
                        .bytes
                        .as_ref()
                        .map(|bytes| Staged::new(&o.path, bytes, None).unwrap())
                })
                .collect::<Vec<_>>();
            // A directory appearing after staging forces a real rename failure
            // after the first destination has already committed.
            fs::create_dir(&failing).unwrap();
            assert!(commit(&outputs, &staged, &restores).is_err());
            if existing {
                assert_eq!(fs::read(&first).unwrap(), b"original");
            } else {
                assert!(!first.exists());
            }
            assert!(failing.is_dir());
            drop(staged);
            drop(restores);
            assert_eq!(
                fs::read_dir(&parent).unwrap().count(),
                1 + usize::from(existing)
            );
            fs::remove_dir_all(parent).unwrap();
        }
    }
    #[test]
    fn rollback_failure_retains_original_bytes_and_reports_recovery_path() {
        let parent = std::env::temp_dir().join(format!(
            "dive-pcal-rollback-recovery-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&parent);
        fs::create_dir_all(&parent).unwrap();
        let target = parent.join("changed-to-directory");
        let saved = Staged::new(&target, b"original private data", None).unwrap();
        let saved_path = saved.path.clone();
        fs::create_dir(&target).unwrap();
        let outputs = [Output {
            path: target.clone(),
            previous: Snapshot {
                bytes: Some(b"original private data".to_vec()),
                permissions: None,
            },
            bytes: b"replacement".to_vec(),
            permissions: None,
        }];
        let restores = [Some(saved)];
        let failures = rollback(&outputs, &restores, 1);
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains(&saved_path.display().to_string()));
        drop(restores);
        assert_eq!(fs::read(saved_path).unwrap(), b"original private data");
        assert!(target.is_dir());
        fs::remove_dir_all(parent).unwrap();
    }
}
