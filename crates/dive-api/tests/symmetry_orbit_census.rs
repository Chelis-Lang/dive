//! Exhaustive labeled controls for the optional orbit measurement.
#![allow(clippy::unwrap_used, reason = "test assertions and temporary fixtures")]

use dive_api::{CheckRequest, Store, check_file_core_opts};
use serde_json::json;

#[test]
fn orbit_totals_match_unreduced_occurrences_and_require_full_state_identities() {
    let root = std::env::temp_dir().join(format!("dive-orbit-census-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("Sets.tla");
    std::fs::write(
        &file,
        r"---- MODULE Sets ----
EXTENDS TLC, FiniteSets
CONSTANT Names
VARIABLE x
Init == x = {} \/ x = {}
Next == (\E n \in Names : x' = x \union {n}) \/ x' = x
Sym == Permutations(Names)
View == Cardinality(x)
====
",
    )
    .unwrap();
    let cfg = root.join("model.cfg");
    let common = "CONSTANT Names = {a, b, c}\nINIT Init\nNEXT Next\n";
    let run = |extra: &str, store, limit, opts, workers| {
        std::fs::write(&cfg, format!("{common}{extra}")).unwrap();
        check_file_core_opts(
            &CheckRequest {
                file: file.clone(),
                config: Some(cfg.clone()),
                workers: Some(workers),
                seed: Some(0),
                fp: Some(0),
                max_states: limit,
                store,
                ..CheckRequest::default()
            },
            opts,
        )
        .unwrap()
        .to_json()
    };
    let original = run("", Store::Exact, None, "", 1);
    assert_eq!(original["statistics"]["distinct"], 8);
    assert_eq!(original["statistics"]["generated"], 34);
    for width in [1, 4] {
        let reduced = run("SYMMETRY Sym\n", Store::Exact, None, "+orbit-census", width);
        let census = &reduced["core"]["metrics"]["orbit_census"];
        assert_eq!(reduced["verdict"], "success");
        assert_eq!(census["representatives"], 4);
        assert_eq!(census["orbit_sizes"], json!({"1": 2, "3": 2}));
        assert_eq!(census["labeled_orbit_union"], "8");
        assert_eq!(census["weighted_generated"], "34");
    }
    let partial = run("SYMMETRY Sym\n", Store::Exact, Some(2), "+orbit-census", 1);
    assert_eq!(partial["verdict"], "stopped");
    assert!(partial["core"]["metrics"].get("orbit_census").is_none());
    for (extra, store) in [
        ("SYMMETRY Sym\nVIEW View\n", Store::Exact),
        ("SYMMETRY Sym\n", Store::Fingerprint),
    ] {
        let plain = run(extra, store, None, "", 1);
        let measured = run(extra, store, None, "+orbit-census", 1);
        assert_eq!(measured["verdict"], plain["verdict"]);
        assert_eq!(measured["statistics"], plain["statistics"]);
        assert_eq!(
            measured["core"]["metrics"]["orbit_census"]["status"],
            "unavailable"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
