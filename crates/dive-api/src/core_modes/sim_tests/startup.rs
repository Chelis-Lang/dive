//! Simulation binds as `dive check` does in each profile (issue #1142): in
//! the default profile the pure constant definitions Core's roles use only
//! as values are left to Core, in both the hosted and the preparation model;
//! under `--compat=tlc` the binder evaluates them at start-up.
#![allow(clippy::expect_used, reason = "test fixture construction")]

use std::path::PathBuf;
use std::sync::Arc;

use dive_stdlib::random::RandomMode;

fn fixture() -> (PathBuf, PathBuf) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sim-startup");
    (root.join("SimStartup.tla"), root.join("SimStartup.cfg"))
}

#[derive(Debug)]
struct Continue;
impl dive_eval::HostControl for Continue {
    fn check(&self) -> Result<(), dive_eval::Interrupted> {
        Ok(())
    }
}

#[test]
fn simulation_leaves_pure_startup_values_to_core_only_in_the_default_profile() {
    let (spec, cfg) = fixture();
    for (compat_tlc, mode) in [(false, RandomMode::Dive), (true, RandomMode::Tlc)] {
        let runtime = dive_sim::Runtime::new(dive_sim::Options::new(0, mode, 4, 1));
        let model = crate::trace::SpecModel::load_hosted(
            &spec,
            &cfg,
            &crate::trace::TraceOptions {
                compat_tlc,
                run: Some((0, 0)),
                ..crate::trace::TraceOptions::default()
            },
            Some(runtime.host()),
        )
        .expect("hosted model");
        let ring = model.model().ir.root_def("Ring").expect("Ring");
        assert_eq!(
            model.model().deferred_constants.contains(&ring),
            !compat_tlc,
            "hosted, compat={compat_tlc}"
        );
        let prepared = model
            .preparation_model(0, mode, Arc::new(Continue))
            .expect("preparation model");
        assert_eq!(
            prepared.deferred_constants.contains(&ring),
            !compat_tlc,
            "preparation, compat={compat_tlc}"
        );
    }
}

#[test]
fn deferred_startup_values_sample_and_validate_as_in_the_tlc_profile() {
    let (spec, cfg) = fixture();
    let run = |compat_tlc| {
        crate::simulate_file(&crate::SimulateRequest {
            file: spec.clone(),
            config: Some(cfg.clone()),
            seed: Some(0),
            depth: Some(6),
            traces: 3,
            compat_tlc,
            ..Default::default()
        })
        .expect("simulation")
        .to_json()
    };
    let (default, tlc) = (run(false), run(true));
    for json in [&default, &tlc] {
        assert_eq!(json["result"], "sampled", "{json}");
        let traces = json["traces"].as_array().expect("traces");
        assert_eq!(traces.len(), 3, "{json}");
        for trace in traces {
            assert_eq!(trace["validation"]["result"], "valid", "{trace}");
        }
    }
    assert_eq!(default["traces"], tlc["traces"]);
}
