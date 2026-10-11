"""Verify origin vectors, all selected outcomes and immutable prior source receipts."""
from pathlib import Path
import gzip,hashlib,json
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/tlc-final-property-origins'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
load=lambda p:json.loads(p.read_text())
selection=load(BASE/'selection.json');assert len(selection)==18 and len({r['name']for r in selection})==18
p=load(BASE/'final-projection/capture.json');assert len(p['rows'])==18
assert p['oracle_sha256']=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
assert p['selection_sha256']==sha(BASE/'selection.json')and p['compiler']['exit']==0
assert [r['name']for r in p['rows']]==[r['name']for r in selection]
for path,d in p['source_sha256'].items():assert sha(BASE/path)==d,path
for row in p['rows']:
 assert not row['timeout']
 for path,d in row['model_source_sha256'].items():assert sha(BASE/path)==d,path
 for stream in ['stdout','stderr']:assert sha(BASE/'final-projection/raw'/row['name']/stream)==row[stream+'_sha256']
assert sum(r['exit']==0 for r in p['rows'])==16
assert [r['name']for r in p['rows']if r['exit']!=0]==['TfInstanceFails','TfInstanceHolds']
# All prior producer maps and source bytes stay separately bound; old raw runs are never retagged.
frozen=BASE/'frozen-scoped-producer';m=load(frozen/'manifest.json')
assert m['commit']=='f27fbe5cf3931a773d2b9678ca3a0c4883581840'and len(m['matrices'])==12
for row in m['matrices']:assert sha(frozen/'matrices'/row['path'])==row['sha256']
for path,d in m['source_sha256'].items():assert hashlib.sha256(gzip.decompress((frozen/'source'/(path+'.gz')).read_bytes())).hexdigest()==d,path
h=BASE/'development-clippy-assertions';hm=load(h/'manifest.json')
for path,d in hm['files'].items():assert sha(h/path)==d,path
for path,d in hm['source_sha256'].items():assert hashlib.sha256(gzip.decompress((h/'source'/(path+'.gz')).read_bytes())).hexdigest()==d,path
assert len(load(h/'native-current/capture.json')['rows'])==72
assert 'assert_is_empty' in (h/'gate-current/workspace.log').read_text()
n=BASE/'native-current';m=load(n/'capture.json');assert m['source_and_binary_unchanged']is True
assert m['selection_sha256']==sha(BASE/'selection.json')and m['capture_source_sha256']==sha(BASE/'capture-native.py')
assert [(r['model'],r['requested_workers'])for r in m['rows']]==[(r['name'],w)for r in selection for w in [1,3,4,8]]
for path,d in m['source_sha256'].items():assert sha(ROOT/path)==d,path
refs={r['name']:r for r in p['rows']};counts={}
clean={r['name']:r for r in load(ROOT/'conformance/probes/tlc-final-scopes/clean-tlc/capture.json')['rows']}
failed=BASE/'development-diagnostic-planner-contract';fm=load(failed/'manifest.json')
for path,d in fm['files'].items():assert sha(failed/path)==d,path
assert '1 passed; 1 failed' in (failed/'focused.log').read_text()
for row in m['rows']:
 b=n/f"{row['model']}-{row['requested_workers']}";assert not row['timeout']
 for stream in ['stdout','stderr']:assert sha(b/stream)==row[stream+'_sha256']
 doc=load(b/'stdout');counts[doc['verdict']]=counts.get(doc['verdict'],0)+1
 assert doc['schema_version']==2 and doc['options']['ordinary_temporal_schedule_supported']is False
 assert doc['options']['workers']['requested']==row['requested_workers']
 ref=refs[row['model']]
 if ref['exit']!=0:
  assert row['exit']==3 and doc['verdict']=='unsupported'and doc['counterexample']is None
  assert 'expanded_property_names'not in doc['execution']and(doc['tlc']is None or doc['tlc']['verified']is False)
  continue
 f=ref['fields'];vectors={k:f[v].split(';')if f[v]else[]for k,v in [('configured_property_names','configured_names'),('expanded_property_names','action_names'),('role_names','role_names')]}
 if not vectors['expanded_property_names']:
  assert doc['execution']['route']=='bounded-safety-only'and doc['execution']['product_constructed']is False
  assert all(k not in doc['execution']for k in vectors)
 else:
  assert doc['execution']['capture_completed']is True and doc['execution']['schedule']=='final-only'
  for k,v in vectors.items():assert doc['execution'][k]==v,(row['model'],k)
  assert doc['options']['workers']['used']==row['requested_workers']
  for k in ['distinct','generated']:assert str(doc['statistics'][k])==f[k]
 assert doc['tlc']['verified']is True and doc['tlc']['exit_status']==row['exit']==clean[row['model']]['exit']
 assert doc['tlc']['code']in clean[row['model']]['message_codes']
 assert ref['model_source_sha256']==clean[row['model']]['model_source_sha256']
 if doc['verdict']in ['liveness-violation','invariant-violation']:
  assert doc['counterexample']['validation']=='valid'
  if doc['verdict']=='liveness-violation':assert doc['counterexample']['trace']['properties']==vectors['configured_property_names']
assert counts=={'liveness-violation':28,'success':32,'invariant-violation':4,'unsupported':8},counts
example=BASE/'user-example-controls';e=load(example/'receipt.json');assert e['compile_exit']==e['run_exit']==0 and e['source_and_binary_unchanged']is True
assert sha(BASE/'user-example.rs')==e['source_sha256']
for path,d in e['crate_source_sha256'].items():assert sha(ROOT/path)==d,path
for path,d in e['files'].items():assert sha(example/path)==d,path
assert(example/'stdout').read_text().strip()=='configured=["P"]; expanded=["P", "P"]'
for path,d in load(BASE/'source-matrix.json')['source_sha256'].items():assert sha(ROOT/path)==d,path
print('Verified18 primary/72 native origin controls: separate configured, expanded Action and derived vectors; retained2 reference failures/8 native refusals; no ordinary or full-family credit')
