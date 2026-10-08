//! dive's collision estimates against TLC's.
//!
//! - Every conformance golden of the pinned oracle that records TLC's
//!   "calculated (optimistic)" figure is reproduced exactly from the golden's distinct
//!   and generated counts, and whether TLC also printed the "actual" figure follows
//!   TLC's rule.
//! - The "actual" figure is reproduced from TLC's own fingerprints for two models
//!   (`tests/fixtures/tlc-fingerprints`).
//! - TLC's probability printing is reproduced on 2,137 sampled cases
//!   (`tests/fixtures/tlc-probability`).

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a failed expectation is a test failure"
)]

use std::path::{Path, PathBuf};

use dive_store::collision::{
    TLC_ACTUAL_THRESHOLD, TLC_DEFAULT_FP_BITS, actual_probability, java_double_to_string,
    optimistic_probability, probability_to_string, tlc_min_gap, tlc_success_figures,
};
use dive_store::{FpSet, StateSet};

const ORACLE: &str = "tlc-2026.10.06.014338";

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn goldens(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            goldens(&p, out);
        } else if p.extension().is_some_and(|e| e == "json") {
            out.push(p);
        }
    }
}

struct Figures {
    model: String,
    distinct: u64,
    generated: u64,
    optimistic: String,
    observed: Option<String>,
}

fn golden_figures() -> Vec<Figures> {
    let root = manifest_dir().join("../../conformance/golden").join(ORACLE);
    let mut files = Vec::new();
    goldens(&root, &mut files);
    let mut out = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).expect("readable golden");
        let g: serde_json::Value = serde_json::from_str(&text).expect("golden JSON");
        let r = &g["result"];
        let Some(opt) = r["fingerprint_collision"]["optimistic"].as_str() else {
            continue;
        };
        out.push(Figures {
            model: g["model"]["id"].as_str().unwrap_or("?").to_owned(),
            distinct: r["counts"]["distinct_states"].as_u64().expect("distinct"),
            generated: r["counts"]["states_generated"].as_u64().expect("generated"),
            optimistic: opt.to_owned(),
            observed: r["fingerprint_collision"]["observed"]
                .as_str()
                .map(str::to_owned),
        });
    }
    out
}

#[test]
fn optimistic_figure_matches_every_golden() {
    let figures = golden_figures();
    // The pinned oracle's goldens record 124 such figures; fail loudly if the
    // goldens moved rather than pass vacuously.
    assert!(figures.len() >= 100, "found only {} figures", figures.len());
    for f in &figures {
        let p = optimistic_probability(f.distinct, f.generated);
        assert_eq!(
            format!("val = {}", probability_to_string(p, 2)),
            f.optimistic,
            "{}: {} distinct, {} generated",
            f.model,
            f.distinct,
            f.generated
        );
        // TLC prints the actual figure exactly when the optimistic one is >= 1E-10.
        assert_eq!(
            f.observed.is_some(),
            p >= TLC_ACTUAL_THRESHOLD,
            "{}: actual-figure rule",
            f.model
        );
        let report = tlc_success_figures(f.distinct, f.generated, || 1);
        assert_eq!(format!("val = {}", report.optimistic), f.optimistic);
        assert_eq!(report.actual.is_some(), f.observed.is_some());
    }
}

fn read_fps(model: &str) -> Vec<u64> {
    let path = manifest_dir()
        .join("tests/fixtures/tlc-fingerprints")
        .join(format!("{model}.fp64"));
    let bytes = std::fs::read(path).expect("fingerprint fixture");
    let (words, rest) = bytes.as_chunks::<8>();
    assert_eq!(rest, &[] as &[u8]);
    words.iter().map(|w| u64::from_le_bytes(*w)).collect()
}

#[test]
fn actual_figure_matches_tlc_from_tlc_fingerprints() {
    let figures = golden_figures();
    for (model, id) in [
        ("VoteProof", "specifications/byzpaxos/VoteProof.cfg"),
        (
            "nbacg_guer01",
            "specifications/nbacg_guer01/nbacg_guer01.cfg",
        ),
    ] {
        let fps = read_fps(model);
        let golden = figures
            .iter()
            .find(|f| f.model == id)
            .expect("golden for the model");
        assert_eq!(
            fps.len() as u64,
            golden.distinct,
            "{model}: one fingerprint per state"
        );

        // From a plain list, and from dive's sharded fingerprint set.
        let gap = tlc_min_gap(fps.iter().copied(), TLC_DEFAULT_FP_BITS);
        for shard_bits in [0, 3, 8] {
            let mut set = FpSet::new(shard_bits);
            for &fp in &fps {
                assert!(set.insert(fp));
            }
            assert_eq!(set.tlc_min_gap(TLC_DEFAULT_FP_BITS), gap, "{model}");
        }
        let report = tlc_success_figures(golden.distinct, golden.generated, || gap);
        assert_eq!(format!("val = {}", report.optimistic), golden.optimistic);
        assert_eq!(
            report.actual.map(|a| format!("val = {a}")),
            golden.observed,
            "{model}"
        );

        // The figures in TLC's own output for the capture run agree too.
        let stdout = std::fs::read_to_string(
            manifest_dir()
                .join("tests/fixtures/tlc-fingerprints")
                .join(format!("{model}.stdout")),
        )
        .expect("captured output");
        let actual = probability_to_string(actual_probability(gap), 2);
        assert!(stdout.contains(&format!(
            "based on the actual fingerprints:  val = {actual}"
        )));
    }
}

#[test]
fn ignoring_tlc_sub_sets_would_give_a_different_figure() {
    // Evidence for the sub-set rule: without it, nbacg_guer01's figure would be
    // 1.7E-10, not TLC's 1.5E-11.
    let fps = read_fps("nbacg_guer01");
    let masked = tlc_min_gap(fps.iter().map(|fp| fp & !(1 << 63)), TLC_DEFAULT_FP_BITS);
    assert_eq!(
        probability_to_string(actual_probability(masked), 2),
        "1.7E-10"
    );
    let tlc = tlc_min_gap(fps.iter().copied(), TLC_DEFAULT_FP_BITS);
    assert_eq!(probability_to_string(actual_probability(tlc), 2), "1.5E-11");
}

#[test]
fn probability_printing_matches_tlc_cases() {
    let path = manifest_dir().join("tests/fixtures/tlc-probability/cases.tsv");
    let text = std::fs::read_to_string(path).expect("cases");
    let mut n = 0;
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let cols: Vec<&str> = line.split('\t').collect();
        let input: Vec<&str> = cols[0].split(' ').collect();
        match input[0] {
            "d" => {
                let x = f64::from_bits(u64::from_str_radix(input[1], 16).expect("bits"));
                assert_eq!(java_double_to_string(x), cols[2], "Double.toString {line}");
                assert_eq!(probability_to_string(x, 2), cols[3], "{line}");
            }
            "o" => {
                let p = optimistic_probability(
                    input[1].parse().expect("distinct"),
                    input[2].parse().expect("generated"),
                );
                assert_eq!(probability_to_string(p, 2), cols[2], "{line}");
            }
            "g" => {
                let p = actual_probability(input[1].parse().expect("gap"));
                assert_eq!(probability_to_string(p, 2), cols[2], "{line}");
            }
            other => panic!("unknown case kind {other}"),
        }
        n += 1;
    }
    assert_eq!(n, 2137);
}
