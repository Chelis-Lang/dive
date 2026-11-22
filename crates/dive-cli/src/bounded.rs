//! Command for transition-bounded safety with an explicit path length.

use std::io::Write;
use std::process::ExitCode;

use dive_api::bounded::{Request, check_file};

const STRATEGY: &str = "bounded-path";

pub(crate) fn requested(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--strategy=bounded-path")
        || args
            .windows(2)
            .any(|args| args[0] == "--strategy" && args[1] == STRATEGY)
}

pub(crate) fn run(args: &[String], out: &mut impl Write, err: &mut impl Write) -> ExitCode {
    let (request, as_json) = match parse(args) {
        Ok(request) => request,
        Err(message) => {
            let _ = writeln!(err, "error: {message}");
            return ExitCode::from(2);
        }
    };
    match check_file(&request) {
        Ok(report) => {
            if as_json {
                let _ = writeln!(out, "{}", report.to_json());
            } else {
                let _ = write!(out, "{}", report.render_human());
            }
            ExitCode::from(report.exit_status())
        }
        Err(error) => {
            let status = match &error {
                dive_api::ApiError::Unsupported { .. } => 3,
                dive_api::ApiError::Rejected { .. } => 1,
                dive_api::ApiError::Io { .. } => 2,
                _ => 4,
            };
            let _ = writeln!(err, "{error}");
            ExitCode::from(status)
        }
    }
}

fn parse(args: &[String]) -> Result<(Request, bool), String> {
    let mut request = Request::default();
    request.trace.compat_tlc = std::env::var("DIVE_COMPAT").is_ok_and(|value| value == "tlc");
    let mut file = None;
    let mut bound = None;
    let mut as_json = false;
    let mut it = args.iter().map(String::as_str);
    while let Some(argument) = it.next() {
        if !argument.starts_with('-') {
            if file.replace(argument).is_some() {
                return Err("bounded path checking expects one TLA+ file".into());
            }
            continue;
        }
        let (flag, inline) = argument
            .split_once('=')
            .map_or((argument, None), |(f, v)| (f, Some(v)));
        let mut value = || {
            inline
                .or_else(|| it.next())
                .ok_or_else(|| format!("{flag} expects a value"))
        };
        match flag {
            "--strategy" if value()? == STRATEGY => {}
            "--path-bound" if bound.is_none() => {
                bound = Some(
                    value()?
                        .parse()
                        .map_err(|_| "--path-bound expects a nonnegative transition count")?,
                );
            }
            "--config" if request.config.is_none() => request.config = Some(value()?.into()),
            "--library-path" => request.trace.library_paths.push(value()?.into()),
            "--module-path" => request.trace.module_paths.push(value()?.into()),
            "--compat" => {
                request.trace.compat_tlc = match value()? {
                    "dive" => false,
                    "tlc" => true,
                    _ => return Err("--compat expects dive or tlc".into()),
                }
            }
            "--output" => {
                as_json = match value()? {
                    "human" => false,
                    "json" => true,
                    _ => return Err("--output expects human or json".into()),
                }
            }
            "--time-limit" => {
                request.time_limit =
                    super::time_limit(value()?).ok_or("--time-limit expects a duration or none")?;
            }
            "--max-memory" => {
                request.max_memory = super::max_memory(value()?)
                    .ok_or("--max-memory expects a size, auto or none")?;
            }
            _ => {
                return Err(format!(
                    "unexpected option {flag} for bounded path checking"
                ));
            }
        }
    }
    request.file = file
        .ok_or("bounded path checking needs a TLA+ file")?
        .into();
    request.path_bound = bound.ok_or("bounded path checking requires --path-bound")?;
    request.trace.run = Some((0, 0));
    Ok((request, as_json))
}
