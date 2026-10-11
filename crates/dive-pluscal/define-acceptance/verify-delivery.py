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

# Root publication readback: raw observations remain their original producer.
portable=base/'runtime-records'
manifest=json.loads((portable/'manifest.json').read_text())
assert manifest['runtime_source_commit'] is None
assert manifest['runtime_parent_commit']==d['runtime_parent_commit']
assert manifest['runtime_binary_sha256']==d['runtime_binary_sha256']
assert manifest['files_sha256']==d['runtime_artifact_sha256']
assert len(manifest['files_sha256'])==4524
for name,digest in manifest['files_sha256'].items():
 relative=Path(name)
 assert not relative.is_absolute() and '..' not in relative.parts,name
 assert sha((portable/relative).read_bytes())==digest,name
# These exact committed test bytes are post-delivery evidence, not retroactively
# part of the scoped uncommitted runtime source archive.
readback=json.loads((base/'root-readback/readback.json').read_text())
assert readback['runtime_source_commit'] is None and readback['family_certified'] is False
assert readback['delivery_commit']=='68456837d9d75bdfc3b8ee5d36c1d658c8c93038'
assert readback['separate_promotion_authority']['crates/dive-pluscal/tests/macro_checksum.rs']['original_current_map_sha256'] is None
promotions=json.loads((base/'root-readback/test-source.json').read_text())
assert promotions['runtime_authority'] is False and promotions['family_certified'] is False
assert len(promotions['inputs'])==5
for name,row in promotions['inputs'].items():
 raw=(base/'root-readback'/row['archive']).read_bytes()
 assert sha(raw)==row['archive_sha256'] and sha(gzip.decompress(raw))==row['delivery_git_sha256'],name
 assert name.startswith('crates/') and '/tests/' in name,name
# Read actual results in addition to the campaign summaries.
assert all(row['exit_code']==0 for row in full['completed'])
nextest=next(row for row in full['completed'] if 'nextest' in row['command'])
assert '1097 tests run: 1097 passed, 0 skipped' in gzip.decompress((base/'gate-logs'/(nextest['log']+'.gz')).read_bytes()).decode()
settings=valid=invalid=0
assert len(models['rows'])==24
for row in models['rows']:
 assert row['outcome']=='finite-model-match' and len(row['settings'])==12
 assert {(x['profile'],x['workers'],x['store']) for x in row['settings']}=={(profile,width,store) for profile in ['dive','tlc'] for width in [1,4,8] for store in ['fp','exact']}
 for setting in row['settings']:
  assert setting['outcome']=='match'
  stream=setting['capture']['stdout'];raw=gzip.decompress((portable/'models-final'/stream['path']).read_bytes());assert sha(raw)==stream['sha256']
  result=json.loads(raw)
  assert {key:result['statistics'][key] for key in row['primary_counts']}==row['primary_counts']
  assert result['verdict']==('success' if row['primary']['exit_code']==0 else 'invariant-violation')
  settings+=1
 for control in row['independent_validation']:
  stream=control['capture']['stdout'];raw=gzip.decompress((portable/'models-final'/stream['path']).read_bytes());assert sha(raw)==stream['sha256'];result=json.loads(raw)
  if control['control']=='original':
   assert result['result']=='valid' and control['capture']['exit_code']==0;valid+=1
  else:
   assert result['result']=='invalid' and control['capture']['exit_code']==1;invalid+=1
assert (settings,valid,invalid)==(288,12,48)
print('Verified4524 portable original runtime files,288 actual model settings,12 Valid/48 Invalid replay controls and separately qualified committed test archives; no runtime retag or family certification.')
