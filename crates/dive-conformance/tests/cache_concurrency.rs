//! Concurrent harness processes sharing one `DIVE_CACHE_DIR` (issue #46).
//!
//! Two `dive-conformance corpus fetch` processes start together against an
//! empty cache and the same pinned commit. The entry's advisory lock lets
//! exactly one of them check the corpus out; the other waits and then finds
//! it cached. Both must succeed, and the checkout must be complete, at the
//! pinned commit and unmodified.

use dive_conformance::cache::Cache;
use dive_conformance::fetch::verified_checkout;
use dive_conformance::util::TempDir;
use std::path::Path;
use std::process::{Command, Stdio};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn git(dir: &Path, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()?;
    if !out.status.success() {
        return Err(format!("git {args:?}: {}", String::from_utf8_lossy(&out.stderr)).into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

#[test]
fn two_concurrent_fetches_share_one_cache() -> TestResult {
    let tmp = TempDir::new("cache-concurrency")?;
    // A small upstream repository, fetched over file:// like a remote.
    let upstream = tmp.path().join("upstream");
    let spec = upstream.join("specifications/x");
    std::fs::create_dir_all(&spec)?;
    for i in 0..200 {
        std::fs::write(
            spec.join(format!("M{i}.tla")),
            format!("---- MODULE M{i} ----\n====\n"),
        )?;
    }
    git(&upstream, &["init", "-q"])?;
    git(&upstream, &["add", "-A"])?;
    git(&upstream, &["commit", "-q", "-m", "c"])?;
    let commit = git(&upstream, &["rev-parse", "HEAD"])?;
    let url = format!("file://{}", upstream.display());

    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(repo.join("conformance"))?;
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/oracles.toml"),
        repo.join("conformance/oracles.toml"),
    )?;
    std::fs::write(
        repo.join("conformance/corpora.toml"),
        format!(
            "[[corpus]]\nid = \"c\"\nrepo = \"{url}\"\ncommit = \"{commit}\"\nlicense = \"MIT\"\nlayout = \"modules-only\"\n"
        ),
    )?;

    // Several rounds, each against a fresh cache, to give a race a chance.
    for round in 0..4 {
        let cache_dir = tmp.path().join(format!("cache-{round}"));
        let spawn = || {
            Command::new(env!("CARGO_BIN_EXE_dive-conformance"))
                .args(["corpus", "fetch", "--corpus", "c", "--repo-root"])
                .arg(&repo)
                .env("DIVE_CACHE_DIR", &cache_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
        };
        let (a, b) = (spawn()?, spawn()?);
        let mut downloaded = 0;
        for child in [a, b] {
            let out = child.wait_with_output()?;
            let err = String::from_utf8_lossy(&out.stderr);
            assert!(out.status.success(), "round {round}: fetch failed: {err}");
            if err.contains("Downloaded corpus `c`") {
                downloaded += 1;
            } else {
                assert!(err.contains("Cached corpus `c`"), "round {round}: {err}");
            }
        }
        assert_eq!(
            downloaded, 1,
            "round {round}: exactly one process checks out"
        );
        let checkout = Cache::new(cache_dir.clone()).checkout_dir("c", &commit);
        verified_checkout(&checkout, &commit, "corpus `c`")?;
        assert!(checkout.join("specifications/x/M199.tla").is_file());
        assert!(cache_dir.join("locks").is_dir());
    }
    Ok(())
}
