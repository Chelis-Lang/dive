use std::sync::Arc;
fn main() {
 let src="---- MODULE M ----\nVARIABLE x\nInit==x=FALSE\nNext==x'=~x\nIdentity(p)==p\nP==Identity([]<>x)\n====\n";
 let a=dive_sema::analyze_source("M.tla",src,&Default::default());
 let ir=Arc::new(dive_ir::lower(&a).unwrap());
 let cfg=dive_config::parse("INIT Init\nNEXT Next\n").into_config().unwrap();
 for bad in ["root","capture","redirect"] {
  let mut m=dive_model::bind(ir.clone(),&cfg,&dive_model::BindOptions::tlc()).unwrap();
  let p=ir.root_def("P").unwrap(); let id=ir.root_def("Identity").unwrap();
  let mut pred=dive_model::Pred::new(ir.def(p).body,m.evaluator.scope(p));
  if bad=="root" {pred.expr=dive_ir::ExprId(u32::MAX);}
  if bad=="capture" {pred=dive_model::Pred::new(ir.def(id).body,m.evaluator.scope_apply(&m.evaluator.scope(p),id,&[dive_ir::Arg::Expr {expr:dive_ir::ExprId(u32::MAX),pass:dive_ir::Pass::Lazy}]).unwrap());}
  if bad=="redirect" {let mut e=dive_eval::Model::new(ir.clone());e.redirects.insert(id,dive_ir::DefId(u32::MAX));m.evaluator=dive_eval::Evaluator::new(e,dive_eval::Options::tlc());pred.scope=m.evaluator.scope(p);}
  let l=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||dive_liveness::plan::lower(&m,&pred,dive_liveness::analyze::Limits::default().formula)));
  let v=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||dive_trace::temporal::evaluate_lasso(&m,&pred,&[Arc::from([Some(false.into())])],0)));
  println!("{bad}: planner_panicked={} validator_panicked={}",l.is_err(),v.is_err());
 }
}
