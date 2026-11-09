//! Source-revision lookup shared by `build.rs` and its test.

use std::path::Path;
use std::process::Command;

/// Runs `git` in `directory` and returns its trimmed standard output on success.
pub fn git(directory: &Path, arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(directory)
        .args(arguments)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Arguments that describe `HEAD` from the nearest release tag (`v<digit>...`)
/// only, so internal tags never appear in `--version`. Without a release tag
/// Git falls back to the abbreviated commit hash.
pub const DESCRIBE: &[&str] = &[
    "describe", "--always", "--tags", "--long", "--match", "v[0-9]*",
];

/// The revision recorded in the binary, or `unknown` without Git.
pub fn revision(directory: &Path) -> String {
    git(directory, DESCRIBE).unwrap_or_else(|| "unknown".into())
}
