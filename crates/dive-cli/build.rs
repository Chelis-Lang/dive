//! Records the source revision without requiring Git in installed binaries.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn git(directory: &Path, arguments: &[&str]) -> Option<String> {
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

fn main() {
    let directory = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let revision =
        git(&directory, &["describe", "--always", "--tags"]).unwrap_or_else(|| "unknown".into());
    println!("cargo::rustc-env=DIVE_GIT_DESCRIBE={revision}");
    println!("cargo::rerun-if-changed=build.rs");
    let mut paths = vec![
        "HEAD".to_owned(),
        "packed-refs".to_owned(),
        "refs/tags".to_owned(),
    ];
    if let Some(branch) = git(&directory, &["symbolic-ref", "--quiet", "HEAD"]) {
        paths.push(branch);
    }
    for path in paths {
        if let Some(path) = git(&directory, &["rev-parse", "--git-path", &path]) {
            let path = directory.join(path);
            if path.exists() {
                println!("cargo::rerun-if-changed={}", path.display());
            }
        }
    }
}
