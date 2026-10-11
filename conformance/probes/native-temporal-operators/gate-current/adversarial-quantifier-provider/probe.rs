use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
#[derive(Debug)] struct Opaque(Arc<AtomicUsize>);
impl dive_value::DeferredSet for Opaque {
 fn kind(&self)->dive_value::DeferredKind{self.0.fetch_add(1,Ordering::Relaxed);dive_value::DeferredKind::Filter}
 fn elements(&self,_:dive_value::Ctx)->dive_value::Result<Vec<dive_value::Value>>{self.0.fetch_add(1,Ordering::Relaxed);Ok(vec![1.into()])}
 fn member(&self,_:&dive_value::Value,_:dive_value::Ctx)->Option<dive_value::Result<bool>>{self.0.fetch_add(1,Ordering::Relaxed);Some(Ok(true))}
 fn key(&self)->Vec<dive_value::Value>{self.0.fetch_add(1,Ordering::Relaxed);vec![]}
}
fn main(){
 let a=dive_sema::analyze_source("Native.tla","---- MODULE Native ----\nEXTENDS Sequences\nVARIABLE x\nInit==x={1}\nNext==UNCHANGED x\nUse(F(_))==[]<>(\\A v \\in x:F(<<v>>)=1)\nLive==Use(Len)\n====\n",&dive_sema::Options::default());assert!(a.accepted(),"{a:?}");let ir=Arc::new(dive_ir::lower(&a).unwrap());
 let cfg=dive_config::parse("INIT Init\nNEXT Next\n").into_config().unwrap();
 for (opt,ctx)in[(dive_model::BindOptions::tlc(),dive_value::Ctx::tlc()),(dive_model::BindOptions::dive(),dive_value::Ctx::dive())]{
 let model=dive_model::bind(ir.clone(),&cfg,&opt).unwrap();let d=ir.root_def("Live").unwrap();let p=dive_model::Pred::new(ir.def(d).body,model.evaluator.scope(d).unwrap());let calls=Arc::new(AtomicUsize::new(0));let val=dive_value::Value::deferred(Arc::new(Opaque(calls.clone())),ctx);let states=vec![Arc::from([Some(val)])];let result=dive_trace::temporal::evaluate_lasso(&model,&p,&states,0);println!("profile {:?} result {:?} callbacks {}",ctx.order,result,calls.load(Ordering::Relaxed));
 }
}
