use std::sync::Arc;
fn main() {
 let a=dive_sema::analyze_source("M.tla", "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=x\n====\n", &Default::default());
 let source=dive_ir::lower(&a).unwrap();
 let mut json=serde_json::to_value(&source).unwrap();
 json["vars"]=serde_json::json!([u32::MAX]);
 let ir=Arc::new(serde_json::from_value(json).unwrap());
 let cfg=dive_config::parse("INIT Init\nNEXT Next\n").into_config().unwrap();
 let opts=dive_check::CheckOptions::tlc(0,7);
 let request=dive_check::checkpoint::Request{path:"".into(),action:dive_check::checkpoint::Action::PauseAfterLevels(0),counters:dive_check::checkpoint::Counters::Tlc,config_source_sha256:None};
 let native=std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dive_check::checkpoint::check(&ir,&cfg,&opts,&request)));
 println!("native malformed variable escaped panic: {}", native.is_err());
 let ordinary=std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dive_check::check(&ir,&cfg,&opts)));
 println!("ordinary malformed variable escaped panic: {}", ordinary.is_err());
}
