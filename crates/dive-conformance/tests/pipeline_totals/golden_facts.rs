//! Synthetic process controls exercise the real generation/write pipeline.
//! These outputs are fixtures, never new corpus oracle evidence.

use super::*;
use dive_conformance::golden::{read_model_golden, read_sany_golden, sany_golden_path};
use dive_conformance::oracle::OracleEnv;
use dive_conformance::pipeline::GenerateOptions;
use dive_conformance::sany::SanyVerdict;
use std::os::unix::fs::PermissionsExt;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/tlc/tlc-2026.10.06.014338")
            .join(name),
    )
    .unwrap_or_else(|error| unreachable!("existing fixture: {error}"))
}

fn env(
    tmp: &Path,
    config: &Config,
    cache: &Cache,
    stdout: &str,
    stderr: &str,
    exit: i32,
) -> Result<OracleEnv, Box<dyn std::error::Error>> {
    let tool = tmp.join("fixture-java");
    // Quoted heredocs keep captured output inert, including shell syntax.
    let script = format!(
        "#!/bin/sh\ncat <<'DIVE_FIXTURE_STDOUT'\n{stdout}\nDIVE_FIXTURE_STDOUT\ncat >&2 <<'DIVE_FIXTURE_STDERR'\n{stderr}\nDIVE_FIXTURE_STDERR\nexit {exit}\n"
    );
    std::fs::write(&tool, script)?;
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755))?;
    let home = tmp.join("home");
    let scratch = tmp.join("scratch");
    std::fs::create_dir_all(&home)?;
    std::fs::create_dir_all(&scratch)?;
    Ok(OracleEnv {
        oracle: config.oracles.default_oracle()?.clone(),
        jar: tmp.join("unused-fixture.jar"),
        classpath: vec![],
        libraries: vec![],
        java: Java {
            path: tool.clone(),
            source: JavaSource::System {
                path: tool.display().to_string(),
                sha256: dive_conformance::hash::sha256_file(&tool)?,
            },
            version_line: "synthetic process fixture, not Java".into(),
        },
        capabilities: Capabilities::default(),
        home,
        tmp_root: scratch,
        cache_root: cache.root().to_path_buf(),
    })
}

fn opts(models: bool) -> GenerateOptions {
    GenerateOptions {
        oracle: "tlc-2026.10.06.014338".into(),
        corpus: "examples".into(),
        selection: Selection {
            tier: "all".into(),
            filters: vec![
                if models {
                    "specifications/x/X.cfg"
                } else {
                    "specifications/x/X.tla"
                }
                .into(),
            ],
        },
        reason: Some("synthetic process regression control".into()),
        limits: Limits {
            jobs: 1,
            xmx: "1g".into(),
            // These synthetic fixtures test classification and golden admission,
            // not child-process latency. Keep timeout/kill semantics in process tests.
            timeout: Some(Duration::from_secs(30)),
            timeout_cap: Duration::from_secs(30),
            budget_cap: None,
        },
        models,
        sany: !models,
        workers: vec![1],
    }
}

#[test]
fn golden_models_reject_nonfacts_before_creation_or_overwrite() -> TestResult {
    for (stdout, stderr, exit) in [
        ("unrecognised oracle output".to_owned(), String::new(), 77),
        (
            fixture("Success.stdout"),
            "unexplained stderr".to_owned(),
            0,
        ),
    ] {
        let tmp = dive_conformance::util::TempDir::new("golden-nonfact")?;
        let (repo, cache) = setup(tmp.path())?;
        let config = Config::load(&repo)?;
        let path = model_golden_path(
            &repo.join("conformance/golden"),
            "tlc-2026.10.06.014338",
            "examples",
            "specifications/x/X.cfg",
        );
        let bad = env(tmp.path(), &config, &cache, &stdout, &stderr, exit)?;
        assert_eq!(
            pipeline::golden_generate(&config, &cache, &bad, &opts(true))?,
            1
        );
        assert!(!path.exists(), "nonfact became a model golden");
        let good = env(
            tmp.path(),
            &config,
            &cache,
            &fixture("Success.stdout"),
            "",
            0,
        )?;
        assert_eq!(
            pipeline::golden_generate(&config, &cache, &good, &opts(true))?,
            0
        );
        let before = std::fs::read(&path)?;
        let bad = env(tmp.path(), &config, &cache, &stdout, &stderr, exit)?;
        assert_eq!(
            pipeline::golden_generate(&config, &cache, &bad, &opts(true))?,
            1
        );
        assert_eq!(
            std::fs::read(&path)?,
            before,
            "--reason must not admit a nonfact"
        );
    }
    Ok(())
}

#[test]
fn golden_models_keep_genuine_success_and_evaluation_failure() -> TestResult {
    for (name, expected) in [
        ("Success", Verdict::Success),
        ("Overflow", Verdict::Overflow),
    ] {
        let tmp = dive_conformance::util::TempDir::new("golden-fact")?;
        let (repo, cache) = setup(tmp.path())?;
        let config = Config::load(&repo)?;
        let tool = env(
            tmp.path(),
            &config,
            &cache,
            &fixture(&format!("{name}.stdout")),
            "",
            fixture(&format!("{name}.exit")).trim().parse()?,
        )?;
        assert_eq!(
            pipeline::golden_generate(&config, &cache, &tool, &opts(true))?,
            0
        );
        let path = model_golden_path(
            &repo.join("conformance/golden"),
            "tlc-2026.10.06.014338",
            "examples",
            "specifications/x/X.cfg",
        );
        assert_eq!(read_model_golden(&path)?.result.verdict, expected);
    }
    Ok(())
}

#[test]
fn golden_sany_rejects_unclassified_before_creation_or_overwrite() -> TestResult {
    let tmp = dive_conformance::util::TempDir::new("golden-sany-nonfact")?;
    let (repo, cache) = setup(tmp.path())?;
    let config = Config::load(&repo)?;
    let path = sany_golden_path(
        &repo.join("conformance/golden"),
        "tlc-2026.10.06.014338",
        "examples",
        "specifications/x/X.tla",
    );
    let bad = env(tmp.path(), &config, &cache, "unexplained rejection", "", 77)?;
    assert_eq!(
        pipeline::golden_generate(&config, &cache, &bad, &opts(false))?,
        1
    );
    assert!(!path.exists(), "unclassified output became a SANY golden");
    let good = env(
        tmp.path(),
        &config,
        &cache,
        &fixture("Success.sany.stdout"),
        "",
        0,
    )?;
    assert_eq!(
        pipeline::golden_generate(&config, &cache, &good, &opts(false))?,
        0
    );
    let before = std::fs::read(&path)?;
    let bad = env(tmp.path(), &config, &cache, "unexplained rejection", "", 77)?;
    assert_eq!(
        pipeline::golden_generate(&config, &cache, &bad, &opts(false))?,
        1
    );
    assert_eq!(std::fs::read(path)?, before);
    Ok(())
}

#[test]
fn golden_sany_keeps_acceptance_and_classified_rejections() -> TestResult {
    for (name, expected) in [
        ("Success", SanyVerdict::Accept),
        ("ParseError", SanyVerdict::ParseError),
        ("SemanticError", SanyVerdict::SemanticError),
    ] {
        let tmp = dive_conformance::util::TempDir::new("golden-sany-fact")?;
        let (repo, cache) = setup(tmp.path())?;
        let config = Config::load(&repo)?;
        let tool = env(
            tmp.path(),
            &config,
            &cache,
            &fixture(&format!("{name}.sany.stdout")),
            "",
            fixture(&format!("{name}.sany.exit")).trim().parse()?,
        )?;
        assert_eq!(
            pipeline::golden_generate(&config, &cache, &tool, &opts(false))?,
            0
        );
        let path = sany_golden_path(
            &repo.join("conformance/golden"),
            "tlc-2026.10.06.014338",
            "examples",
            "specifications/x/X.tla",
        );
        assert_eq!(read_sany_golden(&path)?.result.verdict, expected);
    }
    Ok(())
}
