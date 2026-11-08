//! Public help and human checking-header contracts.

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
    let commands: Vec<_> = section.lines().map(str::trim).collect();
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
    assert_eq!(help, include_str!("golden/command-help.txt"));
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
fn human_header_names_requested_engine_and_actual_workers() -> Result<(), Box<dyn std::error::Error>>
{
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
        let named = engine.unwrap_or("core");
        assert!(header.contains(&format!("requested engine {named},")));
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
