"""Compare native record-codec admission stops with a frozen inline executable.

Usage: python3 -I capture_records.py INLINE_DIVE CURRENT_DIVE FRESH_OUT
This is a native admission-limit control, not a TLC divergence or oracle golden.
"""
import copy, hashlib, json, os, subprocess, sys
from pathlib import Path
root=Path(__file__).resolve().parents[3]
b=root/'conformance/probes/native-checkpoint-parallel/specs/CpRecord'
old,current=[Path(p).resolve() for p in sys.argv[1:3]]
out=Path(sys.argv[3]).resolve();out.mkdir(parents=True,exist_ok=False)
sha=lambda data:hashlib.sha256(data).hexdigest()
env=os.environ.copy()
for key in ['DIVE_COMPAT','DIVE_CHAOS']:env.pop(key,None)
m={'schema_version':1,'runtime_source_commit':'aae23a08c97b1095e9545df530c267ab4552cc17','family_certified':False,
 'binary_sha256':{'inline':sha(old.read_bytes()),'current':sha(current.read_bytes())},
 'model_sha256':{str(p.relative_to(root)):sha(p.read_bytes()) for p in [b/'CpRecord.tla',b/'CpRecord.cfg']},
 'capture_script_sha256':sha(Path(__file__).read_bytes()),
 'source_sha256':{p:sha((root/p).read_bytes()) for p in ['crates/dive-check/src/checkpoint.rs','crates/dive-check/src/engine.rs','crates/dive-check/src/parallel.rs']},
 'controls':[],'archives':{}}
def save():(out/'index.json').write_text(json.dumps(m,indent=2)+'\n')
def run(bin,stem,args):
 r=subprocess.run([str(bin),*args],cwd=root,env=env,capture_output=True,timeout=45)
 (out/(stem+'.stdout.json')).write_bytes(r.stdout);(out/(stem+'.stderr')).write_bytes(r.stderr)
 m['controls'].append({'id':stem,'args':args,'exit':r.returncode,'stdout':stem+'.stdout.json','stdout_sha256':sha(r.stdout),'stderr':stem+'.stderr','stderr_sha256':sha(r.stderr)});save()
 return r.returncode,json.loads(r.stdout)
def obs(v):
 v=copy.deepcopy(v);v.pop('elapsed_ms',None);v.pop('checkpoint',None)
 for k in ['checkpoint','workers','profile']:v['options'].pop(k,None)
 return v
for profile in ['dive','tlc']:
 for store in ['fp','exact']:
  base=['check',str((b/'CpRecord.tla').relative_to(root)),'--config',str((b/'CpRecord.cfg').relative_to(root)),'--seed','7','--fp','0','--store',store,'--output','json']
  if profile=='tlc':base+=['--compat=tlc']
  paths={}
  for label,bin in [('inline',old),('current',current)]:
   stem=f'{profile}-{store}-{label}-pause';path=out/(stem+'.cp');paths[label]=path
   r=run(bin,stem,base+['--workers','8','--checkpoint',str(path),'--checkpoint-after-levels','0','--checkpoint-counters','retain'])
   assert r[1]['checkpoint']['status']=='paused-incomplete';assert r[1]['options']['workers']['used']==1
   m['archives'][path.name]=sha(path.read_bytes())
  ref=run(old,f'{profile}-{store}-inline-recover',base+['--workers','1','--recover',str(paths['inline']),'--checkpoint-counters','retain'])
  assert ref[1]['verdict']=='unsupported' and 'codec' in ref[1]['message'] and 'DIVE-V0030' in ref[1]['message'],ref
  for k,v in [('generated',2),('distinct',2),('depth',1),('left_on_queue',0)]:assert ref[1]['statistics'][k]==v,ref
  assert len(ref[1]['counterexample']['states'])==1,ref
  for width in [1,4,8]:
   for chaos in [None,19]:
    stem=f'{profile}-{store}-w{width}-c{chaos}'
    args=base+['--workers',str(width),'--recover',str(paths['current']),'--checkpoint-counters','retain']
    if chaos is not None:args+=['--chaos',str(chaos)]
    r=run(current,stem,args)
    assert (r[0],obs(r[1]))==(ref[0],obs(ref[1])),stem
    assert r[1]['options']['workers']['used']==width,stem
    print(stem,'native-record-prefix-agrees',flush=True)
assert len(m['controls'])==36 and len(m['archives'])==8
save()
