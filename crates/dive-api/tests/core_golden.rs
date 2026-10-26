//! The M0 spike models under `--engine=core` reproduce the pinned TLC
//! goldens' verdict, distinct and generated counts and depth at one worker.
//!
//! Slow and corpus-dependent: excluded from the `ci` nextest profile. It
//! needs the pinned examples corpus and module libraries in
//! `$DIVE_CACHE_DIR` (default `~/.cache/dive`; `dive-conformance corpus
//! fetch`), and fails, naming the path, when they are missing.
#![allow(clippy::expect_used, clippy::panic, reason = "test fixtures")]

use std::path::{Path, PathBuf};

use dive_api::{CheckRequest, Engine, check_file_engine};

const EXAMPLES: &str = "6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f";
const TLAPM: &str = "tlapm-library-d8b9747/d8b9747a1dce9b4816bcc1ff130ccf7a3af2ce37/library";
const ORACLE: &str = "tlc-2026.10.06.014338";

fn cache() -> PathBuf {
    std::env::var_os("DIVE_CACHE_DIR").map_or_else(
        || PathBuf::from(std::env::var_os("HOME").expect("HOME is set")).join(".cache/dive"),
        PathBuf::from,
    )
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn golden(model: &str) -> serde_json::Value {
    let path = repo()
        .join("conformance/golden")
        .join(ORACLE)
        .join("examples")
        .join(format!("{model}.json"));
    let text = std::fs::read_to_string(&path).expect("golden present");
    serde_json::from_str(&text).expect("golden is JSON")
}

fn check(spec: &Path, cfg: &Path, model: &str) {
    assert!(spec.exists(), "missing corpus file {}", spec.display());
    let report = check_file_engine(
        &CheckRequest {
            file: spec.to_path_buf(),
            config: Some(cfg.to_path_buf()),
            module_paths: vec![cache().join("git").join(TLAPM)],
            workers: Some(1),
            seed: Some(0),
            fp: Some(0),
            ..CheckRequest::default()
        },
        Engine::Core,
    )
    .expect("check runs");
    let got = report.to_json();
    let want = golden(model);
    let counts = &want["result"]["counts"];
    assert_eq!(got["engine"], "core");
    assert_eq!(
        got["verdict"], want["result"]["verdict"],
        "{model}: {got:#}"
    );
    assert_eq!(
        got["statistics"]["distinct"], counts["distinct_states"],
        "{model}"
    );
    assert_eq!(
        got["statistics"]["generated"], counts["states_generated"],
        "{model}"
    );
    assert_eq!(got["statistics"]["depth"], counts["depth"], "{model}");
}

fn example(model: &str) {
    let specs = cache().join("git/examples").join(EXAMPLES);
    let cfg = specs.join(model);
    let spec = cfg.with_extension("tla");
    check(&spec, &cfg, model);
}

#[test]
fn simple_regular_matches_the_golden() {
    example("specifications/TeachingConcurrency/SimpleRegular.cfg");
}

#[test]
#[ignore = "minutes in a debug build; run with `--release --run-ignored all`"]
fn mc_lamport_mutex_matches_the_golden() {
    let specs = cache().join("git/examples").join(EXAMPLES);
    let cfg = specs.join("specifications/lamport_mutex/MCLamportMutex.cfg");
    check(
        &specs.join("specifications/lamport_mutex/MCLamportMutex.tla"),
        &cfg,
        "specifications/lamport_mutex/MCLamportMutex.cfg",
    );
}

#[test]
#[ignore = "minutes in a debug build; run with `--release --run-ignored all`"]
fn game_of_life_matches_the_golden() {
    example("specifications/GameOfLife/GameOfLife.cfg");
}

/// The safety configuration of `dive-bench` (`CoffeeCan1000BeansSafety`):
/// the golden's configuration without its `PROPERTY` section. Liveness does
/// not change TLC's state counts, so the golden's counts apply.
#[test]
fn coffee_can_safety_matches_the_golden() {
    let specs = cache().join("git/examples").join(EXAMPLES);
    let src = specs.join("specifications/CoffeeCan/CoffeeCan.tla");
    assert!(src.exists(), "missing corpus file {}", src.display());
    let dir = std::env::temp_dir().join(format!("dive-core-coffee-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let spec = dir.join("CoffeeCan.tla");
    std::fs::copy(&src, &spec).expect("copy spec");
    let cfg = dir.join("CoffeeCan.cfg");
    std::fs::write(
        &cfg,
        "CONSTANTS\n    MaxBeanCount = 1000\n\nSPECIFICATION\n    Spec\n\nINVARIANTS\n    TypeInvariant\n",
    )
    .expect("write cfg");
    check(
        &spec,
        &cfg,
        "specifications/CoffeeCan/CoffeeCan1000Beans.cfg",
    );
    let _ = std::fs::remove_dir_all(&dir);
}
