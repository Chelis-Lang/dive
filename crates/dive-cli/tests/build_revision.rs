//! The build-time revision names only release tags (#969).

#![forbid(unsafe_code)]

#[path = "../build/revision.rs"]
mod revision;

use std::path::{Path, PathBuf};
use std::process::Command;

fn setup(directory: &Path, arguments: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .current_dir(directory)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_CONFIG_PARAMETERS")
        .env_remove("GIT_CONFIG_COUNT")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
        .args(arguments)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "git {arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn describe_ignores_internal_tags_and_names_release_tags() -> Result<(), Box<dyn std::error::Error>>
{
    let scratch = Scratch(std::env::temp_dir().join(format!(
        "dive-build-revision-{}-{:?}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?
    )));
    std::fs::create_dir_all(&scratch.0)?;
    let directory = scratch.0.as_path();
    setup(directory, &["init", "-q"])?;
    setup(directory, &["commit", "-q", "--allow-empty", "-m", "first"])?;
    // Abbreviate through the production helper so both sides share `core.abbrev`.
    let first = revision::git(directory, &["rev-parse", "--short", "HEAD"]).ok_or("rev-parse")?;
    for tag in ["keep/archive-1", "pre-rescue", "oracle-tlc-1.8.0", "vendor"] {
        setup(directory, &["tag", tag])?;
    }

    // No release tag: the abbreviated commit, never an internal tag.
    assert_eq!(revision::revision(directory), first);

    // A lightweight release tag on HEAD names that tag.
    setup(directory, &["tag", "v0.1.0"])?;
    assert_eq!(revision::revision(directory), format!("v0.1.0-0-g{first}"));

    // Later commits count from the release tag, even past newer internal tags.
    setup(
        directory,
        &["commit", "-q", "--allow-empty", "-m", "second"],
    )?;
    setup(directory, &["tag", "keep/archive-2"])?;
    let second = revision::git(directory, &["rev-parse", "--short", "HEAD"]).ok_or("rev-parse")?;
    assert_eq!(revision::revision(directory), format!("v0.1.0-1-g{second}"));

    // An annotated release tag is described the same way.
    setup(directory, &["tag", "-a", "v0.2.0", "-m", "release"])?;
    assert_eq!(revision::revision(directory), format!("v0.2.0-0-g{second}"));

    // Outside a repository there is no revision to name.
    let outside = scratch.0.join("not-a-repository");
    std::fs::create_dir_all(&outside)?;
    std::fs::write(outside.join(".git"), "gitdir: missing\n")?;
    assert_eq!(revision::revision(&outside), "unknown");
    Ok(())
}
