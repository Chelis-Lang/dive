//! Dispatch the explicitly requested finite safety strategy to its release companion.

use std::path::PathBuf;
use std::process::{Command, ExitCode};

use dive_api::BudgetSetting;

pub(super) fn requested(args: &[String]) -> bool {
    args.iter()
        .any(|a| a == "--strategy" || a.starts_with("--strategy="))
}

fn options(args: &[String], command: &mut Command) -> Result<(), String> {
    let mut module = None;
    let mut config = None;
    let mut strategy = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let (key, inline) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(k, v)| (k, Some(v)));
        let mut value = || {
            inline
                .or_else(|| args.next().map(String::as_str))
                .ok_or_else(|| format!("{key} needs a value"))
        };
        match key {
            "--strategy" => {
                if strategy.replace(value()?).is_some() {
                    return Err("--strategy given twice".to_owned());
                }
            }
            "--config" => {
                if config.replace(PathBuf::from(value()?)).is_some() {
                    return Err("--config given twice".to_owned());
                }
            }
            "--property-proof" | "--type-invariant" | "--write-predicate"
            | "--verify-predicate" | "--trace" | "--max-orbits" | "--workers" => {
                command.arg(key).arg(value()?);
            }
            "--recheck" if inline.is_none() => {
                let revision = value()?;
                let config = args
                    .next()
                    .ok_or_else(|| "--recheck needs MODULE CONFIG".to_owned())?;
                command.arg(key).arg(revision).arg(config);
            }
            "--prove-inductive" | "--refine-domains" if inline.is_none() => {
                command.arg(key);
            }
            "--output" if value()? == "json" => {}
            "--time-limit" => match super::time_limit(value()?) {
                Some(BudgetSetting::Explicit(time)) => {
                    command.arg(key).arg(time.as_secs_f64().to_string());
                }
                Some(BudgetSetting::Unlimited) => {}
                _ => return Err("expected a positive duration or none".to_owned()),
            },
            "--max-memory" => match super::max_memory(value()?) {
                Some(BudgetSetting::Explicit(bytes)) if bytes >= 1 << 20 => {
                    command.arg(key).arg((bytes >> 20).to_string());
                }
                Some(BudgetSetting::Unlimited) => {}
                _ => {
                    return Err("symbolic memory budget must be at least 1 MiB, or none".to_owned());
                }
            },
            _ if !arg.starts_with('-') && module.is_none() => module = Some(PathBuf::from(arg)),
            _ => return Err(format!("unsupported symbolic argument: {arg}")),
        }
    }
    match strategy {
        Some("symbolic" | "auto") => {}
        Some("occupancy") => {
            command.arg("--occupancy");
        }
        Some("compiled") => {
            command.arg("--compiled");
        }
        Some("finite-induction") => {
            command.arg("--finite-induction");
        }
        _ => {
            return Err(
                "--strategy expects auto, symbolic, occupancy, compiled or finite-induction"
                    .to_owned(),
            );
        }
    }
    let module = module.ok_or_else(|| "missing TLA+ module".to_owned())?;
    let config = config.unwrap_or_else(|| module.with_extension("cfg"));
    command.arg("--").arg(module).arg(config);
    Ok(())
}

// Only a single explicit selector may be removed. Leave duplicates for the
// regular strategy parser to reject, even when the first selector is auto.
fn without_auto(args: &[String]) -> Option<Vec<String>> {
    let mut selectors = args
        .iter()
        .enumerate()
        .filter(|(_, arg)| arg.as_str() == "--strategy" || arg.starts_with("--strategy="));
    let (start, flag) = selectors.next()?;
    if selectors.next().is_some() {
        return None;
    }
    let count = match flag.as_str() {
        "--strategy=auto" => 1,
        "--strategy" if args.get(start + 1).is_some_and(|s| s == "auto") => 2,
        _ => return None,
    };
    let mut ordinary = args.to_vec();
    ordinary.drain(start..start + count);
    Some(ordinary)
}

// Core owns every other option, including non-JSON reporting, compatibility,
// limits on state counts and changes to the model's search path.
fn finite_request(args: &[String]) -> bool {
    if std::env::vars_os().any(|(key, _)| {
        matches!(
            key.to_str(),
            Some("DIVE_COMPAT" | "DIVE_CORE_OPT" | "DIVE_CHAOS" | "DIVE_NATIVE_ATOM_AUDIT")
        )
    }) {
        return false;
    }
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if !arg.starts_with('-') {
            continue;
        }
        let (key, inline) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(k, v)| (k, Some(v)));
        let value = inline.or_else(|| it.next().map(String::as_str));
        match (key, value) {
            ("--config" | "--time-limit", Some(_)) | ("--output", Some("json")) => {}
            ("--max-memory", Some(v)) if v != "auto" => {}
            ("--workers", Some(v)) if v.parse::<u8>().is_ok_and(|w| (1..=8).contains(&w)) => {}
            _ => return false,
        }
    }
    true
}

fn ordinary(
    args: &[String],
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
) -> ExitCode {
    let _ = writeln!(err, "dive: auto selected Core");
    super::check_command(args, out, err)
}

pub(super) fn run(
    args: &[String],
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
) -> ExitCode {
    let Ok(dive) = std::env::current_exe() else {
        return super::simulation_usage_error(err, "could not locate the running Dive executable");
    };
    let Some(bin) = dive.parent() else {
        return super::simulation_usage_error(err, "could not locate the Dive installation");
    };
    let worker = bin.join("libexec/dive-symbolic");
    let python = worker.join("python/bin/python3.11");
    let mut command = Command::new(&python);
    let mut auto = without_auto(args);
    if let Some(core) = &mut auto {
        if !finite_request(core) {
            return ordinary(core, out, err);
        }
        // Both the finite worker and its Core alternative use exact identity.
        if !core
            .iter()
            .any(|a| a == "--output" || a.starts_with("--output="))
        {
            core.extend(["--output".to_owned(), "json".to_owned()]);
        }
        core.extend(["--store".to_owned(), "exact".to_owned()]);
        if !cfg!(target_os = "linux")
            || !python.is_file()
            || !worker.join("symbolic_input").is_file()
            || !worker.join("auto.py").is_file()
        {
            return ordinary(core, out, err);
        }
        command
            .arg("-Es")
            .arg(worker.join("check.py"))
            .arg("--auto-core-args")
            .arg(serde_json::Value::from(core.clone()).to_string());
    } else {
        command.arg("-Es").arg(worker.join("check.py"));
    }
    command
        .arg("--exporter")
        .arg(worker.join("symbolic_input"))
        .arg("--dive")
        .arg(dive);
    if let Err(reason) = options(args, &mut command) {
        return super::simulation_usage_error(err, &reason);
    }
    if let Err(reason) = super::engine_from_env() {
        return super::simulation_usage_error(err, &reason);
    }
    // A supervisor sending TERM to the public CLI reaches the worker supervisor
    // even while a native decision-diagram operation is in progress.
    #[cfg(unix)]
    let result: std::io::Result<std::process::ExitStatus> = {
        use std::os::unix::process::CommandExt as _;
        Err(command.exec())
    };
    #[cfg(not(unix))]
    let result = command.status();
    match result {
        Ok(status) => ExitCode::from(
            status
                .code()
                .and_then(|n| u8::try_from(n).ok())
                .unwrap_or(4),
        ),
        Err(error) => {
            if let Some(core) = auto {
                return ordinary(&core, out, err);
            }
            let _ = writeln!(
                err,
                "symbolic worker unavailable in this installation: {error}"
            );
            ExitCode::from(super::EXIT_UNSUPPORTED)
        }
    }
}
