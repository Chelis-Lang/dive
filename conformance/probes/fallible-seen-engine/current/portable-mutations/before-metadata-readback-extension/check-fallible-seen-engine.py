#!/usr/bin/env python3
"""Read back the bounded disk seen-state engine evidence; not certification."""
from pathlib import Path
import copy,gzip,hashlib,itertools,json,re,subprocess
ROOT=Path(__file__).resolve().parent.parent
PROBE=ROOT/'conformance/probes/fallible-seen-engine'
def sha(data):return hashlib.sha256(data).hexdigest()
def load(path):return json.loads(path.read_text())
def safe(name):
 p=Path(name)
 assert not p.is_absolute() and '..' not in p.parts
 resolved=(ROOT/p).resolve();assert resolved.is_relative_to(ROOT.resolve())
 return resolved
def main():
 manifest=load(PROBE/'delivery-index.json');assert manifest['schema_version']==1 and manifest['family_certified'] is False
 expected_payload={p.relative_to(ROOT).as_posix() for p in PROBE.rglob('*') if p.is_file() and p.name!='delivery-index.json'}
 expected_payload.update({'scripts/check-fallible-seen-engine.py','docs/decisions/0068-fallible-disk-seen-safety.md','docs/user/disk-seen-safety.md'})
 assert set(manifest['payload_sha256'])==expected_payload
 for name,digest in manifest['payload_sha256'].items():assert sha(safe(name).read_bytes())==digest,name
 source=load(safe(manifest['runtime_source_index']));producer=manifest['runtime_commit'];assert source['producer_commit']==producer
 for name,row in source['inputs'].items():
  data=gzip.decompress(safe(row['archive']).read_bytes());assert sha(data)==row['sha256'],name
  frozen=subprocess.check_output(['git','show',producer+':'+name],cwd=ROOT);assert data==frozen,name
 current=load(safe(manifest['current_source_index']));assert set(current['current_sha256'])==set(source['inputs'])
 for name,digest in current['current_sha256'].items():assert sha(safe(name).read_bytes())==digest,name
 assert current['current_rust_cargo_equals_runtime']==all(current['current_sha256'][n]==r['sha256'] for n,r in source['inputs'].items())
 gate=load(safe(manifest['gate_receipt']));assert gate['producer_commit']==producer and len(gate['stages'])==8
 assert all(c['exit']==0 for stage in gate['stages'] for c in stage['commands'])
 for stage in gate['stages']:
  for command in stage['commands']:
   log=safe(manifest['gate_directory'])/command['log'];assert sha(log.read_bytes())==command['sha256']
 nextest=(safe(manifest['gate_directory'])/'nextest-0.log').read_text()
 assert re.search(r"1268 tests run: 1268 passed, 0 skipped",nextest)
 runtime=load(safe(manifest['runtime_receipt']));assert runtime['producer']==producer and runtime['family_certified'] is False
 assert runtime['binary_sha256']==next(b['sha256'] for b in manifest['binaries'] if b['name']=='dive')
 assert runtime['memory_selected']==384 and runtime['disk_selected']==576 and runtime['reference_selected']==64
 assert len(runtime['rows'])==1024 and len(runtime['primary'])==8
 assert len({r['case'] for r in runtime['rows']})==1024
 assert len([r for r in runtime['rows'] if r['case'].startswith('memory-')])==384
 assert len([r for r in runtime['rows'] if r['case'].startswith('disk-')])==576
 assert len([r for r in runtime['rows'] if r['case'].startswith('reference-')])==64
 assert runtime['metadata_allowlist']==['elapsed_ms','options.workers','options.seen_disk','seen_storage']
 assert next(r for r in runtime['primary'] if r['case']=='DeterministicError')['native_error_class_verified'] is False
 for row in runtime['rows']:
  for channel in ['stdout','stderr']:
   assert sha((safe(manifest['runtime_directory'])/(row['case']+'.'+channel)).read_bytes())==row[channel+'_sha256']
 reports={r['case']:load(safe(manifest['runtime_directory'])/(r['case']+'.stdout')) for r in runtime['rows']}
 def stable(value):
  value=copy.deepcopy(value);value.pop('elapsed_ms');return value
 for row in runtime['rows']:
  label=row['case'];actual=reports[label]
  if label.startswith('memory-'):
   old=load(PROBE/'current/memory-baseline'/(label.removeprefix('memory-')+'.stdout'))
   assert stable(actual)==stable(old),label
  elif label.startswith('disk-'):
   _,model,profile,store,width,chaos,budget=label.split('-')
   reference=reports[f'reference-{model}-{profile}-{store}-{budget}']
   assert actual['options']['workers']['requested']==int(width) and actual['options']['workers']['used']==1
   assert actual['options']['workers']['sequential'] is True
   assert 'immutable disk read views' in actual['options']['workers']['reason']
   usage=actual['seen_storage'];assert usage['first_failure'] is None and usage['deciding_event_before_failure'] is None
   assert usage['admitted_keys']==actual['statistics']['distinct']==usage['before_cleanup']['physical_entries']
   assert usage['before_cleanup']['resident_key_collection_bytes']==0 and usage['after_cleanup']['cleanup_completed'] is True
   a=stable(actual);b=stable(reference);a.pop('seen_storage');a['options'].pop('seen_disk');a['options'].pop('workers');b['options'].pop('workers')
   assert a==b,label
 api=load(safe(manifest['api_receipt']));assert api['runtime_source_commit']==producer
 controls=load(safe(api['stdout']));assert len(controls['rows'])==144
 assert api['positive_valid']==144 and api['semantic_mutants_invalid']==864
 assert sha(safe(api['stdout']).read_bytes())==api['stdout_sha256']
 assert sum(len(r['mutants']) for r in controls['rows'])==864
 assert {r['case'] for r in controls['rows']}=={'PureViewViolation','EarlyInvariant','SymOrder','SymViolation'}
 assert len([r for r in controls['rows'] if r['case']=='PureViewViolation'])==36
 expected_api=set(itertools.product({'PureViewViolation','EarlyInvariant','SymOrder','SymViolation'},[True,False],['fp','exact'],[1,4,8],[None,0,7]))
 assert {(r['case'],r['compat_tlc'],r['store'],r['requested_workers'],r['chaos']) for r in controls['rows']}==expected_api
 for row in controls['rows']:
  assert row['positive']=='valid' and row['mutants'] and all(m['validation']=='invalid' for m in row['mutants'])
  assert row['report']['options']['workers']['used']==1
  assert row['report']['seen_storage']['after_cleanup']['cleanup_completed'] is True
 extra=load(safe(manifest['extra_receipt']));assert extra['producer']==producer
 assert len(extra['rows'])==60 and len({r['case'] for r in extra['rows']})==60
 assert extra['replay_disk_selected']==36 and extra['replay_memory_selected']==4
 for row in extra['rows']:
  for channel in ['stdout','stderr']:
   assert sha((safe(manifest['extra_directory'])/(row['case']+'.'+channel)).read_bytes())==row[channel+'_sha256']
  if row['case'].startswith('replay-disk-'):
   d=load(safe(manifest['extra_directory'])/(row['case']+'.stdout'))
   assert d['verdict']=='unsupported' and d['counterexample']['trace'] is None
   assert d['statistics']['distinct']==3 and d['statistics']['left_on_queue']==1
   assert d['seen_storage']['after_cleanup']['cleanup_completed'] is True
   assert any('EvaluationError' in n and 'Invariant' in n for n in d['notes'])
 assert len(manifest['width_gate_receipts'])==2
 assert {load(safe(n))['selected'] for n in manifest['width_gate_receipts']}=={21,16}
 for receiptname in manifest['width_gate_receipts']:
  receipt=load(safe(receiptname));assert receipt['producer']==producer and receipt['exit']==0
  assert receipt['workers']=='1,4,8' and receipt['selected'] in [21,16]
 before=load(PROBE/'before-b07-fullgate/index.json');assert before['complete_gate_passed'] is False and before['tests_failed']==1
 for row in before['logs'].values():assert sha(gzip.decompress(safe(row['archive']).read_bytes()))==row['sha256']
 guides=load(safe(manifest['guide_fault_receipt']));assert guides['producer']==producer
 assert guides['cargo_guide']['exit']==0 and guides['cargo_guide']['rows']==4
 assert all(guides['fault_selection'][k]==v for k,v in {'exit':0,'selected':7,'passed':7,'filtered':26}.items())
 assert len(manifest['binaries'])==4 and all(b['runtime_source_commit']==producer for b in manifest['binaries'])
 assert all(len(b['sha256'])==64 and b['bytes']>0 for b in manifest['binaries'])
 print('Verified exact source, full1268 gate, original384 memory, 576 disk, 144 independent witnesses/mutants, replay refusals and37 width gates; F-031 certification remains pending.')
if __name__=='__main__':main()
