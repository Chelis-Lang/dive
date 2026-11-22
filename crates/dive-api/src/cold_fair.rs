//! An exact temporal witness search from one source-generated root.

use serde_json::{Value, json};

use crate::{ApiError, CheckReport, CheckRequest, ExitConvention, Store};

/// Evidence from the retained root's reachable graph. The original
/// model's full initial state space is outside this search's scope.
#[derive(Debug)]
pub struct Report {
    inner: CheckReport,
}

impl Report {
    /// Report of a cold search. Counts refer to its selected-root graph.
    /// A holding result on that graph is a bounded search miss.
    #[must_use]
    pub fn to_json(&self, convention: ExitConvention) -> Value {
        let mut result = self.inner.to_json_with_exit_convention(convention);
        result["check_kind"] = json!("cold-fair");
        result["options"]["initial_selection"] = json!("first-admitted-original-init");
        result["model_proved"] = json!(false);
        result["witness_validated"] = json!(result["counterexample"]["trace"].is_object());
        for key in ["execution", "periodic"] {
            if let Some(facts) = result[key].as_object_mut() {
                if let Some(exhausted) = facts.remove("exhausted") {
                    facts.insert("selected_graph_exhausted".into(), exhausted);
                }
                facts.insert("exhausted".into(), json!(false));
                if let Some(captured) = facts.remove("capture_completed") {
                    facts.insert("selected_capture_completed".into(), captured);
                    facts.insert("capture_completed".into(), json!(false));
                }
            }
        }
        if let Some(stats) = result["statistics"].as_object_mut() {
            stats.insert("exhaustive".into(), json!(false));
            stats.insert("reported_by_tlc".into(), json!(false));
        }
        if self.inner.verdict() == "success" {
            result["verdict"] = json!("bounded-inconclusive");
            result["message"] = json!("no counterexample in the selected-root graph");
            result["tlc"] = Value::Null;
        }
        result
    }

    /// Process status using the selected convention. A completed bounded
    /// miss uses zero with the explicit inconclusive report.
    #[must_use]
    pub fn exit_code(&self, convention: ExitConvention) -> u8 {
        self.inner.exit_code_for(convention)
    }

    /// Human summary that names the scope and never reports a holding model.
    #[must_use]
    pub fn render_human(&self, convention: ExitConvention) -> String {
        if self.inner.verdict() == "success" {
            let report = self.to_json(convention);
            format!(
                "Cold fair search: bounded-inconclusive. First admitted original Init root, seed {}, {} generated, {} distinct.\n",
                report["options"]["seed"],
                report["statistics"]["generated"],
                report["statistics"]["distinct"]
            )
        } else {
            format!(
                "Cold fair search from the first admitted original Init root.\n{}",
                self.inner.render_human_with_exit_convention(convention)
            )
        }
    }
}

/// Checks for a temporal counterexample reachable from the first admitted
/// original Init row. Core explores exact states and uses the original
/// configured transition relation and obligations. Every reported violation
/// is checked by the same independent validator as an ordinary check.
///
/// ```no_run
/// use dive_api::{CheckRequest, ExitConvention, cold_fair::check_file};
/// let request = CheckRequest { file: "Spec.tla".into(), ..CheckRequest::default() };
/// let report = check_file(&request)?;
/// assert_eq!(report.to_json(ExitConvention::Native)["model_proved"], false);
/// # Ok::<(), dive_api::ApiError>(())
/// ```
///
/// # Errors
/// File or resource startup errors as in [`crate::check_file`]. A nonlocal
/// search, explicit TLC compatibility, a debugger or an incompatible mode
/// refuses by name.
pub fn check_file(request: &CheckRequest) -> Result<Report, ApiError> {
    if request.compat_tlc
        || request.distributed.is_some()
        || request.debugger.is_some()
        || request.core_sample.is_some()
        || request.temporal_refusal.is_some()
    {
        return Err(ApiError::Unsupported {
            feature: "cold fair search requires a local Dive temporal check".into(),
        });
    }
    let request = CheckRequest {
        store: Store::Exact,
        ..request.clone()
    };
    crate::check::check_file_in(&request, None, false, None, true).map(|inner| Report { inner })
}
