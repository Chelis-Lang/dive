//! Prove that every adjacent carrier swap preserves the original reachable safety graph.
use std::{env, path::Path};
fn main() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    let [module, cfg, workers, atoms @ ..] = args.as_slice() else {
        return Err("expected MODULE.tla MODEL.cfg WORKERS ATOM ATOM ...".into());
    };
    let workers = workers.parse().map_err(|_| "invalid worker count")?;
    println!(
        "{}",
        dive_api::orbit_graph::certify(Path::new(module), Path::new(cfg), atoms, workers)?
    );
    Ok(())
}
