"""Read back complete stored-successor primary/before/after selections and source epochs."""
from pathlib import Path
import gzip,hashlib,json,re
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/stored-successor-material'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
load=lambda p:json.loads(p.read_text())
PRODUCER=load(BASE/'source-manifest.json')['source_sha256']
def successor_source(path,digest):
 if sha(ROOT/path)==digest:return
 assert PRODUCER[path]==digest,path
 assert hashlib.sha256(gzip.decompress((BASE/'source'/(path+'.gz')).read_bytes())).hexdigest()==digest,path
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
selection=load(BASE/'selection.json');assert len(selection)==14
refs={}
m=load(BASE/'clean-primary/capture.json');assert m['oracle_sha256']==PIN and m['reason'] and m['flags']==dict(workers=1,seed=0,fp=0,lncheck='final')
assert m['selection_sha256']==sha(BASE/'selection.json')and m['capture_source_sha256']==sha(BASE/'capture-clean.py')
assert [r['name']for r in m['rows']]==[r['name']for r in selection]
for r in m['rows']:
 refs[r['name']]=r;assert not r['timeout'] and r['exit']in[0,13]
 for p,h in r['model_source_sha256'].items():assert sha(BASE/p)==h,p
 for stream in ['stdout','stderr']:assert sha(BASE/'clean-primary'/r['name']/stream)==r[stream+'_sha256']
assert sum(r['exit']==13 for r in m['rows'])==7
projection=load(BASE/'capture-projection.json');assert projection['oracle_sha256']==PIN and projection['reason']and projection['compiler']['exit']==0
assert projection['selection_sha256']==sha(BASE/'selection.json')and len(projection['rows'])==14
for p,h in projection['source_sha256'].items():assert sha(BASE/p)==h,p
for r in projection['rows']:
 assert not r['timeout'] and r['exit']==0
 for p,h in r['model_source_sha256'].items():assert sha(BASE/p)==h,p
 for stream in ['stdout','stderr']:assert sha(BASE/'projection-raw'/r['name']/stream)==r[stream+'_sha256']
for epoch in ['native-before','frozen-native-producer']:
 manifest=load(BASE/epoch/('source-manifest.json'if epoch=='native-before'else'manifest.json'))
 for p,h in manifest['source_sha256'].items():assert hashlib.sha256(gzip.decompress((BASE/epoch/'source'/(p+'.gz')).read_bytes())).hexdigest()==h,p
before=load(BASE/'native-before/capture.json');assert before['source_commit']=='225c25e944028e273944850a9cd070791c079196' and len(before['rows'])==56 and before['source_and_binary_unchanged']is True
for r in before['rows']:
 p=BASE/'native-before'/f"{r['model']}-{r['requested_workers']}";d=load(p/'stdout')
 assert not r['timeout']and r['exit']==3 and d['verdict']=='unsupported'and d['diagnostics'][0]['code']=='DIVE-L0042'
 for stream in['stdout','stderr']:assert sha(p/stream)==r[stream+'_sha256']
for p,h in load(BASE/'historical-current-authorities/index.json')['artifact_sha256'].items():assert sha(BASE/'historical-current-authorities'/p)==h,p
current=BASE/'native-current';c=load(current/'capture.json');assert c['source_and_binary_unchanged']is True
assert c['selections_sha256']=={'selection.json':sha(BASE/'selection.json')}
assert c['capture_source_sha256']==sha(BASE/'capture-native.py')
for p,h in c['source_sha256'].items():successor_source(p,h)
assert [(r['model'],r['requested_workers'])for r in c['rows']]==[(r['name'],w)for r in selection for w in [1,3,4,8]]
kind={r['name']:r['kind']for r in selection};counts={}
for r in c['rows']:
 p=current/f"{r['model']}-{r['requested_workers']}";d=load(p/'stdout');ref=refs[r['model']]
 for stream in ['stdout','stderr']:assert sha(p/stream)==r[stream+'_sha256']
 assert not r['timeout']and d['options']['workers']['requested']==r['requested_workers']
 assert d['options']['ordinary_temporal_schedule_supported']is False
 if kind[r['model']]!='stored-successor':
  assert r['exit']in [3,150]and d['verdict']in ['unsupported','rejected']
  assert d['counterexample']is None
  if ref['exit']==150:
   assert r['exit']==150 and d['verdict']=='rejected'and d['tlc']['code']==3002
   assert d['options']['workers']['used']==0
  else:assert d['tlc']is None or not d['tlc']['verified']
  continue
 counts[d['verdict']]=counts.get(d['verdict'],0)+1
 assert r['exit']==ref['exit']==d['tlc']['exit_status'] and d['tlc']['verified']is True
 assert d['tlc']['code']in ref['message_codes'] and d['options']['workers']['used']==r['requested_workers']
 raw=(BASE/'clean-primary'/r['model']/'stdout').read_text()
 stats=re.search(r'@!@!@STARTMSG 2199:0 @!@!@\n(\d+) states generated, (\d+) distinct states found',raw);depth=re.search(r'The depth of the complete state graph search is (\d+)',raw);assert stats and depth
 for field,value in [('generated',stats[1]),('distinct',stats[2]),('depth',depth[1])]:assert d['statistics'][field]==int(value),(r['model'],field)
 if ref['exit']==13:
  assert d['verdict']=='liveness-violation'and d['counterexample']['validation']=='valid'
  assert d['counterexample']['trace']['properties']==['Live']and d['counterexample']['trace']['ending']=={'kind':'stuttering'}
  assert len(d['counterexample']['states'])==raw.count('@!@!@STARTMSG 2217:4 @!@!@')
 else:assert d['verdict']=='success'and d['counterexample']is None
assert counts=={'success':28,'liveness-violation':28},counts

print('Verified14 primary0/13 rows,14 exact projection rows,56 historical refusals and56 current final outcomes; no ordinary/family credit')

regression=BASE/'native-established-regression'
receipt=load(regression/'check-receipt.json');assert receipt['exit']==0
for p,h in receipt['artifact_sha256'].items():assert sha(regression/p)==h,p
import subprocess
assert subprocess.run(['python3','-I',str(regression/'check-current-source.py')],cwd=ROOT).returncode==0
