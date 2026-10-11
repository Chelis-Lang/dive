"""Verify qualified frozen DEFINE runtime/gate evidence without retagging it."""
from pathlib import Path
import argparse,gzip,hashlib,json
p=argparse.ArgumentParser();p.add_argument('--artifacts',type=Path);a=p.parse_args()
base=Path(__file__).resolve().parent
sha=lambda raw:hashlib.sha256(raw).hexdigest()
d=json.loads((base/'delivery.json').read_text());assert d['runtime_source_commit']is None and d['all_consumers_closed']is True and d['family_certified']is False
source=json.loads((base/d['runtime_source_archive']).read_text());assert sha((base/d['runtime_source_archive']).read_bytes())==d['runtime_source_archive_sha256'];assert len(source['source'])==185 and source['source_commit']is None
for n,m in source['source'].items():
 raw=(base/m['archive']).read_bytes();assert sha(raw)==m['archive_sha256'];assert sha(gzip.decompress(raw))==m['raw_sha256'],n
for n,m in d['gate_logs'].items():
 raw=(base/m['archive']).read_bytes();assert sha(raw)==m['archive_sha256']and sha(gzip.decompress(raw))==m['raw_sha256']and m['exit_code']==0,n
for n,h in d['reports'].items():assert sha((base/'reports'/n).read_bytes())==h,n
full=json.loads((base/'reports/full-gate.json').read_text());assert full['passed']is True and len(full['completed'])==full['all_stages']==33
assert d['tests_run']==d['tests_passed']==1097 and d['tests_skipped']==0
assert d['gate_only_source_changes']==['crates/dive-pluscal/tests/definitions.rs','crates/dive-pluscal/tests/rewrite.rs']
public=json.loads((base/'reports/public.json').read_text());assert public['selected']==272 and public['summary']=={'admitted-exact-cst-sany-validator':160,'invalid-primary-uncredited':88,'named-public-refusal':20,'rejection-accounted':4}
models=json.loads((base/'reports/models.json').read_text());assert models['selected']==24 and models['worker_store_controls']==288 and models['valid_witness_controls']==12 and models['rejected_mutants']==48
if a.artifacts:
 for n,h in d['runtime_binary_sha256'].items():assert sha((a.artifacts/'final'/n).read_bytes())==h,n
 for n,h in d['runtime_artifact_sha256'].items():assert sha((a.artifacts/n).read_bytes())==h,n
print('Verified185 qualified source archives,33 gate stages/1097passing tests,37-model gate logs,closed reports and optional4524runtime files; no family certification.')
