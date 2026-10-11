use dive_api::{CheckRequest, SeenOptions, Store, check_file};
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
fn main() -> Result<(), Box<dyn std::error::Error>> {
 let root=std::path::Path::new("/Users/jeff/code/dive-trace-representation-work");
 let cases=[("PureViewViolation","/Users/jeff/code/dive-storage-tuple-function-artifacts/seen-engine-current/api-controls/models"),("EarlyInvariant","crates/dive-check/tests/fixtures/disk-seen"),
 ("SymOrder","crates/dive-liveness/tests/fixtures/tlc-probes/specs"),
 ("SymViolation","crates/dive-liveness/tests/fixtures/tlc-probes/specs")];
 let mut rows=Vec::new();
 for (name,parent) in cases {
  let file=root.join(parent).join(name).join(format!("{name}.tla"));
  for compat_tlc in [true,false] {
   let validator=SpecModel::load(&file,&file.with_extension("cfg"),&TraceOptions{compat_tlc,run:Some((0,7)),..TraceOptions::default()})?;
   for store in [Store::Fingerprint,Store::Exact] {for width in [1,4,8] {for chaos in [None,Some(0),Some(7)] {
    let report=check_file(&CheckRequest{file:file.clone(),compat_tlc,store,seen_disk:Some(SeenOptions::default()),workers:Some(width),chaos,seed:Some(7),fp:Some(0),frontier_memory_bytes:Some(0),predecessor_memory_bytes:Some(0),..CheckRequest::default()})?;
    let json=report.to_json();let doc=TraceDoc::from_json(&json)?;let control=validator.controls(&doc);
    assert_eq!(control.positive.validation,Validation::Valid);assert!(control.failures().is_empty());assert!(!control.mutants.is_empty());
    assert_eq!(json["options"]["workers"]["used"],1);assert_eq!(json["seen_storage"]["after_cleanup"]["cleanup_completed"],true);
    rows.push(serde_json::json!({"case":name,"compat_tlc":compat_tlc,"store":if store==Store::Exact{"exact"}else{"fp"},"requested_workers":width,"chaos":chaos,"report":json,"trace":doc.to_json(),"positive":control.positive.validation.kind(),"mutants":control.mutants.iter().map(|(name,v)|serde_json::json!({"name":name,"validation":v.kind()})).collect::<Vec<_>>() }));
   }}}
  }
 }
 assert_eq!(rows.len(),144);println!("{}",serde_json::json!({"family_certified":false,"rows":rows}));Ok(())
}
