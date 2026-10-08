//! The `dive` binary.
//!
//! `dive parse` is implemented. Every other core command is recognised and
//! refuses with an explicit `unsupported` result until its backend exists
//! (soundness contract rule 3).
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

/// Exit code when the input is rejected (for `parse`: syntax errors).
const EXIT_REJECTED: u8 = 1;
/// Exit code for a malformed command line or unreadable input.
const EXIT_USAGE: u8 = 2;
/// Exit code for a request that `dive` recognises but does not implement.
const EXIT_UNSUPPORTED: u8 = 3;

fn usage() -> String {
    format!(
        "dive {}\nDeterministic Invariant Verification Engine\n\nUSAGE:\n    dive <COMMAND> [ARGS]\n\nCOMMANDS:\n    {}\n\n    dive parse <FILE.tla> [--output human|json] [--cst]\n",
        env!("CARGO_PKG_VERSION"),
        COMMANDS.join("\n    ")
    )
}

/// `dive parse <file> [--output human|json] [--cst]`.
fn parse_command(
    args: &[String],
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
) -> ExitCode {
    let mut file: Option<&str> = None;
    let mut json = false;
    let mut cst = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--output" => match it.next().map(String::as_str) {
                Some("json") => json = true,
                Some("human") => json = false,
                other => {
                    let _ = writeln!(
                        err,
                        "error: --output expects `human` or `json`, got {}",
                        other.unwrap_or("nothing")
                    );
                    return ExitCode::from(EXIT_USAGE);
                }
            },
            "--cst" => cst = true,
            s if s.starts_with('-') => {
                let _ = writeln!(err, "error: unknown option `{s}` for `dive parse`");
                return ExitCode::from(EXIT_USAGE);
            }
            s => {
                if file.replace(s).is_some() {
                    let _ = writeln!(err, "error: `dive parse` takes one file");
                    return ExitCode::from(EXIT_USAGE);
                }
            }
        }
    }
    let Some(file) = file else {
        let _ = writeln!(err, "error: `dive parse` needs a file\n\n{}", usage());
        return ExitCode::from(EXIT_USAGE);
    };
    let report = match dive_api::parse_file(std::path::Path::new(file)) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(err, "error: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    if json {
        let _ = writeln!(out, "{}", report.to_json(cst));
    } else {
        let _ = write!(err, "{}", report.render_human());
        if cst {
            let _ = write!(out, "{:#?}", report.parse().syntax());
        }
        if report.accepted() {
            let _ = writeln!(out, "ok: {} parsed ({})", file, report.modules().join(", "));
        }
    }
    if report.accepted() {
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
