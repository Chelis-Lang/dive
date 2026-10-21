//! Read-only explanations for explicitly catalogued stable diagnostics.

use std::io::Write;
use std::process::ExitCode;

const USAGE: &str = "USAGE: dive explain <CODE> [--output human|json]\n";

pub(crate) fn command(args: &[String], out: &mut impl Write, err: &mut impl Write) -> ExitCode {
    if args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h") {
        let _ = write!(out, "{USAGE}");
        return ExitCode::SUCCESS;
    }
    let mut code = None;
    let mut json = false;
    let mut output_seen = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" if !output_seen => {
                output_seen = true;
                match args.next().map(String::as_str) {
                    Some("human") => json = false,
                    Some("json") => json = true,
                    _ => {
                        let _ = writeln!(err, "error: --output expects human or json\n{USAGE}");
                        return ExitCode::from(super::EXIT_USAGE);
                    }
                }
            }
            value if !value.starts_with('-') && code.is_none() => code = Some(value),
            _ => {
                let _ = writeln!(err, "error: unexpected explain argument {arg:?}\n{USAGE}");
                return ExitCode::from(super::EXIT_USAGE);
            }
        }
    }
    let Some(code) = code else {
        let _ = writeln!(err, "error: explain requires a diagnostic code\n{USAGE}");
        return ExitCode::from(super::EXIT_USAGE);
    };
    let explanation = dive_api::diagnostics::explain(code);
    if json {
        let _ = writeln!(out, "{}", dive_api::diagnostics::explanation_json(code));
    } else if let Some(entry) = explanation {
        let _ = writeln!(
            out,
            "{}: {}\nHelp: {}",
            entry.code, entry.cause, entry.remediation
        );
    } else {
        let _ = writeln!(
            err,
            "error: no explanation is registered for {code:?}; retain the original diagnostic message and source location"
        );
    }
    ExitCode::from(if explanation.is_some() {
        0
    } else {
        super::EXIT_USAGE
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_codes_fail_without_a_guessed_cause_or_model_verdict() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        assert_eq!(
            command(
                &["DIVE-L9999".into(), "--output".into(), "json".into()],
                &mut out,
                &mut err
            ),
            ExitCode::from(super::super::EXIT_USAGE)
        );
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("\"known\":false") && text.contains("\"explanation\":null"));
        assert!(!text.contains("verdict"));
        assert_eq!(err, Vec::<u8>::new());
    }

    #[test]
    fn explains_known_codes_and_rejects_ambiguous_arguments() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        assert_eq!(
            command(&["DIVE-L0021".into()], &mut out, &mut err),
            ExitCode::SUCCESS
        );
        assert!(String::from_utf8_lossy(&out).contains("does not raise formula-count bounds"));
        assert_eq!(err, Vec::<u8>::new());
        for args in [
            vec![],
            vec!["DIVE-L0021", "extra"],
            vec!["DIVE-L0021", "--output"],
            vec!["DIVE-L0021", "--output", "yaml"],
            vec!["DIVE-L0021", "--output", "json", "--output", "human"],
        ] {
            out.clear();
            err.clear();
            assert_eq!(
                command(
                    &args.into_iter().map(str::to_owned).collect::<Vec<_>>(),
                    &mut out,
                    &mut err
                ),
                ExitCode::from(super::super::EXIT_USAGE)
            );
            assert_eq!(out, Vec::<u8>::new());
            assert_ne!(err, Vec::<u8>::new());
        }
    }
}
