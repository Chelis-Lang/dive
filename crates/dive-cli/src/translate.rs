//! Native PlusCal translation command; never invokes an external translator.
use std::io::Write;
use std::process::ExitCode;

pub(super) fn command(args: &[String], out: &mut impl Write, err: &mut impl Write) -> ExitCode {
    let mut request = dive_api::TranslationRequest::default();
    match std::env::var("DIVE_COMPAT").as_deref() {
        Ok("tlc") => request.compat_tlc = true,
        Ok("dive" | "") | Err(_) => {}
        Ok(profile) => {
            let _ = writeln!(err, "error: unsupported compatibility profile {profile}");
            return ExitCode::from(2);
        }
    }
    let mut file = None;
    let mut json = false;
    let mut arguments = args.iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--compat=tlc" => request.compat_tlc = true,
            "--label" | "-label" | "--report-labels" | "-reportLabels" => {
                request.limits.insert_missing_labels = true;
            }
            "--label-root" | "-labelRoot" => {
                let Some(root) = arguments.next() else {
                    let _ = writeln!(err, "error: {argument} expects a label prefix");
                    return ExitCode::from(2);
                };
                request.limits.label_root.clone_from(root);
            }
            "-nof" => request.limits.fairness = dive_api::TranslationFairness::None,
            "-wf" => request.limits.fairness = dive_api::TranslationFairness::Weak,
            "-sf" => request.limits.fairness = dive_api::TranslationFairness::Strong,
            "-wfNext" => request.limits.fairness = dive_api::TranslationFairness::Next,
            "--output" => match arguments.next().map(String::as_str) {
                Some("json") => json = true,
                Some("human") => json = false,
                _ => {
                    let _ = writeln!(err, "error: --output expects human|json");
                    return ExitCode::from(2);
                }
            },
            "--library-path" | "--module-path" => {
                let Some(value) = arguments.next() else {
                    let _ = writeln!(err, "error: {argument} expects a directory");
                    return ExitCode::from(2);
                };
                let paths = if argument == "--library-path" {
                    &mut request.analyze.library_paths
                } else {
                    &mut request.analyze.after_standard
                };
                paths.push(value.into());
            }
            "--fair" | "-termination" | "-nocfg" | "-version" | "-width" | "--write" => {
                let _ = writeln!(
                    err,
                    "unsupported PlusCal translation option {argument}; no artifact produced"
                );
                return ExitCode::from(3);
            }
            option if option.starts_with('-') => {
                let _ = writeln!(err, "error: unknown dive translate option {option}");
                return ExitCode::from(2);
            }
            value => {
                if file.replace(value).is_some() {
                    let _ = writeln!(err, "error: dive translate takes one TLA module");
                    return ExitCode::from(2);
                }
            }
        }
    }
    let Some(file) = file else {
        let _ = writeln!(
            err,
            "error: dive translate needs a TLA module containing one algorithm"
        );
        return ExitCode::from(2);
    };
    let path = std::path::Path::new(file);
    if let Some(parent) = path.parent() {
        request.analyze.after_standard.push(parent.into());
    }
    match dive_api::translate_file(path, &request) {
        Ok(report) => {
            let document = report.to_json();
            if json {
                let _ = writeln!(out, "{document}");
            } else if report.accepted() {
                if let Some(module) = document["module_source"].as_str() {
                    let _ = write!(out, "{module}");
                }
            } else {
                let _ = writeln!(err, "{document}");
            }
            ExitCode::from(report.exit_code())
        }
        Err(error) => {
            let _ = writeln!(err, "error: {error}");
            ExitCode::from(2)
        }
    }
}
