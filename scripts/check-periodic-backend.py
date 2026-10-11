"""Verify complete pointer-growth/source epochs without ordinary schedule/proof credit."""
from pathlib import Path
import gzip,hashlib,json
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/periodic-backend'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
load=lambda p:json.loads(p.read_text())
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
for directory,count in [(BASE,36),(BASE/'development-growth32',32)]:
 c=load(directory/'capture-growth.json');selected=load(directory/'selection.json')
 assert c['oracle_sha256']==PIN and c['reason'] and c['compiler']['exit']==0
 assert len(c['rows'])==len(selected)==count
 assert c['selection_sha256']==sha(directory/'selection.json')
 assert [r['name'] for r in c['rows']]==[r['name'] for r in selected]
 for p,h in c['source_sha256'].items():assert sha(directory/p)==h,p
 for i,r in enumerate(c['rows']):
  assert not r['timeout'] and r['exit']==(0 if i<32 else 1)
  for p,h in r['model_source_sha256'].items():assert sha(BASE/p)==h,p
  for stream in ['stdout','stderr']:assert sha(directory/'raw'/r['name']/stream)==r[stream+'_sha256']
  if i>=32:
   assert r['fields']['fault']=='closed-actual-disk-before-check'
   assert r['fields']['shutdown.g0.last']==r['fields']['check.source.0.before.g0.last']
  else:assert r['fields']['check.initial.code']=='0'
  parsed={}
  for line in (directory/'raw'/r['name']/'stdout').read_text().splitlines():
   if '\t' in line:
    k,v=line.split('\t',1);assert k not in parsed;parsed[k]=v
  assert parsed==r['fields']
c=load(BASE/'capture-tables.json');assert c['oracle_sha256']==PIN and c['reason'] and c['compiler_exit']==0
assert len(c['rows'])==1 and c['rows'][0]['exit']==0 and not c['rows'][0]['timeout']
for p,h in c['source_sha256'].items():assert sha(BASE/p)==h,p
for p,h in c['artifacts'].items():assert sha(BASE/p)==h,p
assert sum(k.startswith('due.') for k in c['rows'][0]['fields'])==30
f=c['rows'][0]['fields'];assert f['null.root.size']=='0' and f['tableau.second_particle.size']=='1' and f['tableau.done_only.size']=='2' and f['tableau.restore.size']=='3' and f['tableau.restored_done_only']=='true'
d=BASE/'development-first-clippy';c=load(d/'record.json');assert c['exit']==101
for p,h in c['source_sha256'].items():assert hashlib.sha256(gzip.decompress((d/'source'/(p+'.gz')).read_bytes())).hexdigest()==h,p
for stream in ['stdout','stderr']:assert sha(d/stream)==c[stream+'_sha256']
for name in ['initial-prefix','constant-init','inline-worker']:
 d=BASE/('development-'+name+'-expectation');c=load(d/'record.json')
 assert c['exit']==101 and c['reason']
 assert hashlib.sha256(gzip.decompress((d/'observer_barrier.rs.gz').read_bytes())).hexdigest()==c['source_sha256']
 assert sha(d/'focused.log')==c['log_sha256']
if (BASE/'historical-before-backend/manifest.json').exists():
 h=BASE/'historical-before-backend';m=load(h/'manifest.json')
 for p,expected in m['source_sha256'].items():assert hashlib.sha256(gzip.decompress((h/'source'/(p+'.gz')).read_bytes())).hexdigest()==expected,p
 for p,expected in m['matrices_sha256'].items():assert sha(h/'matrices'/p)==expected,p
if (BASE/'development-first-gate/after.json').exists():
 d=BASE/'development-first-gate';m=load(d/'after.json')
 assert m['source_and_binaries_unchanged'] and not m['all_steps_passed']
 assert sha(d/'workspace.log')==m['log_sha256']
 steps=load(d/'steps.json');assert steps[-1]['exit']==1 and steps[-1]['command']=='python3 -I scripts/check-tlc-final-safety.py'
 for p,h in load(d/'current-matrices/manifest.json')['source_sha256'].items():
  assert hashlib.sha256(gzip.decompress((d/'current-matrices'/(p+'.gz')).read_bytes())).hexdigest()==h,p
if (BASE/'source-matrix.json').exists():
 for p,h in load(BASE/'source-matrix.json')['source_sha256'].items():assert sha(ROOT/p)==h,p
if (BASE/'native-current/manifest.json').exists():
 d=BASE/'native-current';m=load(d/'manifest.json');assert m['model_proof'] is False
 primary=load(BASE/'capture-growth.json')['rows'][:32]
 assert len(m['rows'])==128 and len(set(m['rows']))==128
 assert set(m['rows'])=={r['name']+'-w'+str(w)+'.json' for r in primary for w in [1,3,4,8]}
 for p,h in m['artifact_sha256'].items():assert sha(d/p)==h,p
 for r in primary:
  fields=r['fields']
  for width in [1,3,4,8]:
   n=load(d/(r['name']+'-w'+str(width)+'.json'))
   assert n['name']==r['name'] and n['workers']==width and n['model_proof'] is False
   assert n['inspections']<=n['graph_inspections']
   expected={k[:-5] for k in fields if k.startswith('check.') and k.endswith('.code')}
   assert len(n['checkpoints'])==len(expected)
   for c in n['checkpoints']:
    k='check.initial' if c['sources']==0 else 'check.source.'+str(c['sources']-1)
    assert k in expected and c['model_proof'] is False
    assert str(c['growth_due']).lower()==fields[k+'.before.due']
    assert (c['decision']=='Candidate')==(fields[k+'.code']=='2116')
    for part in ['before','after']:
     for i,g in enumerate(c[part]):
      assert str(g['size'])==fields[k+'.'+part+'.g'+str(i)+'.size']
      assert str(g['last'])==fields[k+'.'+part+'.g'+str(i)+'.last']
    b=c['barrier']
    assert (b is None)==(c['sources']==0)
    if b is not None:assert b['workers']==width and b['first']<=c['sources']-1<b['end'] and b['end']-b['first']<=8
   candidate=n['checkpoints'][-1]['decision']=='Candidate'
   assert n['complete_safety'] is not candidate
   if candidate:
    assert n['stop']['code']=='DIVE-L0054' and n['stop']['workers']['used']==width
    assert n['stop']['stats']==n['checkpoints'][-1]['stats']
   else:assert n['stop'] is None
print('Verified36 complete source-growth rows (4 real failures uncredited), original32 history, table/threshold projection and source epochs; no ordinary schedule/Holds credit')
