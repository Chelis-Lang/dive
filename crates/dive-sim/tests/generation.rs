//! Producer work bounds are separate from independent full-word replay costs.
use std::sync::Arc;

use dive_sim::{Options, Outcome, simulate};
use dive_stdlib::random::RandomMode;

#[test]
fn one_million_way_action_keeps_only_each_chosen_successor()
-> Result<(), Box<dyn std::error::Error>> {
    let source = "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x>=0 /\\ (\\E n \\in 1..1000000 : x'=n)\n====\n";
    let parsed = dive_sema::analyze_source("M.tla", source, &dive_sema::Options::default());
    assert!(parsed.accepted());
    let ir = Arc::new(dive_ir::lower(&parsed)?);
    let config = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .ok_or("configuration")?;
    for mode in [RandomMode::Dive, RandomMode::Tlc] {
        let model = dive_model::bind(ir.clone(), &config, &dive_model::BindOptions::tlc())?;
        assert_eq!(
            model.actions.len(),
            1,
            "the probe must not split into startup actions"
        );
        let mut options = Options::new(7, mode, 4, 3);
        options.generation = true;
        options.max_candidates = 1;
        let report = simulate(&model, &options);
        assert!(matches!(report.outcome, Outcome::Sampled), "{report:?}");
        assert_eq!(report.generated, 13);
        assert_eq!(report.traces.len(), 3);
        assert!(report.traces.iter().all(|trace| trace.states.len() == 5));
        options.max_generated = 3;
        let stopped = simulate(&model, &options);
        assert!(matches!(stopped.outcome, Outcome::Limit(_)), "{stopped:?}");
        assert_eq!(stopped.generated, 3);
    }
    Ok(())
}
