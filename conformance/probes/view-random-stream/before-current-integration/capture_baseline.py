"""Retain frozen native admission before lifting the transformed random route."""
from pathlib import Path
import hashlib,json,subprocess,sys
out,binary=map(Path,sys.argv[1:3]);out.mkdir(exist_ok=False,parents=True)
b=Path(__file__).resolve().parent
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
index={'runtime_source_commit':'2c2ebda3940109fae34a033749b63a8560f8834c','binary_sha256':sha(binary),'producer':'../configured-random-stream/current/producer.json','cases':[]}
assert index['binary_sha256']=='5c86a8610ef6486c33cd1b3f4a937bd501bbad0105142c216bff9fcd8620f4fd'
for case in ['none','scalar','tuple','record','full-tuple','folded','conditional']:
 for route in ['direct','configured']:
  d=out/case/route;d.mkdir(parents=True)
  for profile in ['tlc','dive']:
   for store in ['fingerprint','exact']:
    ident=f'{profile}-{store}';args=[str(binary),'check',str(b/'primary'/case/route/'ViewStream.tla'),'--seed','7','--fp','0','--workers','8','--store',store,'--output','json']
    if profile=='tlc':args+=['--compat=tlc']
    r=subprocess.run(args,capture_output=True,timeout=90);(d/f'{ident}.stdout').write_bytes(r.stdout);(d/f'{ident}.stderr').write_bytes(r.stderr)
    j=json.loads(r.stdout)
    if case!='none':assert j['verdict']=='unsupported' and j['statistics'] is None and j['counterexample'] is None and not j['output'],j
    else:assert j['statistics']['generated']==4 and j['statistics']['distinct']==4 and j['verdict']=='invariant-violation',j
    index['cases'].append({'case':case,'route':route,'profile':profile,'store':store,'exit':r.returncode,'verdict':j['verdict'],'statistics':j['statistics'],'workers':j['options']['workers']})
index['files']={str(p.relative_to(out)):sha(p) for p in out.rglob('*') if p.is_file()}
(out/'index.json').write_text(json.dumps(index,indent=2)+'\n');print(f'Captured {len(index["cases"])} frozen ordinary admission controls')
