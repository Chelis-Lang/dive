//! Constructive generation reaches concrete behaviors without enumerating an
//! action's complete candidate vector. Every published word is still replayed.
use std::path::PathBuf;

use dive_api::{Engine, SimulateRequest, simulate_file_engine};
use serde_json::Value;

fn request(config: &str, tlc: bool) -> SimulateRequest {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/generation");
    SimulateRequest {
        file: root.join("Generation.tla"),
        config: Some(root.join(format!("{config}.cfg"))),
        compat_tlc: tlc,
        generation: true,
        seed: Some(7),
        traces: 2,
        depth: Some(6),
        ..Default::default()
    }
}

fn words(report: &Value) -> Vec<Value> {
    report["traces"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|trace| trace["document"]["states"].clone())
        .collect()
}

#[test]
fn thousand_way_actions_emit_only_the_requested_sample_and_replay()
-> Result<(), Box<dyn std::error::Error>> {
    for tlc in [false, true] {
        {
            let engine = Engine::Core;
            let req = request("Wide", tlc);
            let a = simulate_file_engine(&req, engine)?.to_json();
            assert_eq!(a["result"], "sampled", "{a}");
            assert_eq!(a["statistics"]["generated"], 13, "{a}");
            assert_eq!(a["options"]["algorithm"], "constructive");
            assert_eq!(a["exhaustive"], false);
            assert_eq!(a["model_proved"], false);
            assert_eq!(a["options"]["workers_used"], 1);
            for trace in a["traces"].as_array().ok_or("traces")? {
                assert_eq!(trace["validation"]["result"], "valid", "{trace}");
            }
            let b = simulate_file_engine(&req, engine)?.to_json();
            assert_eq!(words(&a), words(&b));
            let mut req = req;
            req.seed = Some(19);
            assert_ne!(
                words(&a),
                words(&simulate_file_engine(&req, engine)?.to_json())
            );
        }
    }
    Ok(())
}

#[test]
fn disabled_or_constraint_rejected_choices_do_not_create_a_false_deadlock()
-> Result<(), Box<dyn std::error::Error>> {
    for tlc in [false, true] {
        {
            let engine = Engine::Core;
            for case in ["Guarded", "Constrained", "Multi", "Tuple", "Stored"] {
                let report = simulate_file_engine(&request(case, tlc), engine)?.to_json();
                assert_eq!(
                    report["result"], "sampled",
                    "{case}/{tlc:?}/{engine:?}: {report}"
                );
                for trace in report["traces"].as_array().ok_or("traces")? {
                    assert_eq!(trace["validation"]["result"], "valid", "{trace}");
                }
            }
        }
    }
    Ok(())
}

#[test]
fn ordinary_queries_do_not_inherit_the_generation_stream() -> Result<(), Box<dyn std::error::Error>>
{
    for tlc in [false, true] {
        {
            let engine = Engine::Core;
            let wide = simulate_file_engine(&request("Wide", tlc), engine)?.to_json();
            for case in ["Quantified", "Queried", "Universal"] {
                let report = simulate_file_engine(&request(case, tlc), engine)?.to_json();
                assert_eq!(report["result"], "sampled", "{case}: {report}");
                assert_eq!(
                    words(&wide),
                    words(&report),
                    "query consumed generation draws: {case}"
                );
            }
            let chosen = simulate_file_engine(&request("Chosen", tlc), engine)?.to_json();
            assert_eq!(chosen["result"], "sampled", "{chosen}");
            for trace in chosen["traces"].as_array().ok_or("traces")? {
                for state in trace["document"]["states"]
                    .as_array()
                    .ok_or("states")?
                    .iter()
                    .skip(1)
                {
                    assert_eq!(state["values"]["x"], 3);
                }
            }
        }
    }
    Ok(())
}

#[test]
fn generation_violation_is_freshly_proved_and_a_forged_edge_is_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
    for tlc in [false, true] {
        {
            let engine = Engine::Core;
            let req = request("Violation", tlc);
            let report = simulate_file_engine(&req, engine)?.to_json();
            assert_eq!(report["result"], "violation", "{report}");
            let mut word = report["traces"][0]["document"].clone();
            let source = SpecModel::load(
                &req.file,
                req.config.as_ref().ok_or("config")?,
                &TraceOptions {
                    compat_tlc: tlc,
                    run: Some((7, 0)),
                    ..Default::default()
                },
            )?;
            let real = TraceDoc::parse(&word.to_string())?;
            assert!(matches!(
                source.validate(&real).validation,
                Validation::Valid
            ));
            word["states"][1]["values"]["x"] = serde_json::json!(-1);
            let forged = TraceDoc::parse(&word.to_string())?;
            assert!(matches!(
                source.validate(&forged).validation,
                Validation::Invalid(_)
            ));
        }
    }
    Ok(())
}

#[test]
fn unknown_domains_and_partial_rows_never_become_completed_samples()
-> Result<(), Box<dyn std::error::Error>> {
    for tlc in [false, true] {
        {
            let engine = Engine::Core;
            for case in ["Unknown", "Partial"] {
                let report = simulate_file_engine(&request(case, tlc), engine)?.to_json();
                assert_ne!(report["result"], "sampled", "{case}: {report}");
                assert_eq!(report["model_proved"], false);
                assert_eq!(report["options"]["workers_used"], 1);
                if case == "Unknown" {
                    assert_eq!(report["result"], "unsupported", "{report}");
                    assert!(
                        report["message"]
                            .as_str()
                            .is_some_and(|text| text.contains("constructive")),
                        "{report}"
                    );
                }
            }
        }
    }
    Ok(())
}

#[test]
fn indexed_intervals_keep_tlc_cardinality_admission_before_the_body()
-> Result<(), Box<dyn std::error::Error>> {
    {
        let engine = Engine::Core;
        let report = simulate_file_engine(&request("LargeTlc", true), engine)?.to_json();
        // Finite mode still refuses unowned Next error wrappers. The original
        // cardinality demand must occur before any sampled assignment.
        assert_eq!(report["result"], "unsupported", "{report}");
        assert!(
            report["message"]
                .as_str()
                .is_some_and(|message| message.contains("maximum representable size (32bits)")),
            "{report}"
        );
        assert_eq!(report["statistics"]["generated"], 1);
    }
    Ok(())
}

#[test]
fn process_conventions_and_budget_stops_do_not_change_generation_semantics()
-> Result<(), Box<dyn std::error::Error>> {
    use dive_api::{BudgetSetting, ExitConvention};
    for tlc in [false, true] {
        {
            let engine = Engine::Core;
            let report = simulate_file_engine(&request("Violation", tlc), engine)?;
            assert_eq!(report.exit_code_for(ExitConvention::Native), 10);
            assert_eq!(report.exit_code_for(ExitConvention::Tlc), 12);
            for convention in [ExitConvention::Native, ExitConvention::Tlc] {
                let mapped = report.to_json_with_exit_convention(convention);
                assert_eq!(mapped["traces"], report.to_json()["traces"]);
                assert_eq!(mapped["options"]["exit_codes"], convention.as_str());
            }
            let stopped = simulate_file_engine(
                &SimulateRequest {
                    time_limit: BudgetSetting::Explicit(std::time::Duration::ZERO),
                    ..request("Wide", tlc)
                },
                engine,
            )?;
            assert_eq!(stopped.to_json()["result"], "stopped");
            assert_eq!(stopped.exit_code_for(ExitConvention::Native), 5);
            assert_eq!(stopped.exit_code_for(ExitConvention::Tlc), 5);
            assert_eq!(stopped.to_json()["model_proved"], false);
        }
    }
    Ok(())
}
