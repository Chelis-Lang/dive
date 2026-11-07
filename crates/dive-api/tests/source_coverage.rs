//! Native source work is observed on the producer, independently of proof replay.
#![allow(
    clippy::unwrap_used,
    reason = "test fixture construction and assertions require admitted source rows"
)]
use dive_api::{CheckRequest, Engine, TemporalMode, check_file_engine};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const SOURCE: &str = "---- MODULE Coverage ----\nEXTENDS Naturals,TLC\nVARIABLE x\nC == 1+2\nASSUME C=3\nInit == x=0\nNext == \\/ /\\ x<2 /\\ x'=x+1\n        \\/ /\\ x=2 /\\ x'=x\nInv == x<=2\nUnused == x=99\n====\n";
const CONFIG: &str = "INIT Init\nNEXT Next\nINVARIANT Inv\n";
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Project {
    dir: PathBuf,
    file: PathBuf,
}
impl Project {
    fn new(source: &str, config: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "dive-source-coverage-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Coverage.tla");
        fs::write(&file, source).unwrap();
        fs::write(file.with_extension("cfg"), config).unwrap();
        Self { dir, file }
    }
    fn request(&self, compat: bool, coverage: bool) -> CheckRequest {
        CheckRequest {
            file: self.file.clone(),
            compat_tlc: compat,
            source_coverage: coverage,
            workers: Some(1),
            seed: Some(7),
            fp: Some(0),
            ..CheckRequest::default()
        }
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
fn row<'a>(doc: &'a Value, source: &str, expression: &str) -> &'a Value {
    let start = source.rfind(expression).unwrap();
    doc["source_coverage"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["bytes"]["start"] == start && r["bytes"]["end"] == start + expression.len())
        .unwrap()
}
fn total(doc: &Value, counter: &str) -> u64 {
    doc["source_coverage"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["counts"][counter].as_u64().unwrap())
        .sum()
}
#[test]
fn actual_source_counts_preserve_semantics_and_include_startup_and_unvisited_rows() {
    let project = Project::new(SOURCE, CONFIG);
    for compat in [false, true] {
        let plain = check_file_engine(&project.request(compat, false), Engine::Reference)
            .unwrap()
            .to_json();
        assert!(plain.get("source_coverage").is_none());
        let report = check_file_engine(&project.request(compat, true), Engine::Reference).unwrap();
        let native = report.source_coverage().unwrap();
        assert!(!native.active);
        assert!(native.rows.iter().any(|row| row.counts.evaluations > 0));
        let doc = report.to_json();
        for field in [
            "verdict",
            "property",
            "statistics",
            "tlc",
            "counterexample",
            "output",
            "warnings",
        ] {
            assert_eq!(doc[field], plain[field], "{field}: {doc}");
        }
        assert_eq!(doc["source_coverage"]["metric"], "native-source-work");
        assert_eq!(doc["source_coverage"]["tlc_cost_equivalent"], false);
        assert_eq!(doc["source_coverage"]["active"], false);
        assert_eq!(doc["source_coverage"]["overflowed"], false);
        assert_eq!(row(&doc, SOURCE, "1+2")["counts"]["evaluations"], 1);
        assert_eq!(row(&doc, SOURCE, "x<=2")["counts"]["evaluations"], 3);
        assert_eq!(row(&doc, SOURCE, "x=99")["counts"]["evaluations"], 0);
        assert_eq!(total(&doc, "init_emissions"), 1);
        assert_eq!(total(&doc, "next_emissions"), 3);
        let invariant = row(&doc, SOURCE, "x<=2");
        assert_eq!(invariant["module"], "Coverage");
        assert_eq!(invariant["begin"]["line"], 9);
        assert_eq!(invariant["begin"]["column"], 8);
        assert!(report.render_human().contains("Native source work"));
    }
}
#[test]
fn independent_validation_and_presentation_do_not_inflate_producer_counters() {
    let source = SOURCE.replace("Inv == x<=2", "Inv == x<2");
    let project = Project::new(&source, CONFIG);
    for compat in [false, true] {
        let report = check_file_engine(&project.request(compat, true), Engine::Reference).unwrap();
        let first = report.to_json();
        assert_eq!(first["verdict"], "invariant-violation");
        assert_eq!(total(&first, "init_emissions"), 1);
        assert_eq!(total(&first, "next_emissions"), 2);
        assert_eq!(row(&first, &source, "x<2")["counts"]["evaluations"], 3);
        let model = dive_api::trace::SpecModel::load(
            &project.file,
            &project.file.with_extension("cfg"),
            &dive_api::trace::TraceOptions {
                compat_tlc: compat,
                run: Some((7, 0)),
                ..Default::default()
            },
        )
        .unwrap();
        let word = dive_api::trace::TraceDoc::from_json(&first).unwrap();
        assert_eq!(
            model.validate(&word).validation,
            dive_api::trace::Validation::Valid
        );
        let _ = report.render_human();
        let second = report.to_json();
        assert_eq!(first["source_coverage"], second["source_coverage"]);
    }
}

#[test]
fn failing_demands_retain_the_original_error_and_count_only_producer_entries() {
    let expression = "2 \\div (2-x) >= 0";
    let source = SOURCE.replace("Inv == x<=2", &format!("Inv == {expression}"));
    let project = Project::new(&source, CONFIG);
    for compat in [false, true] {
        let plain = check_file_engine(&project.request(compat, false), Engine::Reference)
            .unwrap()
            .to_json();
        let report = check_file_engine(&project.request(compat, true), Engine::Reference).unwrap();
        let observed = report.to_json();
        assert_eq!(observed["verdict"], "evaluation-error");
        for field in ["verdict", "property", "statistics", "tlc", "counterexample"] {
            assert_eq!(observed[field], plain[field], "{field}: {observed}");
        }
        assert_eq!(
            row(&observed, &source, expression)["counts"]["evaluations"],
            3
        );
        assert_eq!(total(&observed, "init_emissions"), 1);
        assert_eq!(total(&observed, "next_emissions"), 2);
        let _ = report.render_human();
        assert_eq!(
            observed["source_coverage"],
            report.to_json()["source_coverage"]
        );
    }
}
#[test]
fn stops_report_only_the_actual_prefix_and_the_default_engine_refuses_by_name() {
    let project = Project::new(SOURCE, CONFIG);
    for compat in [false, true] {
        let mut request = project.request(compat, true);
        request.max_states = Some(2);
        request.workers = Some(4);
        assert!(matches!(
            dive_api::check_file(&request),
            Err(dive_api::ApiError::Unsupported { feature })
                if feature.contains("source coverage") && feature.contains("--engine reference")
        ));
        let observed = check_file_engine(&request, Engine::Reference)
            .unwrap()
            .to_json();
        let plain = check_file_engine(
            &CheckRequest {
                source_coverage: false,
                ..request
            },
            Engine::Reference,
        )
        .unwrap()
        .to_json();
        assert_eq!(observed["verdict"], "stopped");
        assert_eq!(observed["statistics"], plain["statistics"]);
        assert_eq!(observed["source_coverage"]["scope"], "producer-prefix");
        assert_eq!(observed["source_coverage"]["active"], false);
        assert!(observed.get("engine").is_none());
        assert!(observed.get("engine_fallback").is_none());
        assert!(observed.get("core").is_none());
        assert_eq!(observed["options"]["workers"]["requested"], 4);
        assert_eq!(observed["options"]["workers"]["used"], 1);
        assert!(
            observed["options"]["workers"]["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("source coverage"))
        );
        assert!(total(&observed, "next_steps") > 0);
    }
}
#[test]
fn unsupported_modes_refuse_before_model_loading_and_temporal_obligations_never_pass_as_safety() {
    {
        let engine = Engine::Core;
        let request = CheckRequest {
            file: "missing-coverage-input.tla".into(),
            source_coverage: true,
            temporal: if engine == Engine::Core {
                TemporalMode::Ordinary
            } else {
                TemporalMode::FinalShort
            },
            ..Default::default()
        };
        assert!(matches!(
            check_file_engine(&request, engine),
            Err(dive_api::ApiError::Unsupported { .. })
        ));
    }
    let project = Project::new(
        &SOURCE.replace("Unused == x=99", "Live == <> (x=99)"),
        &format!("{CONFIG}PROPERTY Live\n"),
    );
    let doc = check_file_engine(&project.request(false, true), Engine::Reference)
        .unwrap()
        .to_json();
    assert_eq!(doc["verdict"], "unsupported");
    assert!(doc["counterexample"].is_null());
    assert_eq!(doc["source_coverage"]["active"], false);
}

#[test]
fn compiled_enabled_queries_refuse_coverage_before_startup_instead_of_claiming_zero_work() {
    let source = SOURCE.replace("Inv == x<=2", "Inv == x<=2 /\\ ENABLED (x'=x)");
    let project = Project::new(&source, CONFIG);
    for compat in [false, true] {
        let plain = check_file_engine(&project.request(compat, false), Engine::Reference)
            .unwrap()
            .to_json();
        assert_eq!(plain["verdict"], "success", "{plain}");
        let observed = check_file_engine(&project.request(compat, true), Engine::Reference)
            .unwrap()
            .to_json();
        assert_eq!(observed["verdict"], "unsupported");
        assert!(observed["message"].as_str().unwrap().contains("ENABLED"));
        assert!(observed["statistics"].is_null());
        assert!(observed.get("source_coverage").is_none());
    }
}

#[test]
fn utf8_ranges_and_javacc_tabs_and_supplementary_columns_stay_exact() {
    let source = SOURCE.replace("C == 1+2", "C == 1+2\n\tText == \"😀\"");
    let project = Project::new(&source, CONFIG);
    let doc = check_file_engine(&project.request(false, true), Engine::Reference)
        .unwrap()
        .to_json();
    assert_eq!(doc["verdict"], "success", "{doc}");
    let text = row(&doc, &source, "\"😀\"");
    assert_eq!(text["begin"]["line"], 5);
    assert_eq!(text["begin"]["column"], 17);
    assert_eq!(text["end"]["line"], 5);
    assert_eq!(text["end"]["column"], 20);
    assert_eq!(
        text["bytes"]["end"].as_u64().unwrap() - text["bytes"]["start"].as_u64().unwrap(),
        6
    );
}

#[test]
fn nested_instances_preserve_static_origins_without_claiming_a_dynamic_cost_tree() {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../conformance/corpus/core-instance-scopes/specifications/ScopeHolding/ScopeHolding.tla",
    );
    let inner = fs::read_to_string(file.with_file_name("ScopeInner.tla")).unwrap();
    let start = inner.find("2 * v").unwrap();
    for compat_tlc in [false, true] {
        let report = check_file_engine(
            &CheckRequest {
                file: file.clone(),
                source_coverage: true,
                compat_tlc,
                seed: Some(7),
                fp: Some(0),
                ..CheckRequest::default()
            },
            Engine::Reference,
        )
        .unwrap();
        assert_eq!(report.verdict(), "success");
        let snapshot = report.source_coverage().unwrap();
        let contexts: Vec<_> = snapshot
            .rows
            .iter()
            .filter(|row| {
                row.location.module == "ScopeInner"
                    && usize::try_from(row.location.bytes.0).unwrap() == start
                    && usize::try_from(row.location.bytes.1).unwrap() == start + "2 * v".len()
                    && row.counts.evaluations > 0
            })
            .map(|row| {
                (
                    row.location.instance.as_deref().unwrap(),
                    row.counts.evaluations,
                )
            })
            .collect();
        // A and B both reach this static inner definition. Separating their
        // dynamic calling paths requires the future logical cost-context tree.
        assert_eq!(contexts, [("I!Double", 6)]);
    }
}
