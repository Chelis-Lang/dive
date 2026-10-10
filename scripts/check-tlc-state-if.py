"""Verify complete raw state-IF controls, historical failures and source authority."""
from pathlib import Path
import hashlib, json, gzip
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/tlc-state-if'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def capture(base,count):
 m=json.loads((base/'capture.json').read_text())
 s=json.loads((base/'selection.json').read_text())
 assert len(s)==len(m['rows'])==count
 assert sha(base/'selection.json')==m['selection_sha256']
 assert m['oracle_sha256']=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
 assert m['compiler']['exit']==0
 for path,digest in m['source_sha256'].items():assert sha(base/path)==digest,path
 for row,case in zip(m['rows'],s):
  assert row['name']==case['name'] and not row['timeout']
  for path,digest in row['model_source_sha256'].items():assert sha(base/path)==digest,path
  d=base/'raw'/row['name'];assert sha(d/'stdout')==row['stdout_sha256'];assert sha(d/'stderr')==row['stderr_sha256']
 return m
m=capture(BASE,15)
assert sum(r['exit']==0 for r in m['rows'])==11
assert sum(r['exit']!=0 for r in m['rows'])==4
codes={r['name']:r['fields'].get('deciding_failure_code') for r in m['rows'] if r['exit']!=0}
assert codes=={'IfStateError':'2179','IfCallError':'2179','IfNonBool':'2250','IfElseError':'2179'}
assert next(r for r in m['rows'] if r['name']=='IfHoldTrue')['fields']['final_check_code']=='0'
for dirname,count in [('historical-original-12',12),('historical-complete-13',13),('historical-complete-14',14)]:capture(BASE/dirname,count)
h=BASE/'historical-before-admission';old=json.loads((h/'capture.json').read_text())
assert len(old['rows'])==48 and sha(h/'phase_l.rs')==old['phase_l_sha256']
for row in old['rows']:
 d=h/f"{row['name']}-{row['workers']}";assert sha(d/'stdout')==row['stdout_sha256'];assert sha(d/'stderr')==row['stderr_sha256']
h=BASE/'current-integration-before-admission';old=json.loads((h/'manifest.json').read_text())
for row in old['matrices']:
 p=h/row['matrix'];assert sha(p)==row['matrix_sha256']
 for path,digest in json.loads(p.read_text())['source_sha256'].items():
  assert hashlib.sha256(gzip.decompress((h/'sources'/(path+'.gz')).read_bytes())).hexdigest()==digest,path
h=BASE/'historical-native-before-lint-refactor';old=json.loads((h/'native/capture.json').read_text())
assert len(old['rows'])==60 and old['source_and_binary_unchanged'] is True
for path,digest in old['source_sha256'].items():
 assert hashlib.sha256(gzip.decompress((h/'sources'/(path+'.gz')).read_bytes())).hexdigest()==digest,path
for row in old['rows']:
 d=h/'native'/f"{row['model']}-{row['requested_workers']}"
 assert sha(d/'stdout')==row['stdout_sha256'] and sha(d/'stderr')==row['stderr_sha256']
current=json.loads((BASE/'source-matrix.json').read_text())
for path,digest in current['source_sha256'].items():assert sha(ROOT/path)==digest,path
for path,digest in current['history_sha256'].items():assert sha(BASE/path)==digest,path
print('Verified complete15 conditional controls:11 complete projections/4 reference errors; all histories retained; no periodic or family certification')

clean=BASE/'clean-tlc';raw=json.loads((clean/'capture.json').read_text())
assert raw['selection_sha256']==sha(BASE/'selection.json') and len(raw['rows'])==15
assert raw['flags']=={'workers':1,'seed':0,'fp':0,'lncheck':'final'}
for row in raw['rows']:
 d=clean/row['name'];assert sha(d/'stdout')==row['stdout_sha256'];assert sha(d/'stderr')==row['stderr_sha256']
 expected=2193 if row['name']=='IfHoldTrue' else int(codes[row['name']]) if row['name'] in codes else 2116
 assert expected in row['message_codes']
 assert row['exit']==(0 if expected==2193 else 13 if expected==2116 else 255)
print('Verified complete15 clean final TLC controls, including all four deciding reference errors')

# All selected current CLI outcomes remain present; deciding reference failures
# are refusals here, never promoted to matching evaluation errors.
native=BASE/'native-current';nm=json.loads((native/'capture.json').read_text())
assert nm['selection_sha256']==sha(BASE/'selection.json')
assert nm['source_and_binary_unchanged'] is True and len(nm['rows'])==60
for path,digest in nm['source_sha256'].items():
 assert hashlib.sha256(gzip.decompress((ROOT/'conformance/probes/tlc-temporal-if/historical-state-if-producer/sources'/(path+'.gz')).read_bytes())).hexdigest()==digest,path
expected=[(case['name'],width) for case in json.loads((BASE/'selection.json').read_text()) for width in [1,3,4,8]]
assert [(r['model'],r['requested_workers']) for r in nm['rows']]==expected
counts={'success':0,'liveness-violation':0,'unsupported':0}
for row in nm['rows']:
 d=native/f"{row['model']}-{row['requested_workers']}"
 assert sha(d/'stdout')==row['stdout_sha256'] and sha(d/'stderr')==row['stderr_sha256']
 r=json.loads((d/'stdout').read_text());verdict=r['verdict'];counts[verdict]+=1
 assert row['ordinary_periodic_certification'] is False
 assert r['schema_version']==2 and r['options']['ordinary_temporal_schedule_supported'] is False
 assert r['options']['temporal_scope']=='bounded-final-only'
 assert r['execution']['schedule']=='final-only' and r['execution']['selection']=='tlc'
 assert r['options']['workers']['requested']==row['requested_workers']
 refused=row['model'] in codes or row['model']=='IfTableau'
 assert verdict==('unsupported' if refused else 'success' if row['model']=='IfHoldTrue' else 'liveness-violation')
 assert row['exit']==(3 if refused else 0 if verdict=='success' else 13)
 if refused:
  assert r['counterexample'] is None and r['tlc'] is None
  assert not r['execution']['capture_completed']
  assert r['options']['workers']['used']==(row['requested_workers'] if row['model'] in ['IfCallError','IfNonBool'] else 0)
 else:
  assert r['execution']['capture_completed'] and r['options']['workers']['used']==row['requested_workers']
  assert r['tlc']['verified'] and r['tlc']['code']==(2193 if verdict=='success' else 2116)
  if verdict=='liveness-violation':
   assert r['counterexample']['validation']=='valid'
   assert r['counterexample']['trace']['properties']==r['properties']
assert counts=={'success':4,'liveness-violation':36,'unsupported':20}
print('Verified all60 historical native rows:40 supported final-only outcomes/20 named refusals; no reference-error or periodic credit')

example=BASE/'user-example-controls';em=json.loads((example/'receipt.json').read_text())
assert sha(BASE/'user-example.rs')==em['source_sha256'] and em['compile_exit']==em['run_exit']==0
assert em['crate_source_sha256']==nm['source_sha256']
for path,digest in em['files'].items():assert sha(example/path)==digest,path
assert (example/'stdout').read_text().strip()=='["Live"]'
