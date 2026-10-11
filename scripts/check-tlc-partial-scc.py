"""Verify complete explicit-prefix evidence without ordinary-schedule proof credit."""
from pathlib import Path
import hashlib,json
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/tlc-partial-scc'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
load=lambda p:json.loads(p.read_text())
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
for capture,selection,raw,count in [
 ('capture.json','selection.json','raw',32),
 ('capture-repeated.json','repeated-selection.json','repeated-raw',8),
]:
 c=load(BASE/capture);selected=load(BASE/selection)
 assert c['oracle_sha256']==PIN and c['reason']
 assert c['compiler']['exit']==0 and c['selection_sha256']==sha(BASE/selection)
 assert len(c['rows'])==len(selected)==count
 assert [r['name']for r in c['rows']]==[r['name']for r in selected]
 for p,h in c['source_sha256'].items():assert sha(BASE/p)==h,p
 for r in c['rows']:
  assert r['exit']==0 and not r['timeout']
  for p,h in r['model_source_sha256'].items():assert sha(BASE/p)==h,p
  for stream in ['stdout','stderr']:assert sha(BASE/raw/r['name']/stream)==r[stream+'_sha256']
  fields=r['fields']
  assert fields['configured_names']=='Live'
  if capture=='capture.json':
   assert fields['completion']=='bounded-partial-projection-not-model-proof'
   assert fields['partial_check_code']in ['0','2116']
  else:
   assert fields['completion']in ['partial-candidate-no-model-proof','repeated-partial-checks-not-model-proof']
   assert fields['check.initial.code']=='0'
# Keep failed launcher rows; they do not contain runtime evidence.
d=BASE/'development-repeated-launcher';c=load(d/'capture-repeated.json')
assert len(c['rows'])==8
for r in c['rows']:
 assert r['exit']==1 and not r['timeout']
 for stream in ['stdout','stderr']:assert sha(d/'repeated-raw'/r['name']/stream)==r[stream+'_sha256']
print('Verified32 full prefix rows and8 repeated original checks;8 launcher failures retained; no ordinary clock/model-proof credit')

# Current admission/SCC authority is separate from immutable reference captures.
import gzip
a=BASE/'historical-before-partial'
for p,h in load(a/'manifest.json')['source_sha256'].items():
 assert hashlib.sha256(gzip.decompress((a/'source'/(p+'.gz')).read_bytes())).hexdigest()==h,p
for p,h in load(BASE/'source-matrix.json')['source_sha256'].items():assert sha(ROOT/p)==h,p
