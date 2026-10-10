//! The `dive` binary.
//!
//! `dive parse` (including reading a TLC model configuration with
//! `--config`), `dive analyze`, `dive check` and `dive validate-trace` are
//! implemented. Every other core command is recognised and refuses with an
//! explicit `unsupported` result until its backend exists (soundness contract
//! rule 3).
#![forbid(unsafe_code)]

use std::io::Write as _;
use std::process::ExitCode;

/// Core commands named by the charter.
const COMMANDS: &[&str] = &[
    "check",
    "dfid",
    "simulate",
    "parse",
    "analyze",
    "translate",
    "validate-trace",
    "lsp",
];

/// Exit code when the input is rejected (`parse`: syntax or cfg errors;
/// `analyze`: any error SANY would report).
const EXIT_REJECTED: u8 = 1;
/// Exit code for a malformed command line or unreadable input.
const EXIT_USAGE: u8 = 2;
/// Exit code for a request that `dive` recognises but does not implement.
const EXIT_UNSUPPORTED: u8 = 3;

fn usage() -> String {
    format!(
        "dive {}\nDeterministic Invariant Verification Engine\n\nUSAGE:\n    dive <COMMAND> [ARGS]\n\nCOMMANDS:\n    {}\n\n    dive parse [FILE.tla] [--config FILE.cfg] [--output human|json] [--cst]\n    dive analyze <FILE.tla> [--output human|json] [--library-path DIR]... [--module-path DIR]...\n    dive check <FILE.tla> [--config FILE.cfg] [--compat=tlc] [--store fp|exact] [--seed N] [--fp N]\n               [--workers N|auto] [--sequential] [-deadlock] [--output human|json]
               [--lncheck final --lasso short] [--library-path DIR]... [--module-path DIR]...\n    dive dfid <FILE.tla> --max-level N [--compat=tlc] [--seed N] [--worker-seeds S,...] [--output human|json]\n    dive validate-trace <FILE.tla> --trace TRACE.json [--config FILE.cfg] [--claim ENDING]\n               [--compat=tlc] [--seed N] [--fp N] [-deadlock] [--output human|json] [--library-path DIR]... [--module-path DIR]...\n\n    dive check exit codes (default profile): 0 no error, 1 rejected input, 2 usage, 3 unsupported or\n    resource limit, 4 internal error, 10 property violated, 11 deadlock, 12 assumption, 13 evaluation\n    error. Under --compat=tlc (or DIVE_COMPAT=tlc) a verdict exits with TLC's status.\n",
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

/// `dive analyze <file> [--output human|json] [--library-path DIR]... [--module-path DIR]...`.
///
/// SANY-equivalent semantic analysis of the specification rooted at the file.
/// Modules are found in the file's directory, the library paths, the standard
/// modules, then the module paths.
fn analyze_command(
    args: &[String],
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
) -> ExitCode {
    let mut file: Option<&str> = None;
    let mut json = false;
    let mut opts = dive_api::AnalyzeOptions::default();
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
            "--library-path" | "--module-path" => {
                let Some(dir) = it.next() else {
                    let _ = writeln!(err, "error: {a} expects a directory");
                    return ExitCode::from(EXIT_USAGE);
                };
                if a == "--library-path" {
                    opts.library_paths.push(dir.into());
                } else {
                    opts.after_standard.push(dir.into());
                }
            }
            s if s.starts_with('-') => {
                let _ = writeln!(err, "error: unknown option `{s}` for `dive analyze`");
                return ExitCode::from(EXIT_USAGE);
            }
            s => {
                if file.replace(s).is_some() {
                    let _ = writeln!(err, "error: `dive analyze` takes one file");
                    return ExitCode::from(EXIT_USAGE);
                }
            }
        }
    }
    let Some(file) = file else {
        let _ = writeln!(err, "error: `dive analyze` needs a file\n\n{}", usage());
        return ExitCode::from(EXIT_USAGE);
    };
    let report = match dive_api::analyze_file(std::path::Path::new(file), &opts) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(err, "error: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    if json {
        let _ = writeln!(out, "{}", report.to_json());
    } else {
        let _ = write!(err, "{}", report.render_human());
        if report.accepted() {
            let _ = writeln!(
                out,
                "ok: {} analysed ({})",
                file,
                report.modules().join(", ")
            );
        }
    }
    if report.accepted() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_REJECTED)
    }
}

/// Parses a numeric option value.
fn number<T: std::str::FromStr>(
    flag: &str,
    v: Option<&String>,
    err: &mut impl std::io::Write,
) -> Option<T> {
    let parsed = v.and_then(|v| v.parse().ok());
    if parsed.is_none() {
        let _ = writeln!(err, "error: {flag} expects a number");
    }
    parsed
}

/// `dive check <file> [options]`: safety model checking (see `usage`).
#[allow(clippy::too_many_lines, reason = "one flag per arm")]
fn check_command(
    args: &[String],
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
) -> ExitCode {
    let mut req = dive_api::CheckRequest {
        compat_tlc: std::env::var("DIVE_COMPAT").is_ok_and(|v| v == "tlc"),
        // Test-only (hidden): chaos scheduling for determinism testing.
        chaos: std::env::var("DIVE_CHAOS")
            .ok()
            .and_then(|v| v.parse().ok()),
        ..dive_api::CheckRequest::default()
    };
    let mut final_check = false;
    let mut short_lasso = false;
    let mut file: Option<&str> = None;
    let mut json = false;
    let usage_error = |err: &mut dyn std::io::Write, m: &str| {
        let _ = writeln!(err, "error: {m}");
        ExitCode::from(EXIT_USAGE)
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) if f.starts_with('-') => (f, Some(v.to_owned())),
            _ => (a.as_str(), None),
        };
        let mut value = || inline.clone().or_else(|| it.next().cloned());
        match flag {
            "--output" => match value().as_deref() {
                Some("json") => json = true,
                Some("human") => json = false,
                _ => return usage_error(err, "--output expects `human` or `json`"),
            },
            "--compat" => match value().as_deref() {
                Some("tlc") => req.compat_tlc = true,
                Some("dive" | "none") => req.compat_tlc = false,
                _ => return usage_error(err, "--compat expects `tlc`"),
            },
            "--config" | "-config" => match value() {
                Some(c) => req.config = Some(c.into()),
                None => return usage_error(err, "--config expects a .cfg file"),
            },
            "--lncheck" => match value().as_deref() {
                Some("final") => final_check = true,
                _ => {
                    return usage_error(
                        err,
                        "--lncheck currently supports explicit `final` with --lasso short",
                    );
                }
            },
            "--lasso" => match value().as_deref() {
                Some("short") => short_lasso = true,
                _ => {
                    return usage_error(
                        err,
                        "--lasso currently supports explicit `short` with --lncheck final; TLC-exact selection is pending",
                    );
                }
            },
            "--store" => match value().as_deref() {
                Some("exact") => req.store = dive_api::Store::Exact,
                Some("fp" | "fingerprint") => req.store = dive_api::Store::Fingerprint,
                _ => return usage_error(err, "--store expects `fp` or `exact`"),
            },
            "--seed" | "-seed" => {
                let v = value();
                match v.as_deref().and_then(dive_api::parse_seed) {
                    Some(s) => req.seed = Some(s),
                    None => return usage_error(err, "--seed expects a 64-bit integer"),
                }
            }
            "--fp" | "-fp" => {
                let v = value();
                match number::<usize>(flag, v.as_ref(), err) {
                    Some(n) if n < 131 => req.fp = Some(n),
                    _ => return usage_error(err, "--fp expects a polynomial index from 0 to 130"),
                }
            }
            "--workers" | "-workers" => {
                let v = value();
                if v.as_deref() == Some("auto") {
                    req.workers = None;
                } else {
                    match number::<usize>(flag, v.as_ref(), err) {
                        Some(n) if n > 0 => req.workers = Some(n),
                        _ => {
                            return usage_error(
                                err,
                                "--workers expects a positive number or `auto`",
                            );
                        }
                    }
                }
            }
            "-deadlock" => req.no_deadlock = true,
            "--sequential" => req.sequential = true,
            // Hidden, test-only: randomized scheduling that must not change
            // any result (ADR 0015 §2, chaos mode).
            "--chaos" => match value().as_deref().map(str::parse::<u64>) {
                Some(Ok(seed)) => req.chaos = Some(seed),
                _ => return usage_error(err, "--chaos expects a 64-bit unsigned seed"),
            },
            "--library-path" | "--module-path" => match value() {
                Some(dir) if flag == "--library-path" => req.library_paths.push(dir.into()),
                Some(dir) => req.module_paths.push(dir.into()),
                None => return usage_error(err, &format!("{flag} expects a directory")),
            },
            s if s.starts_with('-') => {
                return usage_error(err, &format!("unknown option `{s}` for `dive check`"));
            }
            s => {
                if file.replace(s).is_some() {
                    return usage_error(err, "`dive check` takes one file");
                }
            }
        }
    }
    let Some(file) = file else {
        let _ = writeln!(err, "error: `dive check` needs a file\n\n{}", usage());
        return ExitCode::from(EXIT_USAGE);
    };
    if final_check != short_lasso {
        return usage_error(
            err,
            "bounded temporal checking needs both --lncheck final and --lasso short",
        );
    }
    req.temporal = if final_check && short_lasso {
        dive_api::TemporalMode::FinalShort
    } else {
        dive_api::TemporalMode::Ordinary
    };
    req.file = file.into();
    let report = match dive_api::check_file(&req) {
        Ok(r) => r,
        Err(e @ dive_api::ApiError::Unsupported { .. }) => {
            let _ = writeln!(err, "{e}; no verdict produced");
            return ExitCode::from(EXIT_UNSUPPORTED);
        }
        Err(e) => {
            let _ = writeln!(err, "error: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    if json {
        let _ = writeln!(out, "{}", report.to_json());
    } else {
        let _ = write!(out, "{}", report.render_human());
    }
    ExitCode::from(report.exit_code())
}

/// `dive validate-trace <file> --trace <trace.json> [--config cfg] [--claim ENDING]
/// [--compat=tlc] [--seed N] [--fp N] [--deadlock] [--output human|json] [--library-path DIR]... [--module-path DIR]...`.
///
/// Validates a counterexample or behavior against the specification with
/// the independent trace validator (ADR 0016). The trace is in dive's format
/// or TLC's `-dumpTrace json` (which needs `--claim`, e.g.
/// `invariant:Inv`). `--seed` (and `--fp`) give the checking run's seed and
/// polynomial, so that random draws are replayed. Exit status: 0 valid, 1 invalid (or the specification
/// is rejected), 2 usage or unreadable input, 3 unsupported.
#[allow(
    clippy::too_many_lines,
    reason = "one command: its options, then loading, validating and reporting"
)]
fn validate_trace_command(
    args: &[String],
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
) -> ExitCode {
    let mut file: Option<&str> = None;
    let mut cfg: Option<&str> = None;
    let mut trace: Option<&str> = None;
    let mut claim: Option<&str> = None;
    let mut json = false;
    let mut seed: Option<i64> = None;
    let mut fp: Option<usize> = None;
    let mut opts = dive_api::trace::TraceOptions {
        compat_tlc: std::env::var("DIVE_COMPAT").is_ok_and(|v| v == "tlc"),
        ..Default::default()
    };
    let mut it = args.iter();
    let usage_error = |err: &mut dyn std::io::Write, msg: &str| {
        let _ = writeln!(err, "error: {msg}");
        ExitCode::from(EXIT_USAGE)
    };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--output" => match it.next().map(String::as_str) {
                Some("json") => json = true,
                Some("human") => json = false,
                other => {
                    return usage_error(
                        err,
                        &format!(
                            "--output expects `human` or `json`, got {}",
                            other.unwrap_or("nothing")
                        ),
                    );
                }
            },
            "--config" | "--trace" | "--claim" => {
                let Some(v) = it.next() else {
                    return usage_error(err, &format!("{a} expects a value"));
                };
                let slot = match a.as_str() {
                    "--config" => &mut cfg,
                    "--trace" => &mut trace,
                    _ => &mut claim,
                };
                if slot.replace(v).is_some() {
                    return usage_error(err, &format!("{a} given twice"));
                }
            }
            "--compat=tlc" => opts.compat_tlc = true,
            "--compat=dive" => opts.compat_tlc = false,
            "--compat" => match it.next().map(String::as_str) {
                Some("tlc") => opts.compat_tlc = true,
                Some("dive") => opts.compat_tlc = false,
                _ => return usage_error(err, "--compat expects `tlc` or `dive`"),
            },
            "--deadlock" | "-deadlock" => opts.no_deadlock = true,
            "--seed" | "-seed" => match it.next().and_then(|v| dive_api::parse_seed(v)) {
                Some(s) => seed = Some(s),
                None => return usage_error(err, "--seed expects a 64-bit integer"),
            },
            "--fp" | "-fp" => match it.next().and_then(|v| v.parse::<usize>().ok()) {
                Some(n) if n < 131 => fp = Some(n),
                _ => return usage_error(err, "--fp expects a polynomial index from 0 to 130"),
            },
            "--library-path" | "--module-path" => {
                let Some(dir) = it.next() else {
                    return usage_error(err, &format!("{a} expects a directory"));
                };
                if a == "--library-path" {
                    opts.library_paths.push(dir.into());
                } else {
                    opts.module_paths.push(dir.into());
                }
            }
            s if s.starts_with('-') => {
                return usage_error(
                    err,
                    &format!("unknown option `{s}` for `dive validate-trace`"),
                );
            }
            s => {
                if file.replace(s).is_some() {
                    return usage_error(err, "`dive validate-trace` takes one specification");
                }
            }
        }
    }
    let (Some(file), Some(trace)) = (file, trace) else {
        return usage_error(
            err,
            &format!(
                "`dive validate-trace` needs a specification and --trace\n\n{}",
                usage()
            ),
        );
    };
    // The checking run's seed and polynomial, as `dive check` chose them:
    // without `--fp`, TLC's (under --compat=tlc) is derived from the seed.
    opts.run = match (seed, fp) {
        (Some(s), fp) => Some((
            s,
            fp.unwrap_or_else(|| {
                if opts.compat_tlc {
                    usize::try_from(s.unsigned_abs() % 131).unwrap_or(0)
                } else {
                    0
                }
            }),
        )),
        (None, Some(_)) => return usage_error(err, "--fp needs --seed"),
        (None, None) => None,
    };
    let spec = std::path::Path::new(file);
    // TLC's default: the configuration next to the specification.
    let cfg = cfg.map_or_else(|| spec.with_extension("cfg"), std::path::PathBuf::from);
    let text = match std::fs::read_to_string(trace) {
        Ok(t) => t,
        Err(e) => return usage_error(err, &format!("cannot read {trace}: {e}")),
    };
    let mut doc = match dive_api::trace::TraceDoc::parse(&text) {
        Ok(d) => d,
        Err(e) => return usage_error(err, &format!("{trace}: {e}")),
    };
    if let Some(c) = claim {
        let ending = match dive_api::trace::Ending::parse(c) {
            Ok(e) => e,
            Err(e) => return usage_error(err, &format!("--claim: {e}")),
        };
        if doc.ending.as_ref().is_some_and(|d| *d != ending) {
            return usage_error(err, "--claim contradicts the trace's own `ending`");
        }
        doc = doc.with_ending(ending);
    }
    let model = match dive_api::trace::SpecModel::load(spec, &cfg, &opts) {
        Ok(m) => m,
        Err(e @ dive_api::ApiError::Unsupported { .. }) => {
            let _ = writeln!(err, "{e}; no result produced");
            return ExitCode::from(EXIT_UNSUPPORTED);
        }
        Err(e @ dive_api::ApiError::Rejected { .. }) => {
            let _ = writeln!(err, "error: {e}");
            return ExitCode::from(EXIT_REJECTED);
        }
        Err(e) => return usage_error(err, &e.to_string()),
    };
    let report = model.validate(&doc);
    if json {
        let _ = writeln!(out, "{}", dive_api::trace::report_json(&report));
    } else {
        let _ = write!(out, "{}", dive_api::trace::render_report(&report));
    }
    match report.validation {
        dive_api::trace::Validation::Valid => ExitCode::SUCCESS,
        dive_api::trace::Validation::Invalid(_) => ExitCode::from(EXIT_REJECTED),
        dive_api::trace::Validation::Unsupported(_) => ExitCode::from(EXIT_UNSUPPORTED),
    }
}

/// Runs the native bounded iterative mode; unknown mode flags refuse by name.
#[allow(
    clippy::too_many_lines,
    reason = "explicit CLI options and typed validation"
)]
fn dfid_command(
    args: &[String],
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
) -> ExitCode {
    let mut req = dive_api::DfidRequest {
        compat_tlc: std::env::var("DIVE_COMPAT").is_ok_and(|v| v == "tlc"),
        ..dive_api::DfidRequest::default()
    };
    let mut json = false;
    let mut has_depth = false;
    let mut file = None;
    let mut it = args.iter();
    let usage_error = |err: &mut dyn std::io::Write, text: &str| {
        let _ = writeln!(err, "error: {text}");
        ExitCode::from(EXIT_USAGE)
    };
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with('-') => (f, Some(v.to_owned())),
            _ => (arg.as_str(), None),
        };
        let mut value = || inline.clone().or_else(|| it.next().cloned());
        match flag {
            "--output" => match value().as_deref() {
                Some("json") => json = true,
                Some("human") => json = false,
                _ => return usage_error(err, "--output expects human or json"),
            },
            "--compat" => match value().as_deref() {
                Some("tlc") => req.compat_tlc = true,
                Some("dive" | "none") => req.compat_tlc = false,
                _ => return usage_error(err, "--compat expects tlc or dive"),
            },
            "--config" | "-config" => match value() {
                Some(v) => req.config = Some(v.into()),
                None => return usage_error(err, "--config expects a cfg file"),
            },
            "--max-level" | "--dfid" | "-dfid" => match value().and_then(|v| v.parse().ok()) {
                Some(n) => {
                    req.max_level = n;
                    has_depth = true;
                }
                None => return usage_error(err, "--max-level expects an unsigned integer"),
            },
            "--seed" | "-seed" => match value().as_deref().and_then(dive_api::parse_seed) {
                Some(seed) => req.seed = Some(seed),
                None => return usage_error(err, "--seed expects a 64-bit seed"),
            },
            "--worker-seeds" => match value().map(|v| {
                v.split(',')
                    .map(dive_api::parse_seed)
                    .collect::<Option<Vec<_>>>()
            }) {
                Some(Some(seeds)) if !seeds.is_empty() => req.worker_seeds = Some(seeds),
                _ => {
                    return usage_error(err, "--worker-seeds expects comma separated 64-bit seeds");
                }
            },
            "--workers" | "-workers" => match value().and_then(|v| v.parse().ok()) {
                Some(n) => req.workers = Some(n),
                None => return usage_error(err, "--workers expects an unsigned integer"),
            },
            "--fp" | "-fp" => match value().and_then(|v| v.parse::<usize>().ok()) {
                Some(n) if n < 131 => req.fp = n,
                _ => return usage_error(err, "--fp expects 0..130"),
            },
            "--store" => match value().as_deref() {
                Some("exact") => req.exact = true,
                Some("fp" | "fingerprint") => req.exact = false,
                _ => return usage_error(err, "--store expects fp or exact"),
            },
            "--library-path" => match value() {
                Some(v) => req.library_paths.push(v.into()),
                None => return usage_error(err, "--library-path expects a directory"),
            },
            "--module-path" => match value() {
                Some(v) => req.module_paths.push(v.into()),
                None => return usage_error(err, "--module-path expects a directory"),
            },
            "-deadlock" | "--no-deadlock" => req.no_deadlock = true,
            s if s.starts_with('-') => {
                let _ = writeln!(err, "unsupported DFID option: {s}; no checking performed");
                return ExitCode::from(EXIT_UNSUPPORTED);
            }
            s => {
                if file.replace(s).is_some() {
                    return usage_error(err, "dfid takes one specification file");
                }
            }
        }
    }
    let Some(file) = file else {
        return usage_error(err, "dfid requires a specification and --max-level N");
    };
    if !has_depth {
        return usage_error(err, "dfid requires explicit --max-level N");
    }
    req.file = file.into();
    let report = match dive_api::dfid_file(&req) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(err, "{e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    if json {
        let _ = writeln!(out, "{}", report.to_json());
    } else {
        let _ = write!(out, "{}", report.render_human());
    }
    ExitCode::from(report.exit_status())
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
        Some("analyze") => analyze_command(&args[1..], &mut stdout, &mut stderr),
        Some("dfid") => dfid_command(&args[1..], &mut stdout, &mut stderr),
        Some("check") => check_command(&args[1..], &mut stdout, &mut stderr),
        Some("validate-trace") => validate_trace_command(&args[1..], &mut stdout, &mut stderr),
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
