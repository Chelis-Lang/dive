//! Repository automation. Run as `cargo xtask <command>`.
#![forbid(unsafe_code)]

mod features;
mod gate;
mod layering;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "usage: cargo xtask <command>

commands:
    install-hooks   copy tracked git hooks (commit-msg authorship check,
                    pre-commit repository budget) into the shared hooks directory
    check-layering  verify internal crate dependencies follow the charter DAG, and
                    that dive-trace builds without dive-eval/enumerate
    protect-main    apply branch protection to main on GitHub (PR mode)
    gate            run the local gate: --fast mirrors CI's Workspace gate;
                    --full [--skip-corpus] adds the whole suite and conformance
";

type Result<T> = std::result::Result<T, String>;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("install-hooks") => install_hooks(),
        Some("check-layering") => check_layering(),
        Some("protect-main") => protect_main(),
        Some("gate") => gate::gate(&args[1..]),
        _ => Err(USAGE.to_owned()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn workspace_root() -> PathBuf {
    // xtask lives one level below the workspace root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn run_capture(cmd: &mut Command) -> Result<String> {
    let output = cmd
        .output()
        .map_err(|e| format!("failed to run {cmd:?}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "{cmd:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|e| format!("{cmd:?} produced non-UTF-8 output: {e}"))
}

/// Copies `.githooks/*` (`commit-msg`, `pre-commit`) into the *common* git
/// directory's hooks folder, so a single install covers every worktree. Deliberately avoids
/// `core.hooksPath`, which would silently disable hooks on branches that
/// predate the tracked hook files.
fn install_hooks() -> Result<()> {
    let root = workspace_root();
    let common = run_capture(Command::new("git").arg("-C").arg(&root).args([
        "rev-parse",
        "--path-format=absolute",
        "--git-common-dir",
    ]))?;
    let hooks_dir = PathBuf::from(common.trim()).join("hooks");
    std::fs::create_dir_all(&hooks_dir)
        .map_err(|e| format!("cannot create {}: {e}", hooks_dir.display()))?;
    let source = root.join(".githooks");
    let entries =
        std::fs::read_dir(&source).map_err(|e| format!("cannot read {}: {e}", source.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read {}: {e}", source.display()))?;
        let from = entry.path();
        let to = hooks_dir.join(entry.file_name());
        std::fs::copy(&from, &to)
            .map_err(|e| format!("cannot copy {} to {}: {e}", from.display(), to.display()))?;
        set_executable(&to)?;
        println!("installed {}", to.display());
    }
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("cannot chmod {}: {e}", path.display()))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}

/// Checks every workspace member's direct `dive-*` dependencies (normal and
/// build; dev-dependencies are exempt) against [`layering::ALLOWED`].
fn check_layering() -> Result<()> {
    let root = workspace_root();
    let raw = run_capture(
        Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
            .current_dir(&root)
            .args([
                "metadata",
                "--format-version",
                "1",
                "--no-deps",
                "--offline",
            ]),
    )?;
    let meta: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("cannot parse cargo metadata: {e}"))?;
    let allowed: BTreeMap<&str, BTreeSet<&str>> = layering::ALLOWED
        .iter()
        .map(|(name, deps)| (*name, deps.iter().copied().collect()))
        .collect();

    let packages = meta["packages"]
        .as_array()
        .ok_or("cargo metadata has no packages array")?;
    let mut violations = Vec::new();
    let mut seen = BTreeSet::new();
    for package in packages {
        let name = package["name"].as_str().unwrap_or_default();
        if !name.starts_with("dive-") {
            continue;
        }
        seen.insert(name.to_owned());
        let Some(permitted) = allowed.get(name) else {
            violations.push(format!(
                "{name}: not listed in xtask/src/layering.rs (add it with an ADR)"
            ));
            continue;
        };
        for dep in package["dependencies"].as_array().into_iter().flatten() {
            let dep_name = dep["name"].as_str().unwrap_or_default();
            let is_dev = dep["kind"].as_str() == Some("dev");
            if dep_name.starts_with("dive-") && !is_dev && !permitted.contains(dep_name) {
                violations.push(format!("{name} -> {dep_name} is not a permitted edge"));
            }
        }
    }
    for root in layering::ENUMERATE_FREE {
        if features::enables(packages, root, "dive-eval", "enumerate") {
            violations.push(format!(
                "{root} enables dive-eval/enumerate (directly or through a dependency); \
                 the successor enumerator must stay out of it (ADR 0016 §2)"
            ));
        }
    }
    for name in allowed.keys() {
        if !seen.contains(*name) {
            violations.push(format!(
                "{name}: listed in layering table but not in the workspace"
            ));
        }
    }
    if violations.is_empty() {
        println!("layering ok ({} crates)", seen.len());
        Ok(())
    } else {
        Err(format!(
            "layering violations:\n  {}",
            violations.join("\n  ")
        ))
    }
}

/// Applies the org-standard branch protection to `main`: PRs required,
/// required checks `No AI authorship markers`, `Workspace gate` and
/// `Conformance gate`, admins
/// included, no force pushes. Run once when the project switches to
/// PR-focused development (see AGENTS.md).
fn protect_main() -> Result<()> {
    let body = serde_json::json!({
        "required_status_checks": {
            "strict": false,
            "contexts": ["No AI authorship markers", "Workspace gate", "Conformance gate"]
        },
        "enforce_admins": true,
        "required_pull_request_reviews": {
            "required_approving_review_count": 0,
            "dismiss_stale_reviews": false,
            "require_code_owner_reviews": false
        },
        "restrictions": null,
        "allow_force_pushes": false,
        "allow_deletions": false
    });
    let mut child = Command::new("gh")
        .args([
            "api",
            "--method",
            "PUT",
            "repos/Chelis-Lang/dive/branches/main/protection",
            "--input",
            "-",
        ])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to run gh: {e}"))?;
    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write as _;
        stdin
            .write_all(body.to_string().as_bytes())
            .map_err(|e| format!("failed to write to gh: {e}"))?;
    }
    let status = child
        .wait()
        .map_err(|e| format!("gh did not finish: {e}"))?;
    if status.success() {
        println!("branch protection applied to main");
        Ok(())
    } else {
        Err(format!("gh api exited with {status}"))
    }
}
