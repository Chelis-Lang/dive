use std::sync::Arc;
fn main() {
 let args:Vec<_> = std::env::args().collect();
 let src=std::fs::read_to_string(&args[1]).unwrap();
 let cfg=dive_config::parse(&std::fs::read_to_string(&args[2]).unwrap()).into_config().unwrap();
 let a=dive_sema::analyze_source(&args[1],&src,&dive_sema::Options::default());
 let ir=Arc::new(dive_ir::lower(&a).unwrap());
 for profile in [dive_eval::Profile::Dive,dive_eval::Profile::Tlc] {
  for pooled in [false,true] {
   let mut opts=dive_check::CheckOptions::tlc(0,7);opts.profile=profile;opts.eval=dive_eval::Options::for_profile(profile);opts.tlc_fingerprints=profile==dive_eval::Profile::Tlc;
   let m=dive_model::bind(ir.clone(),&cfg,&dive_model::BindOptions {eval:opts.eval,..dive_model::BindOptions::tlc()}).unwrap();
   println!("profile {profile:?} pooled{pooled} reasons{:?}",m.sequential);
   let r=if pooled {dive_check::capture::explore_concrete_parallel_with_evidence(&m,&opts,Default::default()).unwrap()}else{dive_check::capture::explore(&m,&opts,Default::default())};
   println!("{r:#?}");
  }
 }
}
