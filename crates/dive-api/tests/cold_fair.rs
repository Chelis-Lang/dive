//! Verify root selection, independently replayed words and bounded misses.

use dive_api::{CheckRequest, ExitConvention, check_file, cold_fair};
use std::path::PathBuf;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

struct Fixtures(PathBuf);
impl Drop for Fixtures {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn cold_scope_never_turns_a_selected_root_miss_into_a_proof() -> Result {
    let fixtures =
        Fixtures(std::env::temp_dir().join(format!("dive-cold-fair-{}", std::process::id())));
    std::fs::create_dir(&fixtures.0)?;
    let file = fixtures.0.join("Cold.tla");
    std::fs::write(
        &file,
        r"---- MODULE Cold ----
EXTENDS Naturals
VARIABLE x
Init == x \in 0..1
Next == UNCHANGED x
Spec == Init /\ [][Next]_x
P == <>[](x=0)
Allow == x=1
GoodInit == x=1
====",
    )?;
    let cfg = file.with_extension("cfg");
    std::fs::write(
        &cfg,
        "SPECIFICATION Spec\nPROPERTY P\nCHECK_DEADLOCK FALSE\n",
    )?;
    let request = CheckRequest {
        file,
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    };
    let ordinary = check_file(&request)?.to_json();
    assert_eq!(ordinary["verdict"], "liveness-violation");
    assert_eq!(ordinary["statistics"]["initial_generated"], 2);
    let report = cold_fair::check_file(&request)?;
    let result = report.to_json(ExitConvention::Native);
    assert_eq!(result["verdict"], "bounded-inconclusive");
    assert_eq!(result["model_proved"], false);
    assert_eq!(result["execution"]["selected_graph_exhausted"], true);
    assert_eq!(result["statistics"]["exhaustive"], false);
    assert_eq!(result["statistics"]["initial_generated"], 1);
    assert_eq!(result["options"]["seed"], 0);
    assert!(result["tlc"].is_null());
    assert!(
        report
            .render_human(ExitConvention::Native)
            .contains("bounded-inconclusive")
    );
    std::fs::write(
        &cfg,
        "SPECIFICATION Spec\nPROPERTY P\nCONSTRAINT Allow\nCHECK_DEADLOCK FALSE\n",
    )?;
    let report = cold_fair::check_file(&request)?;
    let violation = report.to_json(ExitConvention::Native);
    assert_eq!(violation["verdict"], "liveness-violation");
    assert_eq!(violation["statistics"]["initial_generated"], 2);
    assert_eq!(violation["statistics"]["distinct"], 1);
    assert_eq!(report.exit_code(ExitConvention::Native), 10);
    let trace = &violation["counterexample"]["trace"];
    assert_eq!(trace["states"][0]["values"]["x"], 1);
    std::fs::write(
        &cfg,
        "SPECIFICATION Spec\nPROPERTY P\nINVARIANT GoodInit\nCHECK_DEADLOCK FALSE\n",
    )?;
    let safety = cold_fair::check_file(&request)?.to_json(ExitConvention::Native);
    assert_eq!(safety["verdict"], "invariant-violation");
    assert_eq!(safety["witness_validated"], true);
    assert_eq!(
        safety["counterexample"]["trace"]["states"][0]["values"]["x"],
        0
    );
    Ok(())
}
