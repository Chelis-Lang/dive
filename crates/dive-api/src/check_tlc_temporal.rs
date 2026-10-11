//! Report projection of private final-only temporal certificates.
use crate::tlc_temporal::{Error, Failure, Outcome, Report};
use dive_check::capture::Complete;
use serde_json::{Value, json};
pub(super) fn complete(report: &Report) -> Option<&Complete> {
    match report.outcome() {
        Outcome::Holds(e) => Some(&e.execution.capture),
        Outcome::Violation(v) => Some(&v.candidate().evidence.execution.capture),
        Outcome::Safety(_) => None,
    }
}
pub(super) fn kind(failure: &Failure) -> &'static str {
    use dive_liveness::{tlc_final, tlc_scc};
    match &failure.error {
        Error::Thread(_) | Error::Document(_) => "internal-error",
        Error::Validation { report, .. } | Error::SafetyValidation { report, .. }
            if matches!(report.validation, dive_trace::Validation::Invalid(_)) =>
        {
            "internal-error"
        }
        Error::Binding(f) => match f.error.kind {
            dive_model::ErrorKind::Bug => "internal-error",
            dive_model::ErrorKind::Unsupported { .. } => "unsupported",
            _ => "model-error",
        },
        Error::Analysis(e) => match &e.error {
            tlc_final::Error::Limit(_) | tlc_final::Error::Selection(tlc_scc::Error::Limit(_)) => {
                "resource-limit"
            }
            tlc_final::Error::Selection(
                tlc_scc::Error::Input(_) | tlc_scc::Error::Invariant(_),
            ) => "internal-error",
            tlc_final::Error::Execution(e) => match &e.error {
                dive_liveness::phase_graph::Error::Limit(_) => "resource-limit",
                dive_liveness::phase_graph::Error::Input(_)
                | dive_liveness::phase_graph::Error::Execution(_) => "internal-error",
                _ => "unsupported",
            },
            _ => "unsupported",
        },
        _ => "unsupported",
    }
}
fn capture(doc: &mut Value, c: &Complete) {
    let s = &c.stats;
    doc["statistics"] = json!({"generated":s.generated,"distinct":s.distinct,"left_on_queue":s.left_on_queue,"depth":s.depth,"initial_generated":s.initial_generated,"reported_by_tlc":true});
    doc["output"] = json!(c.output);
    doc["warnings"] = json!(
        c.warnings
            .iter()
            .map(|(code, message)| json!({"code":code,"message":message}))
            .collect::<Vec<_>>()
    );
    if let Some(c) = &c.collision {
        doc["collision"] = json!({"store":"fingerprint","scheme":"tlc","optimistic":c.optimistic,"actual":c.actual,"assumption":"fingerprints behave as random 64-bit values (TLC's assumption too)"});
    }
}
pub(super) fn success(doc: &mut Value, report: &Report) {
    if let Some(c) = complete(report) {
        capture(doc, c);
    }
    doc["execution"] = json!({"stage":"complete","capture_completed":true,"schedule":"final-only","selection":"tlc"});
    let execution = match report.outcome() {
        Outcome::Holds(e) => Some(&e.execution),
        Outcome::Violation(v) => Some(&v.candidate().evidence.execution),
        Outcome::Safety(_) => None,
    };
    if let Some(e) = execution {
        doc["execution"]["configured_property_names"] = json!(
            e.configured_properties
                .iter()
                .map(|p| &p.name)
                .collect::<Vec<_>>()
        );
        doc["execution"]["expanded_property_names"] = json!(e.expanded_property_names);
        doc["execution"]["role_names"] = json!(e.role_names);
    }

    doc["elapsed_ms"] = json!(u64::try_from(report.elapsed().as_millis()).unwrap_or(u64::MAX));
    doc["notes"] = json!([
        "Explicit bounded final-only TLC selection; ordinary periodic scheduling, exact temporal storage and unsupported effect/recovery boundaries remain unavailable."
    ]);
    match report.outcome() {
        Outcome::Safety(_) => {}
        Outcome::Holds(_) => {
            doc["tlc"] = json!({"code":2193,"name":"TLC_SUCCESS","exit_status":0,"reported":2193,"verified":true});
        }
        Outcome::Violation(v) => {
            doc["property"] = json!(v.document().properties.join(" and "));
            doc["properties"] = json!(v.document().properties);
            doc["tlc"] = json!({"code":2116,"name":"TLC_TEMPORAL_PROPERTY_VIOLATED","exit_status":13,"reported":2116,"verified":true});
            let stutter = v.document().ending == Some(dive_trace::Ending::Stuttering);
            doc["counterexample"] = json!({"length":v.document().states.len(),"ending":if stutter {"stuttering"} else {"lasso"},"back_to":if stutter {Value::Null} else {json!(v.candidate().back_to+1)},"states":v.document().states.iter().enumerate().map(|(i,s)|json!({"index":i+1,"label":null,"action":null,"values":s})).collect::<Vec<_>>(),"trace":v.document().to_json(),"validation":"valid"});
        }
    }
}
fn stage(error: &Error) -> &'static str {
    use dive_liveness::tlc_final;
    match error {
        Error::Context(_) | Error::Input(_) | Error::Unsupported(_) => "admission",
        Error::Binding(_) => "binding",
        Error::Validation { .. } | Error::SafetyValidation { .. } => "independent-validation",
        Error::Document(_) => "serialization",
        Error::Thread(_) => "execution-evidence-unavailable",
        Error::Analysis(e) => match &e.error {
            tlc_final::Error::Input(_) => "admission",
            tlc_final::Error::Execution(_) => "ordered-execution",
            tlc_final::Error::Projection(_) => "graph-projection",
            tlc_final::Error::Selection(_) => "scc-witness-selection",
            tlc_final::Error::Planning(_) => "attribution-planning",
            tlc_final::Error::Attribution(_) => "property-attribution",
            tlc_final::Error::Unsupported(_) | tlc_final::Error::Limit(_) => "final-analysis",
        },
    }
}
pub(super) fn failure(doc: &mut Value, failure: &Failure) {
    doc["message"] = json!(failure.to_string());
    doc["diagnostics"] = json!([{"code":failure.error.code(),"message":failure.to_string()}]);
    doc["elapsed_ms"] = json!(u64::try_from(failure.elapsed.as_millis()).unwrap_or(u64::MAX));
    doc["execution"] = json!({"stage":stage(&failure.error),"capture_completed":failure.evidence.is_some(),"schedule":"final-only","selection":"tlc"});
    doc["notes"] = json!([
        "No temporal verdict is certified. Available prefix/complete graph evidence does not prove temporal acceptance or absence; unknown deciding TLC classes receive no agreement credit."
    ]);
    if let Error::SafetyValidation {
        result,
        report,
        limits,
    } = &failure.error
    {
        doc["options"]["temporal_bounds"] = limits_json_for(limits);
        doc["execution"] = json!({"stage":"independent-validation", "route":"bounded-safety-only",
            "capture_completed":false, "temporal_obligations":0,
            "temporal_certificate":false, "product_constructed":false,
            "safety_stop":result.verdict.kind()});
        if let Some(s) = result.stats {
            doc["statistics"] = json!({"generated":s.generated,"distinct":s.distinct,
                "left_on_queue":s.left_on_queue,"depth":s.depth,
                "initial_generated":s.initial_generated,"reported_by_tlc":result.tlc_prints_stats});
        }
        doc["output"] = json!(result.output);
        doc["notes"] = json!([format!(
            "Finite safety witness is not certified: {:?}",
            report.validation
        )]);
    }
    if let Error::Binding(e) = &failure.error {
        doc["output"] = json!(
            e.output
                .iter()
                .filter_map(|m| match m {
                    dive_model::Message::Print(text) => Some(text),
                    dive_model::Message::Warning { .. } => None,
                })
                .collect::<Vec<_>>()
        );
        doc["warnings"] = json!(
            e.output
                .iter()
                .filter_map(|m| match m {
                    dive_model::Message::Warning { code, message } =>
                        Some(json!({"code":code.code(),"message":message})),
                    dive_model::Message::Print(_) => None,
                })
                .collect::<Vec<_>>()
        );
    }
    if let Some(e) = &failure.evidence {
        capture(doc, &e.execution.capture);
    }
    if let Error::Analysis(e) = &failure.error
        && let dive_liveness::tlc_final::Error::Execution(e) = &e.error
        && let Some(s) = &e.stats
    {
        doc["statistics"] = json!({"generated":s.generated,"distinct":s.distinct,"left_on_queue":s.left_on_queue,"depth":s.depth,"initial_generated":s.initial_generated,"reported_by_tlc":false});
    }
}
pub(super) fn human(report: &Report) -> String {
    let Some(complete) = complete(report) else {
        return "Bounded safety-only route; no temporal certificate.\n".into();
    };
    let s = &complete.stats;
    let verdict = match report.outcome() {
        Outcome::Safety(_) => "Bounded safety-only outcome; no temporal certificate.",
        Outcome::Holds(_) => "No acceptance in the supported final-only fingerprint graph.",
        Outcome::Violation(_) => {
            "Temporal properties violated; selected periodic word independently validated."
        }
    };
    format!(
        "{verdict}\nExplicit final-only schedule and TLC witness selection.\n{} generated, {} distinct, depth {}.\n",
        s.generated, s.distinct, s.depth
    )
}

/// Fixed entry/work ceilings actually used by this facade; no byte-budget claim.
pub(super) fn limits_json() -> Value {
    limits_json_for(&dive_liveness::phase_graph::Limits::default())
}
pub(super) fn limits_json_for(l: &dive_liveness::phase_graph::Limits) -> Value {
    json!({
        "kind":"entry-and-work-counts", "aggregate_byte_budget":false,
        "capture":{"states":l.capture.states,"edges":l.capture.edges,"generated":l.capture.generated,"successors_per_unit":l.capture.successors_per_unit,"maximum_concurrent_sources":dive_check::capture::MAX_EXPANSION_UNITS},
        "phase_l":{"incoming":l.phase_l.incoming,"slots":l.phase_l.slots,"values":l.phase_l.values,"cells":l.phase_l.cells,"work":l.phase_l.work},
        "formula":{"nodes":l.phase_l.formula.formula_nodes,"tableau_nodes":l.phase_l.formula.tableau_nodes,"steps":l.phase_l.formula.expansion_steps},
        "product":{"vertices":l.graph.vertices,"edges":l.graph.edges,"acceptance_cells":l.graph.acceptance_cells,"steps":l.graph.steps},
        "maximum_workers":l.workers
    })
}
