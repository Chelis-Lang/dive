"""Read back every native-actual raw outcome without promoting refused/reference failures."""
from pathlib import Path
import gzip,hashlib,json,re
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/native-temporal-operators'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
load=lambda p:json.loads(p.read_text())
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
selection=load(BASE/'selection.json');assert len(selection)==20
refs={}
m=load(BASE/'clean-primary-corrected/capture.json');assert m['oracle_sha256']==PIN and m['reason'] and m['flags']==dict(workers=1,seed=0,fp=0,lncheck='final')
assert m['selection_sha256']==sha(BASE/'selection.json')and m['capture_source_sha256']==sha(BASE/'capture-clean.py')
assert [r['name']for r in m['rows']]==[r['name']for r in selection]
for r in m['rows']:
 refs[r['name']]=r;assert not r['timeout']
 for p,h in r['model_source_sha256'].items():assert sha(BASE/p)==h,p
 for stream in ['stdout','stderr']:assert sha(BASE/'clean-primary-corrected'/r['name']/stream)==r[stream+'_sha256']
projection=load(BASE/'capture-projection.json');assert projection['oracle_sha256']==PIN and projection['reason']and projection['compiler']['exit']==0
assert projection['selection_sha256']==sha(BASE/'selection.json')and len(projection['rows'])==20
for p,h in projection['source_sha256'].items():assert sha(BASE/p)==h,p
for r in projection['rows']:
 assert not r['timeout']
 for p,h in r['model_source_sha256'].items():assert sha(BASE/p)==h,p
 for stream in ['stdout','stderr']:assert sha(BASE/'projection-raw'/r['name']/stream)==r[stream+'_sha256']
assert sum(r['exit']==0 for r in projection['rows'])==17
frozen=BASE/'frozen-known-user-producer';f=load(frozen/'manifest.json');assert f['commit']=='7683204a1ab328ffc2dabb9b932f5c545e129b79'
for p,h in f['source_sha256'].items():assert hashlib.sha256(gzip.decompress((frozen/'source'/(p+'.gz')).read_bytes())).hexdigest()==h,p
# The complete malformed-fixture campaign is preserved as its own source epoch.
dev=BASE/'development-whitespace';old=load(dev/'clean-primary/capture.json');assert len(old['rows'])==18
for r in old['rows']:
 for p,h in r['model_source_sha256'].items():assert sha(dev/p)==h,p
 for stream in ['stdout','stderr']:assert sha(dev/'clean-primary'/r['name']/stream)==r[stream+'_sha256']
assert sum(r['exit']==150 for r in old['rows'])==7
current=BASE/'native-current';c=load(current/'capture.json');assert c['source_and_binary_unchanged']is True
assert c['selections_sha256']=={'selection.json':sha(BASE/'selection.json')}
assert c['capture_source_sha256']==sha(BASE/'capture-native.py')
for p,h in c['source_sha256'].items():assert sha(ROOT/p)==h,p
assert [(r['model'],r['requested_workers'])for r in c['rows']]==[(r['name'],w)for r in selection for w in [1,3,4,8]]
kind={r['name']:r['kind']for r in selection};counts={}
for r in c['rows']:
 p=current/f"{r['model']}-{r['requested_workers']}";d=load(p/'stdout');ref=refs[r['model']]
 for stream in ['stdout','stderr']:assert sha(p/stream)==r[stream+'_sha256']
 assert not r['timeout']and d['options']['workers']['requested']==r['requested_workers']
 assert d['options']['ordinary_temporal_schedule_supported']is False
 if kind[r['model']]!='native-pure':
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
 raw=(BASE/'clean-primary-corrected'/r['model']/'stdout').read_text()
 stats=re.search(r'@!@!@STARTMSG 2199:0 @!@!@\n(\d+) states generated, (\d+) distinct states found',raw);depth=re.search(r'The depth of the complete state graph search is (\d+)',raw);assert stats and depth
 for field,value in [('generated',stats[1]),('distinct',stats[2]),('depth',depth[1])]:assert d['statistics'][field]==int(value),(r['model'],field)
 if ref['exit']==13:
  assert d['verdict']=='liveness-violation'and d['counterexample']['validation']=='valid'
  assert d['counterexample']['trace']['properties']==['Live']and d['counterexample']['trace']['ending']=={'kind':'stuttering'}
  assert len(d['counterexample']['states'])==raw.count('@!@!@STARTMSG 2217:4 @!@!@')
 else:assert d['verdict']=='success'and d['counterexample']is None
assert counts=={'success':28,'liveness-violation':20},counts
for p,h in load(BASE/'source-matrix.json')['source_sha256'].items():assert sha(ROOT/p)==h,p
print('Verified20 clean rows/17 field projections/80 native observations:48 supported final outcomes,32 retained refusals/rejections; no ordinary/family/error upgrade')
