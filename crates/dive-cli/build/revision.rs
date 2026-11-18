//! Source-revision lookup shared by `build.rs` and its test.

use std::path::Path;
use std::process::Command;

/// The file Cargo writes into a package built from a Git checkout, recording
/// the commit the package was made from.
pub const VCS_INFO: &str = ".cargo_vcs_info.json";

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

/// The commit recorded in a `.cargo_vcs_info.json` document, abbreviated to
/// 12 hex digits, with `-dirty` when Cargo packaged uncommitted changes.
///
/// Only the `git.sha1` and `git.dirty` fields are read; a document without a
/// full 40-digit lowercase hex `sha1` yields `None`.
pub fn packaged(document: &str) -> Option<String> {
    let sha = string_field(document, "sha1")?;
    if sha.len() != 40
        || !sha
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let dirty = document.split_once("\"dirty\"").is_some_and(|(_, rest)| {
        rest.trim_start()
            .strip_prefix(':')
            .is_some_and(|value| value.trim_start().starts_with("true"))
    });
    Some(format!(
        "{}{}",
        &sha[..12],
        if dirty { "-dirty" } else { "" }
    ))
}

/// The string value of the first `"key": "value"` pair in a JSON document.
fn string_field<'a>(document: &'a str, key: &str) -> Option<&'a str> {
    let (_, rest) = document.split_once(&format!("\"{key}\""))?;
    let rest = rest
        .trim_start()
        .strip_prefix(':')?
        .trim_start()
        .strip_prefix('"')?;
    rest.split_once('"').map(|(value, _)| value)
}

/// The revision recorded in the binary. A crate built from its published
/// package names the commit Cargo recorded when packaging it; a Git checkout
/// is described by Git; anything else is `unknown`.
pub fn revision(directory: &Path) -> String {
    std::fs::read_to_string(directory.join(VCS_INFO))
        .ok()
        .and_then(|document| packaged(&document))
        .or_else(|| git(directory, DESCRIBE))
        .unwrap_or_else(|| "unknown".into())
}
