"""Verify complete temporal IF tag identity controls without verdict promotion."""
from pathlib import Path
import json,hashlib,gzip
ROOT=Path(__file__).resolve().parent.parent;BASE=ROOT/'conformance/probes/tlc-temporal-if';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def capture(base,count,modelbase=None,sourcebase=None):
 m=json.loads((base/'capture.json').read_text());modelbase=modelbase or base;sourcebase=sourcebase or base
 assert len(m['rows'])==count and m['compiler']['exit']==0
 assert m['oracle_sha256']=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
 assert m['selection_sha256']==sha(sourcebase/'selection.json')
 for path,digest in m['source_sha256'].items():assert sha(sourcebase/path)==digest,path
 for row in m['rows']:
  assert not row['timeout'];d=base/'raw'/row['name'];assert sha(d/'stdout')==row['stdout_sha256'];assert sha(d/'stderr')==row['stderr_sha256']
  for path,digest in row['model_source_sha256'].items():assert sha(modelbase/path)==digest,path
 return m
c=capture(BASE,25);assert sum(r['exit']==0 for r in c['rows'])==20
codes={r['name']:r['fields'].get('failure_code') for r in c['rows'] if r['exit']!=0};assert codes=={'IfStateError':'2179','IfElseError':'2179','TiLet':'2213','TiForward':'2213','TiAction':'2214'}
row=next(r for r in c['rows'] if r['name']=='IfTableau');assert row['fields']['tag_count']=='5'
assert row['fields']['tagged']=='N(G(D(C(s3,s2),C(N(s3),D(N(s4),E(s5))))))'
assert row['fields']['actual.g0.tableauNodes']=='6' and row['fields']['actual.g0.initial_count']=='4'
capture(BASE/'historical-initial-15',15);capture(BASE/'historical-before-unwrapped-failures',25,BASE)
f=capture(BASE/'final-projection',25,BASE,BASE);assert sum(r['exit']==0 for r in f['rows'])==15
clean=BASE/'clean-tlc';m=json.loads((clean/'capture.json').read_text());assert len(m['rows'])==25 and m['selection_sha256']==sha(BASE/'selection.json');assert m['source_sha256']==sha(BASE/'capture-clean.py')
for row in m['rows']:
 d=clean/row['name'];assert not row['timeout'];assert sha(d/'stdout')==row['stdout_sha256'] and sha(d/'stderr')==row['stderr_sha256']
 for path,digest in row['model_source_sha256'].items():assert sha(BASE/path)==digest,path
for name in ['TiThen','TiNested','TiMulti']:
 row=next(r for r in m['rows'] if r['name']==name);assert row['exit']==12 and 2116 in row['message_codes']
h=BASE/'current-integration-before-tag-repair';m=json.loads((h/'manifest.json').read_text())
for row in m['matrices']:
 assert sha(h/row['matrix'])==row['sha256'];old=json.loads((h/row['matrix']).read_text())
 for path,digest in old['source_sha256'].items():assert hashlib.sha256(gzip.decompress((h/'sources'/(path+'.gz')).read_bytes())).hexdigest()==digest,path
h=BASE/'historical-original-native';m=json.loads((h/'manifest.json').read_text())
for path,digest in m['files'].items():assert sha(h/path)==digest,path
assert 'IfTableau exact normalization/group field dnf' in (h/'field-mismatch.log').read_text()
for path,digest in json.loads((BASE/'source-matrix.json').read_text())['source_sha256'].items():assert sha(ROOT/path)==digest,path
print('Verified25 complete tag/tableau controls:20 projections/5 preserved failures;25 clean outcomes and15/10 diagnostic final rows, no new verdict or periodic credit')
