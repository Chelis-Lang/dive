//! Exact CLI/public API comparison and independent original/mutant words.
#![forbid(unsafe_code)]
use dive_api::{CheckRequest, SeenOptions, Store, check_file};
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use std::{path::PathBuf, fs};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let root = PathBuf::from(args.next().ok_or("repository root is required")?);
    let runtime = PathBuf::from(args.next().ok_or("exact CLI matrix is required")?);
    if args.next().is_some() { return Err("unexpected argument".into()); }
    let selection: serde_json::Value = serde_json::from_slice(&fs::read(root.join("conformance/probes/disk-safety-roles/declared-selection.json"))?)?;
    let cases = selection["cases"].as_array().ok_or("cases array")?;
    assert_eq!(cases.len(),29);
    let witness_cases = ["ActionProperty","AlwaysProperty","ConjunctionProperty","InstanceProperty","LetProperty","PropertyAlias"];
    let mut rows = Vec::new();
    for name in cases {
        let name = name.as_str().ok_or("case name")?;
        let file = root.join("crates/dive-check/tests/fixtures/disk-seen-roles").join(name).join(format!("{name}.tla"));
        for compat_tlc in [true,false] {
            let profile = if compat_tlc { "tlc" } else { "dive" };
            let validator = if witness_cases.contains(&name) {
                Some(SpecModel::load(&file,&file.with_extension("cfg"),&TraceOptions{compat_tlc,run:Some((0,7)),..TraceOptions::default()})?)
            } else { None };
            for store in [Store::Fingerprint,Store::Exact] {
                let storage = if store == Store::Exact { "exact" } else { "fp" };
                for width in [1,4,8] { for chaos in [None,Some(0),Some(7)] { for budget in [false,true] {
                    let report = check_file(&CheckRequest{file:file.clone(),config:Some(file.with_extension("cfg")),seen_disk:Some(SeenOptions{limits:dive_api::SeenLimits{entries:1000,..dive_api::SeenLimits::default()},..SeenOptions::default()}),compat_tlc,store,workers:Some(width),seed:Some(7),fp:Some(0),chaos,frontier_memory_bytes:budget.then_some(0),predecessor_memory_bytes:budget.then_some(0),..CheckRequest::default()})?;
                    let json=report.to_json();
                    let label=format!("disk-{name}-{profile}-{storage}-{width}-{}-{}",chaos.map_or_else(||"None".into(),|n|n.to_string()),if budget{"True"}else{"False"});
                    let mut expected:serde_json::Value=serde_json::from_slice(&fs::read(runtime.join(format!("{label}.stdout")))?)?;
                    let mut actual=json.clone();actual.as_object_mut().ok_or("report object")?.remove("elapsed_ms");expected.as_object_mut().ok_or("CLI object")?.remove("elapsed_ms");assert_eq!(actual,expected,"{label}");
                    let mut row=serde_json::json!({"case":name,"label":label,"profile":profile,"store":storage,"requested_workers":width,"chaos":chaos,"zero_transient_budgets":budget,"report":json,"cli_api_equal_except_elapsed":true});
                    if let Some(validator)=&validator {
                        let doc=TraceDoc::from_json(&json)?;let controls=validator.controls(&doc);
                        assert_eq!(controls.positive.validation,Validation::Valid,"{label}");assert!(controls.failures().is_empty(),"{label}: {:?}",controls.failures());assert!(!controls.mutants.is_empty());
                        row["word"]=doc.to_json();row["positive"]=serde_json::json!(controls.positive.validation.kind());
                        row["mutants"]=serde_json::json!(controls.mutants.iter().map(|(name,v)|serde_json::json!({"name":name,"validation":v.kind()})).collect::<Vec<_>>());
                    }
                    rows.push(row);
                } } }
            }
        }
    }
    assert_eq!(rows.len(),2088);
    println!("{}",serde_json::json!({"family_certified":false,"rows":rows}));
    Ok(())
}
