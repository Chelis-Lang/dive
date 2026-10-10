//! Pinned concrete slot receipts, deciding bin order and independent witness controls.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_liveness::phase_l::{self, DiagnosticState, Error, Incoming, Limits, Session, State};
use dive_value::{Interner, Value};
use serde_json::Value as Json;
use std::collections::BTreeMap;
use std::sync::Arc;
fn scalar(text: &str, names: &Interner) -> Value {
    match text.as_bytes()[0] {
        b'i' => Value::Int(text[1..].parse().expect("integer")),
        b'b' => Value::Bool(text[1..].parse().expect("bool")),
        b's' => Value::string(names, &text[1..]),
        _ => panic!("unknown scalar"),
    }
}
fn inputs(case: &Json) -> Vec<DiagnosticState> {
    let names = Interner::new();
    case["records"]
        .as_array()
        .expect("records")
        .iter()
        .map(|r| {
            let fields: Vec<_> = r.as_str().expect("record").splitn(3, ':').collect();
            DiagnosticState {
                fingerprint: fields[0].parse().expect("key"),
                label: fields[1].into(),
                value: scalar(fields[2], &names),
            }
        })
        .collect()
}
#[test]
fn all_55_forced_key_structural_rows_match_every_java_field() {
    let selection: Json = serde_json::from_str(include_str!(
        "../../../conformance/probes/tlc-phase-l/slot-selection.json"
    ))
    .expect("selection");
    let record: Json = serde_json::from_str(include_str!(
        "../../../conformance/probes/tlc-phase-l/slot-capture.json"
    ))
    .expect("record");
    let selected = selection.as_array().expect("selection");
    let rows = record["rows"].as_array().expect("rows");
    assert_eq!(selected.len(), 55);
    assert_eq!(rows.len(), 55);
    for (case, row) in selected.iter().zip(rows) {
        assert_eq!(case["name"], row["name"]);
        assert_eq!(row["exit"], 0);
        assert_eq!(row["timeout"], false);
        let capacity =
            usize::try_from(case["capacity"].as_u64().expect("capacity")).expect("capacity fits");
        let snapshots = phase_l::diagnostic_slots(capacity, &inputs(case), Limits::default())
            .expect("native slots");
        let mut fields = BTreeMap::new();
        fields.insert("initial.capacity".to_string(), capacity.to_string());
        let mut iteration = String::new();
        for (i, s) in snapshots.iter().enumerate() {
            fields.insert(format!("p{i}.duplicate"), s.duplicate.to_string());
            fields.insert(format!("p{i}.count"), s.count.to_string());
            fields.insert(format!("p{i}.capacity"), s.capacity.to_string());
            fields.insert(
                format!("p{i}.slots"),
                s.slots
                    .iter()
                    .map(|(slot, l)| format!("{slot}:{l}"))
                    .collect::<Vec<_>>()
                    .join(";"),
            );
            iteration = s
                .slots
                .iter()
                .map(|(_, l)| l.as_str())
                .collect::<Vec<_>>()
                .join(";");
        }
        fields.insert("iteration".into(), iteration.clone());
        fields.insert("reset.iteration".into(), iteration);
        let reference: BTreeMap<String, String> =
            serde_json::from_value(row["fields"].clone()).expect("fields");
        assert_eq!(fields, reference, "{}", case["name"]);
    }
}
fn model(name: &str) -> Option<dive_model::Model> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/probes/tlc-phase-l/models")
        .join(name);
    let a = dive_sema::analyze_file(
        &root.join(format!("{name}.tla")),
        &dive_sema::Options::default(),
    )
    .expect("analysis");
    if !a.accepted() {
        return None;
    }
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse(
        &std::fs::read_to_string(root.join(format!("{name}.cfg"))).expect("cfg"),
    )
    .into_config()
    .expect("config");
    Some(dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("bound model"))
}
fn state(value: i64) -> State {
    Arc::from([Some(Value::Int(value))])
}
fn successors(case: &Json) -> Vec<Incoming> {
    let names = Interner::new();
    case["incoming"]
        .as_array()
        .expect("incoming")
        .iter()
        .enumerate()
        .map(|(i, s)| Incoming {
            state: Arc::from([Some(scalar(s.as_str().expect("scalar"), &names))]),
            ordinal: i as u64,
            label: Some(format!("input{i}")),
        })
        .collect()
}
fn log(receipt: &phase_l::Receipt) -> Vec<String> {
    receipt
        .checks
        .iter()
        .map(|c| {
            format!(
                "g{}.{}{}:{}",
                c.group,
                if c.kind == dive_liveness::formula::AtomKind::Action {
                    "a"
                } else {
                    "s"
                },
                c.bin,
                c.holds
            )
        })
        .collect()
}
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "complete oracle selection and each distinct partial outcome are checked together"
)]
fn real_bound_fp_first_representatives_bins_initial_consistency_and_deciding_errors() {
    let selection: Json = serde_json::from_str(include_str!(
        "../../../conformance/probes/tlc-phase-l/model-selection.json"
    ))
    .expect("selection");
    let record: Json = serde_json::from_str(include_str!(
        "../../../conformance/probes/tlc-phase-l/model-capture.json"
    ))
    .expect("record");
    let mut counts = [0; 5];
    let rows = record["rows"].as_array().expect("rows");
    let selected = selection.as_array().expect("selection");
    assert_eq!(selected.len(), rows.len());
    for (case, row) in selected.iter().zip(rows) {
        assert_eq!(case["name"], row["name"]);
        assert_eq!(row["timeout"], false);
        let name = case["model"].as_str().expect("model");
        let Some(m) = model(name) else {
            assert!(matches!(
                name,
                "PhaseAction" | "PhaseBoth" | "PhaseActionError" | "PhaseDualError"
            ));
            assert_ne!(row["exit"], 0);
            counts[0] += 1;
            continue;
        };
        let Ok(mut session) = Session::new(&m, 0, 1, Limits::default()) else {
            assert!(matches!(
                name,
                "PhaseEffect" | "PhaseLiveGet" | "PhaseStateError"
            ));
            assert_eq!(m.take_output(), Vec::<String>::new());
            counts[1] += 1;
            continue;
        };
        let fields = &row["fields"];

        assert_eq!(
            session
                .configured_properties()
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(";"),
            fields["configured_names"].as_str().expect("names")
        );
        assert_eq!(
            session.role_names().join(";"),
            fields["role_names"].as_str().expect("roles")
        );
        // These controls deliberately keep config/Action identity separate from role labels.
        assert_eq!(fields["action_names"], fields["configured_names"]);
        if name == "PhaseLocalRole" {
            assert_eq!(fields["configured_names"], "Live");
            assert_eq!(fields["role_names"], "Identity");
        }
        let source = state(
            case["source"].as_str().expect("source")[1..]
                .parse()
                .expect("source int"),
        );
        let initial = session.initial(&source).expect("initial consistency");
        let initial_log = fields["initial.log"].as_str().expect("initial log");
        for (g, cells) in initial.iter().enumerate() {
            assert_eq!(
                cells.len(),
                fields[format!("g{g}.initial")]
                    .as_str()
                    .expect("init count")
                    .parse::<usize>()
                    .expect("count")
            );
            for (p, holds) in cells.iter().enumerate() {
                let prefix = format!("g{g}.n{p}.c");
                let observed = !initial_log
                    .split(';')
                    .filter(|s| s.starts_with(&prefix))
                    .any(|s| s.ends_with(":false"));
                assert_eq!(*holds, observed, "{} group{g} particle{p}", case["name"]);
            }
        }
        let result = session.source(&source, &successors(case));
        let (receipt, failed) = match result {
            Ok(r) => (r, false),
            Err(e) => {
                if name == "PhaseTableau" {
                    assert!(matches!(
                        e.error,
                        Error::Unsupported("tableau graph consistency and product revisitation")
                    ));
                    let r = e.partial.expect("collection prefix");
                    assert!(!r.graph_insertion);
                    assert_eq!(r.successors.len(), 3);
                    assert_eq!(r.checks, Vec::<phase_l::Check>::new());
                    counts[4] += 1;
                    continue;
                }
                let Error::Evaluation(error) = &e.error else {
                    panic!("unexpected refusal {} {}", case["name"], e)
                };
                assert_eq!(
                    error.ec().code(),
                    fields["failure_code"]
                        .as_str()
                        .expect("failure code")
                        .parse::<u32>()
                        .expect("code")
                );
                assert_eq!(e.progress.sources, 0);
                assert_eq!(e.progress.actual_workers, 1);
                (
                    e.partial.expect("completed collection and bin prefix"),
                    true,
                )
            }
        };
        assert!(!receipt.graph_insertion);
        assert_eq!(
            receipt.capacity.to_string(),
            fields["capacity"].as_str().expect("capacity")
        );
        assert_eq!(
            receipt.successors.len().to_string(),
            fields["count"].as_str().expect("count")
        );
        assert_eq!(
            receipt
                .successors
                .iter()
                .map(|r| format!("{}:{}", r.slot, r.fingerprint))
                .collect::<Vec<_>>()
                .join(";"),
            fields["slots"].as_str().expect("slots")
        );
        let reference: Vec<_> = fields["log"]
            .as_str()
            .expect("log")
            .split(';')
            .take_while(|s| !s.contains(".n"))
            .filter(|s| !s.is_empty() && !s.ends_with(":error"))
            .map(str::to_string)
            .collect();
        assert_eq!(log(&receipt), reference, "{}", case["name"]);
        if failed {
            counts[2] += 1;
        } else {
            counts[3] += 1;
            assert_eq!(session.progress().sources, 1);
        }
    }
    assert_eq!(counts, [12, 9, 11, 16, 3]);
}
#[test]
fn logical_capacity_history_and_partial_collection_are_preserved() {
    let m = model("PhaseState").expect("model");
    let mut session = Session::new(&m, 0, 1, Limits::default()).expect("session");
    let incoming: Vec<_> = (0..40)
        .map(|v| Incoming {
            state: state(v),
            ordinal: u64::try_from(v).expect("nonnegative ordinal"),
            label: Some(v.to_string()),
        })
        .collect();
    let first = session.source(&state(0), &incoming).expect("grown source");
    assert_eq!(first.multiplier, 1);
    assert_eq!(first.capacity, 135);
    assert_eq!(first.successors.len(), 40);
    let second = session.source(&state(0), &[]).expect("next source");
    assert_eq!(second.multiplier, 2);
    assert_eq!(second.capacity, 32);
    // The first generator's concrete object remains retained even when stutter duplicates it.
    let firstzero = first
        .successors
        .iter()
        .find(|s| s.state[0] == Some(Value::Int(0)))
        .expect("zero");
    assert_eq!(firstzero.ordinal, Some(0));
    assert!(Arc::ptr_eq(&firstzero.state, &incoming[0].state));
    let limits = Limits {
        values: 1,
        ..Limits::default()
    };
    let mut bounded = Session::new(&m, 0, 1, limits).expect("session");
    let error = bounded
        .source(&state(0), &incoming[..2])
        .expect_err("second state material limit");
    assert!(matches!(error.error, Error::Limit("material value visits")));
    let prefix = error.partial.expect("prefix");
    assert_eq!(prefix.successors.len(), 1);
    assert_eq!(prefix.successors[0].ordinal, Some(0));
    assert_eq!(error.progress.sources, 0);
    assert_eq!(error.progress.admissions, 2);
}
#[test]
fn bounds_malformed_inputs_and_worker_metadata_cannot_become_success() {
    let m = model("PhaseState").expect("model");
    let input: Vec<_> = (0..9)
        .map(|v| Incoming {
            state: state(v),
            ordinal: u64::try_from(v).expect("nonnegative ordinal"),
            label: None,
        })
        .collect();
    let mut attempts = 0;
    for incoming in [0, 1, 8, 9, 10, 32] {
        for slots in [0, 8, 16, 17, 32, 33] {
            for cells in [0, 1, 9, 10, 20, 100] {
                let limits = Limits {
                    incoming,
                    slots,
                    cells,
                    ..Limits::default()
                };
                let mut s = Session::new(&m, 0, 1, limits).expect("session");
                match s.source(&state(0), &input) {
                    Ok(r) => {
                        assert_eq!(r.successors.len(), 9);
                        assert_eq!(s.progress().sources, 1);
                        assert!(!r.graph_insertion);
                    }
                    Err(e) => {
                        assert!(matches!(e.error, Error::Limit(_)));
                        assert_eq!(e.progress.sources, 0);
                    }
                }
                attempts += 1;
            }
        }
    }
    assert_eq!(attempts, 216);
    for workers in [0, 2, 4, 64] {
        let e = Session::new(&m, 0, workers, Limits::default())
            .err()
            .expect("named wider refusal");
        assert!(matches!(e.error, Error::Unsupported(_)));
        assert_eq!(e.progress.actual_workers, 0);
    }
    let mut s = Session::new(&m, 0, 1, Limits::default()).expect("session");
    let mut bad = input[..2].to_vec();
    bad[1].ordinal = 0;
    assert!(matches!(
        s.source(&state(0), &bad).expect_err("ordinal").error,
        Error::Input(_)
    ));
    let mut s = Session::new(&m, 0, 1, Limits::default()).expect("session");
    assert!(matches!(
        s.initial(&Arc::from([None])).expect_err("incomplete").error,
        Error::Input(_)
    ));
    let mut s = Session::new(&m, 0, 1, Limits::default()).expect("session");
    assert!(matches!(
        s.initial(&Arc::from([Some(Value::nat(dive_value::Ctx::tlc()))]))
            .expect_err("lazy")
            .error,
        Error::Unsupported(_)
    ));
    assert_eq!(m.take_output(), Vec::<String>::new());
}
#[test]
fn receipts_do_not_certify_forged_words_closing_edges_or_property_claims() {
    let m = model("PhaseState").expect("model");
    let mut session = Session::new(&m, 0, 1, Limits::default()).expect("session");
    // A caller can supply an out-of-Next successor to this diagnostic interface;
    // it must never masquerade as validated graph/proof evidence.
    let receipt = session
        .source(
            &state(0),
            &[Incoming {
                state: state(99),
                ordinal: 0,
                label: None,
            }],
        )
        .expect("diagnostic only");
    assert!(!receipt.graph_insertion);
    let doc = dive_trace::TraceDoc::behavior(vec![serde_json::json!({"x":0})])
        .expect("word")
        .with_properties(vec!["Live".into()]);
    let mut valid = doc;
    valid.ending = Some(dive_trace::Ending::Lasso { back_to: 0 });
    assert_eq!(
        dive_trace::validate_doc(&m, &valid, &[], None).0.validation,
        dive_trace::Validation::Valid
    );
    let mut unknown = valid.clone();
    unknown.properties = vec!["ForgedRole".into()];
    assert_ne!(
        dive_trace::validate_doc(&m, &unknown, &[], None)
            .0
            .validation,
        dive_trace::Validation::Valid
    );
    let mut bad = valid.clone();
    bad.states
        .push(serde_json::from_value(serde_json::json!({"x":99})).expect("state"));
    assert!(matches!(
        dive_trace::validate_doc(&m, &bad, &[], None).0.validation,
        dive_trace::Validation::Invalid(_)
    ));
    let mut closing = valid.clone();
    closing.ending = Some(dive_trace::Ending::Lasso { back_to: 1 });
    assert_ne!(
        dive_trace::validate_doc(&m, &closing, &[], None)
            .0
            .validation,
        dive_trace::Validation::Valid
    );
    let mut holds = valid;
    holds
        .states
        .push(serde_json::from_value(serde_json::json!({"x":2})).expect("state"));
    holds.ending = Some(dive_trace::Ending::Lasso { back_to: 1 });
    assert!(matches!(
        dive_trace::validate_doc(&m, &holds, &[], None).0.validation,
        dive_trace::Validation::Invalid(_)
    ));
}

#[test]
fn public_opaque_values_and_unchecked_predicates_are_refused_without_callbacks_or_panics() {
    #[derive(Debug)]
    struct Opaque(
        Arc<std::sync::atomic::AtomicBool>,
        Arc<std::sync::atomic::AtomicUsize>,
    );
    impl dive_value::DeferredSet for Opaque {
        fn kind(&self) -> dive_value::DeferredKind {
            dive_value::DeferredKind::Filter
        }
        fn elements(&self, _: dive_value::Ctx) -> dive_value::Result<Vec<Value>> {
            self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            assert!(
                !self.0.load(std::sync::atomic::Ordering::SeqCst),
                "opaque elements called"
            );
            Ok(vec![Value::Int(7)])
        }
        fn member(&self, _: &Value, _: dive_value::Ctx) -> Option<dive_value::Result<bool>> {
            self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            panic!("opaque member called")
        }
        fn key(&self) -> Vec<Value> {
            self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            panic!("opaque key called")
        }
    }
    let m = model("PhaseState").expect("model");
    let mut session = Session::new(&m, 0, 1, Limits::default()).expect("session");
    let armed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let callbacks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let v = Value::deferred(
        Arc::new(Opaque(Arc::clone(&armed), Arc::clone(&callbacks))),
        dive_value::Ctx::tlc(),
    );
    // Tuple construction hashes children in this frozen API. Its separately
    // retained constructor-gap observation is not phase-L evidence. Arm the
    // callback trap only after construction; preflight must still reject the
    // nested deferred provider, even though construction populated its cache.
    let wrapped = Arc::from([Some(Value::tuple(vec![v.clone()]))]);
    let constructor_callbacks = callbacks.load(std::sync::atomic::Ordering::SeqCst);
    assert!(constructor_callbacks > 0);
    armed.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(matches!(
        session
            .initial(&Arc::from([Some(v)]))
            .expect_err("direct opaque refusal")
            .error,
        Error::Unsupported(_)
    ));
    assert!(matches!(
        session.initial(&wrapped).expect_err("opaque refusal").error,
        Error::Unsupported(_)
    ));
    assert_eq!(
        callbacks.load(std::sync::atomic::Ordering::SeqCst),
        constructor_callbacks
    );
    let mut m = model("PhaseState").expect("model");
    m.implied_temporals[0].pred.expr = dive_ir::ExprId(u32::MAX);
    let error = Session::new(&m, 0, 1, Limits::default())
        .err()
        .expect("missing predicate");
    assert!(matches!(error.error, Error::Planning(_)));
    assert_eq!(error.progress.actual_workers, 0);
    assert_eq!(m.take_output(), Vec::<String>::new());
}

#[test]
fn all_reference_livecheck_null_tableau_logs_preserve_failed_call_and_complete_selection() {
    let selection: Json = serde_json::from_str(include_str!(
        "../../../conformance/probes/tlc-phase-l/evaluation-selection.json"
    ))
    .expect("selection");
    let capture: Json = serde_json::from_str(include_str!(
        "../../../conformance/probes/tlc-phase-l/evaluation-capture.json"
    ))
    .expect("capture");
    let selected = selection.as_array().expect("selection");
    let rows = capture["rows"].as_array().expect("rows");
    assert_eq!(selected.len(), 35);
    assert_eq!(rows.len(), 35);
    for (case, row) in selected.iter().zip(rows) {
        assert_eq!(case["name"], row["name"]);
        assert_eq!(row["exit"], 0);
        assert_eq!(row["timeout"], false);
        let mut input = inputs(case);
        input.push(DiagnosticState {
            fingerprint: 0,
            value: Value::Int(0),
            label: "source".into(),
        });
        let slots = phase_l::diagnostic_slots(16, &input, Limits::default()).expect("collection");
        let last = slots.last().expect("stutter");
        let groups = case["groups"].as_u64().expect("groups");
        let states = case["states"].as_u64().expect("states");
        let actions = case["actions"].as_u64().expect("actions");
        let fail = usize::try_from(case["fail"].as_u64().expect("fail")).expect("fail fits");
        // Independently specify the source method's callback contract. This
        // verifies the actual Java invocation, not native model/SCC execution.
        let mut calls = Vec::new();
        for g in 0..groups {
            for (_, target) in &last.slots {
                for a in 0..actions {
                    calls.push(format!("g{g}.a{a}:source:{target}"));
                }
            }
            for s in 0..states {
                calls.push(format!("g{g}.s{s}:source:-"));
            }
        }
        let fails = fail != 0 && fail <= calls.len();
        if fails {
            calls.truncate(fail);
        }
        assert_eq!(row["fields"]["log"], calls.join(";"));
        assert_eq!(row["fields"]["initial.log"], "");
        if fails {
            assert_eq!(
                row["fields"]["failure_class"],
                "java.lang.IllegalStateException"
            );
            assert!(row["fields"].get("completion").is_none());
        } else {
            assert_eq!(row["fields"]["completion"], "bin-and-disk-insertion");
            assert!(row["fields"].get("failure_class").is_none());
        }
        assert_eq!(row["fields"]["capacity"], last.capacity.to_string());
        assert_eq!(row["fields"]["count"], last.count.to_string());
    }
}

#[test]
fn phase_l_evidence_hashes_selections_raw_bytes_and_before_observations_are_verified() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert!(
        std::process::Command::new("python3")
            .args(["-I", "scripts/check-phase-l.py"])
            .current_dir(root)
            .status()
            .expect("checker")
            .success()
    );
}

#[test]
fn caller_edited_fingerprint_permutation_returns_typed_input_refusal() {
    for permutation in [
        serde_json::json!([]),
        serde_json::json!([u32::MAX]),
        serde_json::json!([0, 0]),
    ] {
        let mut m = model("PhaseState").expect("model");
        let mut ir = serde_json::to_value(m.ir.as_ref()).expect("IR");
        ir["tlc_vars"] = permutation;
        let edited: Arc<dive_ir::Ir> = Arc::new(serde_json::from_value(ir).expect("caller IR"));
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../conformance/probes/tlc-phase-l/models/PhaseState/PhaseState.cfg");
        let cfg = dive_config::parse(&std::fs::read_to_string(root).expect("cfg"))
            .into_config()
            .expect("config");
        // A malformed permutation is now refused during evaluator admission,
        // before binding can evaluate a predicate or create a receipt session.
        let refusal = dive_model::bind(edited.clone(), &cfg, &dive_model::BindOptions::tlc())
            .expect_err("malformed variable order must be refused before effects");
        assert!(refusal.error.to_string().contains("DIVE-EVAL-C005"));
        assert_eq!(refusal.output, [] as [dive_model::Message; 0]);
        // Swapping an already-bound model's IR independently violates context
        // provenance, including when the caller retains its old predicate IDs.
        m.ir = edited;
        let Err(refusal) = Session::new(&m, 0, 1, Limits::default()) else {
            panic!("foreign model metadata must be refused before predicates");
        };
        assert!(matches!(refusal.error, Error::Planning(_)));
        assert_eq!(refusal.progress.predicates, 0);
        assert_eq!(m.take_output(), Vec::<String>::new());
    }
}

#[test]
fn state_if_planning_error_precedes_fingerprints_and_predicate_work() {
    let m = model("PhaseStateError").expect("bound model");
    let failure = Session::new(&m, 0, 1, Limits::default())
        .err()
        .expect("pinned early planning failure");
    let Error::Planning(dive_liveness::tlc_plan::ModelPlanningError::Bound(
        dive_liveness::plan::PlanningError::Evaluation { error, .. },
    )) = failure.error
    else {
        panic!("expected original typed constant conversion error");
    };
    assert_eq!(error.ec().code(), 2179);
    assert_eq!(failure.progress.actual_workers, 0);
    assert_eq!(failure.progress.predicates, 0);
    assert_eq!(failure.progress.sources, 0);
    assert!(failure.partial.is_none());
    assert_eq!(m.take_output(), Vec::<String>::new());
}
