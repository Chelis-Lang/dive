//! Downloading pinned artifacts and checking out pinned repositories.
//!
//! Downloads shell out to `curl` and checkouts to `git`; see the crate README
//! for why. Every download lands in a `.partial` file, is verified against its
//! pinned SHA-256 and only then renamed into place. A mismatch deletes the
//! file and is a hard error.
//!
//! Each step that creates a cache entry holds the entry's advisory lock
//! ([`Cache::lock`]) and checks again under it, so concurrent harness
//! processes sharing one cache never write the same entry at once.
//!
//! Every artifact is first downloaded from its URL without credentials, which
//! is enough for public GitHub release assets. Assets on a private repository
//! cannot be fetched from the browser URL, even with a token (GitHub answers
//! 404). When the plain download fails and the URL names a GitHub release
//! asset, the harness falls back to the REST API (`GET
//! /repos/{owner}/{repo}/releases/tags/{tag}`) and downloads it from the
//! asset's API URL with `Accept: application/octet-stream`, using an optional
//! token. The token never appears on a command line: it is passed to `curl` on
//! stdin via `--config -`.

use crate::cache::Cache;
use crate::config::ModuleLibrary;
use crate::error::{Error, Result};
use crate::hash::{sha256_file, verify_file};
use crate::util::capture;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// Downloads artifacts into a [`Cache`].
#[derive(Debug)]
pub struct Fetcher {
    cache: Cache,
    token: OnceLock<Option<String>>,
}

/// What happened to one artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchStatus {
    /// It was already cached and verified.
    Cached,
    /// It was downloaded and verified.
    Downloaded,
}

/// Components of a `https://github.com/{owner}/{repo}/releases/download/{tag}/{name}` URL.
#[derive(Debug, PartialEq, Eq)]
struct ReleaseAsset<'a> {
    owner: &'a str,
    repo: &'a str,
    tag: &'a str,
    name: &'a str,
}

fn parse_release_url(url: &str) -> Option<ReleaseAsset<'_>> {
    let rest = url.strip_prefix("https://github.com/")?;
    let parts: Vec<&str> = rest.split('/').collect();
    match parts.as_slice() {
        [owner, repo, "releases", "download", tag, name] => Some(ReleaseAsset {
            owner,
            repo,
            tag,
            name,
        }),
        _ => None,
    }
}

/// Quotes `s` for a curl config file.
fn curl_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Runs curl with `config` on stdin. Returns `Ok(false)` if curl reported an
/// HTTP failure, so callers can fall back.
fn curl(config: &str) -> Result<std::result::Result<(), String>> {
    let mut child = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--location",
            // No --retry: GitHub rate-limits aggressively, and a failed fetch
            // is cheap to re-run by hand.
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--config",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Command(format!("cannot run curl: {e}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(config.as_bytes())
            .map_err(|e| Error::Command(format!("cannot write curl config: {e}")))?;
    }
    let out = child
        .wait_with_output()
        .map_err(|e| Error::Command(format!("curl: {e}")))?;
    if out.status.success() {
        Ok(Ok(()))
    } else {
        Ok(Err(String::from_utf8_lossy(&out.stderr).trim().to_owned()))
    }
}

impl Fetcher {
    /// A fetcher writing into `cache`.
    #[must_use]
    pub fn new(cache: Cache) -> Self {
        Self {
            cache,
            token: OnceLock::new(),
        }
    }

    /// The GitHub token from `GH_TOKEN`, `GITHUB_TOKEN`, or `gh auth token`.
    fn token(&self) -> Option<&str> {
        self.token
            .get_or_init(|| {
                for var in ["GH_TOKEN", "GITHUB_TOKEN"] {
                    if let Ok(t) = std::env::var(var)
                        && !t.trim().is_empty()
                    {
                        return Some(t.trim().to_owned());
                    }
                }
                capture(Command::new("gh").args(["auth", "token"]))
                    .ok()
                    .map(|t| t.trim().to_owned())
                    .filter(|t| !t.is_empty())
            })
            .as_deref()
    }

    fn download_plain(url: &str, dest: &Path) -> Result<std::result::Result<(), String>> {
        let config = format!(
            "url = {}\noutput = {}\n",
            curl_quote(url),
            curl_quote(&dest.display().to_string())
        );
        curl(&config)
    }

    fn download_via_api(&self, asset: &ReleaseAsset<'_>, dest: &Path) -> Result<()> {
        let token = self.token().ok_or_else(|| {
            Error::Command(format!(
                "{}/{} release asset `{}` needs a GitHub token: set GH_TOKEN or GITHUB_TOKEN, or log in with `gh auth login`",
                asset.owner, asset.repo, asset.name
            ))
        })?;
        let auth = format!(
            "header = {}\n",
            curl_quote(&format!("Authorization: Bearer {token}"))
        );
        let release_json = dest.with_extension("release.json");
        let config = format!(
            "url = {}\noutput = {}\n{auth}header = \"Accept: application/vnd.github+json\"\nheader = \"X-GitHub-Api-Version: 2022-11-28\"\n",
            curl_quote(&format!(
                "https://api.github.com/repos/{}/{}/releases/tags/{}",
                asset.owner, asset.repo, asset.tag
            )),
            curl_quote(&release_json.display().to_string())
        );
        curl(&config)?.map_err(|e| Error::Command(format!("GitHub release lookup failed: {e}")))?;
        let text =
            std::fs::read_to_string(&release_json).map_err(|e| Error::io(&release_json, &e))?;
        let _ = std::fs::remove_file(&release_json);
        let release: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| Error::Command(format!("GitHub release JSON: {e}")))?;
        let api_url = release
            .get("assets")
            .and_then(serde_json::Value::as_array)
            .and_then(|assets| {
                assets
                    .iter()
                    .find(|a| a.get("name").and_then(serde_json::Value::as_str) == Some(asset.name))
            })
            .and_then(|a| a.get("url"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                Error::Command(format!(
                    "release `{}` of {}/{} has no asset `{}`",
                    asset.tag, asset.owner, asset.repo, asset.name
                ))
            })?
            .to_owned();
        // curl does not forward custom headers when a redirect leaves the
        // original host, so the token is not sent to the storage backend.
        let config = format!(
            "url = {}\noutput = {}\n{auth}header = \"Accept: application/octet-stream\"\n",
            curl_quote(&api_url),
            curl_quote(&dest.display().to_string())
        );
        curl(&config)?.map_err(|e| Error::Command(format!("GitHub asset download failed: {e}")))
    }

    /// Ensures the artifact `url` with digest `sha256` is cached and verified.
    ///
    /// An existing cached file that fails verification is deleted and the
    /// call fails; run the fetch again to re-download it.
    ///
    /// # Errors
    /// Fails on download failure or digest mismatch.
    pub fn fetch_artifact(
        &self,
        what: &str,
        url: &str,
        sha256: &str,
    ) -> Result<(PathBuf, FetchStatus)> {
        let dest = self.cache.artifact_path(sha256, url);
        if dest.is_file() {
            verify_file(&dest, sha256, what)?;
            return Ok((dest, FetchStatus::Cached));
        }
        let _lock = self.cache.lock(&format!("sha256-{sha256}"))?;
        if dest.is_file() {
            // Another process downloaded it while this one waited.
            verify_file(&dest, sha256, what)?;
            return Ok((dest, FetchStatus::Cached));
        }
        let dir = dest
            .parent()
            .ok_or_else(|| Error::Io(format!("{} has no parent", dest.display())))?;
        std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, &e))?;
        let partial = dest.with_extension("partial");
        let _ = std::fs::remove_file(&partial);
        match Self::download_plain(url, &partial)? {
            Ok(()) => {}
            Err(plain_err) => {
                let _ = std::fs::remove_file(&partial);
                let asset = parse_release_url(url).ok_or_else(|| {
                    Error::Command(format!("download of {url} failed: {plain_err}"))
                })?;
                self.download_via_api(&asset, &partial)?;
            }
        }
        let actual = sha256_file(&partial)?;
        if actual != sha256 {
            std::fs::remove_file(&partial).map_err(|e| Error::io(&partial, &e))?;
            return Err(Error::HashMismatch {
                what: format!("{what} downloaded from {url}"),
                expected: sha256.to_owned(),
                actual,
            });
        }
        std::fs::rename(&partial, &dest).map_err(|e| Error::io(&dest, &e))?;
        Ok((dest, FetchStatus::Downloaded))
    }

    /// Ensures the JRE archive is verified and unpacked, and returns the path
    /// of its `java` executable.
    ///
    /// # Errors
    /// Fails on download, verification or unpacking failure.
    pub fn fetch_jre(&self, url: &str, sha256: &str) -> Result<(PathBuf, FetchStatus)> {
        let (archive, status) = self.fetch_artifact("JRE archive", url, sha256)?;
        let dir = self.cache.jre_dir(sha256);
        if let Ok(java) = find_java(&dir) {
            return Ok((java, status));
        }
        let _lock = self.cache.lock(&format!("jre-{sha256}"))?;
        if let Ok(java) = find_java(&dir) {
            return Ok((java, status));
        }
        let parent = self.cache.root().join("jre");
        let staging = crate::util::TempDir::new_in(&parent, "unpack")?;
        capture(
            Command::new("tar")
                .arg("-xzf")
                .arg(&archive)
                .arg("-C")
                .arg(staging.path()),
        )?;
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::rename(staging.path(), &dir).map_err(|e| Error::io(&dir, &e))?;
        let java = find_java(&dir)?;
        Ok((java, FetchStatus::Downloaded))
    }

    /// Ensures `repo` is checked out at `commit` under `git/<id>/<commit>`.
    /// Safe to call from several processes at once: one checks out, the
    /// others wait and then find it cached.
    ///
    /// # Errors
    /// Fails if git fails or the checkout is not at `commit`.
    pub fn fetch_checkout(
        &self,
        id: &str,
        repo: &str,
        commit: &str,
    ) -> Result<(PathBuf, FetchStatus)> {
        let dir = self.cache.checkout_dir(id, commit);
        if checkout_marker(&dir).is_file() {
            verify_head(&dir, commit)?;
            return Ok((dir, FetchStatus::Cached));
        }
        let _lock = self.cache.lock(&format!("checkout-{id}-{commit}"))?;
        if checkout_marker(&dir).is_file() {
            // Another process checked it out while this one waited.
            verify_head(&dir, commit)?;
            return Ok((dir, FetchStatus::Cached));
        }
        // An interrupted earlier checkout: start again. Under the lock, no
        // other fetch is writing here.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, &e))?;
        let git = |args: &[&str]| capture(Command::new("git").arg("-C").arg(&dir).args(args));
        git(&["init", "-q"])?;
        git(&["remote", "add", "origin", repo])?;
        git(&["fetch", "-q", "--depth", "1", "--no-tags", "origin", commit])?;
        git(&[
            "-c",
            "advice.detachedHead=false",
            "checkout",
            "-q",
            "--detach",
            "FETCH_HEAD",
        ])?;
        verify_head(&dir, commit)?;
        let marker = checkout_marker(&dir);
        std::fs::write(&marker, commit).map_err(|e| Error::io(&marker, &e))?;
        Ok((dir, FetchStatus::Downloaded))
    }

    /// Ensures a module library is checked out and returns its module
    /// directory.
    ///
    /// # Errors
    /// Fails if the checkout fails or lacks the configured subdirectory.
    pub fn fetch_library(&self, lib: &ModuleLibrary) -> Result<(PathBuf, FetchStatus)> {
        let (dir, status) = self.fetch_checkout(&lib.id, &lib.repo, &lib.commit)?;
        let sub = dir.join(&lib.subdir);
        if !sub.is_dir() {
            return Err(Error::Config(format!(
                "module library `{}` has no directory `{}` at {}",
                lib.id, lib.subdir, lib.commit
            )));
        }
        Ok((sub, status))
    }
}

/// The completion marker of a checkout. It lives inside `.git` so the work
/// tree stays pristine.
fn checkout_marker(dir: &Path) -> PathBuf {
    dir.join(".git").join("dive-checkout-complete")
}

/// Fails unless the checkout at `dir` has `HEAD` at `commit`.
///
/// # Errors
/// Fails if git fails or `HEAD` differs.
pub fn verify_head(dir: &Path, commit: &str) -> Result<()> {
    verify_head_with_setup(dir, commit, None)
}

fn verify_head_with_setup(
    dir: &Path,
    commit: &str,
    setup: Option<&crate::oracle::setup::Capture>,
) -> Result<()> {
    let head = capture_setup(
        setup,
        "git-head",
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["rev-parse", "HEAD"]),
    )?;
    if head.trim() == commit {
        Ok(())
    } else {
        Err(Error::Config(format!(
            "checkout {} is at {} but {commit} is pinned",
            dir.display(),
            head.trim()
        )))
    }
}

/// Paths in the checkout at `dir` that differ from the pinned commit
/// (modified, deleted, untracked or ignored), as reported by `git status
/// --untracked-files=all --ignored`. Ignored files matter: tlaplus/Examples
/// ignores everything except a few extensions, so files written by specs
/// (animation SVGs, trace specs) would otherwise go unnoticed.
///
/// # Errors
/// Fails if git fails.
pub fn dirty_paths(dir: &Path) -> Result<Vec<String>> {
    dirty_paths_with_setup(dir, None)
}

pub(crate) fn dirty_paths_with_setup(
    dir: &Path,
    setup: Option<&crate::oracle::setup::Capture>,
) -> Result<Vec<String>> {
    let out = capture_setup(
        setup,
        "git-status",
        Command::new("git").arg("-C").arg(dir).args([
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--ignored",
            "--ignore-submodules=all",
        ]),
    )?;
    Ok(out.lines().map(str::to_owned).collect())
}

/// Ensures the checkout of `dir` is cached, at `commit` and unmodified.
///
/// # Errors
/// Fails if any of those does not hold.
pub fn verified_checkout(dir: &Path, commit: &str, what: &str) -> Result<()> {
    verified_checkout_with_setup(dir, commit, what, None)
}

pub(crate) fn verified_checkout_with_setup(
    dir: &Path,
    commit: &str,
    what: &str,
    setup: Option<&crate::oracle::setup::Capture>,
) -> Result<()> {
    if !checkout_marker(dir).is_file() {
        return Err(Error::Config(format!(
            "{what} is not in the cache ({}); run `dive-conformance corpus fetch` or `oracle fetch`",
            dir.display()
        )));
    }
    verify_head_with_setup(dir, commit, setup)?;
    let dirty = dirty_paths_with_setup(dir, setup)?;
    if !dirty.is_empty() {
        return Err(Error::Config(format!(
            "{what} checkout {} has local changes ({}); delete it and fetch again",
            dir.display(),
            dirty.iter().take(5).cloned().collect::<Vec<_>>().join("; ")
        )));
    }
    Ok(())
}

// Preserve the default unbounded utility path. Opt-in setup uses the existing
// owned deadline runner and records normal nonzero before semantic rejection.
fn capture_setup(
    setup: Option<&crate::oracle::setup::Capture>,
    phase: &str,
    command: &mut Command,
) -> Result<String> {
    let Some(setup) = setup else {
        return capture(command);
    };
    let output = setup.run(phase, command, crate::oracle::setup::TIMEOUT)?;
    if output.exit_code != Some(0) {
        return Err(Error::Command(format!(
            "{phase} failed (exit {:?}): {}",
            output.exit_code,
            output.stderr.trim()
        )));
    }
    Ok(output.stdout)
}

/// Finds the `java` executable inside an unpacked JRE. On macOS the archive
/// holds `<top>/Contents/Home/bin/java`; elsewhere `<top>/bin/java`.
///
/// # Errors
/// Fails if no unique `java` is found.
pub fn find_java(dir: &Path) -> Result<PathBuf> {
    let entries = std::fs::read_dir(dir).map_err(|e| Error::io(dir, &e))?;
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let top = entry.path();
        for candidate in [
            top.join("Contents").join("Home").join("bin").join("java"),
            top.join("bin").join("java"),
        ] {
            if candidate.is_file() {
                found.push(candidate);
                break;
            }
        }
    }
    match found.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(Error::Config(format!(
            "no java executable under {}",
            dir.display()
        ))),
        _ => Err(Error::Config(format!(
            "several java executables under {}",
            dir.display()
        ))),
    }
}

/// Submodule paths declared in `<checkout>/.gitmodules`. They are never
/// fetched; the harness records them as unfetched.
#[must_use]
pub fn submodule_paths(checkout: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(checkout.join(".gitmodules")) else {
        return Vec::new();
    };
    let mut paths: Vec<String> = text
        .lines()
        .filter_map(|l| {
            let (k, v) = l.trim().split_once('=')?;
            (k.trim() == "path").then(|| v.trim().to_owned())
        })
        .collect();
    paths.sort();
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_urls() {
        assert_eq!(
            parse_release_url(
                "https://github.com/Chelis-Lang/dive/releases/download/oracle-tlc-x/tla2tools.jar"
            ),
            Some(ReleaseAsset {
                owner: "Chelis-Lang",
                repo: "dive",
                tag: "oracle-tlc-x",
                name: "tla2tools.jar"
            })
        );
        assert_eq!(parse_release_url("https://example.com/a.jar"), None);
    }

    #[test]
    fn curl_quoting() {
        assert_eq!(curl_quote(r#"a"b\c"#), r#""a\"b\\c""#);
    }

    #[test]
    fn gitmodules_paths() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let dir = crate::util::TempDir::new("gitmodules")?;
        std::fs::write(
            dir.path().join(".gitmodules"),
            "[submodule \"b\"]\n\tpath = specifications/B\n\turl = x\n[submodule \"a\"]\n\tpath = specifications/A\n",
        )?;
        assert_eq!(
            submodule_paths(dir.path()),
            vec!["specifications/A".to_owned(), "specifications/B".to_owned()]
        );
        Ok(())
    }
}
