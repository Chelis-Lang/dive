"""Capture every selected primary row across profiles/stores/widths and replay words."""
from pathlib import Path
import copy, hashlib, json, subprocess, sys, time
out,binary,producer=map(Path,sys.argv[1:4]);out.mkdir(parents=True,exist_ok=False)
base=Path(__file__).resolve().parent
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
identity=json.loads(producer.read_text());assert identity['binary_sha256']==sha(binary)
index={'runtime_producer':identity,'primary_sha256':sha(base/'primary/index.json'),'rows':[],'validation':[],
 'equality_allowlist':['elapsed_ms','options.workers.requested','options.workers.chaos','options.store','collision'],
 'qualification':'Within each profile/source/seed/fp all remaining JSON fields equal; actual used1 and each exact/fingerprint collision qualification asserted before normalization. Default approved RNG differs from clean TLC; strict concrete values compared completely.'}
primary=json.loads((base/'primary/index.json').read_text())

def execute(args,d,name):
 t=time.monotonic();r=subprocess.run(args,capture_output=True,timeout=90)
 (d/(name+'.stdout')).write_bytes(r.stdout);(d/(name+'.stderr')).write_bytes(r.stderr)
 return r,json.loads(r.stdout),time.monotonic()-t

def normalized(j,store):
 j=copy.deepcopy(j);j.pop('elapsed_ms')
 worker=j['options']['workers'];assert worker['used']==1 and worker['sequential'] is True
 worker.pop('requested');worker.pop('chaos')
 assert j['options'].pop('store')==store
 collision=j.pop('collision')
 if store=='exact' or j['verdict']!='success':assert collision is None,collision
 else:
  assert collision=={'store':'fingerprint','scheme':'tlc' if j['options']['profile']=='tlc' else 'dive','optimistic':'1.1E-19','actual':None,'assumption':"fingerprints behave as random 64-bit values (TLC's assumption too)"},collision
 return j

for p in primary['cases']:
 case,route,seed,fp=p['case'],p['route'],p['seed'],p['fp']
 model=base/'primary'/case/route/'ViewStream.tla'
 for profile in ['tlc','dive']:
  directory=out/f'{case}-{route}-seed{seed}-fp{fp}-{profile}';directory.mkdir()
  baseline=None;word=None
  for store in ['fingerprint','exact']:
   for width in [1,4,8]:
    for chaos in [None,19]:
     ident=f'{store}-w{width}-chaos{chaos}';args=[str(binary),'check',str(model),'--seed',str(seed),'--fp',str(fp),'--workers',str(width),'--store',store,'--output','json']
     if profile=='tlc':args+=['--compat=tlc']
     if chaos is not None:args+=['--chaos',str(chaos)]
     r,j,elapsed=execute(args,directory,ident)
     assert r.returncode==(p['exit'] if profile=='tlc' or p['exit']==0 else 10),(ident,p,j)
     assert j['verdict']==('success' if p['exit']==0 else 'invariant-violation'),j
     stats=j['statistics'];assert [stats['generated'],stats['distinct'],stats['left_on_queue']]==p['stats'] and stats['depth']==p['depth'],(p,j)
     assert stats['initial_generated']==1 and stats['reported_by_tlc'] is True
     assert j['output']==[] and j['warnings']==[] and j['diagnostics']==[]
     assert j['options']['seed']==seed and j['options']['fingerprint']['polynomial_index']==fp
     assert j['options']['workers']['requested']==width
     if p['exit']:
      assert j['property']=='Inv' and j['counterexample']['length']==len(p['states'])
      if profile=='tlc':assert [s['values'] for s in j['counterexample']['states']]==p['states'],(p,j)
      word=j['counterexample']['trace'];assert word is not None
     else:assert j['counterexample'] is None
     n=normalized(j,store)
     if baseline is None:baseline=n
     else:assert n==baseline,(case,route,seed,fp,profile,ident,n,baseline)
     index['rows'].append({'case':case,'route':route,'seed':seed,'fp':fp,'profile':profile,'store':store,'width':width,'chaos':chaos,'exit':r.returncode,'verdict':j['verdict'],'statistics':stats,'elapsed_seconds':elapsed,'directory':directory.name,'capture':ident})
  if word is not None:
   for kind in ['original','changed-init','changed-step','changed-ending']:
    w=copy.deepcopy(word)
    if kind=='changed-init':w['states'][0]['values']['x']+=1
    if kind=='changed-step':w['states'][1]['values']['x']=(w['states'][1]['values']['x']%100)+1
    if kind=='changed-ending':w['ending']['name']='NotAnInvariant'
    path=directory/(kind+'.json');path.write_text(json.dumps(w,indent=2)+'\n')
    args=[str(binary),'validate-trace',str(model),'--trace',str(path),'--seed',str(seed),'--fp',str(fp),'--output','json']
    if profile=='tlc':args+=['--compat=tlc']
    r,j,elapsed=execute(args,directory,kind)
    assert j['result']==('valid' if kind=='original' else 'invalid'),(p,profile,kind,j)
    assert r.returncode==(0 if kind=='original' else 1),(kind,r.returncode,j)
    index['validation'].append({'case':case,'route':route,'seed':seed,'fp':fp,'profile':profile,'kind':kind,'result':j['result'],'exit':r.returncode,'directory':directory.name,'capture':kind,'elapsed_seconds':elapsed})
 (out/'in-progress.json').write_text(json.dumps({'rows':len(index['rows']),'validation':len(index['validation'])})+'\n')
 print(case,route,seed,fp,'complete',flush=True)
assert len(primary['cases'])==140 and len(index['rows'])==3360 and len(index['validation'])==960
index['files_sha256']={str(p.relative_to(out)):sha(p) for p in out.rglob('*') if p.is_file()}
(out/'receipt.json').write_text(json.dumps(index,indent=2)+'\n')
print('Closed3360 native cases/240 valid originals/720 invalid mutants')
