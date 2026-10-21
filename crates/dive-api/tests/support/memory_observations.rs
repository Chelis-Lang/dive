//! Validate finite-budget sampling while excluding two run-specific numbers.

/// Retains the entire memory contract and all semantic report fields.
///
/// RSS and watchdog sample counts vary between genuine executions, just like
/// elapsed time. These comparisons require a completed finite-budget sampling
/// lifecycle before normalizing only its observed maximum and sample count.
pub fn normalize(report: &mut serde_json::Value) {
    assert!(report["options"]["budget"]["max_memory_bytes"].is_u64());
    let memory = report["resources"]["memory"]
        .as_object_mut()
        .expect("finite budget reports memory observations");
    assert_eq!(memory["status"], "sampled");
    assert_eq!(memory["initial_sampled"], true);
    assert_eq!(memory["final_sampled"], true);
    assert!(memory["unavailable_at"].is_null());
    assert!(matches!(
        memory["metric"].as_str(),
        Some("rss" | "rss+swap")
    ));
    assert!(memory["observed_max_bytes"].as_u64().is_some_and(|n| n > 0));
    assert!(memory["samples"].as_u64().is_some_and(|n| n >= 2));
    memory["observed_max_bytes"] = serde_json::json!(0);
    memory["samples"] = serde_json::json!(0);
}
