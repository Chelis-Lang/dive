#![allow(
    clippy::expect_used,
    reason = "tests exercise real ordinary outcomes and deliberate admission failures"
)]
use super::{Limits, Program};

const SRC: &str = "---- MODULE Test ----\nEXTENDS Naturals\nCONSTANT K\nVARIABLE x\nInit == x = 0\nNext == x' = (x+1) % 3\nP == [](x<K) ~> (x<K)\nQ == []<>(x<K)\n====";
const OLD: &str = "INIT Init\nNEXT Next\nCONSTANT K = 3\nPROPERTY P Q";
const NEW: &str = "INIT Init\nNEXT Next\nCONSTANT K = 3\nPROPERTY P";

fn limits() -> Limits {
    Limits {
        max_states: Some(64),
        max_depth: Some(30),
    }
}
fn program(source: &str, cfg: &str, limits: Limits) -> Result<Program, super::Miss> {
    Program::from_reports(
        &crate::analyze_source("Test.tla", source, &crate::AnalyzeOptions::default()),
        &crate::parse_config_source("Test.cfg", cfg),
        limits,
    )
}
#[test]
fn checks_only_the_actual_first_product_of_the_same_transition_program() {
    let p = program(SRC, OLD, limits()).expect("prior");
    let q = program(SRC, NEW, limits()).expect("current");
    let report = ordinary(SRC, OLD);
    assert_eq!(
        p.first_claim(&q, &report).expect("identical first product"),
        "P"
    );
    assert!(p.first_claim(&p, &report).is_err());
    assert!(q.first_claim(&q, &report).is_err());
    for (src, cfg) in [
        (SRC.to_owned(), NEW.replace("PROPERTY P", "PROPERTY Q")),
        (SRC.to_owned(), NEW.replace("K = 3", "K = 2")),
        (SRC.replace("(x+1) % 3", "(x+2) % 3"), NEW.to_owned()),
        (SRC.replace("P == [](x<K)", "P == [](x=K)"), NEW.to_owned()),
        (SRC.to_owned(), format!("{NEW}\nCHECK_DEADLOCK FALSE")),
    ] {
        if let Ok(edit) = program(&src, &cfg, limits()) {
            assert!(p.first_claim(&edit, &report).is_err());
        }
    }
    for l in [
        Limits {
            max_states: Some(63),
            ..limits()
        },
        Limits {
            max_depth: Some(31),
            ..limits()
        },
    ] {
        assert!(
            p.first_claim(&program(SRC, NEW, l).expect("admitted policy"), &report)
                .is_err()
        );
    }
    for cap in [None, Some(0), Some(65536)] {
        assert!(
            program(
                SRC,
                NEW,
                Limits {
                    max_states: cap,
                    ..limits()
                }
            )
            .is_err()
        );
    }
}
#[test]
fn removing_a_sibling_can_create_a_current_startup_refusal() {
    let source = "---- MODULE Test ----\nVARIABLE x\nInit == x=0\nNext == x'=x\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\nP == (<>(x=0)) => TRUE\nQ == (x=0) ~> (x=0)\n====";
    let cfg = "INIT Init\nNEXT Next\nPROPERTY P Q";
    assert_eq!(ordinary(source, cfg)["verdict"], "success");
    let _previous = program(source, cfg, limits()).expect("qualified previous plan");
    assert_eq!(
        ordinary(source, &cfg.replace("P Q", "P"))["verdict"],
        "unsupported"
    );
    assert!(program(source, &cfg.replace("P Q", "P"), limits()).is_err());
}
#[test]
fn sole_true_temporal_and_no_earlier_safety_or_mixed_roles() {
    let src = SRC.replace("Q == []<>(x<K)", "Q == []<>(x<K)\nS == x=0");
    for cfg in [
        OLD.replace("P Q", "P Q P"),
        format!("{OLD}\nINVARIANT S"),
        format!("{OLD}\nPROPERTY S"),
        format!("{OLD}\nCONSTRAINT S"),
    ] {
        assert!(program(&src, &cfg, limits()).is_err());
    }
}

fn ordinary(src: &str, cfg: &str) -> serde_json::Value {
    ordinary_with_limits(src, cfg, limits())
}

fn ordinary_with_limits(src: &str, cfg: &str, policy: Limits) -> serde_json::Value {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "dive-temporal-first-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&dir).expect("test directory");
    let file = dir.join("Test.tla");
    let config = dir.join("Test.cfg");
    std::fs::write(&file, src).expect("fixture");
    std::fs::write(&config, cfg).expect("fixture");
    let report = crate::check_file(&crate::CheckRequest {
        file,
        config: Some(config),
        workers: Some(1),
        seed: Some(0),
        fp: Some(0),
        store: crate::Store::Exact,
        time_limit: crate::BudgetSetting::Unlimited,
        max_memory: crate::BudgetSetting::Unlimited,
        max_states: policy.max_states,
        max_depth: policy.max_depth,
        ..Default::default()
    })
    .expect("ordinary checker")
    .to_json();
    std::fs::remove_dir_all(&dir).expect("remove owned fixture");
    report
}

#[test]
fn previous_native_execution_must_actually_have_avoided_fallback() {
    let text = "a".repeat(4097);
    let src = format!(
        "---- MODULE Test ----\nVARIABLE x\nInit == x=\"{text}\"\nNext == x'=x\nSpec == Init /\\ [][Next]_x\nP == (x=x) ~> (x=x)\nQ == []<>(x=x)\n===="
    );
    let old = "SPECIFICATION Spec\nPROPERTY P Q";
    let edited = "SPECIFICATION Spec\nPROPERTY P";
    let previous = program(&src, old, limits()).expect("all atoms compile");
    let current = program(&src, edited, limits()).expect("current atoms compile");
    let basis = ordinary(&src, old);
    assert_eq!(basis["verdict"], "success");
    let facts = &basis["core"]["metrics"]["liveness"]["atoms"];
    assert_eq!(facts["eligible_atoms"], facts["total_atoms"]);
    assert!(
        facts["reference_evaluations"]["state"]
            .as_u64()
            .is_some_and(|n| n > 0)
    );
    assert!(previous.first_claim(&current, &basis).is_err());
}

#[test]
fn truncated_receipts_stops_and_earlier_refutations_never_qualify() {
    let previous = program(SRC, OLD, limits()).expect("previous");
    let current = program(SRC, NEW, limits()).expect("current");
    let basis = ordinary(SRC, OLD);
    for field in [
        "/statistics/left_on_queue",
        "/execution/capture_completed",
        "/periodic/final_check",
        "/periodic/prefix_checks",
        "/options/int",
        "/options/comparisons",
        "/options/choose_order",
        "/options/explore_order",
        "/options/action_composition",
        "/options/tlc_exact_temporal",
        "/options/lasso",
        "/symmetry_liveness/declared",
        "/options/workers/used",
        "/options/budget/max_states",
        "/options/temporal_bounds/generated",
        "/core/metrics/liveness/atoms/audit",
        "/core/metrics/liveness/atoms/reference_evaluations/state",
        "/core/metrics/liveness/atoms/reference_evaluations/action",
        "/core/metrics/liveness/atoms/fallback",
    ] {
        let mut changed = basis.clone();
        *changed.pointer_mut(field).expect("ordinary field") =
            serde_json::Value::String("0".to_owned());
        assert!(
            previous.first_claim(&current, &changed).is_err(),
            "accepted malformed {field}"
        );
    }
    let mut missing = basis.clone();
    missing["core"]["metrics"]["liveness"]
        .as_object_mut()
        .expect("metrics")
        .remove("atoms");
    assert!(previous.first_claim(&current, &missing).is_err());
    let refuted = SRC.replace("Q == []<>(x<K)", "Q == []<>(x=1)");
    let negative = ordinary(&refuted, OLD);
    assert_eq!(negative["verdict"], "liveness-violation");
    assert!(
        program(&refuted, OLD, limits())
            .expect("old bindings")
            .first_claim(
                &program(&refuted, NEW, limits()).expect("current bindings"),
                &negative
            )
            .is_err()
    );
}
#[test]
fn registered_runtime_name_operators_miss() {
    let src = SRC.replace("EXTENDS Naturals", "EXTENDS Naturals, Sequences");
    assert!(program(&src, OLD, limits()).is_err());
}

#[test]
fn compares_a_real_owned_union_over_record_sets_without_decoding_state_rows() {
    let src = r#"---- MODULE Test ----
CONSTANTS A,B
Values == [tag: {"a"}, v:A] \cup [tag: {"b"}, v:B]
VARIABLE x
Init == x \in Values
Next == x'=x
P == (x \in Values) ~> (x \in Values)
Q == []<>(x \in Values)
===="#;
    let cfg = "INIT Init\nNEXT Next\nCONSTANTS A={v1,v2} B={v1}\nPROPERTY P Q";
    let edited = cfg.replace("P Q", "P");
    let basis = ordinary(src, cfg);
    let previous = program(src, cfg, limits()).expect("actual record-valued search, owned union");
    let current = program(src, &edited, limits()).expect("actual retained first plan");
    assert_eq!(previous.first_claim(&current, &basis).expect("held"), "P");
    let changed =
        program(src, &edited.replace("B={v1}", "B={v2}"), limits()).expect("different bound input");
    assert!(previous.first_claim(&changed, &basis).is_err());
}

#[test]
fn actual_stopped_search_does_not_establish_projection() {
    let cap = Limits {
        max_states: Some(1),
        ..limits()
    };
    let previous = program(SRC, OLD, cap).expect("old");
    let current = program(SRC, NEW, cap).expect("current");
    let basis = ordinary_with_limits(SRC, OLD, cap);
    assert_ne!(basis["verdict"], "success");
    assert!(previous.first_claim(&current, &basis).is_err());
}
