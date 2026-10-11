use std::sync::Arc;
fn main() {
 let a=dive_sema::analyze_source("Order.tla", "---- MODULE Order ----\nVARIABLE x\nUnused == \"zeta\"\nOther == \"alpha\"\nInit == x = \"zeta\"\nR(v) == x' = (CHOOSE a \\in v : TRUE)\nNext == R({\"zeta\",\"alpha\"})\n====\n", &dive_sema::Options::default());
 let ir=Arc::new(dive_ir::lower(&a).unwrap());
 let cfg=dive_config::parse("INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n").into_config().unwrap();
 for ctx in [dive_value::Ctx::tlc(),dive_value::Ctx::dive()] {
 let mut m=dive_model::bind(ir.clone(), &cfg,&dive_model::BindOptions::tlc()).unwrap();
 let z=m.names.intern("zeta");let a=m.names.intern("alpha");
 let v=dive_value::Value::set(vec![dive_value::Value::Str(z),dive_value::Value::Str(a)],ctx).unwrap();
 println!("order {:?}, choose {:?}",ctx.order,dive_value::ops::choose(&v,dive_value::Ctx::tlc(), |_|Ok(true)).unwrap());
 let def=m.ir.root_def("R").unwrap();let s=m.evaluator.scope(def).unwrap(); let s=m.evaluator.scope_bind(&s,dive_ir::Slot(0),v).unwrap();
 m.actions=vec![m.evaluator.predicate_in(&s,m.ir.def(def).body,dive_ir::actions::ActionName::Def(def),m.init.len()).unwrap()]; m.next=Some(dive_model::Pred::new(m.ir.def(def).body,s));
 let r=dive_check::capture::explore_concrete_parallel_with_evidence(&m,&dive_check::CheckOptions::tlc(0,0),dive_check::capture::Limits::default()).unwrap();
 match r {dive_check::capture::Exploration::Complete(c)=>println!("COMPLETED states {:?}",c.graph.states), x=>println!("stopped {:?}",x)}
 }
}
