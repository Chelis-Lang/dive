//! Dispatch the explicitly requested symbolic strategy to its release companion.

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
            "--type-invariant" | "--write-predicate" | "--verify-predicate" | "--trace"
            | "--max-orbits" => {
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
        Some("symbolic") => {}
        Some("occupancy") => {
            command.arg("--occupancy");
        }
        _ => return Err("--strategy expects symbolic or occupancy".to_owned()),
    }
    let module = module.ok_or_else(|| "missing TLA+ module".to_owned())?;
    let config = config.unwrap_or_else(|| module.with_extension("cfg"));
    command.arg("--").arg(module).arg(config);
    Ok(())
}

pub(super) fn run(args: &[String], err: &mut impl std::io::Write) -> ExitCode {
    let Ok(dive) = std::env::current_exe() else {
        return super::simulation_usage_error(err, "could not locate the running Dive executable");
    };
    let Some(bin) = dive.parent() else {
        return super::simulation_usage_error(err, "could not locate the Dive installation");
    };
    let worker = bin.join("libexec/dive-symbolic");
    let mut command = Command::new(worker.join("python/bin/python3.11"));
    command
        .arg("-Es")
        .arg(worker.join("check.py"))
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
            let _ = writeln!(
                err,
                "symbolic worker unavailable in this installation: {error}"
            );
            ExitCode::from(super::EXIT_UNSUPPORTED)
        }
    }
}
