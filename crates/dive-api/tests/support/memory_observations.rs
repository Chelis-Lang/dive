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
    // Core's engine block (its disabled switches and memo, store and plan
    // counters) is engine bookkeeping, not an observable result (ADR 0086);
    // the counters differ between a bounded or resumed run and an
    // uninterrupted one.
    if let Some(fields) = report.as_object_mut() {
        fields.remove("core");
    }
    // Phase timings vary between genuine executions like elapsed time.
    zero_timings(report);
}

fn zero_timings(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, child) in fields.iter_mut() {
                if key.ends_with("_ms") && child.is_number() {
                    *child = serde_json::json!(0);
                } else {
                    zero_timings(child);
                }
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(zero_timings),
        _ => {}
    }
}
