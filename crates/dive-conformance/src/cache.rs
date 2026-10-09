//! The content-addressed cache under `$DIVE_CACHE_DIR` (default
//! `~/.cache/dive`).
//!
//! ```text
//! $DIVE_CACHE_DIR/
//!   sha256/<hex>/<file>        downloaded artifacts, one directory per digest
//!   jre/<hex>/                 JRE unpacked from the archive with digest <hex>
//!   git/<id>/<commit>/         corpora and module libraries at pinned commits
//!   jar-modules/<hex>/         `.tla` files extracted from the jar with digest <hex>
//!   home/.tlaplus/esc.txt      hermetic `user.home` that opts out of TLC telemetry
//!   tmp/                       per-run scratch (TLC metadirs, traces)
//!   locks/<entry>.lock         advisory locks, one per cache entry
//! ```
//!
//! **Concurrency.** Several harness processes may share one cache. Every
//! step that creates an entry (download, JRE unpacking, checkout, jar
//! module extraction) holds that entry's exclusive advisory lock
//! ([`Cache::lock`]) and checks again, under the lock, whether another
//! process finished it meanwhile. Entries become visible only when complete:
//! downloads are renamed into place after verification, unpacked trees are
//! renamed from a staging directory, and checkouts are marked complete last.
//! Readers therefore need no lock: they see a complete entry or none, and
//! still verify it before use. Per-run scratch directories carry the process
//! id and a counter, so runs never share one.
//!
//! Each artifact sits alone in its own directory. That matters for the TLC
//! jar: its manifest `Class-Path` names `CommunityModules*.jar` relative to the
//! jar's directory, so an isolated directory guarantees nothing leaks onto the
//! classpath implicitly.

use crate::error::{Error, Result};
use crate::hash::verify_file;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

/// An exclusive advisory lock on one cache entry, released when dropped
/// (or when the process exits, however it exits).
#[derive(Debug)]
pub struct CacheLock {
    // Held only to keep the lock: closing the file releases it.
    _file: File,
}

/// The cache root and its layout.
#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    /// Uses `root` as the cache directory.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Resolves the cache directory from `$DIVE_CACHE_DIR`, falling back to
    /// `$HOME/.cache/dive`.
    ///
    /// # Errors
    /// Fails if neither variable is set.
    pub fn from_env() -> Result<Self> {
        if let Some(dir) = std::env::var_os("DIVE_CACHE_DIR").filter(|v| !v.is_empty()) {
            return Ok(Self::new(PathBuf::from(dir)));
        }
        let home = std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .ok_or_else(|| Error::Config("neither DIVE_CACHE_DIR nor HOME is set".to_owned()))?;
        Ok(Self::new(PathBuf::from(home).join(".cache").join("dive")))
    }

    /// The cache root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where the artifact with digest `sha256` downloaded from `url` lives.
    #[must_use]
    pub fn artifact_path(&self, sha256: &str, url: &str) -> PathBuf {
        self.root
            .join("sha256")
            .join(sha256)
            .join(file_name_from_url(url))
    }

    /// Where the JRE archive with digest `sha256` is unpacked.
    #[must_use]
    pub fn jre_dir(&self, sha256: &str) -> PathBuf {
        self.root.join("jre").join(sha256)
    }

    /// Where repository `id` is checked out at `commit`.
    #[must_use]
    pub fn checkout_dir(&self, id: &str, commit: &str) -> PathBuf {
        self.root.join("git").join(id).join(commit)
    }

    /// Scratch space for runs.
    #[must_use]
    pub fn tmp_dir(&self) -> PathBuf {
        self.root.join("tmp")
    }

    /// Takes the exclusive advisory lock of cache entry `key`, waiting for
    /// another process (or thread) that holds it. `key` names the entry,
    /// e.g. `checkout-examples-<commit>`; characters outside
    /// `[A-Za-z0-9._-]` become `_`. Lock files live in `locks/` and are never
    /// deleted, since deleting a lock file while another process waits on it
    /// would let two holders in at once.
    ///
    /// The lock is `std::fs::File::lock`: `flock(2)` on macOS and Linux. It
    /// coordinates processes on one machine; it is not meant for a cache on
    /// a network file system.
    ///
    /// # Errors
    /// Fails if the lock file cannot be created or locked.
    pub fn lock(&self, key: &str) -> Result<CacheLock> {
        let dir = self.root.join("locks");
        std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, &e))?;
        let path = dir.join(format!("{}.lock", lock_name(key)));
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|e| Error::io(&path, &e))?;
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                eprintln!(
                    "waiting for another harness process to finish cache entry `{key}` ({})",
                    path.display()
                );
                file.lock().map_err(|e| Error::io(&path, &e))?;
            }
            Err(TryLockError::Error(e)) => return Err(Error::io(&path, &e)),
        }
        Ok(CacheLock { _file: file })
    }

    /// A hermetic `user.home` for TLC whose `.tlaplus/esc.txt` opts out of
    /// TLC's execution-statistics collection (`util.ExecutionStatisticsCollector`),
    /// which otherwise performs a DNS query and may report usage.
    ///
    /// # Errors
    /// Fails if the directory cannot be created.
    pub fn tlc_home(&self) -> Result<PathBuf> {
        let home = self.root.join("home");
        let dir = home.join(".tlaplus");
        std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, &e))?;
        let esc = dir.join("esc.txt");
        let want = "NO_STATISTICS\n";
        if std::fs::read_to_string(&esc).ok().as_deref() != Some(want) {
            // Written aside and renamed, so a concurrent TLC never reads a
            // partly written file.
            let tmp = dir.join(format!("esc.txt.{}", std::process::id()));
            std::fs::write(&tmp, want).map_err(|e| Error::io(&tmp, &e))?;
            std::fs::rename(&tmp, &esc).map_err(|e| Error::io(&esc, &e))?;
        }
        Ok(home)
    }

    /// Returns the cached artifact after verifying its SHA-256. A mismatching
    /// file is deleted and the call fails; a missing file fails with a hint to
    /// run the fetch command.
    ///
    /// # Errors
    /// Fails if the artifact is missing or corrupt.
    pub fn verified_artifact(&self, what: &str, url: &str, sha256: &str) -> Result<PathBuf> {
        let path = self.artifact_path(sha256, url);
        if !path.is_file() {
            return Err(Error::Config(format!(
                "{what} is not in the cache ({}); run `dive-conformance oracle fetch`",
                path.display()
            )));
        }
        verify_file(&path, sha256, what)?;
        Ok(path)
    }
}

/// The file name of the lock for `key`: `[A-Za-z0-9._-]` kept, anything
/// else replaced by `_`.
fn lock_name(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The last path segment of `url`, percent-decoded for `%2B`/`%2b`, or
/// `artifact` if it has none.
#[must_use]
pub fn file_name_from_url(url: &str) -> String {
    let last = url
        .split(['?', '#'])
        .next()
        .and_then(|u| u.rsplit('/').next())
        .unwrap_or_default();
    let name = last.replace("%2B", "+").replace("%2b", "+");
    if name.is_empty() || name == "." || name == ".." || name.contains('%') {
        "artifact".to_owned()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_is_content_addressed() {
        let c = Cache::new(PathBuf::from("/c"));
        let sha = "a".repeat(64);
        assert_eq!(
            c.artifact_path(&sha, "https://x/releases/download/v1/tla2tools.jar"),
            PathBuf::from(format!("/c/sha256/{sha}/tla2tools.jar"))
        );
        assert_eq!(
            file_name_from_url("https://x/jdk-17%2B1/OpenJDK.tar.gz?x=1"),
            "OpenJDK.tar.gz"
        );
        assert_eq!(file_name_from_url("https://x/"), "artifact");
    }

    #[test]
    fn locks_exclude_each_other() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = crate::util::TempDir::new("cache-lock")?;
        let cache = Cache::new(dir.path().to_path_buf());
        let held = cache.lock("entry/../x y")?;
        let path = dir.path().join("locks").join("entry_.._x_y.lock");
        assert!(path.is_file());
        // Another open file description cannot take it while it is held,
        // even in the same process (flock semantics).
        let other = OpenOptions::new().write(true).open(&path)?;
        assert!(matches!(other.try_lock(), Err(TryLockError::WouldBlock)));
        drop(held);
        assert!(other.try_lock().is_ok());
        Ok(())
    }
}
