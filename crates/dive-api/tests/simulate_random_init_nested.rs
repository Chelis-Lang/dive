//! Random Init keeps nested ENABLED successor semantics and independent replay.
#[path = "support/refused.rs"]
mod refused;
use dive_api::SimulateRequest;
use dive_api::trace::{Check, SpecModel, TraceDoc, TraceOptions, Validation};
use serde_json::json;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn random_init_with_nested_queries_keeps_words_and_refuses_random_enabled() -> TestResult {
    let directory = std::env::temp_dir().join(format!(
        "dive-sim-random-nested-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&directory)?;
    let result = (|| -> TestResult {
        let source = r"---- MODULE RandomNested ----
EXTENDS Integers,TLC
VARIABLE x
Init == x=RandomElement({0,1})
Next == ~ENABLED (x'=x /\ x=2) /\ x'=1-x
Safe == x \in {0,1}
====
";
        let config = "INIT Init\nNEXT Next\nINVARIANT Safe\n";
        let file = directory.join("RandomNested.tla");
        std::fs::write(&file, source)?;
        std::fs::write(file.with_extension("cfg"), config)?;
        for compat_tlc in [false, true] {
            let request = SimulateRequest {
                file: file.clone(),
                compat_tlc,
                seed: Some(0),
                depth: Some(3),
                traces: 2,
                ..Default::default()
            };
            // Only the retired reference engine sampled this draw (#791, #910).
            let report = dive_api::simulate_file(&request)?;
            refused::assert_refused(
                &report.to_json(),
                report.exit_code(),
                refused::FINITE_RANDOM,
            );
            let fresh = SpecModel::from_source(
                "RandomNested.tla",
                source,
                config,
                &TraceOptions {
                    compat_tlc,
                    ..Default::default()
                },
            )?;
            // The independent validator still accepts the nested-query word
            // and rejects forged Init and Next states.
            let word = TraceDoc::behavior(vec![
                json!({"x":0}),
                json!({"x":1}),
                json!({"x":0}),
                json!({"x":1}),
            ])?;
            assert_eq!(fresh.validate(&word).validation, Validation::Valid);
            for (index, check) in [(0, Check::Init), (1, Check::Next)] {
                let mut forged = word.clone();
                forged.states[index]["x"] = json!(9);
                assert!(matches!(fresh.validate(&forged).validation,
                    Validation::Invalid(f) if f.check == check));
            }
            let alternate = TraceDoc::behavior(vec![json!({"x":0}), json!({"x":1})])?;
            assert_eq!(fresh.validate(&alternate).validation, Validation::Valid);
            let reached =
                source.replace("~ENABLED (x'=x /\\ x=2)", "ENABLED (x'=RandomElement({x}))");
            std::fs::write(&file, reached)?;
            let refused = dive_api::simulate_file(&request)?.to_json();
            assert_eq!(refused["result"], "unsupported");
            assert!(
                refused["message"]
                    .as_str()
                    .is_some_and(|s| s.contains("RandomElement"))
            );
            std::fs::write(&file, source)?;
        }
        Ok(())
    })();
    std::fs::remove_dir_all(directory)?;
    result
}
