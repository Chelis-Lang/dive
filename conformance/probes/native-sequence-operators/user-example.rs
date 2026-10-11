use std::{path::Path, sync::Arc};
fn main() -> Result<(),Box<dyn std::error::Error>> {
 let file=Path::new("conformance/probes/native-sequence-operators/models/SequenceAppendLetFails/SequenceAppendLetFails.tla");
 let analysis=dive_sema::analyze_file(file,&Default::default())?;
 let ir=Arc::new(dive_ir::lower(&analysis)?);
 let cfg=dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\n").into_config().ok_or("invalid cfg")?;
 let options=dive_check::CheckOptions{workers:4,..dive_check::CheckOptions::tlc(0,0)};
 let report=dive_api::tlc_temporal::check_final(&ir,&cfg,&options,Default::default())?;
 let dive_api::tlc_temporal::Outcome::Violation(v)=report.outcome()else{return Err("expected violation".into())};
 assert_eq!(v.document().properties,vec!["Live"]);
 assert_eq!(v.validation().validation,dive_trace::Validation::Valid);
 println!("{:?}",v.document().properties); Ok(())
}
