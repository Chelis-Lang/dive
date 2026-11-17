//! Public help and human checking-header contracts.
//!
//! The pinned help is `tests/golden/command-help.txt`; regenerate it with
//! `DIVE_UPDATE_HELP_GOLDENS=1 cargo test -p dive-cli --test command_help`.

use std::fmt::Write as _;
use std::process::{Command, Output};

fn dive(arguments: &[&str]) -> std::io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_dive"))
        .args(arguments)
        .env_remove("DIVE_COMPAT")
        .env_remove("DIVE_ENGINE")
        .env_remove("DIVE_CHAOS")
        .output()
}

#[test]
fn every_advertised_command_has_pinned_help() -> Result<(), Box<dyn std::error::Error>> {
    let root = dive(&["--help"])?;
    assert!(root.status.success());
    assert_eq!(root.stderr, [] as [u8; 0]);
    let root = String::from_utf8(root.stdout)?;
    assert!(root.starts_with(&format!("dive {}\n", env!("CARGO_PKG_VERSION"))));
    let section = root
        .split_once("COMMANDS:\n")
        .ok_or("missing command list")?
        .1
        .split_once("\n\n")
        .ok_or("missing command-list terminator")?
        .0;
    let commands: Vec<_> = section
        .lines()
        .map(|line| line.split_whitespace().next().unwrap_or_default())
        .collect();
    for line in section.lines() {
        assert!(
            line.split_whitespace().count() > 1,
            "command without a description: {line:?}"
        );
    }
    assert!(!commands.contains(&"lsp"));
    let root = root.replacen(env!("CARGO_PKG_VERSION"), "{version}", 1);
    let mut help = format!("=== dive --help ===\n{root}");
    for command in commands {
        let output = dive(&[command, "--help"])?;
        assert!(output.status.success(), "{command}: {output:?}");
        assert!(output.stderr.is_empty(), "{command}: {output:?}");
        write!(
            help,
            "\n=== dive {command} --help ===\n{}",
            String::from_utf8(output.stdout)?
        )?;
    }
    let golden =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/command-help.txt");
    // Regenerate with `DIVE_UPDATE_HELP_GOLDENS=1 cargo test -p dive-cli --test command_help`.
    if std::env::var("DIVE_UPDATE_HELP_GOLDENS").as_deref() == Ok("1") {
        std::fs::write(&golden, &help)?;
    }
    assert_eq!(help, std::fs::read_to_string(&golden)?);
    Ok(())
}

#[test]
fn version_is_pinned_and_needs_no_runtime_git() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_dive"))
        .arg("--version")
        .env("PATH", "")
        .output()?;
    assert!(output.status.success());
    assert_eq!(output.stderr, [] as [u8; 0]);
    let expected = include_str!("golden/version.txt")
        .replace("{version}", env!("CARGO_PKG_VERSION"))
        .replace("{revision}", env!("DIVE_GIT_DESCRIBE"));
    assert_eq!(String::from_utf8(output.stdout)?, expected);
    Ok(())
}

#[test]
fn hidden_commands_still_refuse_explicitly() -> std::io::Result<()> {
    let output = dive(&["lsp", "--help"])?;
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(output.stdout, [] as [u8; 0]);
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("unsupported:"));
    Ok(())
}

#[test]
fn human_header_names_actual_workers_and_no_engine() -> Result<(), Box<dyn std::error::Error>> {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/user/examples/Counter.tla");
    // `None` is the default engine, Core.
    for engine in [Some("core"), None] {
        let path = file.to_str().ok_or("non-UTF-8 fixture path")?;
        let mut arguments = vec!["check", path, "--workers=1", "--seed=7", "--fp=0"];
        if let Some(engine) = engine {
            arguments.extend(["--engine", engine]);
        }
        let output = dive(&arguments)?;
        assert!(output.status.success(), "{engine:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{engine:?}: {output:?}");
        let human = String::from_utf8(output.stdout)?;
        let header = human.lines().next().ok_or("missing human header")?;
        // Core is the only engine, so the header does not name one.
        assert!(header.contains(": profile dive, store fingerprint (fp 0), seed 7,"));
        assert!(!header.contains("engine"));
        assert!(header.contains("1 worker(s) requested, 1 used"));
        assert!(!human.contains("engine auto"));
    }
    let removed = dive(&["check", "Counter.tla", "--engine", "auto"])?;
    assert_eq!(removed.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&removed.stderr).contains("--engine auto was removed"));
    let removed = dive(&["check", "Counter.tla", "--engine", "reference"])?;
    assert_eq!(removed.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&removed.stderr).contains("--engine reference was removed"));
    let removed = Command::new(env!("CARGO_BIN_EXE_dive"))
        .args(["simulate", "Counter.tla", "--traces", "1"])
        .env("DIVE_ENGINE", "reference")
        .output()?;
    assert_eq!(removed.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&removed.stderr).contains("DIVE_ENGINE reference was removed"));
    Ok(())
}

/// Each hidden option, with the feature its refusal names. The listed
/// `--symmetry-liveness tlc` (the TLC-profile default) is included for its
/// refused combination with `SYMMETRY` and temporal properties.
const HIDDEN: &[(&[&str], &str)] = &[
    (
        &[
            "check",
            "Counter.tla",
            "--config",
            "Temporal.cfg",
            "--lncheck",
            "final",
            "--lasso",
            "short",
        ],
        "bounded final checking (--lncheck final --lasso short) is not implemented on Core",
    ),
    (
        &["check", "Counter.tla", "--action-composition", "tlc"],
        "--action-composition=tlc under the core engine",
    ),
    (
        &["check", "Counter.tla", "-Dtlc2.tool.impl.Tool.cdot", "true"],
        "--action-composition=tlc under the core engine",
    ),
    (
        &["check", "Counter.tla", "--allow-io=env,files,exec"],
        "external inputs (--allow-io, IOUtils/IOEnv) are not implemented on Core",
    ),
    (
        &["check", "Counter.tla", "--source-coverage"],
        "source coverage and TLC logical coverage (--source-coverage, --coverage) are not implemented on Core",
    ),
    (
        &["check", "Counter.tla", "--coverage", "0"],
        "source coverage and TLC logical coverage (--source-coverage, --coverage) are not implemented on Core",
    ),
    (
        &["check", "Sym.tla", "--symmetry-liveness", "tlc"],
        "SYMMETRY Perms",
    ),
    (
        &["simulate", "Counter.tla", "--traces", "2", "--workers", "2"],
        "parallel simulation workers",
    ),
    (
        &["dfid", "Counter.tla", "--max-level", "6", "--workers", "2"],
        "DFID multiple workers",
    ),
    (
        &["tlc", "-coverage", "0", "Counter"],
        "source coverage and TLC logical coverage (--source-coverage, --coverage) are not implemented on Core",
    ),
    (
        &[
            "tlc",
            "-lncheck",
            "final",
            "-config",
            "Temporal.cfg",
            "Counter",
        ],
        "bounded final checking (--lncheck final --lasso short) is not implemented on Core",
    ),
    (
        &[
            "tlc",
            "--symmetry-liveness",
            "tlc",
            "-config",
            "Sym.cfg",
            "Sym",
        ],
        "SYMMETRY Perms",
    ),
];

/// Removes a per-test scratch directory of specifications.
struct Scratch(std::path::PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Writes the small specifications the hidden-option cases run.
fn write_specs(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(
        dir.join("Counter.tla"),
        "---- MODULE Counter ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\n\
         Next == x' = IF x < 4 THEN x + 1 ELSE x\nBound == x <= 4\n\
         EventuallyFour == <>(x = 4)\n====\n",
    )?;
    std::fs::write(
        dir.join("Counter.cfg"),
        "INIT Init\nNEXT Next\nINVARIANT Bound\n",
    )?;
    std::fs::write(
        dir.join("Temporal.cfg"),
        "INIT Init\nNEXT Next\nPROPERTY EventuallyFour\n",
    )?;
    std::fs::write(
        dir.join("Sym.tla"),
        "---- MODULE Sym ----\nEXTENDS Naturals, TLC\nCONSTANT P\nVARIABLE s\nInit == s = {}\n\
         Next == \\E p \\in P : s' = s \\cup {p}\nLive == <>(s = P)\nPerms == Permutations(P)\n====\n",
    )?;
    std::fs::write(
        dir.join("Sym.cfg"),
        "CONSTANT P = {p1, p2}\nINIT Init\nNEXT Next\nSYMMETRY Perms\nPROPERTY Live\n",
    )?;
    Ok(())
}

/// Options Core always refuses are absent from every help text, but still
/// parse and refuse by name with exit 3 rather than becoming usage errors.
#[test]
fn hidden_options_still_parse_and_refuse_by_name() -> Result<(), Box<dyn std::error::Error>> {
    let scratch =
        Scratch(std::env::temp_dir().join(format!("dive-hidden-options-{}", std::process::id())));
    write_specs(&scratch.0)?;
    let dir = &scratch.0;
    let mut help = String::new();
    for command in ["check", "simulate", "dfid", "tlc"] {
        let output = dive(&[command, "--help"])?;
        help.push_str(&String::from_utf8(output.stdout)?);
    }
    // The root summary, without validate-trace: the independent validator
    // keeps every symmetry-liveness policy.
    let root = String::from_utf8(dive(&["--help"])?.stdout)?;
    let (summary, validator) = root
        .split_once("    dive validate-trace")
        .ok_or("missing validate-trace summary")?;
    help.push_str(summary);
    help.push_str(validator.split_once("\n\n").ok_or("missing summary end")?.1);
    for hidden in [
        "--lncheck final",
        "-lncheck final",
        "--lasso",
        "--allow-io",
        "--source-coverage",
        "coverage 0",
        "off|tlc|tla",
        "--workers N]",
        "--workers N ",
    ] {
        assert!(!help.contains(hidden), "help still shows {hidden}");
    }
    // `--symmetry-liveness tlc` runs whenever SYMMETRY and temporal
    // properties do not meet, so it stays listed.
    assert!(help.contains("--symmetry-liveness unreduced|refuse|tlc"));
    for (arguments, feature) in HIDDEN {
        let (command, rest) = arguments.split_first().ok_or("empty case")?;
        let mut arguments = vec![*command, "--output", "json"];
        if matches!(*command, "check" | "tlc") {
            arguments.push("--workers=1");
        }
        arguments.extend(rest);
        let output = Command::new(env!("CARGO_BIN_EXE_dive"))
            .args(&arguments)
            .current_dir(dir)
            .env_remove("DIVE_COMPAT")
            .env_remove("DIVE_ENGINE")
            .env_remove("DIVE_CHAOS")
            .output()?;
        assert_eq!(output.status.code(), Some(3), "{arguments:?}: {output:?}");
        let stdout = String::from_utf8(output.stdout)?;
        let stderr = String::from_utf8(output.stderr)?;
        assert!(
            stdout.contains(feature) || stderr.contains(feature),
            "{arguments:?} does not name {feature}: {stdout} {stderr}"
        );
        if let Ok(report) = serde_json::from_str::<serde_json::Value>(&stdout) {
            let verdict = report.get("verdict").or_else(|| report.get("result"));
            assert_eq!(
                verdict.and_then(serde_json::Value::as_str),
                Some("unsupported"),
                "{arguments:?}: {stdout}"
            );
        }
    }
    Ok(())
}
