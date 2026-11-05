//! Product ranges and nested literal enumeration retain receiver authority.
use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_api::{CheckRequest, Engine, ExitConvention, check_file_core_opts, check_file_engine};
use serde_json::{Value, json};
use std::path::PathBuf;

fn facts(report: &dive_api::CheckReport) -> Value {
    let doc = report.to_json();
    json!({"verdict": doc["verdict"], "property": doc["property"],
        "statistics": doc["statistics"], "tlc": doc["tlc"],
        "exit": report.exit_code_for(ExitConvention::Tlc)})
}

fn request(name: &str, compat_tlc: bool) -> CheckRequest {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/core-receiver-consumers")
        .join(format!("{name}.tla"));
    CheckRequest {
        config: Some(file.with_extension("cfg")),
        file,
        compat_tlc,
        workers: Some(1),
        seed: Some(0),
        fp: Some(0),
        ..Default::default()
    }
}

#[test]
fn product_sequence_ranges_and_mapped_subsets_match_reference()
-> Result<(), Box<dyn std::error::Error>> {
    for name in [
        "SeqProduct",
        "SeqProductSkipRaw",
        "MappedSubsets",
        "MappedEmptySubsets",
    ] {
        for compat_tlc in [false, true] {
            let request = request(name, compat_tlc);
            let reference = check_file_engine(&request, Engine::Reference)?;
            assert_eq!(reference.verdict(), "success");
            for switches in ["", "-fold", "-memo", "-hashcons", "-symbolic-member"] {
                let core = check_file_core_opts(&request, switches)?;
                assert_eq!(core.to_json()["engine"], "core");
                assert_eq!(
                    facts(&core),
                    facts(&reference),
                    "{name}/{compat_tlc}/{switches}"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn product_non_tuple_errors_and_demanded_mixed_query_refusals_remain()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        let request = request("SeqProductNonTuple", compat_tlc);
        let reference = check_file_engine(&request, Engine::Reference)?;
        assert_eq!(reference.verdict(), "evaluation-error");
        assert_eq!(reference.to_json()["tlc"]["verified"], false);
        for switches in ["", "-fold", "-memo", "-hashcons", "-symbolic-member"] {
            let core = check_file_core_opts(&request, switches)?;
            assert_eq!(facts(&core), facts(&reference));
            assert_eq!(core.to_json()["message"], reference.to_json()["message"]);
        }
        let request = self::request("MappedMixedSubsets", compat_tlc);
        let reference = check_file_engine(&request, Engine::Reference)?;
        assert_eq!(reference.verdict(), "evaluation-error");
        for switches in ["", "-fold", "-memo", "-hashcons"] {
            let core = check_file_core_opts(&request, switches)?;
            let doc = core.to_json();
            assert_eq!(doc["verdict"], "unsupported", "{doc}");
            assert!(
                doc["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("local scalar/tuple comparison"))
            );
        }
    }
    Ok(())
}

#[test]
fn mapped_subset_violation_words_validate_and_mutations_fail()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        let request = request("MappedSubsetsViolation", compat_tlc);
        let reference = check_file_engine(&request, Engine::Reference)?;
        assert_eq!(reference.verdict(), "invariant-violation");
        let model = SpecModel::load(
            &request.file,
            &request.file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc,
                run: Some((0, 0)),
                ..Default::default()
            },
        )?;
        for workers in [1, 4] {
            let request = CheckRequest {
                workers: Some(workers),
                ..request.clone()
            };
            for switches in ["", "-fold,-memo,-hashcons", "-symbolic-member"] {
                let core = check_file_core_opts(&request, switches)?;
                assert_eq!(facts(&core), facts(&reference));
                let doc = core.to_json();
                let word = TraceDoc::from_json(&doc["counterexample"]["trace"])?;
                assert!(matches!(
                    model.validate(&word).validation,
                    Validation::Valid
                ));
                let mut mutation = doc["counterexample"]["trace"].clone();
                mutation["states"][1]["values"]["x"] = json!(3);
                let mutation = TraceDoc::from_json(&mutation)?;
                assert!(matches!(
                    model.validate(&mutation).validation,
                    Validation::Invalid(_)
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn an_outer_collection_of_multiple_raw_sets_keeps_its_authority_refusal()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        let request = request("MappedMultipleSubsets", compat_tlc);
        assert_eq!(
            check_file_engine(&request, Engine::Reference)?.verdict(),
            "success"
        );
        for switches in ["", "-fold", "-memo", "-hashcons", "-symbolic-member"] {
            let core = check_file_core_opts(&request, switches)?;
            let doc = core.to_json();
            assert_eq!(doc["verdict"], "unsupported", "{doc}");
            assert!(
                doc["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("set of values of different shapes"))
            );
        }
    }
    Ok(())
}

#[test]
fn sequence_finiteness_uses_original_range_methods_and_preserves_alias_demand()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        for name in [
            "SeqProductFiniteEmpty",
            "SeqProductFiniteSkipRaw",
            "SeqProductFiniteSubset",
            "SeqProductFiniteNestedInfinite",
        ] {
            let request = request(name, compat_tlc);
            let reference = check_file_engine(&request, Engine::Reference)?;
            assert_eq!(reference.verdict(), "success");
            if name == "SeqProductFiniteEmpty" {
                // Independent pinned TLC review for #780: SUCCESS 2193/0,
                // one distinct state, two generated and depth one.
                let doc = reference.to_json();
                assert_eq!(doc["tlc"]["code"], 2193);
                assert_eq!(doc["statistics"]["distinct"], 1);
                assert_eq!(doc["statistics"]["generated"], 2);
                assert_eq!(doc["statistics"]["depth"], 1);
            }
            for switches in ["", "-fold", "-memo", "-hashcons", "-symbolic-member"] {
                let core = check_file_core_opts(&request, switches)?;
                assert_eq!(
                    facts(&core),
                    facts(&reference),
                    "{name}/{compat_tlc}/{switches}"
                );
            }
        }
        for name in ["SeqProductFiniteMalformed", "SeqProductFiniteDemandRaw"] {
            let request = request(name, compat_tlc);
            let reference = check_file_engine(&request, Engine::Reference)?;
            assert_eq!(reference.verdict(), "evaluation-error");
            for switches in ["", "-fold,-memo,-hashcons,-symbolic-member"] {
                let core = check_file_core_opts(&request, switches)?;
                let doc = core.to_json();
                assert_eq!(doc["verdict"], "unsupported", "{name}: {doc}");
                let expected = if name == "SeqProductFiniteMalformed" {
                    "unproved factor shape or size"
                } else {
                    "local scalar/tuple comparison"
                };
                assert!(
                    doc["message"]
                        .as_str()
                        .is_some_and(|message| message.contains(expected)),
                    "{name}: {doc}"
                );
            }
        }
        let request = request("SeqProductFiniteUnknownWrapper", compat_tlc);
        assert_eq!(
            check_file_engine(&request, Engine::Reference)?.verdict(),
            "success"
        );
        for switches in ["", "-fold,-memo,-hashcons,-symbolic-member"] {
            let core = check_file_core_opts(&request, switches)?;
            let doc = core.to_json();
            assert_eq!(doc["verdict"], "unsupported", "{doc}");
            assert!(
                doc["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("unproved constructor"))
            );
        }
    }
    Ok(())
}

#[test]
fn negating_empty_range_finiteness_cannot_report_a_false_holds()
-> Result<(), Box<dyn std::error::Error>> {
    for compat_tlc in [false, true] {
        let request = request("SeqProductFiniteEmptyNegated", compat_tlc);
        let reference = check_file_engine(&request, Engine::Reference)?;
        assert_eq!(reference.verdict(), "invariant-violation");
        let model = SpecModel::load(
            &request.file,
            &request.file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc,
                run: Some((0, 0)),
                ..Default::default()
            },
        )?;
        for workers in [1, 4] {
            let request = CheckRequest {
                workers: Some(workers),
                ..request.clone()
            };
            for switches in ["", "-fold,-memo,-hashcons,-symbolic-member"] {
                let core = check_file_core_opts(&request, switches)?;
                assert_eq!(facts(&core), facts(&reference));
                let doc = core.to_json();
                let word = TraceDoc::from_json(&doc["counterexample"]["trace"])?;
                assert!(matches!(
                    model.validate(&word).validation,
                    Validation::Valid
                ));
                let mut mutation = doc["counterexample"]["trace"].clone();
                mutation["states"][0]["values"]["x"] = json!(1);
                let mutation = TraceDoc::from_json(&mutation)?;
                assert!(matches!(
                    model.validate(&mutation).validation,
                    Validation::Invalid(_)
                ));
            }
        }
    }
    Ok(())
}
