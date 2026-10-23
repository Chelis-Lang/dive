//! Synthetic process controls, not new pinned-oracle observations.

use super::golden_facts::{env, fixture, opts};
use super::*;
use dive_conformance::golden::read_model_golden;
use dive_conformance::oracle::{CheckMode, OracleEnv, TlcJob};
use serde_json::Value;

fn instrument(tool: &OracleEnv, prefix: &str) -> TestResult {
    let script = std::fs::read_to_string(&tool.java.path)?;
    std::fs::write(
        &tool.java.path,
        script.replacen(
            "#!/bin/sh\n",
            &format!("#!/bin/sh\nprintf 'call\\n' >> \"$0.calls\"\n{prefix}\n"),
            1,
        ),
    )?;
    Ok(())
}

fn calls(tool: &OracleEnv) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(
        std::fs::read_to_string(tool.java.path.with_extension("calls"))?
            .lines()
            .count(),
    )
}

fn captures(root: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    std::fs::read_dir(root)?
        .map(|entry| Ok(entry?.path()))
        .collect()
}

fn json(path: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

fn golden(repo: &Path) -> PathBuf {
    model_golden_path(
        &repo.join("conformance/golden"),
        "tlc-2026.10.06.014338",
        "examples",
        "specifications/x/X.cfg",
    )
}

#[test]
fn raw_success_is_opt_in_and_does_not_add_passes_or_change_result() -> TestResult {
    let tmp = dive_conformance::util::TempDir::new("raw-success")?;
    let (repo, cache) = setup(tmp.path())?;
    let config = Config::load(&repo)?;
    let stdout = fixture("Success.stdout");
    let mut tool = env(tmp.path(), &config, &cache, &stdout, "", 0)?;
    instrument(&tool, "")?;
    let root = tmp.path().join("raw");
    assert_eq!(
        pipeline::golden_generate(&config, &cache, &tool, &opts(true))?,
        0
    );
    let original = read_model_golden(&golden(&repo))?;
    assert!(!root.exists());
    assert_eq!(std::fs::read_dir(&tool.tmp_root)?.count(), 0);
    assert_eq!(calls(&tool)?, 1);

    tool.raw_invocations = Some(root.clone());
    assert_eq!(
        pipeline::golden_generate(&config, &cache, &tool, &opts(true))?,
        0
    );
    let recorded = read_model_golden(&golden(&repo))?;
    assert_eq!(recorded.result, original.result);
    assert_eq!(recorded.provenance.command, original.provenance.command);
    assert_eq!(calls(&tool)?, 2);
    let dirs = captures(&root)?;
    assert_eq!(dirs.len(), 1);
    assert_eq!(
        std::fs::read(dirs[0].join("stdout.txt"))?,
        format!("{stdout}\n").as_bytes()
    );
    let started = json(&dirs[0].join("started.json"))?;
    assert_eq!(started["phase"], "clean");
    assert_eq!(started["timeout"]["seconds"], 30);
    assert_eq!(
        started["argv"][0],
        tool.java.path.to_string_lossy().as_ref()
    );
    assert!(started.get("env").is_none());
    assert_eq!(json(&dirs[0].join("completed.json"))?["exit_code"], 0);
    assert!(!dirs[0].join("trace.json").exists());
    Ok(())
}

#[test]
fn raw_counterexample_retains_exact_clean_and_trace_with_only_two_passes() -> TestResult {
    let tmp = dive_conformance::util::TempDir::new("raw-trace")?;
    let (repo, cache) = setup(tmp.path())?;
    let config = Config::load(&repo)?;
    let stdout = fixture("InitInvariant.stdout");
    let mut tool = env(tmp.path(), &config, &cache, &stdout, "", 12)?;
    let trace = fixture("InitInvariant.trace.json");
    instrument(
        &tool,
        &format!(
            "last=\nfor arg do\nif [ \"$last\" = json ]; then\ncat > \"$arg\" <<'DIVE_TRACE'\n{trace}\nDIVE_TRACE\nfi\nlast=$arg\ndone"
        ),
    )?;
    tool.capabilities.dump_trace_json = true;
    let root = tmp.path().join("raw");
    tool.raw_invocations = Some(root.clone());
    assert_eq!(
        pipeline::golden_generate(&config, &cache, &tool, &opts(true))?,
        0
    );
    assert_eq!(calls(&tool)?, 2);
    let dirs = captures(&root)?;
    assert_eq!(dirs.len(), 2);
    let mut phases = Vec::new();
    for dir in dirs {
        let started = json(&dir.join("started.json"))?;
        let phase = started["phase"].as_str().ok_or("missing phase")?;
        phases.push(phase.to_owned());
        assert_eq!(
            std::fs::read(dir.join("stdout.txt"))?,
            format!("{stdout}\n").as_bytes()
        );
        assert_eq!(json(&dir.join("completed.json"))?["exit_code"], 12);
        if phase == "trace" {
            assert_eq!(
                std::fs::read(dir.join("trace.json"))?,
                format!("{trace}\n").as_bytes()
            );
        } else {
            assert!(!dir.join("trace.json").exists());
        }
    }
    phases.sort();
    assert_eq!(phases, ["clean", "trace"]);
    let generated = read_model_golden(&golden(&repo))?;
    assert_eq!(generated.result.verdict, Verdict::SafetyViolation);
    assert!(generated.provenance.trace_command.is_some());
    Ok(())
}

#[test]
fn raw_nonfact_keeps_original_bytes_without_a_golden() -> TestResult {
    let tmp = dive_conformance::util::TempDir::new("raw-errors")?;
    let (repo, cache) = setup(tmp.path())?;
    let config = Config::load(&repo)?;
    let mut tool = env(tmp.path(), &config, &cache, "unknown stream", "", 77)?;
    instrument(&tool, "printf '\\377' >&2")?;
    let root = tmp.path().join("raw");
    tool.raw_invocations = Some(root.clone());
    assert_eq!(
        pipeline::golden_generate(&config, &cache, &tool, &opts(true))?,
        1
    );
    assert_eq!(calls(&tool)?, 1);
    assert!(!golden(&repo).exists());
    let dirs = captures(&root)?;
    assert_eq!(dirs.len(), 1);
    assert_eq!(std::fs::read(dirs[0].join("stderr.txt"))?, [255, b'\n']);
    assert_eq!(json(&dirs[0].join("completed.json"))?["exit_code"], 77);

    Ok(())
}

#[test]
fn raw_recording_failures_do_not_dispatch_or_admit_an_uncaptured_golden() -> TestResult {
    let tmp = dive_conformance::util::TempDir::new("raw-record-failure")?;
    let (repo, cache) = setup(tmp.path())?;
    let config = Config::load(&repo)?;
    let mut tool = env(
        tmp.path(),
        &config,
        &cache,
        &fixture("Success.stdout"),
        "",
        0,
    )?;
    instrument(
        &tool,
        "for arg do\ncase $arg in -Djava.io.tmpdir=*) scratch=${arg#*=} ;; esac\ndone\nmkdir \"$scratch/completed.json\"",
    )?;
    let root = tmp.path().join("raw");
    std::fs::write(&root, "not a directory")?;
    tool.raw_invocations = Some(root.clone());
    assert_eq!(
        pipeline::golden_generate(&config, &cache, &tool, &opts(true))?,
        1
    );
    assert!(!tool.java.path.with_extension("calls").exists());
    assert!(!golden(&repo).exists());
    std::fs::remove_file(&root)?;
    assert_eq!(
        pipeline::golden_generate(&config, &cache, &tool, &opts(true))?,
        1
    );
    assert_eq!(calls(&tool)?, 1);
    let dirs = captures(&root)?;
    assert_eq!(dirs.len(), 1);
    assert!(dirs[0].join("started.json").exists());
    assert!(dirs[0].join("stdout.txt").is_file());
    assert!(dirs[0].join("completed.json").is_dir());
    assert!(!golden(&repo).exists());
    Ok(())
}

#[test]
fn raw_timeout_retains_partial_original_output_without_a_golden() -> TestResult {
    let tmp = dive_conformance::util::TempDir::new("raw-timeout")?;
    let (repo, cache) = setup(tmp.path())?;
    let config = Config::load(&repo)?;
    let mut tool = env(tmp.path(), &config, &cache, "unreached", "", 0)?;
    instrument(&tool, "printf 'partial\\n'\nsleep 30")?;
    let root = tmp.path().join("raw");
    tool.raw_invocations = Some(root.clone());
    let mut options = opts(true);
    options.limits.timeout = Some(Duration::from_secs(2));
    assert_eq!(
        pipeline::golden_generate(&config, &cache, &tool, &options)?,
        1
    );
    assert_eq!(calls(&tool)?, 1);
    assert!(!golden(&repo).exists());
    let dirs = captures(&root)?;
    assert_eq!(dirs.len(), 1);
    assert_eq!(std::fs::read(dirs[0].join("stdout.txt"))?, b"partial\n");
    let completed = json(&dirs[0].join("completed.json"))?;
    assert_eq!(completed["timed_out"], true);
    assert!(completed["exit_code"].is_null());
    Ok(())
}

fn clean_job(corpus: &Path) -> TlcJob<'_> {
    TlcJob {
        corpus_root: corpus,
        module: "specifications/x/X.tla",
        config: "specifications/x/X.cfg",
        mode: CheckMode::Exhaustive,
        features: &[],
        workers: 1,
        timeout: Duration::from_secs(30),
        xmx: "256m",
        seed: 0,
        lncheck: "final",
        fp: 0,
        dump_trace: false,
    }
}

#[test]
fn raw_source_descendants_and_destination_aliases_fail_before_mkdir_or_dispatch() -> TestResult {
    let tmp = dive_conformance::util::TempDir::new("raw-overlap")?;
    let (repo, cache) = setup(tmp.path())?;
    let config = Config::load(&repo)?;
    let corpus = pipeline::load_corpus(&config, &cache, "examples")?;
    let source = corpus.root.join("specifications/x");
    let mut tool = env(
        tmp.path(),
        &config,
        &cache,
        &fixture("Success.stdout"),
        "",
        0,
    )?;
    instrument(&tool, "")?;
    let alias = tmp.path().join("source-alias");
    std::os::unix::fs::symlink(&source, &alias)?;
    let missing = tmp.path().join("not-created");
    for destination in [
        source.join("raw/nested"),
        alias.join("raw/nested"),
        missing.join("../source-alias/raw/nested"),
    ] {
        tool.raw_invocations = Some(destination);
        let error = tool.run_tlc(&clean_job(&corpus.root));
        assert!(
            matches!(error, Err(Error::Config(message)) if message.contains("overlaps copied source"))
        );
        assert!(!source.join("raw").exists());
        assert!(!missing.exists());
        assert!(!tool.java.path.with_extension("calls").exists());
    }
    Ok(())
}

#[test]
fn raw_source_symlink_to_external_capture_ancestor_fails_before_mkdir_or_dispatch() -> TestResult {
    let tmp = dive_conformance::util::TempDir::new("raw-source-alias")?;
    let (repo, cache) = setup(tmp.path())?;
    let config = Config::load(&repo)?;
    let corpus = pipeline::load_corpus(&config, &cache, "examples")?;
    let external = tmp.path().join("external");
    std::fs::create_dir(&external)?;
    std::os::unix::fs::symlink(&external, corpus.root.join("specifications/x/external"))?;
    let mut tool = env(
        tmp.path(),
        &config,
        &cache,
        &fixture("Success.stdout"),
        "",
        0,
    )?;
    instrument(&tool, "")?;
    for destination in [external.clone(), external.join("raw/nested")] {
        tool.raw_invocations = Some(destination);
        let error = tool.run_tlc(&clean_job(&corpus.root));
        assert!(
            matches!(error, Err(Error::Config(message)) if message.contains("overlaps copied source"))
        );
        assert_eq!(std::fs::read_dir(&external)?.count(), 0);
        assert!(!tool.java.path.with_extension("calls").exists());
    }
    Ok(())
}
