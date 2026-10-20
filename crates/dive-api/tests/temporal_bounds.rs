//! `--temporal-bounds`: the default profile keeps dive's fixed temporal
//! analysis bounds; `--compat=tlc` selects TLC's setting, no fixed bound.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
use dive_api::{CheckRequest, TemporalBounds, TemporalMemory, check_file};

/// One more temporal property than the default bound admits (256).
const PROPERTIES: usize = 257;

fn request(
    directory: &std::path::Path,
    compat_tlc: bool,
    bounds: Option<TemporalBounds>,
) -> CheckRequest {
    CheckRequest {
        file: directory.join("Bounds.tla"),
        compat_tlc,
        temporal_bounds: bounds,
        seed: Some(0),
        fp: Some(0),
        workers: Some(1),
        ..CheckRequest::default()
    }
}

#[test]
fn compat_lifts_fixed_temporal_bounds_and_explicit_settings_win() {
    let directory =
        std::env::temp_dir().join(format!("dive-temporal-bounds-{}", std::process::id()));
    std::fs::create_dir(&directory).expect("private directory");
    let names: Vec<String> = (0..PROPERTIES).map(|i| format!("P{i}")).collect();
    let definitions = names.iter().fold(String::new(), |mut text, name| {
        text.push_str(name);
        text.push_str(" == []<>(~x)\n");
        text
    });
    std::fs::write(
        directory.join("Bounds.tla"),
        format!(
            "---- MODULE Bounds ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nSpec == Init /\\ [][Next]_x /\\ WF_x(Next)\n{definitions}====\n"
        ),
    )
    .expect("source");
    std::fs::write(
        directory.join("Bounds.cfg"),
        format!("SPECIFICATION Spec\nPROPERTY {}\n", names.join(" ")),
    )
    .expect("cfg");
    for (compat_tlc, bounds, setting) in [
        (false, None, "default"),
        (true, None, "unbounded"),
        (true, Some(TemporalBounds::Default), "default"),
        (false, Some(TemporalBounds::Unbounded), "unbounded"),
    ] {
        let doc = check_file(&request(&directory, compat_tlc, bounds))
            .expect("checked")
            .to_json();
        let reported = &doc["options"]["temporal_bounds"];
        assert_eq!(reported["setting"], setting, "{doc}");
        if setting == "default" {
            assert_eq!(doc["verdict"], "resource-limit", "{doc}");
            assert_eq!(reported["properties"], 256);
            assert!(
                doc["message"]
                    .as_str()
                    .is_some_and(|m| m.contains("properties")),
                "{doc}"
            );
        } else {
            assert_eq!(doc["verdict"], "success", "{doc}");
            assert_eq!(reported["kind"], "unbounded");
            assert_eq!(
                reported["resident_ceiling_setting"],
                "quarter-of-physical-memory"
            );
            assert_eq!(
                reported["resident_ceiling_bytes"],
                serde_json::json!(dive_liveness::resident::default_ceiling())
            );
            assert_eq!(doc["statistics"]["distinct"], 2);
            assert_eq!(doc["statistics"]["generated"], 3);
        }
    }
    // An explicit ceiling is applied and reported. One byte is below any
    // process, so the analysis refuses by name at its first stage boundary,
    // never with a holding result.
    if dive_liveness::resident::resident_bytes().is_some() {
        let mut tight = request(&directory, true, None);
        tight.temporal_memory = TemporalMemory::Bytes(1);
        let doc = check_file(&tight).expect("checked").to_json();
        let reported = &doc["options"]["temporal_bounds"];
        assert_eq!(reported["resident_ceiling_setting"], "explicit", "{doc}");
        assert_eq!(reported["resident_ceiling_bytes"], 1, "{doc}");
        assert_eq!(doc["verdict"], "resource-limit", "{doc}");
        assert!(
            doc["message"]
                .as_str()
                .is_some_and(|m| m.contains("resident memory")),
            "{doc}"
        );
    }
    // A generous explicit ceiling changes nothing but the report.
    let mut generous = request(&directory, true, None);
    generous.temporal_memory = TemporalMemory::Bytes(u64::MAX);
    let doc = check_file(&generous).expect("checked").to_json();
    assert_eq!(doc["verdict"], "success", "{doc}");
    assert_eq!(
        doc["options"]["temporal_bounds"]["resident_ceiling_bytes"],
        u64::MAX
    );
    std::fs::remove_dir_all(directory).expect("cleanup");
}
