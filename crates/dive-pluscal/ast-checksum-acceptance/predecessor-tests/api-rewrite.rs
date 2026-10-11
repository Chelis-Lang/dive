//! Source replacement admission stays separate from checksum and model claims.
use dive_api::{ChecksumPolicy, RewriteOptions, RewriteRequest, rewrite_source};
const SOURCE: &str = "---- MODULE R ----\nEXTENDS Integers\n(* --algorithm A variable x=0; begin L:x:=1; E:skip; end algorithm; *)\n\\* footer\n====\n";
fn request() -> RewriteRequest {
    RewriteRequest {
        editing: RewriteOptions {
            checksums: ChecksumPolicy::TranslationOnly,
            ..RewriteOptions::default()
        },
        ..RewriteRequest::default()
    }
}
#[test]
fn explicit_partial_replacement_is_admitted_without_proof() {
    let request = request();
    let report = rewrite_source("R.tla", SOURCE, &request);
    assert!(report.accepted(), "{}", report.to_json());
    let doc = report.to_json();
    assert_eq!(doc["result"], "replacement");
    assert_eq!(doc["model_proved"], false);
    assert_eq!(doc["exhaustive"], false);
    assert_eq!(doc["algorithm_checksum_verified"], false);
    assert_eq!(doc["body_checksum_verified"], true);
    assert_eq!(doc["exact_primary_generated_bytes"], false);
    assert!(doc["replacement"]["module_source"].is_string());
    let Some(source) = doc["replacement"]["module_source"].as_str() else {
        return;
    };
    let next = rewrite_source("R.tla", source, &request);
    assert!(next.accepted(), "{}", next.to_json());
    assert_eq!(
        next.to_json()["replacement"]["selection"]["checksum"]["matched"],
        true
    );
    let mut compatibility = request;
    compatibility.translation.compat_tlc = true;
    let report = rewrite_source("R.tla", SOURCE, &compatibility);
    assert!(report.accepted());
    assert_eq!(report.to_json()["profile"], "tlc+overrides");
    let full = rewrite_source("R.tla", SOURCE, &RewriteRequest::default());
    assert!(!full.accepted());
    assert_eq!(full.to_json()["stage"], "checksum-policy");
}
#[test]
fn failed_generation_and_bad_old_checksum_do_not_return_replacement() {
    let request = request();
    let bad = SOURCE.replace("x:=1", "undefined:=1");
    let report = rewrite_source("R.tla", &bad, &request);
    assert!(!report.accepted());
    assert!(report.to_json().get("replacement").is_none());
    let report = rewrite_source("R.tla", SOURCE, &request);
    assert!(report.accepted());
    let doc = report.to_json();
    assert!(doc["replacement"]["module_source"].is_string());
    if let Some(source) = doc["replacement"]["module_source"].as_str() {
        let modified = source.replace("x' = 1", "x' = 2");
        assert_ne!(modified, source);
        let denied = rewrite_source("R.tla", &modified, &request);
        assert!(!denied.accepted());
        assert_eq!(denied.to_json()["stage"], "replacement");
        assert!(denied.to_json().get("replacement").is_none());
        let grant = RewriteRequest {
            editing: RewriteOptions {
                replace_modified: true,
                ..request.editing
            },
            ..request
        };
        assert!(rewrite_source("R.tla", &modified, &grant).accepted());
    }
}
