//! Records the source revision without requiring Git in installed binaries.

#![forbid(unsafe_code)]

use std::path::PathBuf;

#[path = "build/revision.rs"]
mod revision;

use revision::git;

fn main() {
    let directory = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let revision = revision::revision(&directory);
    println!("cargo::rustc-env=DIVE_GIT_DESCRIBE={revision}");
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=build/revision.rs");
    if directory.join(revision::VCS_INFO).exists() {
        // A published package: the revision is fixed by its recorded commit.
        println!("cargo::rerun-if-changed={}", revision::VCS_INFO);
        return;
    }
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
