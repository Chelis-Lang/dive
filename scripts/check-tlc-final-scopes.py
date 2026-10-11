"""Verify complete scoped final controls and immutable mismatch/source history."""
from pathlib import Path
import json,hashlib,gzip,re
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/tlc-final-scopes'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
load=lambda p:json.loads(p.read_text())
FROZEN=ROOT/'conformance/probes/tlc-final-property-origins/frozen-scoped-producer'
def scoped_producer_source(path,digest):
 assert hashlib.sha256(gzip.decompress((FROZEN/'source'/(path+'.gz')).read_bytes())).hexdigest()==digest,path
selection=load(BASE/'selection.json')
assert len(selection)==18 and len({r['name']for r in selection})==18

def planning(base,folder,count):
 m=load(base/folder/'capture.json');assert m['oracle_sha256']=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
 assert m['selection_sha256']==sha(base/'selection.json')
 assert len(m['rows'])==count and m['compiler']['exit']==0
 assert [r['name']for r in m['rows']]==[r['name']for r in load(base/'selection.json')]
 for path,d in m['source_sha256'].items():assert sha(BASE/path)==d,path
 for r in m['rows']:
  assert not r['timeout']
  for path,d in r['model_source_sha256'].items():assert sha(BASE/path)==d,path
  for stream in ['stdout','stderr']:assert sha(base/folder/'raw'/r['name']/stream)==r[stream+'_sha256']
 return m

def clean(base,count):
 m=load(base/'clean-tlc/capture.json');assert len(m['rows'])==count
 assert m['selection_sha256']==sha(base/'selection.json')
 assert m['source_sha256']==sha(BASE/'capture-clean.py')
 assert m['flags']=={'workers':1,'seed':0,'fp':0,'lncheck':'final'}
 for r in m['rows']:
  assert not r['timeout']
  for path,d in r['model_source_sha256'].items():assert sha(BASE/path)==d,path
  for stream in ['stdout','stderr']:assert sha(base/'clean-tlc'/r['name']/stream)==r[stream+'_sha256']
 return m
p=planning(BASE,'',18);f=planning(BASE,'final-projection',18);c=clean(BASE,18)
assert sum(r['exit']==0 for r in p['rows'])==16
assert sum(r['exit']==0 for r in f['rows'])==16
assert [r['name']for r in p['rows']if r['exit']!=0]==['TfInstanceFails','TfInstanceHolds']
for r in c['rows']:
 if r['name']in ['TfInstanceFails','TfInstanceHolds']:assert r['exit']==255 and 2213 in r['message_codes']
 else:assert r['exit']==(12 if r['name']=='TfEnabledFails' else 13 if r['name'].endswith('Fails')else 0)
# Forced helper fairness on safety-only models earns no temporal-verdict credit.
for name in ['TfEnabledFails','TfEnabledHolds']:
 assert next(r for r in f['rows']if r['name']==name)['fields']['final_check_code']=='2116'
old=BASE/'historical-original-sixteen';planning(old,'',16);planning(old,'final-projection',16);clean(old,16)
for folder in ['development-derived-role-mismatch','development-dnf-singleton-mismatch','development-negated-enabled-shape']:
 b=BASE/folder;m=load(b/'manifest.json')
 for path,d in m['source_sha256'].items():assert sha(b/'source'/path)==d,path
 assert sha(b/'focused.log')==m['log_sha256']
parent=BASE/'shipped-parent-source';m=load(parent/'manifest.json')
assert m['parent_commit']=='92beded1ac83f1114e34a02cdbc384cf5ab94bc3'
for r in m['matrices']:assert sha(parent/'matrices'/r['path'])==r['sha256']
for path,d in m['source_sha256'].items():assert hashlib.sha256(gzip.decompress((parent/'source'/(path+'.gz')).read_bytes())).hexdigest()==d,path
old=BASE/'native-before-role-repair';m=load(old/'capture.json');assert len(m['rows'])==64
for path,d in m['source_sha256'].items():assert hashlib.sha256(gzip.decompress((old/'source'/(path+'.gz')).read_bytes())).hexdigest()==d,path
for r in m['rows']:
 for stream in ['stdout','stderr']:assert sha(old/f"{r['model']}-{r['requested_workers']}"/stream)==r[stream+'_sha256']
# Historical outcomes preceded name/shape repairs and earn no field-equality credit.
n=BASE/'native-current';m=load(n/'capture.json');assert m['source_and_binary_unchanged']is True
assert m['selection_sha256']==sha(BASE/'selection.json')
assert [(r['model'],r['requested_workers'])for r in m['rows']]==[(r['name'],w)for r in selection for w in [1,3,4,8]]
for path,d in m['source_sha256'].items():scoped_producer_source(path,d)
refs={r['name']:r for r in c['rows']};final_refs={r['name']:r for r in f['rows']};counts={}
for r in m['rows']:
 b=n/f"{r['model']}-{r['requested_workers']}"
 for stream in ['stdout','stderr']:assert sha(b/stream)==r[stream+'_sha256']
 doc=load(b/'stdout');counts[doc['verdict']]=counts.get(doc['verdict'],0)+1
 assert doc['schema_version']==2 and doc['options']['ordinary_temporal_schedule_supported']is False
 assert doc['options']['workers']['requested']==r['requested_workers']
 if r['model']in ['TfInstanceFails','TfInstanceHolds']:
  assert r['exit']==3 and doc['verdict']=='unsupported'and doc['counterexample']is None
  assert doc['tlc']is None or doc['tlc']['verified']is False
  continue
 assert doc['tlc']['verified']is True and r['exit']==refs[r['model']]['exit']==doc['tlc']['exit_status']
 assert doc['tlc']['code']in refs[r['model']]['message_codes']
 if r['model']in ['TfEnabledFails','TfEnabledHolds']:
  assert doc['execution']['route']=='bounded-safety-only'
  assert doc['execution']['product_constructed']is False and doc['execution']['temporal_certificate']is False
 else:
  assert doc['execution']['schedule']=='final-only'and doc['execution']['capture_completed']is True
  assert doc['options']['workers']['used']==r['requested_workers']
  for field in ['distinct','generated']:assert str(doc['statistics'][field])==final_refs[r['model']]['fields'][field]
  depth=re.search(r'depth of the complete state graph search is (\d+)',(BASE/'clean-tlc'/r['model']/'stdout').read_text());assert depth and doc['statistics']['depth']==int(depth[1])
 if doc['verdict']in ['liveness-violation','invariant-violation']:
  assert doc['counterexample']['validation']=='valid'
  if doc['verdict']=='liveness-violation':assert doc['counterexample']['trace']['properties']==doc['properties']
assert counts=={'liveness-violation':28,'success':32,'invariant-violation':4,'unsupported':8},counts
failed=BASE/'development-user-example-build';fm=load(failed/'manifest.json')
assert fm['compile_exit']==1
for path,d in fm['files'].items():assert sha(failed/path)==d,path
assert 'error[E0308]' in (failed/'compile.stderr').read_text()
assert 'multiple different versions of crate' in (failed/'compile.stderr').read_text()
example=BASE/'user-example-controls';em=load(example/'receipt.json')
assert em['compile_exit']==em['run_exit']==0 and em['source_and_binary_unchanged']is True
assert sha(BASE/'user-example.rs')==em['source_sha256']
for path,d in em['crate_source_sha256'].items():scoped_producer_source(path,d)
for path,d in em['files'].items():assert sha(example/path)==d,path
assert (example/'stdout').read_text().strip()=='["P", "Q"]'
for path,d in load(BASE/'source-matrix.json')['source_sha256'].items():assert sha(ROOT/path)==d,path
print('Verified complete18 primary/72 native scoped final outcomes and retained name/shape failures;56 temporal,8 safety,8refused; expanded-action and ordinary scheduling remain unclaimed')
