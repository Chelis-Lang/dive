use std::{path::Path, sync::Arc, time::Duration};
use dive_api::periodic_candidate::{self, Origin, Terminal};
use dive_liveness::{periodic_lifecycle, phase_graph};
use serde_json::json;
fn main()->Result<(),String>{
 let dir=std::env::args().nth(1).ok_or("output directory")?;std::fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
 for width in [1,3,4,8]{
  for kind in ["post-candidate","post-final-error","post-cached-pass","post-cached-candidate","post-budget","post-init","post-source","post-unbounded","post-effect"] {run_post(Path::new(&dir),kind,width)?;}
  for kind in ["eof","holds-no-proof","safe-false","safe-integer","safe-division","false-hook","integer-hook","division-hook","failed-init","failed-next","capture-limit","conjunction","empty-init","absent-unforced","absent-forced"]{run(Path::new(&dir),kind,width,None)?;}
 }
 for width in [1,3,4,8] {
  for kind in ["evaluation","invariant","action","incomplete","deadlock"] {for hook in ["absent","true","false"] {run_worker(Path::new(&dir),kind,hook,width,false)?;}}
  for hook in ["absent","false"] {run_worker(Path::new(&dir),"invariant",hook,width,true)?;}
  for kind in ["prefix-evaluation","prefix-invariant"] {run_worker(Path::new(&dir),kind,"false",width,false)?;}
 }
 for (kind,value) in [("string","\"literal\""),("tuple","<<\"literal\",0>>"),("record","[field |-> \"literal\"]"),("set","{\"a\",\"b\"}")]{run(Path::new(&dir),kind,4,Some(value))?;}
 for kind in ["default-profile","effectful-startup","state-hook","missing-hook","arity-hook","constant-hook"]{run(Path::new(&dir),kind,4,None)?;}
 Ok(())
}
fn run(dir:&Path,kind:&str,width:usize,value:Option<&str>)->Result<(),String>{
 let extra=value.map_or(String::new(),|v|format!(" /\\ s = {v}"));let variable=if value.is_some(){"x,s"}else{"x"};let update=if value.is_some(){" /\\ s'=s"}else{""};
 let property=if ["holds-no-proof","safe-false","safe-integer","safe-division","absent-unforced","absent-forced"].contains(&kind){"<>(x=0)"}else if kind=="conjunction"{"<>(x=2) /\\ <>(x=3)"}else{"<>(x=2)"};
 let hook=match kind{"false-hook"|"safe-false"=>"FALSE","integer-hook"|"safe-integer"=>"1","division-hook"|"safe-division"=>"1 \\div 0","state-hook"=>"x=0",_=>"TRUE"};let initial=if kind=="failed-init"{"1 \\div 0"}else{"0"};let startup=if kind=="effectful-startup"{"Unused == PrintT(TRUE)\n"}else{""};
 let next=if kind=="failed-next"{"1 \\div 0"}else{"IF x=0 THEN 1 ELSE 1"};let initial_predicate=if kind=="empty-init"{String::from("x \\in {}") }else{format!("x=({initial}){extra}")};
 let hook_decl=if kind=="arity-hook"{format!("Hook(v) == {hook}")}else{format!("Hook == {hook}")};let constant=if kind=="constant-hook"{"CONSTANT K\n"}else{""};
 let source=format!("---- MODULE RuntimeCandidate ----\nEXTENDS Integers,TLC\nVARIABLES {variable}\n{constant}{startup}Init == {initial_predicate}\nNext == x'=({next}){update}\nLive == {property}\n{hook_decl}\n====\n");
 let config=format!("INIT Init\nNEXT Next\nPROPERTY Live\n{}CHECK_DEADLOCK FALSE\n",if kind=="missing-hook"{"_PERIODIC Missing\n"}else if kind=="constant-hook"{"CONSTANT K=TRUE\n_PERIODIC K\n"}else if ["false-hook","integer-hook","division-hook","safe-false","safe-integer","safe-division","state-hook","arity-hook"].contains(&kind){"_PERIODIC Hook\n"}else{""});
 let a=dive_sema::analyze_source("RuntimeCandidate.tla",&source,&dive_sema::Options::default());let ir=Arc::new(dive_ir::lower(&a).map_err(|e|e.to_string())?);let cfg=dive_config::parse(&config).into_config().ok_or("configuration")?;
 let opts=if kind=="default-profile"{dive_check::CheckOptions{workers:width,..dive_check::CheckOptions::dive(0)}}else{dive_check::CheckOptions{workers:width,chaos:Some(7),..dive_check::CheckOptions::tlc(0,0)}};
 let mut limits=phase_graph::Limits::default();if kind=="capture-limit"{limits.capture.states=1;}
 let out=periodic_candidate::check_optional_stopped(ir.clone(),cfg.clone(),opts,limits,periodic_lifecycle::Options{force:kind=="absent-forced",..periodic_lifecycle::Options::default()},dive_check::progress::Options{interval:Duration::from_secs(3600),force:["false-hook","integer-hook","division-hook","failed-init","absent-unforced","absent-forced"].contains(&kind)});
 let mut record=json!({"kind":kind,"requested_width":width,"source":source,"configuration":config,"model_proof":false,"ordinary_schedule_certified":false,"limits":format!("{limits:?}")});
 match out{
  Err(e)=>{if !["default-profile","effectful-startup","state-hook","missing-hook","arity-hook","constant-hook"].contains(&kind){return Err(format!("{kind}/{width}: {e}"));}assert!(e.run.is_none());if kind=="effectful-startup"{let periodic_candidate::Error::Binding(binding)=&e.error else{return Err("expected pre-startup binding refusal".into());};assert!(binding.output.is_empty());record["startup_output"]=json!([]);}record["error_code"]=json!(e.error.code());record["error"]=json!(e.error.to_string());record["execution_retained"]=json!(false);},
  Ok(report)=>{
   assert!(!report.model_proof());
   record["terminal_classification"]=json!(format!("{:?}",report.terminal()));
   record["finalization"]=json!(format!("{:?}",report.run().lifecycle().finalization()));
   let present=["false-hook","integer-hook","division-hook","safe-false","safe-integer","safe-division"].contains(&kind);
   assert_eq!(report.run().configured_name(),present.then_some("Hook"));
   if !present{assert!(report.run().definition().is_none());assert!(report.run().expression().is_none());assert!(report.run().lifecycle().service_evaluations().is_empty());}
   if kind=="failed-init"{assert!(report.run().lifecycle().request_services().is_empty());assert!(report.run().lifecycle().safety_eof().is_none());}
   if ["absent-unforced","absent-forced"].contains(&kind){let first=report.run().lifecycle().request_services().first().ok_or("actual initial service")?;assert_eq!(first.graph_source(),None);assert!(first.committed().is_empty());assert_eq!(first.resume_requested(),kind=="absent-forced");}
   let lifecycle=report.run().lifecycle();
   if kind=="failed-next" {let t=lifecycle.stopped_service().ok_or("actual worker error transaction")?;assert_eq!(t.code(),2179);assert!(t.joined_stats().is_some());record["stopped_service"]=json!({"kind":format!("{:?}",t.kind()),"code":t.code(),"source":t.source(),"prejoin_stats":format!("{:?}",t.stats()),"joined_stats":format!("{:?}",t.joined_stats()),"pool":t.pool(),"graph_source":t.graph_source(),"suspension_attempted":t.suspension_attempted(),"outcome":format!("{:?}",t.outcome()),"model_proof":false});}
   else{assert!(lifecycle.stopped_service().is_none());record["stopped_service"]=json!(null);}
   record["stopped_faults"]=json!(format!("{:?}",lifecycle.stopped_faults().collect::<Vec<_>>()));
   let should_terminal=!["false-hook","integer-hook","division-hook","failed-init","failed-next","capture-limit"].contains(&kind);
   assert_eq!(lifecycle.terminal_service().is_some(),should_terminal,"{kind}/{width}");
   if let Some(t)=lifecycle.terminal_service(){
    assert!(t.joins_confirmed());assert_eq!(t.pool().0,width);assert_eq!(t.stats().left_on_queue,0);assert_eq!(Some(t.stats()),lifecycle.safety_eof());assert!(!t.model_proof());
    assert_eq!(t.outcome(),&Ok(dive_liveness::periodic::Outcome::Complete));
    if kind=="empty-init"{assert_eq!((t.stats().distinct,t.stats().generated,t.stats().initial_generated),(0,0,0));}
    if ["safe-false","safe-integer","safe-division"].contains(&kind){assert!(lifecycle.service_evaluations().is_empty());assert!(lifecycle.request_services().is_empty());assert!(t.suspension_attempted());}
    record["terminal_service"]=json!({"receipt":format!("{t:?}"),"joins_confirmed":t.joins_confirmed(),"actual_pool":t.pool(),"generated":t.stats().generated,"distinct":t.stats().distinct,"left_on_queue":t.stats().left_on_queue,"suspension_attempted":t.suspension_attempted(),"model_proof":false});
   }else{assert!(lifecycle.safety_eof().is_none());assert!(lifecycle.final_checks().is_empty());record["terminal_service"]=json!(null);}
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
   }else{assert!(["holds-no-proof","safe-false","safe-integer","safe-division","false-hook","integer-hook","division-hook","failed-init","failed-next","capture-limit","empty-init","absent-unforced","absent-forced"].contains(&kind));record["witness"]=json!(null);if !should_terminal{assert!(report.run().lifecycle().safety_eof().is_none());}}
  }
 }
 let name=format!("{kind}-w{width}.json");std::fs::write(dir.join(&name),serde_json::to_vec_pretty(&record).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;println!("{name}");Ok(())
}

fn run_worker(dir:&Path,kind:&str,hook:&str,width:usize,forced:bool)->Result<(),String>{
 use dive_check::capture::WorkerStop;
 let (next,role,expected,code)=match kind {
  "evaluation"=>("x'=1 \\div 0 /\\ y'=y","",WorkerStop::Evaluation,2179),
  "invariant"=>("x'=1 /\\ y'=y","INVARIANT Inv",WorkerStop::Invariant,2110),
  "action"=>("x'=1 /\\ y'=y","PROPERTY Action",WorkerStop::ActionProperty,2112),
  "incomplete"=>("x'=1","",WorkerStop::Incomplete,2109),
  "deadlock"=>("x<0 /\\ x'=x /\\ y'=y","",WorkerStop::Deadlock,2114),
  "prefix-evaluation"=>("x'=(IF x<3 THEN x+1 ELSE 1 \\div 0) /\\ y'=y","",WorkerStop::Evaluation,2179),
  "prefix-invariant"=>("x'=x+1 /\\ y'=y","INVARIANT PrefixInv",WorkerStop::Invariant,2110),
  _=>return Err("unknown declared worker case".into()),
 };
 let source=format!("---- MODULE WorkerRuntime ----\nEXTENDS Integers\nVARIABLES x,y\nInit == x=0 /\\ y=0\nNext == {next}\nInv == x=0\nPrefixInv == x<4\nAction == [][x'=x]_<<x,y>>\nLive == <>(x=0)\nHook == {}\n====\n",if hook=="false"{"FALSE"}else{"TRUE"});
 let config=format!("INIT Init\nNEXT Next\nPROPERTY Live\n{role}\n{}",if hook=="absent"{""}else{"_PERIODIC Hook\n"});
 let a=dive_sema::analyze_source("WorkerRuntime.tla",&source,&dive_sema::Options::default());let ir=Arc::new(dive_ir::lower(&a).map_err(|e|e.to_string())?);let cfg=dive_config::parse(&config).into_config().ok_or("configuration")?;
 let options=dive_check::CheckOptions{workers:width,chaos:Some(7),..dive_check::CheckOptions::tlc(0,0)};
 let policy=periodic_lifecycle::Options{force:forced,..periodic_lifecycle::Options::default()};let requests=dive_check::progress::Options{interval:Duration::from_secs(3600),force:false};let limits=phase_graph::Limits::default();
 let old=periodic_candidate::check_optional_terminal(ir.clone(),cfg.clone(),options.clone(),limits,policy,requests).map_err(|e|e.to_string())?;
 let r=periodic_candidate::check_optional_stopped(ir.clone(),cfg.clone(),options,limits,policy,requests).map_err(|e|e.to_string())?;
 let run=r.run().lifecycle();let t=run.stopped_service().ok_or("actual worker stop")?;let stopped=run.backend().stop().ok_or("actual original error")?;
 let phase_graph::Error::Capture(result)=&stopped.error else{return Err("must preserve source result".into());};
 let phase_graph::Error::Capture(baseline)=&old.run().lifecycle().backend().stop().ok_or("baseline source error")?.error else{return Err("baseline error".into());};
 assert_eq!(result.verdict,baseline.verdict);assert_eq!(result.stats,baseline.stats);assert_eq!(result.output,baseline.output);
 assert_eq!((t.kind(),t.code()),(expected,code));assert_eq!(t.pool().0,width);assert_eq!(t.pool().1==0,width==1);assert!(t.joined_stats().is_some());assert_eq!(t.joined_stats(),result.stats);assert!(!t.model_proof());
 let prefix=kind.starts_with("prefix-");assert_eq!(t.source(),if prefix{3}else{0});assert_eq!(t.graph_source(),if prefix{Some(2)}else{None});
 assert!(run.terminal_service().is_none());assert!(run.safety_eof().is_none());assert!(run.final_checks().is_empty());assert!(run.request_services().is_empty());assert!(run.service_evaluations().is_empty());assert_eq!(run.stopped_faults().count(),0);assert!(r.witness().is_none());assert!(!r.model_proof());
 assert_eq!(r.terminal(),&Terminal::OrderedSourceStop { source: t.source(), kind:expected });assert!(run.finalization().is_none());assert_eq!(t.outcome(),&Ok(dive_liveness::periodic::Outcome::Complete));
 if forced{assert!(t.suspension_attempted());assert_eq!(t.force(),(true,true));}
 let mut record=json!({"kind":kind,"hook":hook,"requested_width":width,"forced":forced,"source":source,"configuration":config,"execution_retained":true,"model_proof":false,"ordinary_schedule_certified":false,"limits":format!("{limits:?}"),"stopped_service":{"kind":format!("{:?}",t.kind()),"code":t.code(),"source":t.source(),"prejoin_stats":format!("{:?}",t.stats()),"joined_stats":format!("{:?}",t.joined_stats()),"pool":t.pool(),"graph_source":t.graph_source(),"suspension_attempted":t.suspension_attempted(),"pending_request":format!("{:?}",t.pending_request()),"outcome":format!("{:?}",t.outcome()),"model_proof":false},"stopped_faults":[],"original_verdict":format!("{:?}",result.verdict),"original_stats":format!("{:?}",result.stats),"original_output":format!("{:?}",result.output),"baseline_verdict":format!("{:?}",baseline.verdict),"baseline_stats":format!("{:?}",baseline.stats),"baseline_output":format!("{:?}",baseline.output)});
 if let Some(cx)=&result.counterexample{record["original_trace"]=json!(format!("{cx:?}"));}
 let name=format!("worker-{kind}-{hook}-force{forced}-w{width}.json");std::fs::write(dir.join(&name),serde_json::to_vec_pretty(&record).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;println!("{name}");Ok(())
}

fn run_post(dir:&Path,kind:&str,width:usize)->Result<(),String>{
 let live=if ["post-candidate","post-cached-candidate","post-init","post-unbounded","post-effect"].contains(&kind){"<>(x=2)"}else{"<>(x=0)"};
 let post=if ["post-cached-pass","post-cached-candidate"].contains(&kind){"FALSE"}else if kind=="post-unbounded"{"x=x /\\ SUBSET (1..1000000000)={}"}else if kind=="post-effect"{"PrintT(TRUE)"}else{"x=x"};
 let init=if kind=="post-init"{"x=1 \\div 0"}else{"x=0"};
 let source=format!("---- MODULE PostRuntime ----\nEXTENDS Naturals,TLC\nVARIABLE x\nInit == {init}\nNext == x'=1\nInv == x=0\nLive == {live}\nPost == {post}\n====\n");
 let config=format!("INIT Init\nNEXT Next\nPROPERTY Live\nPOSTCONDITION Post\n{}",if kind=="post-source"{"INVARIANT Inv\n"}else{""});
 let a=dive_sema::analyze_source("PostRuntime.tla",&source,&dive_sema::Options::default());let ir=Arc::new(dive_ir::lower(&a).map_err(|e|e.to_string())?);let cfg=dive_config::parse(&config).into_config().ok_or("cfg")?;
 let mut limits=phase_graph::Limits::default();if kind=="post-budget"{limits.graph.steps=16;}
 let result=periodic_candidate::check_optional_stopped(ir.clone(),cfg.clone(),dive_check::CheckOptions{workers:width,chaos:Some(7),..dive_check::CheckOptions::tlc(0,0)},limits,periodic_lifecycle::Options::default(),dive_check::progress::Options{interval:Duration::from_secs(3600),force:false});
 let mut record=json!({"kind":kind,"source":source,"configuration":config,"requested_width":width,"model_proof":false,"ordinary_schedule_certified":false,"limits":format!("{limits:?}")});
 match result {
  Err(error)=>{assert_eq!(kind,"post-effect");assert!(error.run.is_none());record["execution_retained"]=json!(false);record["error"]=json!(error.to_string());},
  Ok(report)=>{
   assert!(!report.model_proof());let run=report.run().lifecycle();record["execution_retained"]=json!(true);record["terminal_classification"]=json!(format!("{:?}",report.terminal()));record["finalization"]=json!(format!("{:?}",run.finalization()));record["postcondition_failure"]=json!(format!("{:?}",run.postcondition_failure()));record["render_post_failure"]=json!(format!("{:?}",run.counterexample_postcondition_failure()));record["backend"]=json!(format!("{:?}",run.backend()));record["eof"]=json!(format!("{:?}",run.safety_eof()));
   match kind{
    "post-candidate"=>{assert_eq!(report.terminal(),&Terminal::EofWitness);assert!(run.finalization().ok_or("final graph")?.candidate().is_some());assert!(run.postcondition_failure().is_none());assert_eq!(run.counterexample_postcondition_failure().ok_or("actual render return")?.verdict.tlc().ok_or("class")?.code,2405);},
    "post-final-error"|"post-cached-pass"=>{let code=if kind=="post-final-error"{2405}else{1000};assert_eq!(report.terminal(),&Terminal::PostconditionFailure{code:Some(code)});assert!(run.finalization().ok_or("final graph")?.all_checked());let post=run.postcondition_failure().ok_or("deciding post")?;assert_eq!(post.stats.ok_or("actual stats")?.distinct,2);assert_eq!(post.workers.used,width);},
    "post-source"=>{assert_eq!(report.terminal(),&Terminal::OrderedSourceStop{source:0,kind:dive_check::capture::WorkerStop::Invariant});assert!(run.finalization().is_none());assert!(run.postcondition_failure().is_none());},
    _=>{assert_eq!(report.terminal(),&Terminal::Incomplete);assert!(report.witness().is_none());assert!(run.postcondition_failure().is_none());assert!(!run.postconditions_passed());if kind=="post-cached-candidate"{assert!(run.finalization().ok_or("actual graph")?.candidate().is_some());}else{assert!(run.finalization().is_none_or(|f|!f.all_checked()));}}
   }
   if let Some(w)=report.witness(){
    assert_eq!(w.memory_validation().validation,dive_trace::Validation::Valid);assert_eq!(w.json_validation().validation,dive_trace::Validation::Valid);record["document"]=w.document().to_json();record["memory_validation"]=json!(format!("{:?}",w.memory_validation()));record["json_validation"]=json!(format!("{:?}",w.json_validation()));
    let fresh=dive_liveness::periodic_snapshot::bind_optional(ir,&cfg).map_err(|e|e.to_string())?;let mut mutants=vec![];
    for mode in ["init","claim","next","shape"]{
     let mut d=w.document().clone();match mode{
      "init"=>{d.states[0].insert("x".into(),json!(99));},
      "claim"=>d.properties=vec!["Unknown".into()],
      "next"=>{let mut state=d.states[0].clone();state.insert("x".into(),json!(99));d.states.push(state);d.labels.push(None);d.length=Some(d.states.len());d.ending=Some(dive_trace::Ending::Lasso{back_to:0});},
      _=>d.ending=Some(dive_trace::Ending::Lasso{back_to:d.states.len()})
     }
     let t=dive_trace::Trace::decode(fresh.model(),&d).map_err(|e|e.detail)?;let memory=dive_trace::validate(fresh.model(),&t,None);let parsed=dive_trace::TraceDoc::parse(&serde_json::to_string(&d.to_json()).map_err(|e|e.to_string())?)?;let json=dive_trace::validate_doc(fresh.model(),&parsed,&cfg.model_values,None).0;assert!(matches!(memory.validation,dive_trace::Validation::Invalid(_)));assert!(matches!(json.validation,dive_trace::Validation::Invalid(_)));mutants.push(json!({"kind":mode,"document":parsed.to_json(),"memory_validation":format!("{memory:?}"),"json_validation":format!("{json:?}")}));
    }record["mutants"]=json!(mutants);
   }
  }
 }
 let name=format!("{kind}-w{width}.json");std::fs::write(dir.join(&name),serde_json::to_vec_pretty(&record).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;println!("{name}");Ok(())
}
