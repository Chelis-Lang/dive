//! A refused start-up evaluation of an impure constant definition refuses the
//! model only when the checking path reaches the definition or TLC's
//! evaluation of it may have had a lasting effect (issue #852).
#![allow(
    clippy::expect_used,
    reason = "fixture setup and behavioral assertions"
)]

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Dir(PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Checks `M.tla` made of `defs` and `cfg` with `extra` flags.
fn check(defs: &str, cfg: &str, tlc: bool, extra: &[&str]) -> (Option<i32>, serde_json::Value) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = Dir(std::env::temp_dir().join(format!(
        "dive-cli-unreachable-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    std::fs::create_dir(&dir.0).expect("fixture");
    let source = format!(
        "---- MODULE M ----\nEXTENDS Naturals, TLC, IOUtils\nVARIABLE x\n\
         Init == x = 0\nNext == x' = (x + 1) % 3\nTypeOK == x \\in 0..2\n{defs}\n====\n"
    );
    std::fs::write(dir.0.join("M.tla"), source).expect("spec");
    std::fs::write(dir.0.join("M.cfg"), cfg).expect("config");
    let modules = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-eval/tests/fixtures/community-modules");
    let mut run = Command::new(env!("CARGO_BIN_EXE_dive"));
    run.env_clear()
        .env("BF", "{1}")
        .current_dir(&dir.0)
        // External inputs run on the reference engine only (Core refuses).
        .args(["check", "M.tla", "--output", "json", "--workers", "1"])
        .args(["--engine", "reference"])
        .arg("--library-path")
        .arg(modules)
        .args(extra);
    if tlc {
        run.arg("--compat=tlc");
    }
    let output: Output = run.output().expect("CLI");
    let report = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        serde_json::json!({
            "stdout": String::from_utf8_lossy(&output.stdout),
            "stderr": String::from_utf8_lossy(&output.stderr),
        })
    });
    (output.status.code(), report)
}

const CFG: &str = "INIT Init\nNEXT Next\nINVARIANT TypeOK\n";
const READ: &str = "BF == CHOOSE s \\in SUBSET (1..3) : ToString(s) = IOEnv.BF";

fn assert_success((code, report): (Option<i32>, serde_json::Value)) {
    assert_eq!(code, Some(0), "{report}");
    assert_eq!(report["verdict"], "success", "{report}");
    assert_eq!(report["statistics"]["distinct"], 3, "{report}");
}

fn assert_env_refusal((code, report): (Option<i32>, serde_json::Value)) {
    assert_eq!(code, Some(3), "{report}");
    assert_eq!(report["verdict"], "unsupported", "{report}");
    let message = report["message"].as_str().expect("refusal");
    assert!(message.contains("--allow-io=env"), "{report}");
}

#[test]
fn an_unreachable_environment_read_does_not_refuse_the_model() {
    assert_success(check(READ, CFG, false, &[]));
    assert_success(check(READ, CFG, false, &["--allow-io=env"]));
}

#[test]
fn compatibility_order_keeps_an_unreachable_environment_read_refused() {
    // TLC's evaluation would have interned the environment's strings and
    // the `ToString` results, which changes later TLC order.
    assert_env_refusal(check(READ, CFG, true, &[]));
}

#[test]
fn a_reachable_environment_read_still_needs_the_grant() {
    let defs = format!("{READ}\nUses == x \\in BF \\cup 0..2");
    let cfg = "INIT Init\nNEXT Next\nINVARIANT TypeOK\nINVARIANT Uses\n";
    for tlc in [false, true] {
        assert_env_refusal(check(&defs, cfg, tlc, &[]));
    }
    // `--compat=tlc` refuses this IOEnv construction order separately.
    assert_success(check(&defs, cfg, false, &["--allow-io=env"]));
    let live = format!("{READ}\nLive == <>(x \\in BF)");
    for tlc in [false, true] {
        assert_env_refusal(check(
            &live,
            "INIT Init\nNEXT Next\nPROPERTY Live\n",
            tlc,
            &[],
        ));
    }
}

#[test]
fn an_unreachable_read_with_a_lasting_effect_is_still_refused() {
    // TLC prints when the variable is set; dive cannot know without the grant.
    let defs = "Shown == PrintT(IOEnv.BF)";
    for tlc in [false, true] {
        assert_env_refusal(check(defs, CFG, tlc, &[]));
    }
}
