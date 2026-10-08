//! Small shared helpers: temporary directories, UTC timestamps, glob
//! matching, path redaction and command capture.

use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A uniquely named directory that is removed (recursively) on drop.
#[derive(Debug)]
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Creates a fresh directory under the system temporary directory.
    ///
    /// # Errors
    /// Fails if the directory cannot be created.
    pub fn new(prefix: &str) -> Result<Self> {
        Self::new_in(&std::env::temp_dir(), prefix)
    }

    /// Creates a fresh directory under `parent`.
    ///
    /// # Errors
    /// Fails if the directory cannot be created.
    pub fn new_in(parent: &Path, prefix: &str) -> Result<Self> {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, &e))?;
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos());
        let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!("{prefix}-{}-{n}-{nanos}", std::process::id()));
        std::fs::create_dir(&path).map_err(|e| Error::io(&path, &e))?;
        Ok(Self { path })
    }

    /// The directory's path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Best effort: a leftover temporary directory is not an error.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Seconds since the Unix epoch.
#[must_use]
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Converts days since 1970-01-01 to a civil `(year, month, day)`
/// (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    // `d` is in 1..=31 and `m` in 1..=12 by construction.
    (
        y,
        u32::try_from(m).unwrap_or(1),
        u32::try_from(d).unwrap_or(1),
    )
}

/// Formats `secs` since the epoch as RFC 3339 UTC, e.g. `2026-10-08T12:00:00Z`.
#[must_use]
pub fn rfc3339(secs: u64) -> String {
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let rem = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Formats `secs` as a compact UTC stamp usable in file names,
/// e.g. `20261008T120000Z`.
#[must_use]
pub fn compact_stamp(secs: u64) -> String {
    rfc3339(secs).replace(['-', ':'], "")
}

/// Matches `text` against a glob `pattern`. `*` matches any run of characters
/// except `/`, `**` matches any run including `/`, and `?` matches one
/// character other than `/`. Everything else matches literally.
#[must_use]
pub fn glob_match(pattern: &str, text: &str) -> bool {
    fn go(p: &[char], t: &[char]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some('*') if p.get(1) == Some(&'*') => {
                // Swallow a following '/' so `a/**/b` also matches `a/b`.
                let rest = if p.get(2) == Some(&'/') {
                    &p[3..]
                } else {
                    &p[2..]
                };
                (0..=t.len()).any(|i| go(rest, &t[i..]))
                    || (p.get(2) == Some(&'/') && go(&p[2..], t))
            }
            Some('*') => {
                let rest = &p[1..];
                let mut i = 0;
                loop {
                    if go(rest, &t[i..]) {
                        return true;
                    }
                    if i == t.len() || t[i] == '/' {
                        return false;
                    }
                    i += 1;
                }
            }
            Some('?') => t.first().is_some_and(|c| *c != '/') && go(&p[1..], &t[1..]),
            Some(c) => t.first() == Some(c) && go(&p[1..], &t[1..]),
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    go(&p, &t)
}

/// Whether `text` matches any of `patterns`; an empty list matches everything.
#[must_use]
pub fn matches_any(patterns: &[String], text: &str) -> bool {
    patterns.is_empty() || patterns.iter().any(|p| glob_match(p, text))
}

/// Replaces machine-specific absolute paths in `text` with stable
/// placeholders, longest path first. Used for SANY error text, recorded
/// command lines and committed fixtures, so they carry no local paths.
#[must_use]
pub fn redact(text: &str, replacements: &[(PathBuf, &str)]) -> String {
    let mut pairs: Vec<(String, &str)> = Vec::new();
    for (path, placeholder) in replacements {
        let s = path.display().to_string();
        if !s.is_empty() {
            pairs.push((s.clone(), placeholder));
            // macOS reports /var/... and /tmp/... both with and without /private.
            if let Some(stripped) = s.strip_prefix("/private") {
                pairs.push((stripped.to_owned(), placeholder));
            } else if s.starts_with("/var/") || s.starts_with("/tmp/") {
                pairs.push((format!("/private{s}"), placeholder));
            }
        }
    }
    pairs.sort_by_key(|(s, _)| std::cmp::Reverse(s.len()));
    let mut out = text.to_owned();
    for (from, to) in pairs {
        out = out.replace(&from, to);
    }
    out
}

/// Runs `cmd` to completion and returns its stdout, failing on a non-zero
/// exit with the command's stderr in the message.
///
/// # Errors
/// Fails if the command cannot be started or exits unsuccessfully.
pub fn capture(cmd: &mut Command) -> Result<String> {
    let output = cmd
        .output()
        .map_err(|e| Error::Command(format!("cannot run {}: {e}", cmd.get_program().display())))?;
    if !output.status.success() {
        return Err(Error::Command(format!(
            "{} {:?} failed ({}): {}",
            cmd.get_program().display(),
            cmd.get_args().collect::<Vec<_>>(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Recursively copies the directory `src` to `dst` (created), skipping `.git`.
/// Symbolic links are followed (their target's content is copied).
///
/// # Errors
/// Fails on any file-system error.
pub fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst).map_err(|e| Error::io(dst, &e))?;
    let entries = std::fs::read_dir(src).map_err(|e| Error::io(src, &e))?;
    for entry in entries {
        let entry = entry.map_err(|e| Error::io(src, &e))?;
        if entry.file_name() == ".git" {
            continue;
        }
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let meta = std::fs::metadata(&from).map_err(|e| Error::io(&from, &e))?;
        if meta.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| Error::io(&from, &e))?;
        }
    }
    Ok(())
}

/// Writes `bytes` to `path` atomically (write to a sibling, then rename),
/// creating parent directories.
///
/// # Errors
/// Fails on any file-system error.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, &e))?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| Error::io(&tmp, &e))?;
    std::fs::rename(&tmp, path).map_err(|e| Error::io(path, &e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_known_values() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339(1_791_460_800), "2026-10-08T12:00:00Z");
        assert_eq!(compact_stamp(1_791_460_800), "20261008T120000Z");
    }

    #[test]
    fn globbing() {
        assert!(glob_match(
            "specifications/*/*.cfg",
            "specifications/ewd840/EWD840.cfg"
        ));
        assert!(!glob_match(
            "specifications/*.cfg",
            "specifications/ewd840/EWD840.cfg"
        ));
        assert!(glob_match(
            "**/EWD840.cfg",
            "specifications/ewd840/EWD840.cfg"
        ));
        assert!(glob_match(
            "specifications/**/MC.cfg",
            "specifications/a/b/c/MC.cfg"
        ));
        assert!(glob_match(
            "specifications/**/MC.cfg",
            "specifications/MC.cfg"
        ));
        assert!(glob_match("*DieHard*", "DieHard.cfg"));
        assert!(glob_match("a?c", "abc"));
        assert!(!glob_match("a?c", "a/c"));
        assert!(matches_any(&[], "anything"));
    }

    #[test]
    fn redaction_prefers_longest_and_handles_private() {
        let r = redact(
            "/private/tmp/x/cache/jre /tmp/x/cache /tmp/x",
            &[
                (PathBuf::from("/tmp/x"), "$TMP"),
                (PathBuf::from("/tmp/x/cache"), "$DIVE_CACHE_DIR"),
            ],
        );
        assert_eq!(r, "$DIVE_CACHE_DIR/jre $DIVE_CACHE_DIR $TMP");
    }
}
