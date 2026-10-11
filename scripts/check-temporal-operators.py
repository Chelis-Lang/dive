"""Verify bounded operator forwarding, every refusal and immutable producer evidence."""
from pathlib import Path
import gzip,hashlib,json,re
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/temporal-operators'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
load=lambda p:json.loads(p.read_text())
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
selection=load(BASE/'selection.json');instance=load(BASE/'selection-instance.json')
assert len(selection)==14 and len(instance)==4
all_cases=selection+instance
assert len({x['name']for x in all_cases})==18
refs={}
for directory,sfile,script,cases in [('clean-primary','selection-before-model-key.json','capture-clean.py',selection),('clean-instance','selection-instance.json','capture-instance.py',instance)]:
 m=load(BASE/directory/'capture.json')
 assert m['oracle_sha256']==PIN and m['reason'] and m['flags']['workers']==1
 assert m['selection_sha256']==sha(BASE/sfile) and m['capture_source_sha256']==sha(BASE/script)
 assert [x['name']for x in m['rows']]==[x['name']for x in cases]
 for row in m['rows']:
  refs[row['name']]=row
  assert not row['timeout'] and row['exit']==(13 if row['name'].endswith('Fails') and row['kind']not in ['Constant','Effect']else 0)
  for path,digest in row['model_source_sha256'].items():assert sha(BASE/path)==digest,path
  for stream in ['stdout','stderr']:assert sha(BASE/directory/row['name']/stream)==row[stream+'_sha256']
for file,directory,sfile,cases in [('capture.json','raw','selection.json',selection),('capture-instance-projection.json','raw-instance','selection-instance.json',instance)]:
 m=load(BASE/file);assert m['oracle_sha256']==PIN and m['reason']and m['compiler']['exit']==0
 assert m['selection_sha256']==sha(BASE/sfile)
 for path,digest in m['source_sha256'].items():assert sha(BASE/path)==digest,path
 assert [x['name']for x in m['rows']]==[x['name']for x in cases]
 for row in m['rows']:
  assert not row['timeout']
  for path,digest in row['model_source_sha256'].items():assert sha(BASE/path)==digest,path
  for stream in ['stdout','stderr']:assert sha(BASE/directory/row['name']/stream)==row[stream+'_sha256']
  if row['name'].startswith('OpConstant'):
   assert row['exit']==1 and 'NullPointerException'in(BASE/directory/row['name']/'stderr').read_text()
  else:assert row['exit']==0
frozen=BASE/'frozen-material-producer';fm=load(frozen/'manifest.json')
assert fm['commit']=='9b098e172d02abcf2dd3126faabf719ad5d1940a'
for path,digest in fm['source_sha256'].items():assert hashlib.sha256(gzip.decompress((frozen/'source'/(path+'.gz')).read_bytes())).hexdigest()==digest,path
for row in fm['matrices']:assert sha(frozen/'matrices'/row['path'])==row['sha256']
for directory,cases in [('native-before',selection),('native-instance-before',instance)]:
 m=load(BASE/directory/'capture.json');assert m['source_and_binary_unchanged']is True
 assert len(m['rows'])==len(cases)*4
 assert [(x['model'],x['requested_workers'])for x in m['rows']]==[(x['name'],w)for x in cases for w in [1,3,4,8]]
 for path,digest in m['source_sha256'].items():assert fm['source_sha256'][path]==digest,path
 for row in m['rows']:
  assert row['exit']==3 and row['verdict']=='unsupported'and not row.get('timeout',False)
  d=BASE/directory/f"{row['model']}-{row['requested_workers']}"
  for stream in ['stdout','stderr']:assert sha(d/stream)==row[stream+'_sha256']
  doc=load(d/'stdout');assert doc['counterexample']is None and doc['options']['workers']['used']==0
current=BASE/'native-current';m=load(current/'capture.json')
assert m['source_and_binary_unchanged']is True and m['capture_source_sha256']==sha(BASE/'capture-native.py')
assert m['selections_sha256']=={f:sha(BASE/f)for f in ['selection.json','selection-instance.json']}
assert [(x['model'],x['requested_workers'])for x in m['rows']]==[(x['name'],w)for x in all_cases for w in [1,3,4,8]]
for path,digest in m['source_sha256'].items():assert sha(ROOT/path)==digest,path
fields={r['name']:r['fields']for file in ['capture.json','capture-instance-projection.json']for r in load(BASE/file)['rows']}
counts={}
for row in m['rows']:
 d=current/f"{row['model']}-{row['requested_workers']}";ref=refs[row['model']];doc=load(d/'stdout')
 assert not row['timeout'] and doc['schema_version']==2
 for stream in ['stdout','stderr']:assert sha(d/stream)==row[stream+'_sha256']
 counts[doc['verdict']]=counts.get(doc['verdict'],0)+1
 assert doc['options']['workers']['requested']==row['requested_workers']and doc['options']['ordinary_temporal_schedule_supported']is False
 if 'Effect'in row['model']:
  assert row['exit']==3 and doc['verdict']=='unsupported'and doc['counterexample']is None and doc['options']['workers']['used']==0
  assert doc['tlc']is None or doc['tlc']['verified']is False
  continue
 assert row['exit']==ref['exit']==doc['tlc']['exit_status']and doc['tlc']['verified']is True
 assert doc['tlc']['code']in ref['message_codes']
 raw=(BASE/('clean-instance'if row['model']in[x['name']for x in instance]else'clean-primary')/row['model']/'stdout').read_text()
 stats=re.search(r'@!@!@STARTMSG 2199:0 @!@!@\n(\d+) states generated, (\d+) distinct states found',raw);depth=re.search(r'The depth of the complete state graph search is (\d+)',raw);assert stats and depth
 for name,value in [('generated',stats[1]),('distinct',stats[2]),('depth',depth[1])]:assert doc['statistics'][name]==int(value),(row['model'],name)
 assert doc['options']['workers']['used']==row['requested_workers']
 if row['model'].startswith('OpConstant'):
  assert doc['execution']['route']=='bounded-safety-only'and doc['execution']['product_constructed']is False
 else:
  assert doc['execution']['capture_completed']is True and doc['execution']['schedule']=='final-only'and doc['execution']['selection']=='tlc'
  f=fields[row['model']]
  for target,source in [('configured_property_names','configured_properties'),('expanded_property_names','properties'),('role_names','temporal_role_names')]:assert doc['execution'][target]==f[source].split(';'),(row['model'],target)
 if ref['exit']==13:
  c=doc['counterexample'];assert doc['verdict']=='liveness-violation'and c['validation']=='valid'and c['ending']=='stuttering'
  assert c['trace']['properties']==['Live']and c['trace']['ending']=={'kind':'stuttering'}
  values=[s.splitlines()[2]=='x = TRUE'for s in raw.split('@!@!@STARTMSG 2217:4 @!@!@')[1:]]
  assert [s['values']['x']for s in c['states']]==values
 else:assert doc['verdict']=='success'and doc['counterexample']is None
assert counts=={'success':36,'liveness-violation':28,'unsupported':8},counts
example=BASE/'user-example-controls';e=load(example/'receipt.json')
assert e['compile_exit']==e['run_exit']==0 and e['source_and_binary_unchanged']is True
assert sha(BASE/'user-example.rs')==e['source_sha256']
for path,digest in e['crate_source_sha256'].items():assert sha(ROOT/path)==digest,path
for path,digest in e['files'].items():assert sha(example/path)==digest,path
assert(example/'stdout').read_text().strip()=='["Live"]'
# The whole140 census belongs to the previous9b producer, never the current one.
study=ROOT/'conformance/studies/material-capture-final-small';closed=load(study/'closed-after.json');before=load(study/'before.json')
assert closed['source_commit']==fm['commit']and closed['source_and_binaries_unchanged']is True
assert closed['initial_receipt_sha256']==sha(study/'after.json')
assert [x['path']for x in closed['initial_receipt_mismatches']]==['launcher.log']
for path,digest in closed['artifact_sha256'].items():assert sha(study/path)==digest,path
for path,digest in closed['source_sha256'].items():assert hashlib.sha256(gzip.decompress((study/'source'/(path+'.gz')).read_bytes())).hexdigest()==digest,path
r=load(next((study/'reports').glob('*.json')));assert len(r['models'])==140
assert r['summary']['models']['agree']==46 and r['summary']['models']['unsupported']==92 and r['summary']['models']['timeout']==2 and r['summary']['models']['disagree']==0
# Exact formerly-refused sources retain their clean oracle observations.
former=BASE/'former-refusal-controls';f=load(former/'capture.json')
assert f['oracle_sha256']==PIN and f['flags']==dict(workers=1,seed=0,fp=0,lncheck='final')
assert sha(BASE/'capture-former-refusals.py')==f['capture_source_sha256']
assert [(x['name'],x['primary_exit'],x['native_exit'])for x in f['rows']]==[('Refusal',12,3),('OpaqueTemporal',13,13)]
for row in f['rows']:
 for name,digest in row['files'].items():assert sha(former/row['name']/name)==digest,name
known=load(former/'OpaqueTemporal/native-stdout')
assert known['verdict']=='liveness-violation'and known['counterexample']['validation']=='valid'
assert known['counterexample']['ending']=='stuttering'
refused=load(former/'Refusal/native-stdout');assert refused['verdict']=='unsupported'and refused['counterexample']is None
assert any(d['code']=='DIVE-API-L010'for d in refused['diagnostics'])
higher=BASE/'higher-target-primary';h=load(higher/'capture.json');assert h['exit']==150 and h['oracle_sha256']==PIN
for name,digest in h['files'].items():assert sha(higher/name)==digest,name
failed=BASE/'development-full-gate-refusals';failed_before=load(failed/'gate-current/before.json')
for path,digest in failed_before['source_sha256'].items():assert hashlib.sha256(gzip.decompress((failed/'source'/(path+'.gz')).read_bytes())).hexdigest()==digest,path
assert load(failed/'gate-current/after.json')['all_steps_passed']is False
assert '941 tests run: 939 passed, 2 failed' in (failed/'gate-current/workspace.log').read_text()
for path,digest in load(BASE/'source-matrix.json')['source_sha256'].items():assert sha(ROOT/path)==digest,path
print('Verified18 clean primary/72 current native rows,14 exact temporal projections,72 original refusals, bounded independent captures/replay; prior9b140 census46/92/2 remains separate, no ordinary/family credit')
