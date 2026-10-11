use dive_check::CheckOptions;
use dive_liveness::{periodic_lifecycle, phase_graph};
use std::sync::Arc;
fn main() -> Result<(), String> {
 for width in [1,3,4,8] {
  let source="---- MODULE Resume ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=IF x<3 THEN x+1 ELSE x\nLive == []<>(x>=0)\n====\n";
  let a=dive_sema::analyze_source("Resume.tla",source,&dive_sema::Options::default());
  let ir=Arc::new(dive_ir::lower(&a).map_err(|e|e.to_string())?);
  let cfg=dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\nCHECK_DEADLOCK FALSE\n").into_config().ok_or("config")?;
  let opts=CheckOptions{workers:width,..CheckOptions::tlc(0,0)};
  let m=dive_model::bind(ir,&cfg,&dive_model::BindOptions{eval:opts.eval,no_deadlock:opts.no_deadlock,host:None}).map_err(|e|e.to_string())?;
  let r=periodic_lifecycle::execute(&m,&opts,phase_graph::Limits::default(),periodic_lifecycle::Options{progress_interval:0,force:true,..periodic_lifecycle::Options::default()});
  let first=r.invocations().first().ok_or("missing actual invocation")?;
  if first.force()!=(true,false)||!first.resume_requested()||r.safety_eof().is_none()||r.backend().complete().is_none()||r.final_checks().is_empty()||r.final_checks().iter().any(|c|c.selection().candidate.is_some())||r.model_proof(){return Err(format!("safe lifecycle control failed width{width}: {:?}",r.backend().stop().map(|s|&s.error)));}
  println!("{}",serde_json::json!({"width":width,"used":first.workers(),"invocations":r.invocations().len(),"force_before":first.force().0,"force_after":first.force().1,"resume_requested":first.resume_requested(),"first_outcome":format!("{:?}",first.outcome()),"safety_eof":r.safety_eof().map(|s|serde_json::json!({"generated":s.generated,"distinct":s.distinct,"queue":s.left_on_queue})),"final_checks":r.final_checks().len(),"model_proof":r.model_proof()}));
 }
 Ok(())
}
