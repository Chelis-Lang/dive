"""Verify complete final temporal-IF controls, refusals and original source receipts."""
from pathlib import Path
import json,hashlib,gzip,re
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/tlc-temporal-if-final'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
selection=json.loads((BASE/'selection.json').read_text())
assert len(selection)==30 and len({r['name']for r in selection})==30
def capture(folder):
 m=json.loads((BASE/folder/'capture.json').read_text())
 assert m['oracle_sha256']=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
 assert m['compiler']['exit']==0 and len(m['rows'])==30
 assert m['selection_sha256']==sha(BASE/'selection.json')
 assert [r['name']for r in m['rows']]==[r['name']for r in selection]
 for path,d in m['source_sha256'].items():assert sha(BASE/path)==d,path
 for r in m['rows']:
  assert not r['timeout'];raw=BASE/folder/'raw'/r['name']
  for stream in ['stdout','stderr']:assert sha(raw/stream)==r[stream+'_sha256']
  for path,d in r['model_source_sha256'].items():assert sha(BASE/path)==d,path
 return m
# The original planning capture keeps raw at the top-level path.
m=json.loads((BASE/'capture.json').read_text());assert len(m['rows'])==30
assert sum(r['exit']==0 for r in m['rows'])==25
for path,d in m['source_sha256'].items():assert sha(BASE/path)==d,path
for r in m['rows']:
 assert not r['timeout']
 for stream in ['stdout','stderr']:assert sha(BASE/'raw'/r['name']/stream)==r[stream+'_sha256']
 for path,d in r['model_source_sha256'].items():assert sha(BASE/path)==d,path
final=capture('final-projection');assert sum(r['exit']==0 for r in final['rows'])==20
clean=json.loads((BASE/'clean-tlc/capture.json').read_text());assert len(clean['rows'])==30
assert clean['flags']=={'workers':1,'seed':0,'fp':0,'lncheck':'final'}
assert clean['selection_sha256']==sha(BASE/'selection.json')
assert clean['source_sha256']==sha(BASE/'capture-clean.py')
for r in clean['rows']:
 assert not r['timeout']
 for stream in ['stdout','stderr']:assert sha(BASE/'clean-tlc'/r['name']/stream)==r[stream+'_sha256']
 for path,d in r['model_source_sha256'].items():assert sha(BASE/path)==d,path
for name in ['TiThen','TiNested','TiMulti']:
 r=next(r for r in clean['rows']if r['name']==name);assert r['exit']==12 and 2116 in r['message_codes']
for folder in ['current-parent-before-integration','historical-tag-repair-source']:
 b=BASE/folder;record=json.loads((b/'manifest.json').read_text())
 for r in record['matrices']:assert sha(b/r['path'])==r['sha256']
 for path,d in record['source_sha256'].items():assert hashlib.sha256(gzip.decompress((b/'sources'/(path+'.gz')).read_bytes())).hexdigest()==d,path
for folder in ['development-first-api','development-unused-budget','before-runtime-admission']:
 b=BASE/folder;record=json.loads((b/'manifest.json').read_text())
 for path,d in record.get('files',record.get('source_sha256',{})).items():assert sha(b/path)==d,path
h=BASE/'historical-native-before-format';nm=json.loads((h/'capture.json').read_text());assert len(nm['rows'])==120
for path,d in nm['source_sha256'].items():assert hashlib.sha256(gzip.decompress((h/row.get('source_dir','source')/(path+'.gz')).read_bytes())).hexdigest()==d,path
for r in nm['rows']:
 for stream in ['stdout','stderr']:assert sha(h/f"{r['model']}-{r['requested_workers']}"/stream)==r[stream+'_sha256']
h=BASE/'historical-native-before-cli-expectation';previous=json.loads((h/'capture.json').read_text());assert len(previous['rows'])==120
for path,d in previous['source_sha256'].items():assert hashlib.sha256(gzip.decompress((h/row.get('source_dir','source')/(path+'.gz')).read_bytes())).hexdigest()==d,path
for r in previous['rows']:
 for stream in ['stdout','stderr']:assert sha(h/f"{r['model']}-{r['requested_workers']}"/stream)==r[stream+'_sha256']
h=BASE/'development-stale-cli-expectation';previous=json.loads((h/'manifest.json').read_text())
for path,d in previous['files'].items():assert sha(h/path)==d,path
assert '915 tests run: 914 passed' in (h/'workspace-gate.log').read_text()
h=BASE/'metadata-before-cli-test-refresh';previous=json.loads((h/'manifest.json').read_text())
for row in previous['matrices']:
 assert sha(h/row['path'])==row['sha256']
 for path,d in json.loads((h/row['path']).read_text())['source_sha256'].items():assert hashlib.sha256(gzip.decompress((h/row.get('source_dir','source')/(path+'.gz')).read_bytes())).hexdigest()==d,path
native=BASE/'native-current';nm=json.loads((native/'capture.json').read_text());assert nm['source_and_binary_unchanged']is True
assert nm['selection_sha256']==sha(BASE/'selection.json')and len(nm['rows'])==120
assert [(r['model'],r['requested_workers'])for r in nm['rows']]==[(r['name'],w)for r in selection for w in [1,3,4,8]]
for path,d in nm['source_sha256'].items():assert sha(ROOT/path)==d,path
supported={r['name']:r for r in final['rows']if r['exit']==0};reference={r['name']:r for r in clean['rows']}
counts={'liveness-violation':0,'success':0,'unsupported':0}
for row in nm['rows']:
 b=native/f"{row['model']}-{row['requested_workers']}"
 for stream in ['stdout','stderr']:assert sha(b/stream)==row[stream+'_sha256']
 r=json.loads((b/'stdout').read_text());counts[r['verdict']]+=1;name=row['model']
 assert r['schema_version']==2 and r['execution']['schedule']=='final-only'
 assert r['options']['ordinary_temporal_schedule_supported']is False
 assert r['options']['workers']['requested']==row['requested_workers']
 if name not in supported:
  assert r['verdict']=='unsupported'and row['exit']==3 and r['counterexample']is None
  assert r['tlc']is None or r['tlc']['verified']is False
  continue
 assert r['tlc']['verified']is True and r['execution']['capture_completed']is True
 assert row['exit']==reference[name]['exit']==r['tlc']['exit_status']
 assert r['options']['workers']['used']==row['requested_workers']
 assert r['tlc']['code']in reference[name]['message_codes']
 fields=supported[name]['fields']
 for field,key in [('distinct','distinct'),('generated','generated')]:assert str(r['statistics'][field])==fields[key],(name,field)
 depth=re.search(r'depth of the complete state graph search is (\d+)',(BASE/'clean-tlc'/name/'stdout').read_text());assert depth and r['statistics']['depth']==int(depth[1]),name
 if r['verdict']=='liveness-violation':
  assert r['counterexample']['validation']=='valid'
  assert r['counterexample']['trace']['properties']==r['properties']
 else:assert r['verdict']=='success'and r['tlc']['code']==2193
assert counts=={'liveness-violation':76,'success':4,'unsupported':40}
aux=BASE/'unused-admission';a=json.loads((aux/'clean-tlc/capture.json').read_text());assert len(a['rows'])==1 and a['rows'][0]['exit']==13
for r in a['rows']:
 for stream in ['stdout','stderr']:assert sha(aux/'clean-tlc'/r['name']/stream)==r[stream+'_sha256']
 for path,d in r['model_source_sha256'].items():assert sha(aux/path)==d,path
example=BASE/'user-example-controls';em=json.loads((example/'receipt.json').read_text())
assert em['compile_exit']==0 and em['run_exit']==0 and em['source_and_binary_unchanged']is True
assert em['source_sha256']==sha(BASE/'user-example.rs')
for path,d in em['files'].items():assert sha(example/path)==d,path
assert (example/'stdout').read_text().strip()=='["Live"]'
for path,d in json.loads((BASE/'source-matrix.json').read_text())['source_sha256'].items():assert sha(ROOT/path)==d,path
print('Verified all30 primary selections/120 native outcomes:80 supported final outcomes,40 named refusals; independent replay and all source histories retained, no periodic/family credit')

# Full unchanged corpus evidence has actual launcher failure history and all raw outcomes.
study=ROOT/'conformance/studies/tlc-temporal-if-final-current'
failed=study/'development-zero-native-launcher';f=json.loads(next((failed/'reports').glob('*.json')).read_text())
assert len(f['models'])==140 and f['summary']['models']['disagree']==138
assert not (failed/'raw').exists() and json.loads((failed/'after.json').read_text())['harness_exit']==1
complete=study/'full-small-study';a=json.loads((complete/'after.json').read_text());b=json.loads((complete/'before.json').read_text())
assert a['source_and_binaries_unchanged'] is True and a['harness_exit']==0
for path,d in a['artifact_sha256'].items():assert sha(complete/path)==d,path
report=json.loads(next((complete/'reports').glob('*.json')).read_text())
assert len(report['models'])==140 and report['summary']['models']['agree']==6 and report['summary']['models']['unsupported']==134
assert all(report['summary']['models'][k]==0 for k in ['disagree','oracle-error','timeout','agree-pending-trace-validation'])
raw=list((complete/'raw').iterdir());assert len(raw)==138
for entry in raw:
 outcome=json.loads((entry/'outcome.json').read_text());assert outcome['exit_code'] in [0,3,12]
print('Complete unchanged140 census retained:6 agree/134 unsupported; earlier138 zero-native launcher failures remain separate history')

assert sum(json.loads((e/'outcome.json').read_text())['exit_code']==12 for e in raw)==1
assert sum(json.loads((e/'outcome.json').read_text())['exit_code']==0 for e in raw)==5
