//! Exit presentation must not mutate evaluator semantics or historical API contracts.
#[path = "support/agreed.rs"]
mod agreed;
const AGREED: &str = include_str!("fixtures/agreed/exit_conventions.json");
use dive_api::{CheckRequest, ExitConvention};
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scratch() -> Result<Scratch, Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "dive-exit-conventions-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&root)?;
    Ok(Scratch(root))
}
fn request(file: &Path, compat_tlc: bool) -> CheckRequest {
    CheckRequest {
        file: file.to_owned(),
        compat_tlc,
        seed: Some(0),
        fp: Some(0),
        workers: Some(1),
        ..CheckRequest::default()
    }
}
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "phase/class/convention matrix preserves all semantic and API observations"
)]
fn deciding_exits_preserve_api_history_and_semantic_options()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = scratch()?;
    for (name, body, config, native, tlc, code) in [
        (
            "Initial",
            "VARIABLE x\nInit == x=0\nNext == x'=x\nInv == x=1",
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
            10,
            12,
            2107,
        ),
        (
            "Successor",
            "VARIABLE x\nInit == x=0\nNext == x'=1\nInv == x=0",
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
            10,
            12,
            2110,
        ),
        (
            "Action",
            "VARIABLE x\nInit == x=0\nNext == x'=1\nAP == [][x'=x]_x",
            "INIT Init\nNEXT Next\nPROPERTY AP\n",
            10,
            13,
            2112,
        ),
        (
            "Post",
            "VARIABLE x\nInit == x=0\nNext == x'=x\nPost == TLCGet(\"distinct\") = 999",
            "INIT Init\nNEXT Next\nPOSTCONDITION Post\n",
            10,
            10,
            2404,
        ),
        (
            "Assumption",
            "ASSUME FALSE\nVARIABLE x\nInit == x=0\nNext == x'=x",
            "INIT Init\nNEXT Next\n",
            12,
            10,
            2104,
        ),
        (
            "AssumptionError",
            "ASSUME 1 \\div 0 = 0\nVARIABLE x\nInit == x=0\nNext == x'=x",
            "INIT Init\nNEXT Next\n",
            12,
            10,
            2105,
        ),
        (
            "Deadlock",
            "VARIABLE x\nInit == x=0\nNext == x'=x /\\ FALSE",
            "INIT Init\nNEXT Next\n",
            11,
            11,
            2114,
        ),
        (
            "Success",
            "VARIABLE x\nInit == x=0\nNext == x'=x",
            "INIT Init\nNEXT Next\n",
            0,
            0,
            2193,
        ),
    ] {
        let file = scratch.0.join(format!("{name}.tla"));
        std::fs::write(
            &file,
            format!("---- MODULE {name} ----\nEXTENDS Integers, TLC\n{body}\n====\n"),
        )?;
        std::fs::write(file.with_extension("cfg"), config)?;
        for compat in [false, true] {
            let report = dive_api::check_file(&request(&file, compat))?;
            let old = report.to_json();
            if name == "Post" {
                // Only the retired reference engine checked POSTCONDITION
                // (#791, #910); Core refuses it by name.
                assert_eq!(old["verdict"], "unsupported", "{old}");
                assert_eq!(report.exit_code(), 3);
                assert!(
                    old["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("POSTCONDITION")),
                    "{old}"
                );
                continue;
            }
            assert_eq!(old["tlc"]["code"], code, "{name}: {old}");
            assert_eq!(
                report.exit_code(),
                if compat { tlc } else { native },
                "{name}"
            );
            assert_eq!(report.exit_code_for(ExitConvention::Native), native);
            assert_eq!(report.exit_code_for(ExitConvention::Tlc), tlc);
            for convention in [ExitConvention::Native, ExitConvention::Tlc] {
                let doc = report.to_json_with_exit_convention(convention);
                assert_eq!(doc["tlc"], old["tlc"]);
                assert_eq!(doc["statistics"], old["statistics"]);
                assert_eq!(doc["counterexample"], old["counterexample"]);
                for option in [
                    "int",
                    "choose_order",
                    "explore_order",
                    "workers",
                    "seed",
                    "fingerprint",
                    "action_composition",
                ] {
                    assert_eq!(doc["options"][option], old["options"][option]);
                }
                assert_eq!(report.exit_code_for(convention), doc["exit_code"]);
                assert_eq!(
                    doc["options"]["semantic_profile"],
                    if compat { "tlc" } else { "dive" }
                );
                assert_eq!(
                    doc["options"]["profile"],
                    if compat && convention == ExitConvention::Native {
                        "tlc+overrides"
                    } else if compat {
                        "tlc"
                    } else {
                        "dive"
                    }
                );
                assert!(
                    report
                        .render_human_with_exit_convention(convention)
                        .contains(&format!(
                            "process status {}",
                            report.exit_code_for(convention)
                        ))
                );
            }
            assert_eq!(old, report.to_json());
        }
    }
    Ok(())
}
#[test]
fn frontend_config_and_unverified_errors_remain_distinct() -> Result<(), Box<dyn std::error::Error>>
{
    let scratch = scratch()?;
    let file = scratch.0.join("Rejected.tla");
    for source in [
        "---- MODULE Rejected ----\nx == @@@\n====\n",
        "---- MODULE Rejected ----\nx == missing\n====\n",
    ] {
        std::fs::write(&file, source)?;
        for compat in [false, true] {
            let report = dive_api::check_file(&request(&file, compat))?;
            assert_eq!(report.exit_code_for(ExitConvention::Native), 1);
            assert_eq!(report.exit_code_for(ExitConvention::Tlc), 150);
            assert_eq!(
                report.to_json_with_exit_convention(ExitConvention::Tlc)["tlc"]["verified"],
                true
            );
        }
    }
    std::fs::write(
        &file,
        "---- MODULE Rejected ----\nVARIABLE x\nInit == x=0\nNext == x'=x\n====\n",
    )?;
    std::fs::write(file.with_extension("cfg"), "NOT_A_CONFIG_DIRECTIVE\n")?;
    let report = dive_api::check_file(&request(&file, false))?;
    assert_eq!(report.exit_code_for(ExitConvention::Native), 1);
    assert_eq!(report.exit_code_for(ExitConvention::Tlc), 151);
    assert!(report.to_json_with_exit_convention(ExitConvention::Tlc)["tlc"].is_null());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/checker/errors/specs");
    for (name, native, tlc) in [("ErNextAssert", 13, 14), ("ErNextDivZero", 13, 255)] {
        let report = dive_api::check_file(&request(
            &root.join(name).join(format!("{name}.tla")),
            false,
        ))?;
        agreed::check_eq(
            AGREED,
            "frontend_config_and_unverified_errors_remain_distinct/0",
            &(native),
            &(report.exit_code_for(ExitConvention::Native)),
        );
        agreed::check_eq(
            AGREED,
            "frontend_config_and_unverified_errors_remain_distinct/1",
            &(tlc),
            &(report.exit_code_for(ExitConvention::Tlc)),
        );
    }
    let report = dive_api::check_file(&request(&root.join("ErFpTuple/ErFpTuple.tla"), false))?;
    let before = report.to_json();
    assert_eq!(before["tlc"]["verified"], false);
    let mut copied_metadata = report.to_json_with_exit_convention(ExitConvention::Tlc);
    copied_metadata["tlc"]["verified"] = serde_json::json!(true);
    copied_metadata["tlc"]["exit_status"] = serde_json::json!(0);
    let after = report.to_json_with_exit_convention(ExitConvention::Tlc);
    assert_eq!(
        after["tlc"], before["tlc"],
        "caller-owned JSON cannot certify a class"
    );
    assert_ne!(report.exit_code_for(ExitConvention::Tlc), 0);
    Ok(())
}
#[test]
fn selecting_tlc_statuses_does_not_change_big_integers_or_choose()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/probes/specifications");
    for (module, config) in [("P1Overflow", "P1Overflow"), ("P2Choose", "P2Choose_str")] {
        let mut req = request(&root.join(module).join(format!("{module}.tla")), false);
        req.config = Some(root.join(module).join(format!("{config}.cfg")));
        let report = dive_api::check_file(&req)?;
        assert_eq!(report.verdict(), "success");
        assert_eq!(report.exit_code_for(ExitConvention::Tlc), 0);
        assert_eq!(report.to_json()["options"]["int"], "big");
        assert_eq!(report.to_json()["options"]["choose_order"], "canonical");
    }
    Ok(())
}
