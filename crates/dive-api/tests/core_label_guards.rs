//! Leading process-label checks preserve action occurrences and first failures.
use dive_api::{CheckRequest, check_file_core_opts};
use std::path::PathBuf;

#[test]
fn ordered_labels_keep_complete_observables() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core-label-guards");
    for compat_tlc in [false, true] {
        for workers in [1, 4] {
            for (case, generated) in [
                ("Ordered", 10),
                ("Partial", 6),
                ("Shadow", 19),
                ("Violation", 2),
                ("Error", 1),
                ("NonString", 0),
                ("Effects", 19),
            ] {
                let request = CheckRequest {
                    config: Some(root.join(format!("{case}.cfg"))),
                    file: root.join("OrderedLabels.tla"),
                    workers: Some(workers),
                    compat_tlc,
                    seed: Some(0),
                    fp: Some(0),
                    ..CheckRequest::default()
                };
                let ordinary = check_file_core_opts(&request, "-memo")?.to_json();
                let lowered = check_file_core_opts(&request, "")?.to_json();
                for field in [
                    "verdict",
                    "statistics",
                    "property",
                    "error_site",
                    "message",
                    "tlc",
                    "exit_code",
                    "counterexample",
                    "output",
                    "warnings",
                ] {
                    assert_eq!(
                        lowered[field], ordinary[field],
                        "{case}, {workers}, {compat_tlc}, {field}"
                    );
                }
                match case {
                    "Violation" => assert_eq!(lowered["property"], "NeverOne"),
                    "NonString" if compat_tlc => assert_eq!(lowered["verdict"], "evaluation-error"),
                    "NonString" => (),
                    "Error" => assert_eq!(lowered["verdict"], "evaluation-error"),
                    "Effects" => assert_eq!(lowered["output"].as_array().map(Vec::len), Some(4)),
                    _ => assert_eq!(lowered["verdict"], "success"),
                }
                if generated != 0 {
                    assert_eq!(lowered["statistics"]["generated"], generated);
                }
            }
        }
    }
    Ok(())
}
