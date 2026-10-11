//! Cargo-owned process option/Full-checksum guide; replacement is never a proof.
use dive_api::{RewriteRequest, TranslationFairness, rewrite_source};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: pluscal_process_options FILE [--compat=tlc] [-label] [-labelRoot PREFIX] [-nof|-wf|-sf|-wfNext]")?;
    let mut request = RewriteRequest::default();
    let mut selected = false;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--compat=tlc" => request.translation.compat_tlc = true,
            "-label" => request.translation.limits.insert_missing_labels = true,
            "-labelRoot" => {
                request.translation.limits.label_root = args.next().ok_or("missing label root")?;
            }
            "-nof" | "-wf" | "-sf" | "-wfNext" => {
                if selected {
                    return Err("select exactly one fairness option".into());
                }
                selected = true;
                request.translation.limits.fairness = match flag.as_str() {
                    "-nof" => TranslationFairness::None,
                    "-wf" => TranslationFairness::Weak,
                    "-sf" => TranslationFairness::Strong,
                    _ => TranslationFairness::Next,
                };
            }
            _ => return Err("unknown guide option".into()),
        }
    }
    let source = std::fs::read_to_string(&path)?;
    let report = rewrite_source(&path, &source, &request);
    println!("{}", report.to_json());
    if !report.accepted() {
        return Err("rewrite rejected or unsupported; inspect the report".into());
    }
    Ok(())
}
