"""Capture every pure runtime-name control against the clean pinned TLC oracle."""
import hashlib,json,re,subprocess,sys,tempfile
from pathlib import Path
root=Path(__file__).resolve().parents[3]
base=Path(__file__).resolve().parent
java,jar=map(lambda p:Path(p).resolve(),sys.argv[1:3])
out=Path(sys.argv[3]).resolve();out.mkdir(parents=True,exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
idx={'oracle_sha256':sha(jar),'java_sha256':sha(java),'capture_script_sha256':sha(Path(__file__)), 'trace_capture':'clean primary tool/human trace; no dumpTrace flags','source_sha256':{},'controls':[]}
for directory in sorted((base/'specs').iterdir()):
 case=directory.name;tla=directory/(case+'.tla');cfg=tla.with_suffix('.cfg')
 for p in [tla,cfg]:idx['source_sha256'][str(p.relative_to(root))]=sha(p)
 for seed in [0,7,14]:
  with tempfile.TemporaryDirectory(prefix='dive-native-name-primary-') as meta:
   flags=['-tool','-workers','1','-fp','0','-seed',str(seed),'-cleanup','-metadir',meta,'-config',str(cfg),str(tla)]
   r=subprocess.run([str(java),'-cp',str(jar),'tlc2.TLC',*flags],capture_output=True,timeout=45)
  stem=f'{case}-seed{seed}'; streams={}
  for stream in ['stdout','stderr']:
   p=out/(stem+'.'+stream);p.write_bytes(getattr(r,stream));streams[stream]=p.name;streams[stream+'_sha256']=sha(p)
  counts=re.search(rb'(\d+) states generated, (\d+) distinct states found, (\d+) states left on queue',r.stdout)
  assert r.returncode==12 and counts,(case,seed,r.returncode,r.stdout)
  idx['controls'].append({'case':case,'seed':seed,'exit':r.returncode,'flags':flags,**streams,'counts':list(map(int,counts.groups()))})
  (out/'index.json').write_text(json.dumps(idx,indent=2)+'\n')
  print(case,seed,idx['controls'][-1]['counts'],flush=True)
assert len(idx['controls'])==15
