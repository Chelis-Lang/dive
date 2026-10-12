use std::{path::Path,sync::Arc};
fn bound(path:&Path)->Result<dive_liveness::periodic_snapshot::OptionalSnapshot,String>{
 let a=dive_sema::analyze_file(&path.join("Terminal.tla"),&dive_sema::Options::default()).map_err(|e|e.to_string())?;
 let ir=Arc::new(dive_ir::lower(&a).map_err(|e|e.to_string())?);
 let c=dive_config::parse(&std::fs::read_to_string(path.join("Terminal.cfg")).map_err(|e|e.to_string())?).into_config().ok_or("cfg")?;
 dive_liveness::periodic_snapshot::bind_optional(ir,&c).map_err(|e|e.to_string())
}
fn main()->Result<(),String>{
 let args=std::env::args().collect::<Vec<_>>();let path=Path::new(&args[1]);let bytes=std::fs::read_to_string(&args[2]).map_err(|e|e.to_string())?;
 let doc=dive_trace::TraceDoc::parse(&bytes)?;
 let memory=bound(path)?;let trace=dive_trace::Trace::decode(memory.model(),&doc).map_err(|e|e.detail)?;
 let m=dive_trace::validate(memory.model(),&trace,None);println!("memory={m:?}");assert_eq!(m.validation,dive_trace::Validation::Valid);
 let fresh=bound(path)?;let j=dive_trace::validate_doc(fresh.model(),&doc,&[],None).0;println!("json={j:?}");assert_eq!(j.validation,dive_trace::Validation::Valid);
 for kind in ["init","claim","closing","shape"]{
  let mut d=doc.clone();match kind{
   "init"=>{d.states[0].insert("x".into(),serde_json::json!(99));},
   "claim"=>d.properties=vec!["Unknown".into()],
   "closing"=>{d.states.push(std::collections::BTreeMap::from([("x".into(),serde_json::json!(99))]));d.labels.push(None);d.length=Some(d.states.len());d.ending=Some(dive_trace::Ending::Lasso{back_to:0});},
   _=>d.ending=Some(dive_trace::Ending::Lasso{back_to:d.states.len()}),
  }
  let t=dive_trace::Trace::decode(memory.model(),&d).map_err(|e|e.detail)?;let a=dive_trace::validate(memory.model(),&t,None);
  let text=serde_json::to_string(&d.to_json()).map_err(|e|e.to_string())?;let d2=dive_trace::TraceDoc::parse(&text)?;let b=dive_trace::validate_doc(fresh.model(),&d2,&[],None).0;
  println!("mutant={kind} memory={a:?} json={b:?}");assert!(matches!(a.validation,dive_trace::Validation::Invalid(_)));assert!(matches!(b.validation,dive_trace::Validation::Invalid(_)));
 }
 Ok(())
}
