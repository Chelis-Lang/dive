//! Runnable bounded LOCAL/function DEFINE Full rewrite guide; artifact admission is not proof.
use dive_api::{RewriteRequest, rewrite_source};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args().skip(1);
    let file = arguments
        .next()
        .ok_or("usage: pluscal_definition_forms SOURCE [tlc]")?;
    let source = std::fs::read_to_string(&file)?;
    let mut request = RewriteRequest::default();
    request.translation.compat_tlc = arguments.next().as_deref() == Some("tlc");
    let report = rewrite_source(&file, &source, &request);
    println!("{}", serde_json::to_string(&report.to_json())?);
    if !report.accepted() {
        return Err("DEFINE source did not yield an admitted artifact".into());
    }
    Ok(())
}
