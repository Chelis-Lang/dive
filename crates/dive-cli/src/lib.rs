//! Shared command dispatch for the `dive` and `tlc` binaries.
//!
//! Native commands retain their own defaults. The checking-only TLC adapter
//! selects TLC evaluation and accepts common checking spellings through
//! `dive tlc` or an executable named `tlc`. Unimplemented commands and features
//! return named refusals.
#![forbid(unsafe_code)]

mod explain;
mod tlc;
mod translate;

use std::io::Write as _;
use std::process::ExitCode;

/// Core commands named by the charter.
const COMMANDS: &[&str] = &[
    "check",
    "tlc",
    "dfid",
    "simulate",
    "parse",
    "analyze",
    "translate",
    "validate-trace",
    "explain",
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
        "dive {}\nDeterministic Invariant Verification Engine\n\nUSAGE:\n    dive <COMMAND> [ARGS]\n\nCOMMANDS:\n    {}\n\n    dive parse [FILE.tla] [--config FILE.cfg] [--output human|json] [--cst]\n    dive translate <FILE.tla> [--compat=tlc] [--rewrite --checksums tla|full] [--replace-modified] [--output human|json] [--library-path DIR]... [--module-path DIR]...\n    dive analyze <FILE.tla> [--output human|json] [--library-path DIR]... [--module-path DIR]...\n    dive explain <CODE> [--output human|json]\n    dive tlc [OPTIONS] SPEC[.tla] (TLC checking arguments; see dive tlc -help)\n    dive check <FILE.tla> [--config FILE.cfg] [--compat=tlc] [--exit-codes native|tlc] [--store fp|exact] [--seed N] [--fp N]\n               [--workers N|auto] [--sequential] [-deadlock] [--action-composition off|tlc|tla]\n               [--symmetry-liveness unreduced|refuse|tlc]\n               [--frontier-memory-bytes N] [--frontier-directory DIR] [--output human|json]\n               [--time-limit DURATION|none] [--max-memory BYTES|auto|none]
               [--max-states N|none] [--max-depth N|none]
               [--predecessor-memory-bytes N] [--predecessor-directory DIR]\n               [--seen-disk-entries N] [--seen-directory DIR] [--seen-key-bytes N]\n               [--seen-probe-steps N] [--seen-summary-pairs N]
               [--checkpoint FILE --checkpoint-after-levels N | --recover FILE]
               [--checkpoint-counters tlc|retain]
               [--lncheck final --lasso short] [--engine reference|core|auto] [--core-sample N] [--library-path DIR]... [--module-path DIR]...\n    dive simulate <FILE.tla> --traces N [--depth N] [--seed N] [--aril N] [--stats none|basic] [--compat=tlc] [--output human|json]\n    dive dfid <FILE.tla> --max-level N [--compat=tlc] [--seed N] [--worker-seeds S,...] [--output human|json]\n    dive validate-trace <FILE.tla> --trace TRACE.json [--config FILE.cfg] [--claim ENDING]\n               [--compat=tlc] [--symmetry-liveness unreduced|refuse|tlc] [--seed N] [--fp N] [-deadlock] [--output human|json] [--library-path DIR]... [--module-path DIR]...\n\n    Eligible ordinary temporal checks use deterministic completed-level checks in the dive
    profile and final checking in the TLC profile; explicit final/short keeps final checking.
    dive check uses TLC process statuses by default, independently of semantic profile.\n    --exit-codes native retains: 0 no error, 1 rejected input, 10 property violated, 11 deadlock,\n    12 assumption, 13 evaluation error. Both conventions keep 2 usage, 3 unsupported/resource\n    limit, 4 internal error and 5 stopped by a run budget (no verdict).\n    --time-limit defaults to 30m in the dive profile and none under --compat=tlc; --max-memory\n    defaults to half of physical memory (auto) in both. --compat=tlc (or DIVE_COMPAT=tlc) also selects TLC semantics.\n    --engine core (or DIVE_ENGINE=core) runs the experimental new core: default profile, safety\n    only; anything else it reports as unsupported. --engine auto runs the core where it can and\n    otherwise the whole check on the reference engine, naming the engine that ran and why. The\n    reference engine is the default. --core-sample N (with the core) also compares the successors\n    of N sampled states with the reference evaluator's, in the JSON report (core.sample); it is\n    diagnostic and changes no result.\n",
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

/// `--time-limit`: `none`, or a positive duration in seconds by default, or
/// with a unit (`ms`, `s`, `m`, `h`).
fn time_limit(text: &str) -> Option<dive_api::BudgetSetting<std::time::Duration>> {
    if text == "none" {
        return Some(dive_api::BudgetSetting::Unlimited);
    }
    let split = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let (digits, unit) = text.split_at(split);
    let n: u64 = digits.parse().ok().filter(|n| *n > 0)?;
    let duration = match unit {
        "ms" => std::time::Duration::from_millis(n),
        "" | "s" => std::time::Duration::from_secs(n),
        "m" => std::time::Duration::from_secs(n.checked_mul(60)?),
        "h" => std::time::Duration::from_secs(n.checked_mul(3600)?),
        _ => return None,
    };
    Some(dive_api::BudgetSetting::Explicit(duration))
}

/// `--max-memory`: `auto`, `none`, or a positive byte count, optionally with a
/// binary unit as for the JVM's `-Xmx` (`k`, `m`, `g`, `t`, either case).
fn max_memory(text: &str) -> Option<dive_api::BudgetSetting<u64>> {
    match text {
        "auto" => return Some(dive_api::BudgetSetting::Default),
        "none" => return Some(dive_api::BudgetSetting::Unlimited),
        _ => {}
    }
    let split = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let (digits, unit) = text.split_at(split);
    let n: u64 = digits.parse().ok().filter(|n| *n > 0)?;
    let shift = match unit.to_ascii_lowercase().as_str() {
        "" => 0,
        "k" => 10,
        "m" => 20,
        "g" => 30,
        "t" => 40,
        _ => return None,
    };
    n.checked_mul(1 << shift)
        .map(dive_api::BudgetSetting::Explicit)
}

/// Search limits admit zero and use `none` for TLC-equivalent unlimited search.
fn search_limit(text: &str) -> Result<Option<u64>, std::num::ParseIntError> {
    if text == "none" {
        Ok(None)
    } else {
        text.parse().map(Some)
    }
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
    let mut seen_policy = dive_api::SeenOptions::default();
    let mut seen_entries = false;
    let mut seen_extra = false;
    let mut checkpoint_path: Option<std::path::PathBuf> = None;
    let mut recover_path: Option<std::path::PathBuf> = None;
    let mut checkpoint_after: Option<u64> = None;
    let mut checkpoint_counters = dive_api::checkpoint::Counters::Tlc;
    let mut counters_given = false;
    let mut final_check = false;
    let mut short_lasso = false;
    let mut file: Option<&str> = None;
    let mut json = false;
    let mut exit_convention = dive_api::ExitConvention::Tlc;
    let mut engine = match std::env::var("DIVE_ENGINE").ok().as_deref() {
        None | Some("") => dive_api::Engine::Reference,
        Some(name) => {
            let Some(e) = dive_api::Engine::parse(name) else {
                let _ = writeln!(
                    err,
                    "error: DIVE_ENGINE expects `reference`, `core` or `auto`"
                );
                return ExitCode::from(EXIT_USAGE);
            };
            e
        }
    };
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
            "--exit-codes" => match value().as_deref() {
                Some("native") => exit_convention = dive_api::ExitConvention::Native,
                Some("tlc") => exit_convention = dive_api::ExitConvention::Tlc,
                _ => return usage_error(err, "--exit-codes expects `native` or `tlc`"),
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
                Some("tlc") => {
                    return usage_error(
                        err,
                        "--lasso tlc is not supported: dive does not reproduce TLC's lasso choice; use --lasso short (an independently validated counterexample)",
                    );
                }
                _ => return usage_error(err, "--lasso expects `short` with --lncheck final"),
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
            "--symmetry-liveness" => {
                let Some(policy) = value().as_deref().and_then(symmetry_liveness) else {
                    return usage_error(
                        err,
                        "--symmetry-liveness expects unreduced, refuse or tlc",
                    );
                };
                req.symmetry_liveness = Some(policy);
            }
            "--action-composition" => match value().as_deref() {
                Some("off") => req.action_composition = Some(dive_api::ActionComposition::Off),
                Some("tlc") => req.action_composition = Some(dive_api::ActionComposition::Tlc),
                Some("tla") => req.action_composition = Some(dive_api::ActionComposition::Tla),
                _ => return usage_error(err, "--action-composition expects off, tlc or tla"),
            },
            // TLC's JVM switch, spelled as TLC users pass it.
            "-Dtlc2.tool.impl.Tool.cdot" => match value().as_deref() {
                Some("true") => req.action_composition = Some(dive_api::ActionComposition::Tlc),
                _ => {
                    return usage_error(
                        err,
                        "-Dtlc2.tool.impl.Tool.cdot expects true (or use --action-composition)",
                    );
                }
            },
            "--sequential" => req.sequential = true,
            "--engine" => match value().as_deref().and_then(dive_api::Engine::parse) {
                Some(e) => engine = e,
                None => return usage_error(err, "--engine expects `reference`, `core` or `auto`"),
            },
            "--core-sample" => match value().and_then(|v| v.parse().ok()) {
                Some(n) => req.core_sample = Some(n),
                None => return usage_error(err, "--core-sample expects a state count"),
            },
            "--seen-disk-entries" => match value().and_then(|v| v.parse().ok()) {
                Some(n) => {
                    seen_policy.limits.entries = n;
                    seen_entries = true;
                }
                None => {
                    return usage_error(
                        err,
                        "--seen-disk-entries expects a positive bounded entry count",
                    );
                }
            },
            "--seen-key-bytes" => match value().and_then(|v| v.parse().ok()) {
                Some(n) => {
                    seen_policy.limits.key_bytes = n;
                    seen_extra = true;
                }
                None => return usage_error(err, "--seen-key-bytes expects a key byte bound"),
            },
            "--seen-probe-steps" => match value().and_then(|v| v.parse().ok()) {
                Some(n) => {
                    seen_policy.limits.probe_steps = n;
                    seen_extra = true;
                }
                None => {
                    return usage_error(err, "--seen-probe-steps expects a positive probe bound");
                }
            },
            "--seen-summary-pairs" => match value().and_then(|v| v.parse().ok()) {
                Some(n) => {
                    seen_policy.limits.summary_pairs = n;
                    seen_extra = true;
                }
                None => {
                    return usage_error(
                        err,
                        "--seen-summary-pairs expects a pair-comparison bound",
                    );
                }
            },
            "--seen-directory" => match value() {
                Some(dir) => {
                    seen_policy.directory = Some(dir.into());
                    seen_extra = true;
                }
                None => return usage_error(err, "--seen-directory expects a parent directory"),
            },
            "--frontier-memory-bytes" => match value().as_deref().map(str::parse::<usize>) {
                Some(Ok(n)) => req.frontier_memory_bytes = Some(n),
                _ => {
                    return usage_error(
                        err,
                        "--frontier-memory-bytes expects a byte count (zero forces disk)",
                    );
                }
            },
            "--frontier-directory" => match value() {
                Some(dir) => req.frontier_directory = Some(dir.into()),
                None => return usage_error(err, "--frontier-directory expects a directory"),
            },
            "--checkpoint" => match value() {
                Some(path) if checkpoint_path.is_none() => checkpoint_path = Some(path.into()),
                _ => return usage_error(err, "--checkpoint expects one native file"),
            },
            "--recover" => match value() {
                Some(path) if recover_path.is_none() => recover_path = Some(path.into()),
                _ => return usage_error(err, "--recover expects one native file"),
            },
            "--checkpoint-after-levels" => match value().and_then(|s| s.parse().ok()) {
                Some(n) if checkpoint_after.is_none() => checkpoint_after = Some(n),
                _ => {
                    return usage_error(
                        err,
                        "--checkpoint-after-levels expects a nonnegative count",
                    );
                }
            },
            "--checkpoint-counters" => {
                counters_given = true;
                checkpoint_counters = match value().as_deref() {
                    Some("tlc") => dive_api::checkpoint::Counters::Tlc,
                    Some("retain") => dive_api::checkpoint::Counters::Retain,
                    _ => return usage_error(err, "--checkpoint-counters expects tlc or retain"),
                };
            }
            "--predecessor-memory-bytes" => {
                let v = value();
                match number::<usize>(flag, v.as_ref(), err) {
                    Some(n) => req.predecessor_memory_bytes = Some(n),
                    None => {
                        return usage_error(
                            err,
                            "--predecessor-memory-bytes expects a nonnegative byte count",
                        );
                    }
                }
            }
            "--predecessor-directory" => match value() {
                Some(dir) => req.predecessor_directory = Some(dir.into()),
                None => return usage_error(err, "--predecessor-directory expects a directory"),
            },
            "--time-limit" => match value().as_deref().map(time_limit) {
                Some(Some(setting)) => req.time_limit = setting,
                _ => {
                    return usage_error(
                        err,
                        "--time-limit expects a positive duration (90, 90s, 500ms, 10m, 2h) or `none`",
                    );
                }
            },
            "--max-memory" => match value().as_deref().map(max_memory) {
                Some(Some(setting)) => req.max_memory = setting,
                _ => {
                    return usage_error(
                        err,
                        "--max-memory expects a positive byte count (3g, 512m, 1048576), `auto` or `none`",
                    );
                }
            },
            "--max-states" | "--max-depth" => match value().as_deref().map(search_limit) {
                Some(Ok(limit)) => {
                    if flag == "--max-states" {
                        req.max_states = limit;
                    } else {
                        req.max_depth = limit;
                    }
                }
                _ => {
                    return usage_error(
                        err,
                        &format!("{flag} expects a nonnegative integer or `none`"),
                    );
                }
            },
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
    if seen_extra && !seen_entries {
        return usage_error(
            err,
            "--seen-directory and seen limits require --seen-disk-entries",
        );
    }
    if seen_entries {
        if let Err(error) = seen_policy.limits.validate() {
            return usage_error(err, &error.to_string());
        }
        if final_check || checkpoint_path.is_some() || recover_path.is_some() {
            return usage_error(
                err,
                "disk seen-state retention with temporal/capture or checkpoint/recovery is not implemented",
            );
        }
        req.seen_disk = Some(seen_policy);
    }
    if final_check != short_lasso {
        return usage_error(
            err,
            "bounded temporal checking needs --lncheck final and --lasso short",
        );
    }
    if req.frontier_directory.is_some() && req.frontier_memory_bytes.is_none() {
        return usage_error(err, "--frontier-directory requires --frontier-memory-bytes");
    }
    req.temporal = if final_check && short_lasso {
        dive_api::TemporalMode::FinalShort
    } else {
        dive_api::TemporalMode::Ordinary
    };
    if req.predecessor_directory.is_some() && req.predecessor_memory_bytes.is_none() {
        return usage_error(
            err,
            "--predecessor-directory requires --predecessor-memory-bytes",
        );
    }
    let checkpoint = match (checkpoint_path, recover_path, checkpoint_after) {
        (Some(path), None, Some(n)) => Some(dive_api::checkpoint::Request {
            path,
            action: dive_api::checkpoint::Action::PauseAfterLevels(n),
            counters: checkpoint_counters,
            config_source_sha256: None,
        }),
        (None, Some(path), None) => Some(dive_api::checkpoint::Request {
            path,
            action: dive_api::checkpoint::Action::Recover,
            counters: checkpoint_counters,
            config_source_sha256: None,
        }),
        (None, None, None) if !counters_given => None,
        _ => {
            return usage_error(
                err,
                "use --checkpoint FILE --checkpoint-after-levels N or --recover FILE, exclusively",
            );
        }
    };
    req.file = file.into();
    if engine == dive_api::Engine::Core && checkpoint.is_some() {
        return usage_error(err, "--engine core does not support checkpoints");
    }
    let checked = if let Some(request) = &checkpoint {
        dive_api::check_file_checkpoint(&req, request)
    } else {
        dive_api::check_file_engine(&req, engine)
    };
    let report = match checked {
        Ok(r) => r,
        Err(dive_api::ApiError::BudgetStartup(error)) => {
            let _ = writeln!(err, "{error}; no verdict produced");
            return ExitCode::from(4);
        }
        Err(dive_api::ApiError::BudgetMeasurement(error)) => {
            let _ = writeln!(err, "{error}; no verdict produced");
            return ExitCode::from(4);
        }
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
        let _ = writeln!(
            out,
            "{}",
            report.to_json_with_exit_convention(exit_convention)
        );
    } else {
        let _ = write!(
            out,
            "{}",
            report.render_human_with_exit_convention(exit_convention)
        );
    }
    ExitCode::from(report.exit_code_for(exit_convention))
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
            "--symmetry-liveness" => {
                let Some(policy) = it.next().and_then(|v| symmetry_liveness(v)) else {
                    return usage_error(
                        err,
                        "--symmetry-liveness expects unreduced, refuse or tlc",
                    );
                };
                opts.symmetry_liveness = Some(policy);
            }
            s if s.starts_with("--symmetry-liveness=") => {
                let Some(policy) = s
                    .strip_prefix("--symmetry-liveness=")
                    .and_then(symmetry_liveness)
                else {
                    return usage_error(
                        err,
                        "--symmetry-liveness expects unreduced, refuse or tlc",
                    );
                };
                opts.symmetry_liveness = Some(policy);
            }
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
    let report = match model.try_validate(&doc) {
        Ok(report) => report,
        Err(error) => return validation_exception(out, err, &error, json),
    };
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

fn validation_exception(
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    error: &dive_api::trace::ValidationRunError,
    json: bool,
) -> ExitCode {
    use dive_api::trace::ValidationRunError;
    let (kind, progress, status) = match error {
        ValidationRunError::ResourceLimit { progress, .. } => {
            ("resource-limit", Some(progress), EXIT_UNSUPPORTED)
        }
        ValidationRunError::Interrupted { progress, .. } => ("interrupted", Some(progress), 5),
        ValidationRunError::EvaluationFailure { progress, .. }
        | ValidationRunError::BindingFailure { progress, .. } => {
            ("execution-error", Some(progress), 4)
        }
        ValidationRunError::ThreadFailure(_) | ValidationRunError::ControlConflict(_) => {
            ("execution-error", None, 4)
        }
    };
    if json {
        let progress=progress.map(|progress|serde_json::json!({"checks_passed":progress.checks_passed,"state":progress.state,"check":progress.check.map(dive_api::trace::Check::name)}));
        let _ = writeln!(
            out,
            "{}",
            serde_json::json!({"schema_version":3,"command":"validate-trace","validation_completed":false,"error":{"kind":kind,"phase":"validation","message":error.to_string(),"progress":progress}})
        );
    } else {
        let _ = writeln!(
            err,
            "{error}; validation did not complete; no validation verdict produced"
        );
    }
    ExitCode::from(status)
}

fn simulation_usage_error(err: &mut impl std::io::Write, message: &str) -> ExitCode {
    let _ = writeln!(err, "error: {message}");
    ExitCode::from(EXIT_USAGE)
}

/// Finite seeded simulation, with explicit sampling metadata.
#[allow(
    clippy::too_many_lines,
    reason = "one command parser keeps finite sampling options and explicit refusals together"
)]
fn simulate_command(
    args: &[String],
    out: &mut impl std::io::Write,
    err: &mut impl std::io::Write,
) -> ExitCode {
    let mut request = dive_api::SimulateRequest {
        compat_tlc: std::env::var("DIVE_COMPAT").is_ok_and(|s| s == "tlc"),
        ..Default::default()
    };
    let mut json = false;
    let mut file = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            s if matches!(
                s.split('=').next(),
                Some("--predecessor-memory-bytes" | "--predecessor-directory")
            ) =>
            {
                let _ = writeln!(err, "predecessor spill with simulation is not implemented");
                return ExitCode::from(EXIT_UNSUPPORTED);
            }
            "--compat=tlc" => request.compat_tlc = true,
            "--config" => {
                let Some(value) = it.next() else {
                    return simulation_usage_error(err, "--config expects a path");
                };
                if request.config.replace(value.into()).is_some() {
                    return simulation_usage_error(err, "--config given twice");
                }
            }
            "--seed" => {
                let Some(value) = it.next() else {
                    return simulation_usage_error(err, "--seed expects an integer");
                };
                let Some(seed) = dive_api::parse_seed(value) else {
                    return simulation_usage_error(err, "invalid --seed");
                };
                if request.seed.replace(seed).is_some() {
                    return simulation_usage_error(err, "--seed given twice");
                }
            }
            "--aril" | "--depth" | "--traces" | "--workers" => {
                let Some(value) = it.next() else {
                    return simulation_usage_error(
                        err,
                        "simulation numeric option expects an integer",
                    );
                };
                let Ok(number) = value.parse::<u64>() else {
                    return simulation_usage_error(
                        err,
                        "simulation option must be a nonnegative integer",
                    );
                };
                match arg.as_str() {
                    "--aril" => request.aril = number,
                    "--traces" => request.traces = number,
                    "--depth" => {
                        let Ok(number) = usize::try_from(number) else {
                            return simulation_usage_error(err, "depth exceeds addressable range");
                        };
                        request.depth = Some(number);
                    }
                    _ => {
                        let Ok(number) = usize::try_from(number) else {
                            return simulation_usage_error(
                                err,
                                "worker count exceeds addressable range",
                            );
                        };
                        if number == 0 {
                            return simulation_usage_error(
                                err,
                                "simulation workers must be positive",
                            );
                        }
                        request.workers = Some(number);
                    }
                }
            }
            "--stats" => match it.next().map(String::as_str) {
                Some("basic") => request.action_stats = true,
                Some("none") => request.action_stats = false,
                Some("full") => {
                    let _ = writeln!(err, "unsupported: simulation full action statistics");
                    return ExitCode::from(EXIT_UNSUPPORTED);
                }
                _ => return simulation_usage_error(err, "--stats expects none or basic"),
            },
            "-deadlock" => request.no_deadlock = true,
            "--output" => match it.next().map(String::as_str) {
                Some("json") => json = true,
                Some("human") => json = false,
                _ => return simulation_usage_error(err, "--output expects human or json"),
            },
            "--library-path" | "--module-path" => {
                let Some(path) = it.next() else {
                    return simulation_usage_error(err, "module option expects a path");
                };
                if arg == "--library-path" {
                    request.library_paths.push(path.into());
                } else {
                    request.module_paths.push(path.into());
                }
            }
            "--continue"
            | "--generate"
            | "--probabilistic"
            | "--rl"
            | "--distributed"
            | "--frontier-memory-bytes"
            | "--frontier-directory" => {
                let _ = writeln!(err, "unsupported: simulation {arg}; no result produced");
                return ExitCode::from(EXIT_UNSUPPORTED);
            }
            value if value.starts_with('-') => {
                return simulation_usage_error(err, &format!("unknown simulation option {value}"));
            }
            value => {
                if file.replace(value).is_some() {
                    return simulation_usage_error(err, "simulate takes one specification file");
                }
            }
        }
    }
    let Some(file) = file else {
        return simulation_usage_error(err, "simulate needs a specification file and --traces N");
    };
    if request.traces == 0 {
        return simulation_usage_error(err, "simulate requires a positive finite --traces N");
    }
    request.file = file.into();
    let report = match dive_api::simulate_file(&request) {
        Ok(report) => report,
        Err(dive_api::ApiError::Unsupported { feature }) => {
            let _ = writeln!(err, "unsupported: {feature}; no result produced");
            return ExitCode::from(EXIT_UNSUPPORTED);
        }
        Err(error) => {
            let _ = writeln!(err, "error: {error}");
            return ExitCode::from(EXIT_REJECTED);
        }
    };
    if json {
        let _ = writeln!(out, "{}", report.to_json());
    } else {
        let _ = write!(out, "{}", report.render_human());
    }
    ExitCode::from(report.exit_code())
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
            "--predecessor-memory-bytes" | "--predecessor-directory" => {
                let _ = writeln!(err, "predecessor spill with DFID is not implemented");
                return ExitCode::from(EXIT_UNSUPPORTED);
            }
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

/// Runs the shared CLI, including an executable whose basename is `tlc`.
///
/// Both installed binaries use this entrypoint; native command defaults remain
/// independent from the TLC checking adapter.
///
/// ```no_run
/// let status = dive_cli::main();
/// # let _ = status;
/// ```
#[must_use]
pub fn main() -> ExitCode {
    let mut arguments = std::env::args_os();
    let shim = arguments.next().is_some_and(|executable| {
        std::path::Path::new(&executable)
            .file_name()
            .is_some_and(|name| name == "tlc" || name == "tlc.exe")
    });
    let arguments: Vec<_> = arguments.collect();
    let adapter = shim || arguments.first().is_some_and(|argument| argument == "tlc");
    let args = arguments
        .into_iter()
        .map(std::ffi::OsString::into_string)
        .collect::<Result<Vec<_>, _>>();
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    let Ok(args) = args else {
        let _ = writeln!(stderr, "DIVE-X0001: command arguments must be valid UTF-8");
        return ExitCode::from(if adapter { 1 } else { EXIT_USAGE });
    };
    if shim {
        return tlc::command(&args, &mut stdout, &mut stderr);
    }
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
        Some("explain") => explain::command(&args[1..], &mut stdout, &mut stderr),
        Some("analyze") => analyze_command(&args[1..], &mut stdout, &mut stderr),
        Some("dfid") => dfid_command(&args[1..], &mut stdout, &mut stderr),
        Some("translate") => translate::command(&args[1..], &mut stdout, &mut stderr),
        Some("check") => check_command(&args[1..], &mut stdout, &mut stderr),
        Some("tlc") => tlc::command(&args[1..], &mut stdout, &mut stderr),
        Some("simulate") => simulate_command(&args[1..], &mut stdout, &mut stderr),
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

#[cfg(test)]
mod predecessor_tests {
    use super::*;
    #[test]
    fn orphan_directory_and_other_modes_refuse_before_input_access() {
        let args = [
            "missing.tla".into(),
            "--predecessor-directory".into(),
            "missing-parent".into(),
        ];
        let mut output = Vec::new();
        let mut errors = Vec::new();
        assert_eq!(
            check_command(&args, &mut output, &mut errors),
            ExitCode::from(EXIT_USAGE)
        );
        assert!(String::from_utf8_lossy(&errors).contains("requires --predecessor-memory-bytes"));
        for flag in [
            "--predecessor-memory-bytes=0",
            "--predecessor-directory=missing",
        ] {
            errors.clear();
            assert_eq!(
                simulate_command(&[flag.into()], &mut output, &mut errors),
                ExitCode::from(EXIT_UNSUPPORTED)
            );
            assert!(String::from_utf8_lossy(&errors).contains("predecessor spill with simulation"));
            errors.clear();
            assert_eq!(
                dfid_command(&[flag.into()], &mut output, &mut errors),
                ExitCode::from(EXIT_UNSUPPORTED)
            );
            assert!(String::from_utf8_lossy(&errors).contains("predecessor spill with DFID"));
        }
        assert_eq!(output, [] as [u8; 0]);
    }
}

#[cfg(test)]
mod seen_tests {
    use super::*;
    #[test]
    fn malformed_or_orphan_disk_policy_refuses_before_reading_model() {
        for flags in [
            vec!["--seen-directory", "unused-parent"],
            vec!["--seen-key-bytes", "64"],
            vec!["--seen-probe-steps", "1"],
            vec!["--seen-summary-pairs", "0"],
            vec!["--seen-disk-entries", "0"],
            vec!["--seen-disk-entries", "4", "--seen-key-bytes", "0"],
            vec!["--seen-disk-entries", "4", "--seen-probe-steps", "0"],
            vec!["--seen-disk-entries", "4", "--recover", "missing.cp"],
            vec![
                "--seen-disk-entries",
                "4",
                "--lncheck",
                "final",
                "--lasso",
                "short",
            ],
        ] {
            let mut arguments = vec!["missing-seen-model.tla".to_owned()];
            arguments.extend(flags.iter().map(|s| (*s).to_owned()));
            let mut output = Vec::new();
            let mut errors = Vec::new();
            assert_eq!(
                check_command(&arguments, &mut output, &mut errors),
                ExitCode::from(EXIT_USAGE)
            );
            assert_eq!(output, [] as [u8; 0]);
            let error = String::from_utf8_lossy(&errors);
            assert!(!error.contains("cannot read"), "{error}");
            assert!(
                error.contains("--seen-directory and seen limits require --seen-disk-entries")
                    || error.contains("DIVE-S0700")
                    || error.contains("is not implemented"),
                "{error}"
            );
        }
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    use dive_api::BudgetSetting::{Default, Explicit, Unlimited};
    use std::time::Duration;

    #[test]
    fn time_limits_parse_units_and_none() {
        assert_eq!(time_limit("90"), Some(Explicit(Duration::from_secs(90))));
        assert_eq!(time_limit("90s"), Some(Explicit(Duration::from_secs(90))));
        assert_eq!(
            time_limit("500ms"),
            Some(Explicit(Duration::from_millis(500)))
        );
        assert_eq!(time_limit("10m"), Some(Explicit(Duration::from_secs(600))));
        assert_eq!(time_limit("2h"), Some(Explicit(Duration::from_secs(7200))));
        assert_eq!(time_limit("none"), Some(Unlimited));
        for bad in ["", "0", "-1", "1.5s", "10d", "s", "auto"] {
            assert_eq!(time_limit(bad), None, "{bad}");
        }
    }

    #[test]
    fn memory_ceilings_parse_jvm_units_auto_and_none() {
        assert_eq!(max_memory("3g"), Some(Explicit(3 << 30)));
        assert_eq!(max_memory("512M"), Some(Explicit(512 << 20)));
        assert_eq!(max_memory("1048576"), Some(Explicit(1 << 20)));
        assert_eq!(max_memory("auto"), Some(Default));
        assert_eq!(max_memory("none"), Some(Unlimited));
        for bad in ["", "0", "0g", "-1", "3gb", "1.5g", "99999999999t"] {
            assert_eq!(max_memory(bad), None, "{bad}");
        }
    }

    #[test]
    fn malformed_budgets_are_usage_errors_before_reading_the_model() {
        for flags in [
            ["--time-limit", "0"],
            ["--max-memory", "lots"],
            ["--max-states", "-1"],
            ["--max-depth", "1.5"],
        ] {
            let args: Vec<String> = std::iter::once("missing.tla")
                .chain(flags)
                .map(Into::into)
                .collect();
            let (mut output, mut errors) = (Vec::new(), Vec::new());
            assert_eq!(
                check_command(&args, &mut output, &mut errors),
                ExitCode::from(EXIT_USAGE)
            );
        }
    }

    #[test]
    fn search_ceilings_parse_zero_exact_counts_and_none() {
        assert_eq!(search_limit("0"), Ok(Some(0)));
        assert_eq!(search_limit("42"), Ok(Some(42)));
        assert_eq!(search_limit("none"), Ok(None));
        for bad in ["", "-1", "1.5", "18446744073709551616", "auto"] {
            assert!(search_limit(bad).is_err(), "{bad}");
        }
    }
}

fn symmetry_liveness(value: &str) -> Option<dive_api::SymmetryLiveness> {
    match value {
        "unreduced" => Some(dive_api::SymmetryLiveness::Unreduced),
        "refuse" => Some(dive_api::SymmetryLiveness::Refuse),
        "tlc" => Some(dive_api::SymmetryLiveness::Tlc),
        _ => None,
    }
}

#[cfg(test)]
mod symmetry_liveness_tests {
    #![allow(clippy::expect_used, reason = "finite argument and report controls")]
    use super::*;
    fn arguments(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| (*arg).into()).collect()
    }
    #[test]
    fn unknown_modes_are_usage_errors_before_any_input_access() {
        for args in [
            arguments(&["Missing.tla", "--symmetry-liveness=unknown"]),
            arguments(&["--symmetry-liveness", "bad", "Missing.tla"]),
        ] {
            let mut out = Vec::new();
            let mut err = Vec::new();
            assert_eq!(
                check_command(&args, &mut out, &mut err),
                ExitCode::from(EXIT_USAGE)
            );
            assert!(String::from_utf8_lossy(&err).contains("--symmetry-liveness expects"));
            assert_eq!(out, [] as [u8; 0]);
            assert_eq!(
                validate_trace_command(&args, &mut out, &mut err),
                ExitCode::from(EXIT_USAGE)
            );
        }
    }
    #[test]
    fn explicit_mode_survives_native_profile_flag_order_and_adapter_transport() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../conformance/corpus/probes/specifications/P4SymLive/P4SymLive.tla");
        let file = file.to_str().expect("fixture path");
        for native in [true, false] {
            for reverse in [false, true] {
                let mut args = if native {
                    arguments(&[
                        file,
                        "--output=json",
                        "--workers=1",
                        "--seed=0",
                        "--fp=0",
                        "--max-memory=none",
                        "--time-limit=none",
                    ])
                } else {
                    arguments(&[
                        file,
                        "--output=json",
                        "--workers=1",
                        "--seed=0",
                        "--fp=0",
                        "--max-memory=none",
                    ])
                };
                let selection = if reverse {
                    ["--symmetry-liveness=unreduced", "--compat=tlc"]
                } else {
                    ["--compat=tlc", "--symmetry-liveness=unreduced"]
                };
                if native {
                    args.extend(arguments(&selection));
                } else {
                    args.push("--symmetry-liveness=unreduced".into());
                }
                let mut out = Vec::new();
                let mut err = Vec::new();
                let status = if native {
                    check_command(&args, &mut out, &mut err)
                } else {
                    tlc::command(&args, &mut out, &mut err)
                };
                assert_eq!(
                    status,
                    ExitCode::SUCCESS,
                    "{}",
                    String::from_utf8_lossy(&err)
                );
                let report: serde_json::Value = serde_json::from_slice(&out).expect("actual JSON");
                assert_eq!(report["options"]["profile"], "tlc+overrides");
                assert_eq!(report["options"]["symmetry_liveness"], "unreduced");
                assert_eq!(report["symmetry_liveness"]["scope"], "whole-run");
                assert_eq!(report["statistics"]["distinct"], 2);
            }
        }
    }
}

#[cfg(test)]
mod validation_exception_tests {
    #[test]
    fn aborted_validation_json_is_an_exception_without_a_result()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let error = dive_api::trace::ValidationRunError::ThreadFailure(
            "test isolated thread did not return".into(),
        );
        let status = super::validation_exception(&mut out, &mut err, &error, true);
        assert_eq!(status, std::process::ExitCode::from(4));
        let value: serde_json::Value = serde_json::from_slice(&out)?;
        assert_eq!(value["schema_version"], 3);
        assert_eq!(value["command"], "validate-trace");
        assert_eq!(value["validation_completed"], false);
        assert_eq!(value["error"]["kind"], "execution-error");
        assert!(value.get("result").is_none());
        assert!(value.get("verdict").is_none());
        assert_eq!(err, [] as [u8; 0]);
        Ok(())
    }
}
