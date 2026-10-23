//! A real failed launch intentionally halts the process-global child owner.
//! Keep this control in its own integration process so the other synthetic
//! pipeline controls still exercise genuine child dispatch after it runs.
#![cfg(unix)]

use dive_conformance::cache::Cache;
use dive_conformance::config::Config;
use dive_conformance::golden::model_golden_path;
use dive_conformance::oracle::{Capabilities, Java, JavaSource, OracleEnv};
use dive_conformance::pipeline::{self, GenerateOptions, Limits, Selection};
use dive_conformance::util::TempDir;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn registry(root: &Path) -> Result<(PathBuf, Config, Cache), Box<dyn std::error::Error>> {
    let repo = root.join("repo");
    let spec = repo.join("conformance/corpus/specifications/x");
    std::fs::create_dir_all(&spec)?;
    std::fs::create_dir_all(repo.join("conformance"))?;
    std::fs::write(spec.join("X.tla"), "---- MODULE X ----\n====\n")?;
    std::fs::write(spec.join("X.cfg"), "\n")?;
    std::fs::write(
        spec.join("manifest.json"),
        r#"{"sources":[],"authors":["synthetic control"],"tags":[],"modules":[{"path":"specifications/x/X.tla","features":[],"models":[{"path":"specifications/x/X.cfg","runtime":"unknown","mode":"exhaustive search","result":"unknown"}]}]}"#,
    )?;
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/oracles.toml"),
        repo.join("conformance/oracles.toml"),
    )?;
    std::fs::write(
        repo.join("conformance/corpora.toml"),
        "[[corpus]]\nid = \"examples\"\npath = \"conformance/corpus\"\nlicense = \"MIT\"\nlayout = \"examples-manifest\"\n",
    )?;
    let config = Config::load(&repo)?;
    let cache = Cache::new(root.join("cache"));
    Ok((repo, config, cache))
}

#[test]
fn raw_launch_failure_has_original_error_and_no_invented_exit_or_golden()
-> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new("raw-launch-error")?;
    let (repo, config, cache) = registry(tmp.path())?;
    // Identify the synthetic file, then remove it to exercise a real failed
    // executable launch. No Java runtime or pinned oracle is executed here.
    let missing = tmp.path().join("missing-java");
    std::fs::write(&missing, "#!/bin/sh\nexit 0\n")?;
    let digest = dive_conformance::hash::sha256_file(&missing)?;
    std::fs::remove_file(&missing)?;
    let raw = tmp.path().join("raw");
    let tool = OracleEnv {
        oracle: config.oracles.default_oracle()?.clone(),
        jar: tmp.path().join("unused-fixture.jar"),
        classpath: vec![],
        libraries: vec![],
        java: Java {
            path: missing.clone(),
            source: JavaSource::System {
                path: missing.display().to_string(),
                sha256: digest,
            },
            version_line: "synthetic failed-launch control, not Java".into(),
        },
        capabilities: Capabilities::default(),
        home: tmp.path().join("home"),
        tmp_root: tmp.path().join("temporary"),
        cache_root: cache.root().to_path_buf(),
        raw_invocations: Some(raw.clone()),
    };
    let options = GenerateOptions {
        oracle: "tlc-2026.10.06.014338".into(),
        corpus: "examples".into(),
        selection: Selection {
            tier: "all".into(),
            filters: vec!["specifications/x/X.cfg".into()],
        },
        reason: Some("synthetic process regression control".into()),
        limits: Limits {
            jobs: 1,
            xmx: "1g".into(),
            timeout: Some(Duration::from_secs(30)),
            timeout_cap: Duration::from_secs(30),
            budget_cap: None,
        },
        models: true,
        sany: false,
        workers: vec![1],
    };
    assert_eq!(
        pipeline::golden_generate(&config, &cache, &tool, &options)?,
        1
    );
    let dirs = std::fs::read_dir(&raw)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(dirs.len(), 1, "one launch in this isolated process");
    let failed = dirs
        .iter()
        .find(|dir| dir.join("harness-error.json").exists())
        .ok_or("missing launch error capture")?;
    assert!(failed.join("started.json").exists());
    assert!(!failed.join("completed.json").exists());
    let error: serde_json::Value =
        serde_json::from_slice(&std::fs::read(failed.join("harness-error.json"))?)?;
    assert!(error.get("exit_code").is_none());
    assert!(
        !model_golden_path(
            &repo.join("conformance/golden"),
            "tlc-2026.10.06.014338",
            "examples",
            "specifications/x/X.cfg",
        )
        .exists()
    );
    Ok(())
}
