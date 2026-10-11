fn main() {
use dive_api::{RewriteOptions, RewriteRequest, rewrite_source};
let source = "---- MODULE R ----\nEXTENDS Integers\n(* --algorithm A variable x=0; begin L:x:=1; E:skip; end algorithm; *)\n====\n";
let request = RewriteRequest {
    editing: RewriteOptions::default(),
    ..RewriteRequest::default()
};
let report = rewrite_source("R.tla", source, &request);
assert!(report.accepted());
assert_eq!(report.to_json()["model_proved"], false);
assert_eq!(report.to_json()["algorithm_checksum_verified"], true);
println!("{}", report.to_json());
}
