//! Cargo-owned executable counterpart of the bounded process checksum guide.
use dive_api::{RewriteRequest, rewrite_source};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or("usage: pluscal_process_checksum FILE [--compat=tlc]")?;
    let mut request = RewriteRequest::default();
    for flag in args {
        match flag.as_str() {
            "--compat=tlc" => request.translation.compat_tlc = true,
            _ => return Err("unknown example option".into()),
        }
    }
    let source = std::fs::read_to_string(&path)?;
    let report = rewrite_source(&path, &source, &request);
    println!("{}", report.to_json());
    if !report.accepted() {
        return Err("rewrite rejected or explicitly unsupported; inspect the report".into());
    }
    Ok(())
}
