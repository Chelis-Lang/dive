use std::sync::Arc;
fn main()->Result<(),Box<dyn std::error::Error>> {
 let file=std::path::Path::new("/Users/jeff/code/dive-trace-representation-work/crates/dive-check/tests/fixtures/disk-seen-roles/AssumePrintSafety/AssumePrintSafety.tla");
 let analysis=dive_sema::analyze_file(file,&dive_sema::Options::default())?;assert!(analysis.accepted());
 let original=dive_ir::lower(&analysis)?;
 let mut json=serde_json::to_value(&original)?;json["modules"][original.root().index()]["standard"]=serde_json::json!(true);
 let changed:dive_ir::Ir=serde_json::from_value(json.clone())?;
 std::fs::write("modified-ir.json",serde_json::to_vec_pretty(&json)?)?;
 let cfg=dive_config::parse(&std::fs::read_to_string(file.with_extension("cfg"))?).into_config().ok_or("configuration")?;
 for (label,ir) in [("source",original),("caller-standard-root",changed)] {
  let ir=Arc::new(ir);
  for mut options in [dive_check::CheckOptions::tlc(0,7),dive_check::CheckOptions::dive(7)] {
   options.seen_disk=Some(dive_check::SeenOptions::default());let report=dive_check::check(&ir,&cfg,&options);
   println!("{label} {:?} verdict={:?} output={:?} stats={:?} seen={:?}",options.profile,report.verdict,report.output,report.stats,report.seen);
  }
 }
 Ok(())
}
