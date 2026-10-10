//! Transformed key admission refuses coherent corruption rather than inventing model verdicts.
#![allow(clippy::expect_used, clippy::panic, reason = "native archive controls")]
use dive_check::{CheckOptions, Verdict, checkpoint};
use sha2::{Digest, Sha256};
use std::sync::Arc;

fn directory() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let p = std::env::temp_dir().join(format!(
        "dive-native-key-check-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&p).expect("directory");
    p
}
fn write(path: &std::path::Path, value: &serde_json::Value) {
    let payload = serde_json::to_vec(value).expect("payload");
    let mut bytes = b"DIVECP01".to_vec();
    bytes.extend((payload.len() as u64).to_le_bytes());
    bytes.extend(Sha256::digest(&payload));
    bytes.extend(payload);
    std::fs::write(path, bytes).expect("coherent checksum");
}

#[test]
fn configured_view_key_failure_is_import_failure_not_a_model_verdict() {
    let analysis = dive_sema::analyze_source(
        "ViewKey.tla",
        "---- MODULE ViewKey ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x<4 /\\ x'=x+1\nView == x+0\n====\n",
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lower"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nVIEW View\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .expect("cfg");
    let dir = directory();
    for store in [
        dive_store::StoreMode::Fingerprint,
        dive_store::StoreMode::Exact,
    ] {
        for mut options in [CheckOptions::tlc(0, 7), CheckOptions::dive(7)] {
            options.store = store;
            let cp = checkpoint::Request {
                path: dir.join(format!("{store:?}-{:?}.cp", options.profile)),
                action: checkpoint::Action::PauseAfterLevels(1),
                counters: checkpoint::Counters::Retain,
                config_source_sha256: None,
            };
            let paused = checkpoint::check(&ir, &cfg, &options, &cp);
            assert!(paused.receipt.is_some(), "{:?}", paused.check);
            let raw = std::fs::read(&cp.path).expect("archive");
            let original: serde_json::Value = serde_json::from_slice(&raw[48..]).expect("payload");
            for kind in [0, 1] {
                let mut bad = original.clone();
                if kind == 0 {
                    // TRUE is a correctly encoded single concrete value, but
                    // cannot be evaluated by the configured arithmetic VIEW.
                    bad["discoveries"][0]["state"] = serde_json::json!([1]);
                } else {
                    let key = bad["discoveries"][0]["key"].as_array_mut().expect("key");
                    key[0] = serde_json::json!(key[0].as_u64().expect("byte") ^ 255);
                }
                let path = dir.join(format!("bad-{store:?}-{:?}-{kind}.cp", options.profile));
                write(&path, &bad);
                let recovered = checkpoint::check(
                    &ir,
                    &cfg,
                    &options,
                    &checkpoint::Request {
                        path,
                        action: checkpoint::Action::Recover,
                        ..cp.clone()
                    },
                );
                let Verdict::ResourceLimit { message } = &recovered.check.verdict else {
                    panic!("{:?}", recovered.check);
                };
                assert!(
                    message.contains(if kind == 0 {
                        "configured state/key admission"
                    } else {
                        "state/key mismatch"
                    }),
                    "{message}"
                );
                assert!(recovered.receipt.is_none());
                assert!(recovered.check.counterexample.is_none());
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn native_override_authority_and_substitutions_preserve_pre_effect_refusals() {
    // All cases also declare symmetry and VIEW: neither key transformation may
    // conceal a source constructor/effect. Native Permutations is a positive
    // override; a local same-name body is not treated as native by spelling.
    let cases = [
        (
            "native-pure",
            "",
            "Init == x=0",
            "View == x",
            "Perms == Permutations({A,B})",
            "",
            true,
        ),
        (
            "constant-pure",
            "CONSTANT Maker(_)\n",
            "Init == x=0",
            "View == x",
            "Perms == Maker({A,B})",
            "CONSTANT Maker <- Permutations\n",
            true,
        ),
        (
            "local-shadow-pure",
            "",
            "Init == (LET PrintT(v) == v IN PrintT(TRUE)) /\\ x=0",
            "View == x",
            "Perms == Permutations({A,B})",
            "",
            true,
        ),
        (
            "local-fallback-effect",
            "",
            "Init == (LET PrintT(v) == TLCSet(0,v) IN PrintT(TRUE)) /\\ x=0",
            "View == x",
            "Perms == Permutations({A,B})",
            "",
            false,
        ),
        (
            "constant-effect",
            "CONSTANT Gate(_)\n",
            "Init == Gate(TRUE) /\\ x=0",
            "View == x",
            "Perms == Permutations({A,B})",
            "CONSTANT Gate <- PrintT\n",
            false,
        ),
        (
            "definition-effect",
            "Gate(v) == v\n",
            "Init == Gate(TRUE) /\\ x=0",
            "View == x",
            "Perms == Permutations({A,B})",
            "CONSTANT Gate <- PrintT\n",
            false,
        ),
        (
            "view-effect",
            "",
            "Init == x=0",
            "View == IF PrintT(x) THEN x ELSE x",
            "Perms == Permutations({A,B})",
            "",
            false,
        ),
        (
            "symmetry-effect",
            "",
            "Init == x=0",
            "View == x",
            "Perms == IF PrintT(A) THEN Permutations({A,B}) ELSE {}",
            "",
            false,
        ),
        (
            "view-name",
            "",
            "Init == x=0",
            "View == ToString(x)",
            "Perms == Permutations({A,B})",
            "",
            false,
        ),
        (
            "unused-startup-effect",
            "Unused == PrintT(1)\n",
            "Init == x=0",
            "View == x",
            "Perms == Permutations({A,B})",
            "",
            false,
        ),
    ];
    let dir = directory();
    for (label, declarations, init, view, symmetry, substitutions, accepted) in cases {
        let source = format!(
            "---- MODULE NativeAdmission ----\nEXTENDS Naturals,TLC\nCONSTANTS A,B\nVARIABLE x\n{declarations}{init}\nNext == x<3 /\\ x'=x+1\n{view}\n{symmetry}\n====\n"
        );
        let analysis = dive_sema::analyze_source(
            "NativeAdmission.tla",
            &source,
            &dive_sema::Options::default(),
        );
        assert!(analysis.accepted(), "{label}: {:?}", analysis.diagnostics());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        assert!(
            ir.defs()
                .iter()
                .any(|d| d.std == Some(dive_ir::StdOp::TlcPermutations)),
            "{label}: lowered native authority"
        );
        if label.starts_with("local-") {
            assert!(
                ir.defs()
                    .iter()
                    .any(|d| d.name == "PrintT" && d.std.is_none()),
                "{label}: local same-name body"
            );
        }
        let config = format!(
            "CONSTANT A=p1 B=p2\n{substitutions}INIT Init\nNEXT Next\nVIEW View\nSYMMETRY Perms\nCHECK_DEADLOCK FALSE\n"
        );
        let cfg = dive_config::parse(&config).into_config().expect("config");
        for (profile, options) in [
            ("tlc", CheckOptions::tlc(0, 7)),
            ("dive", CheckOptions::dive(7)),
        ] {
            let path = dir.join(format!("{label}-{profile}.cp"));
            let out = checkpoint::check(
                &ir,
                &cfg,
                &options,
                &checkpoint::Request {
                    path: path.clone(),
                    action: checkpoint::Action::PauseAfterLevels(0),
                    counters: checkpoint::Counters::Retain,
                    config_source_sha256: None,
                },
            );
            if accepted {
                assert!(
                    matches!(out.check.verdict, Verdict::ResourceLimit { .. }),
                    "{label}: {:?}",
                    out.check
                );
                assert!(out.receipt.is_some(), "{label}: {:?}", out.check);
                assert!(path.is_file());
            } else {
                assert!(
                    matches!(out.check.verdict, Verdict::Unsupported { .. }),
                    "{label}: {:?}",
                    out.check
                );
                assert!(out.receipt.is_none());
                assert_eq!(out.check.workers.used, 0);
                assert!(
                    out.check.output.is_empty(),
                    "{label}: source effect executed"
                );
                assert!(!path.exists());
            }
        }
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}
