//! Parses one file repeatedly, for profiling.
//!
//! ```text
//! cargo run --release -p dive-syntax --example profile -- FILE [ITERATIONS]
//! ```

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: profile FILE [ITERATIONS]");
        return std::process::ExitCode::from(2);
    };
    let n: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);
    let Ok(src) = std::fs::read_to_string(path) else {
        eprintln!("cannot read {path}");
        return std::process::ExitCode::from(2);
    };
    let t0 = std::time::Instant::now();
    let mut ok = true;
    for _ in 0..n {
        ok &= dive_syntax::parse_module(&src).is_ok();
    }
    eprintln!("{n} parses in {:?} (ok={ok})", t0.elapsed());
    std::process::ExitCode::SUCCESS
}
