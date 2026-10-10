use std::sync::Arc;
fn main() {
 let source=r"---- MODULE Probe ----
EXTENDS Naturals, TLC
Q(p) == \A v \in {f \in [{1} -> BOOLEAN] : TRUE} : v
====
";
 let a=dive_sema::analyze_source("Probe.tla",source,&dive_sema::Options::default());
 assert!(a.accepted(),"{a:?}");
 let ir=Arc::new(dive_ir::lower(&a).unwrap());
 let d=ir.root_def("Q").unwrap();
 println!("Q effects {:?}",ir.node(ir.def(d).body).effects);
 let ev=dive_eval::Evaluator::new(dive_eval::Model::new(ir.clone()),dive_eval::Options::tlc());
 let scope=ev.scope_apply(&ev.scope(d),d,&[dive_ir::Arg::Expr{expr:dive_ir::ExprId(u32::MAX),pass:dive_ir::Pass::Lazy}]).unwrap();
 let result=ev.scope_bindings_bounded(&scope,ir.def(d).body,&dive_eval::St::constant(),4096);
 println!("result {result:?}");
 println!("output {:?}",ev.take_output());
}
