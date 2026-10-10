use std::{env, fs, path::Path, sync::Arc};
use dive_check::{CheckOptions, Verdict, capture::{explore, Exploration, Limits}};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args:Vec<_>=env::args().collect();
    let analysis=dive_sema::analyze_file(Path::new(&args[1]),&dive_sema::Options::default())?;
    let ir=Arc::new(dive_ir::lower(&analysis)?);
    let cfg=dive_config::parse(&fs::read_to_string(&args[2])?).into_config().ok_or("config")?;
    let tlc=args[3]=="tlc";
    let mut options=if tlc {CheckOptions::tlc(0,0)} else {CheckOptions::dive(0)};
    options.store=if args[4]=="exact" {dive_store::StoreMode::Exact} else {dive_store::StoreMode::Fingerprint};
    let settings=if tlc {dive_model::BindOptions::tlc()} else {dive_model::BindOptions::dive()};
    let model=dive_model::bind(ir,&cfg,&settings)?;
    let limits=Limits{states:args[5].parse()?,edges:args[6].parse()?,generated:args[7].parse()?, ..Limits::default()};
    match explore(&model,&options,limits) {
        Exploration::Stopped(result)=>{
            let (disposition,feature)=match result.verdict {
                Verdict::ResourceLimit{message} if message.starts_with("behavior graph capture ")=>("resource-limit",message),
                Verdict::Unsupported{feature} if tlc && feature.starts_with("graph capture with stateful checking:") && feature.ends_with("(runtime names require TLC encounter order)")=>("unsupported",feature),
                other=>return Err(format!("not a bounded or named namespace stop: {other:?}").into()),
            };
            let stats=result.stats.as_ref().map(|s|serde_json::json!({"generated":s.generated,"distinct":s.distinct,"depth":s.depth,"left_on_queue":s.left_on_queue}));
            println!("{}",serde_json::json!({"result":disposition,"feature":feature,"model_proved":false,"exhaustive":false,"statistics":stats,"profile":args[3],"store":args[4],"limits":{"states":limits.states,"edges":limits.edges,"generated":limits.generated}}));
        },
        Exploration::Complete(_)=>return Err("unbounded recursive model unexpectedly complete".into()),
    }
    Ok(())
}
