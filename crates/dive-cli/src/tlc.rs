//! Checking-only transport for the pinned TLC argument spellings.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{Value, json};

const HELP: &str = "dive TLC checking adapter (supported subset)\n\nUSAGE: dive tlc [OPTIONS] SPEC[.tla]\n       tlc [OPTIONS] SPEC[.tla]\n\n    -config FILE[.cfg]   configuration (default SPEC.cfg)\n    -workers N|auto     worker count (default 1)\n    -seed N             signed 64-bit replay seed (always reported)\n    -fp N               fingerprint polynomial, 0..130\n    -deadlock           disable deadlock checking\n    -modelcheck         explicit checking mode\n    -lncheck final      bounded final checking with deterministic short lassos\n    -lncheck default    ordinary; eligible pure requests with -fp 0 use final\n    -XmxSIZE            memory ceiling, as for the JVM heap (default: half of memory)\n    -h, -help           this supported-subset help\n\nThe adapter selects TLC evaluation and TLC verdict statuses. Final checking\nretains its reported limits, refusals and independent counterexample validation;\nits deterministic short lasso need not be TLC's chosen lasso. Dive extensions:\n--output human|json; --max-memory SIZE|auto|none; --config/--workers/--seed/--fp/--lncheck aliases. Native\ndive check defaults and its explicit final/short flags are unchanged.\n\n-tool output, simulation, DFID, checkpoint/storage and debugger TLC protocols\nare not implemented here and refuse by name. This is not full TLC CLI parity.\n";

#[derive(Default)]
struct Arguments {
    request: dive_api::CheckRequest,
    json: bool,
    help: bool,
}

#[derive(Debug)]
enum Failure {
    Usage(String),
    Unsupported(String),
    Invalid(String),
}

impl Failure {
    const fn status(&self) -> u8 {
        match self {
            Self::Usage(_) => 1,
            Self::Unsupported(_) => 3,
            Self::Invalid(_) => 4,
        }
    }

    const fn code(&self) -> &'static str {
        match self {
            Self::Usage(_) => "DIVE-X0001",
            Self::Unsupported(_) => "DIVE-X0002",
            Self::Invalid(_) => "DIVE-X0003",
        }
    }

    fn message(&self) -> &str {
        match self {
            Self::Usage(message) | Self::Unsupported(message) | Self::Invalid(message) => message,
        }
    }

    fn emit(&self, err: &mut impl Write) -> ExitCode {
        let _ = writeln!(
            err,
            "{}: {}; no verdict produced",
            self.code(),
            self.message()
        );
        ExitCode::from(self.status())
    }
}

fn extension(name: &str, suffix: &str) -> PathBuf {
    if name.ends_with(suffix) {
        name.into()
    } else {
        format!("{name}{suffix}").into()
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "explicit checking flag transport and named refusals"
)]
fn parse(args: &[String]) -> Result<Arguments, Failure> {
    let mut parsed = Arguments {
        request: dive_api::CheckRequest {
            compat_tlc: true,
            workers: Some(1),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut file = None;
    let mut short = false;
    let mut it = args.iter();
    while let Some(argument) = it.next() {
        // TLC flags have separate values. Existing double-dash aliases also
        // retain dive's inline-value spelling.
        let (flag, inline) = if argument.starts_with("--") {
            argument
                .split_once('=')
                .map_or((argument.as_str(), None), |(f, v)| (f, Some(v)))
        } else {
            (argument.as_str(), None)
        };
        let mut value = || {
            inline
                .or_else(|| it.next().map(String::as_str))
                .ok_or_else(|| Failure::Usage(format!("{flag} expects a value")))
        };
        match flag {
            "-h" | "-help" | "--help" => {
                parsed.help = true;
                return Ok(parsed);
            }
            "-modelcheck" => {}
            "-deadlock" => parsed.request.no_deadlock = true,
            "-config" | "--config" => {
                let name = value()?;
                // Embedded configurations remain the API's explicit refusal.
                #[allow(
                    clippy::case_sensitive_file_extension_comparisons,
                    reason = "pinned TLC.java uses exact case-sensitive .tla suffix matching"
                )]
                let embedded = name.ends_with(".tla");
                parsed.request.config = Some(if embedded {
                    name.into()
                } else {
                    extension(name, ".cfg")
                });
            }
            "-seed" | "--seed" => {
                parsed.request.seed = Some(value()?.parse().map_err(|_| {
                    Failure::Usage(format!("{flag} expects a signed 64-bit integer"))
                })?);
            }
            "-fp" | "--fp" => {
                parsed.request.fp = Some(
                    value()?
                        .parse::<i32>()
                        .ok()
                        .filter(|n| (0..131).contains(n))
                        .and_then(|n| usize::try_from(n).ok())
                        .ok_or_else(|| {
                            Failure::Usage(format!("{flag} expects an index from 0 to 130"))
                        })?,
                );
            }
            "-workers" | "--workers" => {
                let workers = value()?;
                parsed.request.workers = Some(if workers.trim().eq_ignore_ascii_case("auto") {
                    std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
                } else {
                    workers
                        .parse::<i32>()
                        .ok()
                        .filter(|n| *n > 0)
                        .and_then(|n| usize::try_from(n).ok())
                        .ok_or_else(|| {
                            Failure::Usage(format!("{flag} expects a positive integer or auto"))
                        })?
                });
            }
            "-lncheck" | "--lncheck" => {
                let strategy = value()?;
                parsed.request.temporal = if strategy.eq_ignore_ascii_case("final") {
                    dive_api::TemporalMode::FinalShort
                } else if strategy.eq_ignore_ascii_case("default") {
                    dive_api::TemporalMode::Ordinary
                } else {
                    return Err(Failure::Unsupported(format!(
                        "{flag} {strategy}: checking strategy is not implemented"
                    )));
                };
            }
            "--lasso" => {
                let selection = value()?;
                if selection != "short" {
                    return Err(Failure::Unsupported(format!(
                        "--lasso {selection}: only deterministic short selection is implemented"
                    )));
                }
                short = true;
            }
            "--output" => match value()? {
                "json" => parsed.json = true,
                "human" => parsed.json = false,
                _ => return Err(Failure::Usage("--output expects human or json".into())),
            },
            // The memory ceiling stands for TLC's heap (ADR 0107): the JVM's
            // `-Xmx` spelling, or dive's `--max-memory` with `auto`/`none`.
            "--max-memory" => {
                parsed.request.max_memory = crate::max_memory(value()?).ok_or_else(|| {
                    Failure::Usage(
                        "--max-memory expects a positive byte count (3g, 512m), auto or none"
                            .into(),
                    )
                })?;
            }
            flag if flag.starts_with("-Xmx") => {
                parsed.request.max_memory = flag
                    .strip_prefix("-Xmx")
                    .and_then(crate::max_memory)
                    .filter(|setting| matches!(setting, dive_api::BudgetSetting::Explicit(_)))
                    .ok_or_else(|| {
                        Failure::Usage(format!("{flag}: expects a heap size such as -Xmx8g"))
                    })?;
            }
            "-tool"
            | "-simulate"
            | "-generate"
            | "-dfid"
            | "-depth"
            | "-aril"
            | "-checkpoint"
            | "-recover"
            | "-metadir"
            | "-cleanup"
            | "-debugger"
            | "-continue"
            | "-coverage"
            | "-dump"
            | "-dumpTrace"
            | "-loadTrace"
            | "-fpbits"
            | "-fpmem"
            | "-maxSetSize"
            | "-inv"
            | "-invlevel"
            | "-postCondition"
            | "-messagesAsErrors"
            | "-suppressMessages"
            | "-noGenerateSpecTE"
            | "-generateSpecTE"
            | "-noGenerateSpecTEBin"
            | "-teSpecOutDir"
            | "-userFile"
            | "-gzip"
            | "-debug"
            | "-difftrace"
            | "-nowarning"
            | "-terse"
            | "-view" => {
                return Err(Failure::Unsupported(format!(
                    "{flag}: TLC option is not implemented by this checking adapter"
                )));
            }
            flag if flag.starts_with('-') => {
                return Err(Failure::Usage(format!("unrecognized option: {flag}")));
            }
            name => {
                if name.is_empty() || file.replace(extension(name, ".tla")).is_some() {
                    return Err(Failure::Usage(
                        "expected one nonempty TLA+ module name".into(),
                    ));
                }
            }
        }
    }
    parsed.request.file = file.ok_or_else(|| Failure::Usage("missing input TLA+ module".into()))?;
    if short && parsed.request.temporal != dive_api::TemporalMode::FinalShort {
        return Err(Failure::Usage(
            "--lasso short requires -lncheck final".into(),
        ));
    }
    // TLC.java selects the absolute root's directory as ToolIO.userDir. With
    // a relative root, an explicit relative -config is resolved from the CWD.
    // Never choose between these locations by testing which file exists.
    if parsed.request.file.is_absolute()
        && let Some(config) = &mut parsed.request.config
        && config.is_relative()
    {
        *config = parsed
            .request
            .file
            .parent()
            .unwrap_or(Path::new("."))
            .join(&*config);
    }
    Ok(parsed)
}

/// Every emitted word is replayed against fresh source/configuration inputs.
fn validate_trace(request: &dive_api::CheckRequest, doc: &mut Value) -> Result<(), Failure> {
    if doc["counterexample"].is_null() {
        return Ok(());
    }
    doc["counterexample"]["validation"] = Value::Null;
    let trace = dive_api::trace::TraceDoc::parse(&doc["counterexample"]["trace"].to_string())
        .map_err(|e| Failure::Unsupported(format!("counterexample transport: {e}")))?;
    let seed = doc["options"]["seed"].as_i64();
    let fp = doc["options"]["fingerprint"]["polynomial_index"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok());
    let run = seed
        .zip(fp)
        .ok_or_else(|| Failure::Unsupported("counterexample run metadata is missing".into()))?;
    let options = dive_api::trace::TraceOptions {
        compat_tlc: true,
        no_deadlock: request.no_deadlock,
        run: Some(run),
        ..Default::default()
    };
    let config = request
        .config
        .clone()
        .unwrap_or_else(|| request.file.with_extension("cfg"));
    let model = dive_api::trace::SpecModel::load(&request.file, &config, &options)
        .map_err(|e| Failure::Unsupported(format!("independent counterexample validation: {e}")))?;
    match model.validate(&trace).validation {
        dive_api::trace::Validation::Valid => {
            doc["counterexample"]["validation"] = json!("valid");
            Ok(())
        }
        dive_api::trace::Validation::Unsupported(reason) => Err(Failure::Unsupported(format!(
            "independent counterexample validation: {reason}"
        ))),
        dive_api::trace::Validation::Invalid(reason) => Err(Failure::Invalid(format!(
            "independently rejected counterexample: {reason:?}"
        ))),
    }
}

pub(super) fn command(args: &[String], out: &mut impl Write, err: &mut impl Write) -> ExitCode {
    let parsed = match parse(args) {
        Ok(parsed) => parsed,
        Err(error) => return error.emit(err),
    };
    if parsed.help {
        let _ = write!(out, "{HELP}");
        return ExitCode::SUCCESS;
    }
    let report = match dive_api::check_file_tlc_inputs(&parsed.request) {
        Ok(report) => report,
        Err(dive_api::ApiError::Unsupported { feature }) => {
            return Failure::Unsupported(feature).emit(err);
        }
        Err(dive_api::ApiError::Io { path, error }) => {
            let _ = writeln!(err, "DIVE-X0004: cannot read {path}: {error}");
            let config = parsed
                .request
                .config
                .clone()
                .unwrap_or_else(|| parsed.request.file.with_extension("cfg"));
            return ExitCode::from(if Path::new(&path) == config { 151 } else { 150 });
        }
        Err(error) => return Failure::Usage(error.to_string()).emit(err),
    };
    let mut doc = report.to_json_with_exit_convention(dive_api::ExitConvention::Tlc);
    if let Err(error) = validate_trace(&parsed.request, &mut doc) {
        if parsed.json {
            doc["verdict"] = json!(if error.status() == 4 {
                "internal-error"
            } else {
                "unsupported"
            });
            doc["message"] = json!(error.message());
            doc["tlc"] = Value::Null;
            doc["property"] = Value::Null;
            doc["properties"] = Value::Null;
            doc["counterexample"] = Value::Null;
            doc["exit_code"] = json!(error.status());
            if let Some(diagnostics) = doc["diagnostics"].as_array_mut() {
                diagnostics.push(json!({"code": error.code(), "message": error.message()}));
            }
            let _ = writeln!(out, "{doc}");
        } else {
            for line in doc["output"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                let _ = writeln!(out, "{line}");
            }
            return error.emit(err);
        }
        return ExitCode::from(error.status());
    }
    if parsed.json {
        let _ = writeln!(out, "{doc}");
    } else {
        let _ = write!(
            out,
            "{}",
            report.render_human_with_exit_convention(dive_api::ExitConvention::Tlc)
        );
    }
    ExitCode::from(report.exit_code_for(dive_api::ExitConvention::Tlc))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| (*s).into()).collect()
    }

    #[test]
    fn the_memory_ceiling_takes_the_jvm_spelling_or_max_memory() {
        use dive_api::BudgetSetting::{Default, Explicit, Unlimited};
        let memory = |args: &[&str]| parse(&words(args)).map(|p| p.request.max_memory).ok();
        assert_eq!(memory(&["Spec"]), Some(Default));
        assert_eq!(memory(&["-Xmx8g", "Spec"]), Some(Explicit(8 << 30)));
        assert_eq!(memory(&["--max-memory", "none", "Spec"]), Some(Unlimited));
        assert_eq!(
            memory(&["--max-memory=512m", "Spec"]),
            Some(Explicit(512 << 20))
        );
        for bad in [["-Xmx", "Spec"], ["-Xmxnone", "Spec"], ["-Xmx8gb", "Spec"]] {
            assert!(memory(&bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn path_resolution_has_no_existing_file_fallback() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parse(&words(&["models/Probe", "-config", "Alt"]))
            .map_err(|e| e.message().to_owned())?;
        assert_eq!(parsed.request.file, Path::new("models/Probe.tla"));
        assert_eq!(parsed.request.config.as_deref(), Some(Path::new("Alt.cfg")));
        let root = std::env::current_dir()?.join("models/Probe");
        let parsed = parse(&words(&[
            root.to_str().ok_or("UTF-8 root")?,
            "-config",
            "Alt.cfg",
        ]))
        .map_err(|e| e.message().to_owned())?;
        assert_eq!(
            parsed.request.config,
            root.parent().map(|p| p.join("Alt.cfg"))
        );
        Ok(())
    }

    #[test]
    fn independently_replayed_word_passes_and_changed_successor_fails()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tlc-check");
        let request = dive_api::CheckRequest {
            file: root.join("Probe.tla"),
            config: Some(root.join("Fail.cfg")),
            compat_tlc: true,
            seed: Some(7),
            fp: Some(0),
            workers: Some(1),
            ..Default::default()
        };
        let mut doc = dive_api::check_file(&request)?.to_json();
        assert!(validate_trace(&request, &mut doc).is_ok());
        assert_eq!(doc["counterexample"]["validation"], "valid");
        doc["counterexample"]["trace"]["states"][1]["values"]["x"] = json!(99);
        assert!(matches!(
            validate_trace(&request, &mut doc),
            Err(Failure::Invalid(_))
        ));
        assert!(doc["counterexample"]["validation"].is_null());
        Ok(())
    }
}
