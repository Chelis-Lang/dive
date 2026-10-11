"""Read back the complete sequence selection without giving refused shapes proof credit."""
from pathlib import Path
import gzip,hashlib,json,re
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/native-sequence-operators'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
load=lambda p:json.loads(p.read_text())
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
selection=load(BASE/'selection.json');assert len(selection)==33
primary=load(BASE/'clean-primary/capture.json');assert primary['oracle_sha256']==PIN and primary['reason']
assert primary['flags']==dict(workers=1,seed=0,fp=0,lncheck='final')
assert primary['selection_sha256']==sha(BASE/'selection.json')and primary['capture_source_sha256']==sha(BASE/'capture-clean.py')
assert [r['name']for r in primary['rows']]==[r['name']for r in selection]
refs={r['name']:r for r in primary['rows']}
for r in primary['rows']:
 assert not r['timeout']
 for path,h in r['model_source_sha256'].items():assert sha(BASE/path)==h,path
 for stream in ['stdout','stderr']:assert sha(BASE/'clean-primary'/r['name']/stream)==r[stream+'_sha256']
assert sum(r['exit']==0 for r in primary['rows'])==13
assert sum(r['exit']==13 for r in primary['rows'])==12
assert sum(r['exit']==150 for r in primary['rows'])==4
assert sum(r['exit']==255 for r in primary['rows'])==4
projection=load(BASE/'capture-projection.json');assert projection['oracle_sha256']==PIN and projection['reason']and projection['compiler']['exit']==0
assert projection['selection_sha256']==sha(BASE/'selection.json')and len(projection['rows'])==33
for path,h in projection['source_sha256'].items():assert sha(BASE/path)==h,path
for r in projection['rows']:
 assert not r['timeout']
 for path,h in r['model_source_sha256'].items():assert sha(BASE/path)==h,path
 for stream in ['stdout','stderr']:assert sha(BASE/'projection-raw'/r['name']/stream)==r[stream+'_sha256']
assert sum(r['exit']==0 for r in projection['rows'])==29
# Keep the original failed lexical campaign intact; no runtime credit.
dev=BASE/'development-whitespace';old=load(dev/'clean-primary/capture.json');assert len(old['rows'])==33
for r in old['rows']:
 assert r['exit']==150 and not r['timeout']
 for path,h in r['model_source_sha256'].items():assert sha(dev/path)==h,path
 for stream in ['stdout','stderr']:assert sha(dev/'clean-primary'/r['name']/stream)==r[stream+'_sha256']
before=load(BASE/'native-before/capture.json');assert len(before['rows'])==132 and before['source_and_binary_unchanged']
archive=ROOT/'conformance/probes/stored-successor-material';oldmap=load(archive/'source-manifest.json')['source_sha256']
for path,h in before['source_sha256'].items():
 assert oldmap[path]==h and hashlib.sha256(gzip.decompress((archive/'source'/(path+'.gz')).read_bytes())).hexdigest()==h,path
assert sum(r['verdict']=='unsupported'for r in before['rows'])==116
assert sum(r['verdict']=='rejected'for r in before['rows'])==16
for r in before['rows']:
 for stream in ['stdout','stderr']:assert sha(BASE/'native-before'/f"{r['model']}-{r['requested_workers']}"/stream)==r[stream+'_sha256']
# Previously refreshed source authorities are preserved rather than silently retagged.
a=BASE/'historical-current-authorities'
for path,h in load(a/'manifest.json')['source_sha256'].items():assert hashlib.sha256(gzip.decompress((a/'source'/(path+'.gz')).read_bytes())).hexdigest()==h,path
current=BASE/'native-current';c=load(current/'capture.json');assert c['source_and_binary_unchanged']is True
assert c['selections_sha256']=={'selection.json':sha(BASE/'selection.json')}
assert c['capture_source_sha256']==sha(BASE/'capture-native.py')
source=load(BASE/'source-manifest.json')['source_sha256']
for path,h in c['source_sha256'].items():
 assert source[path]==h
 if sha(ROOT/path)!=h:assert hashlib.sha256(gzip.decompress((BASE/'source'/(path+'.gz')).read_bytes())).hexdigest()==h,path
assert [(r['model'],r['requested_workers'])for r in c['rows']]==[(r['name'],w)for r in selection for w in [1,3,4,8]]
kind={r['name']:r['kind']for r in selection};counts={}
for r in c['rows']:
 p=current/f"{r['model']}-{r['requested_workers']}";d=load(p/'stdout');ref=refs[r['model']]
 for stream in ['stdout','stderr']:assert sha(p/stream)==r[stream+'_sha256']
 assert not r['timeout']and d['options']['workers']['requested']==r['requested_workers']
 assert d['options']['ordinary_temporal_schedule_supported']is False
 supported=kind[r['model']]=='pure-sequence-native'and 'Append'in r['model']
 if not supported:
  assert r['exit']in [3,150]and d['verdict']in ['unsupported','rejected'],r['model']
  assert d['counterexample']is None
  if ref['exit']==150:
   assert r['exit']==150 and d['verdict']=='rejected'and d['tlc']['code']==3002
   assert d['options']['workers']['used']==0
  else:assert d['tlc']is None or not d['tlc']['verified']
  continue
 counts[d['verdict']]=counts.get(d['verdict'],0)+1
 assert r['exit']==ref['exit']==d['tlc']['exit_status'] and d['tlc']['verified']is True
 assert d['tlc']['code']in ref['message_codes']and d['options']['workers']['used']==r['requested_workers']
 raw=(BASE/'clean-primary'/r['model']/'stdout').read_text()
 stats=re.search(r'@!@!@STARTMSG 2199:0 @!@!@\n(\d+) states generated, (\d+) distinct states found',raw);depth=re.search(r'The depth of the complete state graph search is (\d+)',raw);assert stats and depth
 for field,value in [('generated',stats[1]),('distinct',stats[2]),('depth',depth[1])]:assert d['statistics'][field]==int(value),(r['model'],field)
 if ref['exit']==13:
  assert d['verdict']=='liveness-violation'and d['counterexample']['validation']=='valid'
  assert d['counterexample']['trace']['properties']==['Live']and d['counterexample']['trace']['ending']=={'kind':'stuttering'}
  assert len(d['counterexample']['states'])==raw.count('@!@!@STARTMSG 2217:4 @!@!@')
 else:assert d['verdict']=='success'and d['counterexample']is None
assert counts=={'success':16,'liveness-violation':16},counts
print('Verified33 clean primary/29 Java projections/132 native observations:32 supported bounded Append outcomes; Tail/SubSeq/error/refusal histories remain uncredited')
