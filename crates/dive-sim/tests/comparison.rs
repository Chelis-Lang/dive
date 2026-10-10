//! The control comparator must reject forged proof and witness metadata.
#[test]
#[allow(
    clippy::expect_used,
    reason = "comparison control setup must fail the test"
)]
fn comparison_rejects_forged_sampling_evidence() {
    let script =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test-comparison.py");
    let output = std::process::Command::new("python3")
        .arg("-I")
        .arg(script)
        .output()
        .expect("comparison controls must run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
