use std::{path::Path, sync::Arc, time::Duration};
use dive_api::periodic_candidate::{self, Origin};
use dive_liveness::{periodic_lifecycle, phase_graph};
use serde_json::json;
fn main()->Result<(),String>{
 let dir=std::env::args().nth(1).ok_or("output directory")?;std::fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
 for width in [1,3,4,8]{
  for kind in ["eof","holds-no-proof","false-hook","integer-hook","division-hook","failed-init","capture-limit","conjunction","absent-unforced","absent-forced"]{run(Path::new(&dir),kind,width,None)?;}
 }
 for (kind,value) in [("string","\"literal\""),("tuple","<<\"literal\",0>>"),("record","[field |-> \"literal\"]"),("set","{\"a\",\"b\"}")]{run(Path::new(&dir),kind,4,Some(value))?;}
 for kind in ["default-profile","effectful-startup"]{run(Path::new(&dir),kind,4,None)?;}
 Ok(())
}
fn run(dir:&Path,kind:&str,width:usize,value:Option<&str>)->Result<(),String>{
 let extra=value.map_or(String::new(),|v|format!(" /\\ s = {v}"));let variable=if value.is_some(){"x,s"}else{"x"};let update=if value.is_some(){" /\\ s'=s"}else{""};
 let property=if ["holds-no-proof","absent-unforced","absent-forced"].contains(&kind){"<>(x=0)"}else if kind=="conjunction"{"<>(x=2) /\\ <>(x=3)"}else{"<>(x=2)"};
 let hook=match kind{"false-hook"=>"FALSE","integer-hook"=>"1","division-hook"=>"1 \\div 0",_=>"TRUE"};let initial=if kind=="failed-init"{"1 \\div 0"}else{"0"};let startup=if kind=="effectful-startup"{"Unused == PrintT(TRUE)\n"}else{""};
 let source=format!("---- MODULE RuntimeCandidate ----\nEXTENDS Integers,TLC\nVARIABLES {variable}\n{startup}Init == x=({initial}){extra}\nNext == x'=(IF x=0 THEN 1 ELSE 1){update}\nLive == {property}\nHook == {hook}\n====\n");
 let config=format!("INIT Init\nNEXT Next\nPROPERTY Live\n{}CHECK_DEADLOCK FALSE\n",if ["false-hook","integer-hook","division-hook"].contains(&kind){"_PERIODIC Hook\n"}else{""});
 let a=dive_sema::analyze_source("RuntimeCandidate.tla",&source,&dive_sema::Options::default());let ir=Arc::new(dive_ir::lower(&a).map_err(|e|e.to_string())?);let cfg=dive_config::parse(&config).into_config().ok_or("configuration")?;
 let opts=if kind=="default-profile"{dive_check::CheckOptions{workers:width,..dive_check::CheckOptions::dive(0)}}else{dive_check::CheckOptions{workers:width,chaos:Some(7),..dive_check::CheckOptions::tlc(0,0)}};
 let mut limits=phase_graph::Limits::default();if kind=="capture-limit"{limits.capture.states=1;}
 let out=periodic_candidate::check_optional(ir.clone(),cfg.clone(),opts,limits,periodic_lifecycle::Options{force:kind=="absent-forced",..periodic_lifecycle::Options::default()},dive_check::progress::Options{interval:Duration::from_secs(3600),force:["false-hook","integer-hook","division-hook","failed-init","absent-unforced","absent-forced"].contains(&kind)});
 let mut record=json!({"kind":kind,"requested_width":width,"source":source,"configuration":config,"model_proof":false,"ordinary_schedule_certified":false,"limits":format!("{limits:?}")});
 match out{
  Err(e)=>{if !["default-profile","effectful-startup"].contains(&kind){return Err(format!("{kind}/{width}: {e}"));}assert!(e.run.is_none());if kind=="effectful-startup"{let periodic_candidate::Error::Binding(binding)=&e.error else{return Err("expected pre-startup binding refusal".into());};assert!(binding.output.is_empty());record["startup_output"]=json!([]);}record["error_code"]=json!(e.error.code());record["error"]=json!(e.error.to_string());record["execution_retained"]=json!(false);},
  Ok(report)=>{
   assert!(!report.model_proof());
   let present=["false-hook","integer-hook","division-hook"].contains(&kind);
   assert_eq!(report.run().configured_name(),present.then_some("Hook"));
   if !present{assert!(report.run().definition().is_none());assert!(report.run().expression().is_none());assert!(report.run().lifecycle().service_evaluations().is_empty());}
   if kind=="failed-init"{assert!(report.run().lifecycle().request_services().is_empty());assert!(report.run().lifecycle().safety_eof().is_none());}
   if ["absent-unforced","absent-forced"].contains(&kind){let first=report.run().lifecycle().request_services().first().ok_or("actual initial service")?;assert_eq!(first.graph_source(),None);assert!(first.committed().is_empty());assert_eq!(first.resume_requested(),kind=="absent-forced");}
   record["selected_role"]=json!({"configured":report.run().configured_name(),"definition":format!("{:?}",report.run().definition()),"expression":format!("{:?}",report.run().expression())});
   record["execution_retained"]=json!(true);record["backend"]=json!(format!("{:?}",report.run().lifecycle().backend()));record["safety_eof"]=json!(format!("{:?}",report.run().lifecycle().safety_eof()));record["retained_source_count"]=json!(report.run().lifecycle().concrete_sources().len());record["services"]=json!(format!("{:?}",report.run().lifecycle().request_services()));
   if let Some(w)=report.witness(){
    assert!(matches!(w.origin(),Origin::Eof(_)));assert_eq!(w.memory_validation().validation,dive_trace::Validation::Valid);assert_eq!(w.json_validation().validation,dive_trace::Validation::Valid);
    record["origin"]=json!(format!("{:?}",w.origin()));record["order"]=json!(w.order());record["selection"]=json!(format!("{:?}",w.selection()));record["document"]=w.document().to_json();record["memory_validation"]=json!(format!("{:?}",w.memory_validation()));record["json_validation"]=json!(format!("{:?}",w.json_validation()));
    let fresh=dive_liveness::periodic_snapshot::bind_optional(ir,&cfg).map_err(|e|e.to_string())?;let mut mutants=vec![];
    for mode in ["init","claim","next","shape"]{
     let mut d=w.document().clone();match mode{
      "init"=>{d.states[0].insert("x".into(),json!(99));},
      "claim"=>d.properties=vec!["Unknown".into()],
      "next"=>{let mut s=d.states[0].clone();s.insert("x".into(),json!(99));d.states.push(s);d.labels.push(None);d.length=Some(d.states.len());d.ending=Some(dive_trace::Ending::Lasso{back_to:0});},
      _=>d.ending=Some(dive_trace::Ending::Lasso{back_to:d.states.len()}),
     }
     let t=dive_trace::Trace::decode(fresh.model(),&d).map_err(|e|e.detail)?;let mem=dive_trace::validate(fresh.model(),&t,None);let bytes=serde_json::to_string(&d.to_json()).map_err(|e|e.to_string())?;let parsed=dive_trace::TraceDoc::parse(&bytes)?;let j=dive_trace::validate_doc(fresh.model(),&parsed,&cfg.model_values,None).0;
     assert!(matches!(mem.validation,dive_trace::Validation::Invalid(_)));assert!(matches!(j.validation,dive_trace::Validation::Invalid(_)));
     mutants.push(json!({"kind":mode,"document":parsed.to_json(),"memory_validation":format!("{mem:?}"),"json_validation":format!("{j:?}")}));
    }record["mutants"]=json!(mutants);
   }else{assert!(["holds-no-proof","false-hook","integer-hook","division-hook","failed-init","capture-limit","absent-unforced","absent-forced"].contains(&kind));record["witness"]=json!(null);if !["holds-no-proof","absent-unforced","absent-forced"].contains(&kind){assert!(report.run().lifecycle().safety_eof().is_none());}}
  }
 }
 let name=format!("{kind}-w{width}.json");std::fs::write(dir.join(&name),serde_json::to_vec_pretty(&record).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;println!("{name}");Ok(())
}
