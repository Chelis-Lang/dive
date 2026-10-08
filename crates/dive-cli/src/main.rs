//! The `dive` binary.
//!
//! Every core command is recognised, and each one refuses with an explicit
//! `unsupported` result until its backend exists (soundness contract rule 3).
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

/// Exit code for a request that `dive` recognises but does not implement.
const EXIT_UNSUPPORTED: u8 = 3;
/// Exit code for a malformed command line.
const EXIT_USAGE: u8 = 2;

fn usage() -> String {
    format!(
        "dive {}\nDeterministic Invariant Verification Engine\n\nUSAGE:\n    dive <COMMAND> [ARGS]\n\nCOMMANDS:\n    {}\n",
        env!("CARGO_PKG_VERSION"),
        COMMANDS.join("\n    ")
    )
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
