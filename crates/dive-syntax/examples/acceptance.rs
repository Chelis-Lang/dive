//! Prints dive-syntax's verdict for each file, as JSON lines.
//!
//! ```text
//! cargo run --release -p dive-syntax --example acceptance -- FILE...
//! cargo run --release -p dive-syntax --example acceptance -- --list LIST_FILE
//! ```
//!
//! Each line is `{"path": ..., "accept": bool, "error": "CODE line:col message"}`.
//! The last line on stderr reports total bytes, time and throughput.
//! See `crates/dive-syntax/acceptance/README.md`.

use std::fmt::Write as _;
use std::io::Write as _;
use std::time::{Duration, Instant};

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn main() -> std::process::ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--list") {
        let Some(list) = args.get(1).cloned() else {
            eprintln!("--list needs a file");
            return std::process::ExitCode::from(2);
        };
        match std::fs::read_to_string(&list) {
            Ok(s) => args = s.lines().map(str::to_owned).collect(),
            Err(e) => {
                eprintln!("cannot read {list}: {e}");
                return std::process::ExitCode::from(2);
            }
        }
    }
    let mut stdout = std::io::stdout().lock();
    let mut total = Duration::ZERO;
    let mut bytes = 0usize;
    for path in &args {
        let Ok(raw) = std::fs::read(path) else {
            let _ = writeln!(
                stdout,
                "{{\"path\":{},\"accept\":false,\"error\":\"unreadable\"}}",
                json_string(path)
            );
            continue;
        };
        let src = String::from_utf8_lossy(&raw);
        let t0 = Instant::now();
        let parse = dive_syntax::parse_module(&src);
        let took = t0.elapsed();
        total += took;
        bytes += src.len();
        let error = parse.diagnostics().iter().find(|d| d.is_error()).map(|d| {
            let (line, col) = dive_syntax::diagnostics::line_col(&src, d.range.start());
            format!("{} {line}:{col} {}", d.code, d.message)
        });
        let _ = writeln!(
            stdout,
            "{{\"path\":{},\"accept\":{},\"micros\":{},\"error\":{}}}",
            json_string(path),
            parse.is_ok(),
            took.as_micros(),
            json_string(error.as_deref().unwrap_or(""))
        );
    }
    let secs = total.as_secs_f64();
    #[allow(clippy::cast_precision_loss, reason = "throughput is approximate")]
    let mib = bytes as f64 / (1024.0 * 1024.0);
    eprintln!(
        "parsed {} files, {bytes} bytes in {:.3} s ({:.1} MiB/s)",
        args.len(),
        secs,
        if secs > 0.0 { mib / secs } else { 0.0 }
    );
    std::process::ExitCode::SUCCESS
}
