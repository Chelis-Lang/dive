use std::sync::Arc;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let source = std::fs::read_to_string(&args[1])?;
    let cfg = dive_config::parse(&std::fs::read_to_string(&args[2])?).into_config().ok_or("valid cfg required")?;
    let analysis = dive_sema::analyze_source(&args[1], &source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let mut captures = 0;
    for profile in [dive_eval::Profile::Dive, dive_eval::Profile::Tlc] {
        for store in [dive_store::StoreMode::Fingerprint, dive_store::StoreMode::Exact] {
            for workers in [1,4,8] {
                for chaos in [None,Some(19)] {
                    let mut options = dive_check::CheckOptions::tlc(0,7);
                    options.profile=profile; options.eval=dive_eval::Options::for_profile(profile);
                    options.tlc_fingerprints=profile==dive_eval::Profile::Tlc;
                    options.store=store; options.workers=workers; options.chaos=chaos;
                    let public = dive_api::check_file(&dive_api::CheckRequest {
                        file: args[1].clone().into(), compat_tlc: profile==dive_eval::Profile::Tlc,
                        store: if store==dive_store::StoreMode::Exact {dive_api::Store::Exact}else{dive_api::Store::Fingerprint},
                        workers:Some(workers),chaos,seed:Some(7),fp:Some(0),..Default::default()
                    })?;
                    let json=public.to_json(); assert_eq!(public.exit_code(),3);
                    assert_eq!(json["verdict"],"unsupported"); assert_eq!(json["options"]["workers"]["used"],0);
                    assert_eq!(json["options"]["workers"]["requested"],workers);
                    assert!(json["statistics"].is_null()&&json["counterexample"].is_null());
                    assert_eq!(json["output"].as_array().map(Vec::len),Some(0));
                    let model=dive_model::bind(ir.clone(),&cfg,&dive_model::BindOptions { eval:options.eval,..dive_model::BindOptions::tlc()})?;
                    for pooled in [false,true] {
                        let value=if pooled {dive_check::capture::explore_concrete_parallel_with_evidence(&model,&options,dive_check::capture::Limits::default())?} else {dive_check::capture::explore(&model,&options,dive_check::capture::Limits::default())};
                        let dive_check::capture::Exploration::Stopped(r)=value else {return Err("unproved capture completed".into())};
                        assert!(matches!(&r.verdict,dive_check::Verdict::Unsupported {feature} if feature=="graph capture with configured native random replacements"));
                        assert_eq!((r.workers.requested,r.workers.used),(workers,0));
                        assert!(r.stats.is_none()&&r.counterexample.is_none());
                        println!("{profile:?} {store:?} w{workers} chaos{chaos:?} pool{pooled}: {r:?}"); captures+=1;
                    }
                }
            }
        }
    }
    println!("24 public API and {captures} supplied-model capture refusals passed");
    Ok(())
}
