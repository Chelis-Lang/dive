//! The `dive` binary.
//!
//! `dive parse` is implemented, including reading a TLC model configuration
//! with `--config`. Every other core command is recognised and refuses with
//! an explicit `unsupported` result until its backend exists (soundness
//! contract rule 3).
#![forbid(unsafe_code)]

use std::io::Write as _;
use std::process::ExitCode;

/// Core commands named by the charter.
const COMMANDS: &[&str] = &[
    "check",
    "simulate",
    "parse",
    "translate",
    "validate-trace",
    "lsp",
];

/// Exit code when the input is rejected (for `parse`: syntax or cfg errors).
const EXIT_REJECTED: u8 = 1;
/// Exit code for a malformed command line or unreadable input.
const EXIT_USAGE: u8 = 2;
/// Exit code for a request that `dive` recognises but does not implement.
const EXIT_UNSUPPORTED: u8 = 3;

fn usage() -> String {
    format!(
        "dive {}\nDeterministic Invariant Verification Engine\n\nUSAGE:\n    dive <COMMAND> [ARGS]\n\nCOMMANDS:\n    {}\n\n    dive parse [FILE.tla] [--config FILE.cfg] [--output human|json] [--cst]\n",
        env!("CARGO_PKG_VERSION"),
        COMMANDS.join("\n    ")
    )
}

/// Parsed `dive parse` arguments.
#[derive(Default)]
struct ParseArgs<'a> {
    file: Option<&'a str>,
    config: Option<&'a str>,
    json: bool,
    cst: bool,
}

fn parse_args<'a>(args: &'a [String], err: &mut impl std::io::Write) -> Option<ParseArgs<'a>> {
    let mut p = ParseArgs::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--output" => match it.next().map(String::as_str) {
                Some("json") => p.json = true,
                Some("human") => p.json = false,
                other => {
                    let _ = writeln!(
                        err,
                        "error: --output expects `human` or `json`, got {}",
                        other.unwrap_or("nothing")
                    );
                    return None;
                }
            },
            "--config" => {
                let Some(c) = it.next() else {
                    let _ = writeln!(err, "error: --config expects a .cfg file");
                    return None;
                };
                if p.config.replace(c).is_some() {
                    let _ = writeln!(err, "error: --config given twice");
                    return None;
                }
            }
            "--cst" => p.cst = true,
            s if s.starts_with('-') => {
                let _ = writeln!(err, "error: unknown option `{s}` for `dive parse`");
                return None;
            }
            s => {
                if p.file.replace(s).is_some() {
                    let _ = writeln!(err, "error: `dive parse` takes one file");
                    return None;
                }
            }
        }
    }
    if p.file.is_none() && p.config.is_none() {
        let _ = writeln!(
            err,
            "error: `dive parse` needs a file or --config\n\n{}",
            usage()
        );
        return None;
    }
    Some(p)
}

/// `dive parse [file] [--config cfg] [--output human|json] [--cst]`.
fn parse_command(
    args: &[String],
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
) -> ExitCode {
    let Some(p) = parse_args(args, err) else {
        return ExitCode::from(EXIT_USAGE);
    };
    let report = match p
        .file
        .map(|f| dive_api::parse_file(std::path::Path::new(f)))
    {
        None => None,
        Some(Ok(r)) => Some(r),
        Some(Err(e)) => {
            let _ = writeln!(err, "error: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let config = match p
        .config
        .map(|c| dive_api::parse_config_file(std::path::Path::new(c)))
    {
        None => None,
        Some(Ok(r)) => Some(r),
        Some(Err(e @ dive_api::ApiError::Unsupported { .. })) => {
            let _ = writeln!(err, "{e}; no result produced");
            return ExitCode::from(EXIT_UNSUPPORTED);
        }
        Some(Err(e)) => {
            let _ = writeln!(err, "error: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let accepted = report.as_ref().is_none_or(dive_api::ParseReport::accepted)
        && config.as_ref().is_none_or(dive_api::ConfigReport::accepted);
    if p.json {
        let mut v = match &report {
            Some(r) => r.to_json(p.cst),
            None => serde_json::json!({
                "schema_version": dive_api::PARSE_SCHEMA_VERSION,
                "command": "parse",
                "file": null,
                "module": null,
                "modules": [],
                "diagnostics": [],
            }),
        };
        if let Some(obj) = v.as_object_mut() {
            obj.insert("accepted".to_owned(), serde_json::Value::Bool(accepted));
            if let Some(c) = &config {
                obj.insert("config".to_owned(), c.to_json());
            }
        }
        let _ = writeln!(out, "{v}");
    } else {
        if let Some(r) = &report {
            let _ = write!(err, "{}", r.render_human());
            if p.cst {
                let _ = write!(out, "{:#?}", r.parse().syntax());
            }
            if r.accepted() {
                let _ = writeln!(
                    out,
                    "ok: {} parsed ({})",
                    p.file.unwrap_or_default(),
                    r.modules().join(", ")
                );
            }
        }
        if let Some(c) = &config {
            let _ = write!(err, "{}", c.render_human());
            if let Some(m) = c.config() {
                let _ = writeln!(
                    out,
                    "ok: {} read ({} constants, {} invariants, {} properties)",
                    p.config.unwrap_or_default(),
                    m.constants.len(),
                    m.invariants.len(),
                    m.properties.len()
                );
            }
        }
    }
    if accepted {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_REJECTED)
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    // Write failures (e.g. a closed pipe) are not actionable here.
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help" | "help") => {
            let _ = write!(stdout, "{}", usage());
            ExitCode::SUCCESS
        }
        Some("-V" | "--version") => {
            let _ = writeln!(stdout, "dive {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("parse") => parse_command(&args[1..], &mut stdout, &mut stderr),
        Some(cmd) if COMMANDS.contains(&cmd) => {
            let _ = writeln!(
                stderr,
                "unsupported: `dive {cmd}` is not implemented yet; no verdict produced"
            );
            ExitCode::from(EXIT_UNSUPPORTED)
        }
        Some(other) => {
            let _ = write!(stderr, "error: unknown command `{other}`\n\n{}", usage());
            ExitCode::from(EXIT_USAGE)
        }
    }
}
