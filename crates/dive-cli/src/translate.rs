//! Native PlusCal translation command; never invokes an external translator.
use std::io::Write;
use std::process::ExitCode;

#[allow(
    clippy::too_many_lines,
    reason = "ordered CLI option mapping retains explicit rewrite/checksum opt-ins and failure exits"
)]
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
    let mut editing = dive_api::RewriteOptions::default();
    let mut rewrite = false;
    let mut write = false;
    let mut original_ast = false;
    let mut editing_flag = false;
    let mut file = None;
    let mut json = false;
    let mut fairness_selected = false;
    let mut arguments = args.iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--compat=tlc" => request.compat_tlc = true,
            "--rewrite" => rewrite = true,
            "--write" => {
                write = true;
                rewrite = true;
            }
            "--ast" => original_ast = true,
            "--replace-modified" => {
                editing.replace_modified = true;
                editing_flag = true;
            }
            "--checksums" => {
                editing_flag = true;
                match arguments.next().map(String::as_str) {
                    Some("tla") => editing.checksums = dive_api::ChecksumPolicy::TranslationOnly,
                    Some("full") => editing.checksums = dive_api::ChecksumPolicy::Full,
                    _ => {
                        let _ = writeln!(err, "error: --checksums expects tla|full");
                        return ExitCode::from(2);
                    }
                }
            }
            "--label" | "-label" => request.limits.insert_missing_labels = true,
            "--report-labels" | "-reportLabels" => request.limits.report_labels = true,
            "-termination" => request.limits.check_termination = true,
            "-noDoneDisjunct" => request.limits.no_done_disjunct = true,
            "-lineWidth" => {
                let Some(width) = arguments
                    .next()
                    .and_then(|s| s.parse::<i32>().ok())
                    .filter(|n| *n >= 60)
                else {
                    let _ = writeln!(err, "error: -lineWidth expects an i32 value at least 60");
                    return ExitCode::from(2);
                };
                request.limits.line_width = width;
            }
            "-version" => {
                let Some(version) = arguments.next() else {
                    let _ = writeln!(err, "error: -version expects a language version");
                    return ExitCode::from(2);
                };
                request.limits.version = Some(version.clone());
            }
            "-noold" => request.limits.no_old = true,
            "-nocfg" => request.limits.no_configuration = true,
            "-unixEOL" => request.limits.unix_eol = true,
            "-debug" => request.limits.debug = true,
            "-writeAST" => request.limits.write_ast = true,
            "-spec" | "-myspec" | "-spec2" | "-myspec2" => {
                let Some(name) = arguments.next() else {
                    let _ = writeln!(err, "error: {argument} expects a specification name");
                    return ExitCode::from(2);
                };
                if request.limits.specification_backend.is_some() {
                    let _ = writeln!(
                        err,
                        "error: conflicting specification-backed translator options"
                    );
                    return ExitCode::from(2);
                }
                request.limits.specification_backend = Some((argument.clone(), name.clone()));
            }
            "--help" | "-help" => {
                let _ = writeln!(
                    out,
                    "dive translate [OPTIONS] MODULE.tla\n\
Native translation; --rewrite returns a replacement; --write publishes it.\n\
Supported: -label -labelRoot ROOT -reportLabels -nof -wf -sf -wfNext\n\
           -termination -noDoneDisjunct -lineWidth N -version V -debug\n\
Write policy: -noold -nocfg -unixEOL (--write opts in to publication)\n\
Original AST: --ast returns bytes; -writeAST writes AST.tla and exits 255\n\
Explicitly unsupported: -spec -myspec -spec2 -myspec2\n\
See docs/pluscal-translation.md for version/AST limits and JSON reports."
                );
                return ExitCode::SUCCESS;
            }
            "--label-root" | "-labelRoot" => {
                let Some(root) = arguments.next() else {
                    let _ = writeln!(err, "error: {argument} expects a label prefix");
                    return ExitCode::from(2);
                };
                request.limits.label_root.clone_from(root);
            }
            "-nof" | "-wf" | "-sf" | "-wfNext" => {
                if fairness_selected {
                    let _ = writeln!(err, "error: select only one of -nof, -wf, -sf and -wfNext");
                    return ExitCode::from(2);
                }
                fairness_selected = true;
                request.limits.fairness = match argument.as_str() {
                    "-nof" => dive_api::TranslationFairness::None,
                    "-wf" => dive_api::TranslationFairness::Weak,
                    "-sf" => dive_api::TranslationFairness::Strong,
                    _ => dive_api::TranslationFairness::Next,
                };
            }
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
            "--fair" | "-width" => {
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
    if editing_flag && !rewrite {
        let _ = writeln!(err, "error: checksum/replacement flags require --rewrite");
        return ExitCode::from(2);
    }
    if (original_ast && rewrite) || (request.limits.write_ast && (original_ast || rewrite)) {
        let _ = writeln!(
            err,
            "error: --ast and --rewrite select different artifact stages"
        );
        return ExitCode::from(2);
    }
    let path = std::path::Path::new(file);
    if let Some(parent) = path.parent() {
        request.analyze.after_standard.push(parent.into());
    }
    let ast_write = request.limits.write_ast;
    let result = if ast_write {
        dive_api::write_original_ast_file(path, std::path::Path::new("AST.tla"), &request)
            .map(|report| (report.exit_code(), report.written(), report.to_json()))
    } else if original_ast {
        dive_api::original_ast_file(path, &request).map(|report| {
            (
                report.exit_code(),
                report.artifact_available(),
                report.to_json(),
            )
        })
    } else if write {
        dive_api::write_translation_file(
            path,
            &dive_api::RewriteRequest {
                translation: request,
                editing,
            },
        )
        .map(|report| (report.exit_code(), report.accepted(), report.to_json()))
    } else if rewrite {
        dive_api::rewrite_file(
            path,
            &dive_api::RewriteRequest {
                translation: request,
                editing,
            },
        )
        .map(|report| (report.exit_code(), report.accepted(), report.to_json()))
    } else {
        dive_api::translate_file(path, &request)
            .map(|report| (report.exit_code(), report.accepted(), report.to_json()))
    };
    match result {
        Ok((status, accepted, document)) => {
            if json {
                let _ = writeln!(out, "{document}");
            } else if accepted {
                if let Some(warnings) = document["replacement"]["warnings"].as_array() {
                    for warning in warnings {
                        if let Some(message) = warning["message"].as_str() {
                            let _ =
                                writeln!(err, "warning: algorithm checksum unavailable: {message}");
                        }
                    }
                }
                if document["translation_options"]["report_labels"] == true
                    && let Some(labels) = document["artifact"]["inserted_labels"].as_array()
                    && !labels.is_empty()
                {
                    let _ = writeln!(
                        err,
                        "{}",
                        if labels.len() == 1 {
                            "The following label was added:"
                        } else {
                            "The following labels were added:"
                        }
                    );
                    for label in labels {
                        if let (Some(name), Some(line), Some(column)) = (
                            label["name"].as_str(),
                            label["position"]["line"].as_u64(),
                            label["position"]["column"].as_u64(),
                        ) {
                            if let (Some(definition_line), Some(definition_column)) = (
                                label["macro_definition_position"]["line"].as_u64(),
                                label["macro_definition_position"]["column"].as_u64(),
                            ) {
                                let _ = writeln!(
                                    err,
                                    "  {name} at line {definition_line}, column {definition_column} of macro called at line {line}, column {column}"
                                );
                            } else {
                                let _ = writeln!(err, "  {name} at line {line}, column {column}");
                            }
                        }
                    }
                }
                let source = if write || ast_write {
                    let _ = writeln!(out, "Written {}", if ast_write { "AST.tla" } else { file });
                    &serde_json::Value::Null
                } else if original_ast {
                    &document["ast_artifact"]["module_source"]
                } else if rewrite {
                    &document["replacement"]["module_source"]
                } else {
                    &document["module_source"]
                };
                if let Some(module) = source.as_str() {
                    let _ = write!(out, "{module}");
                }
            } else {
                let _ = writeln!(err, "{document}");
            }
            ExitCode::from(status)
        }
        Err(error) => {
            let _ = writeln!(err, "error: {error}");
            ExitCode::from(if ast_write { 255 } else { 2 })
        }
    }
}
