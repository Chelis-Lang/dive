"""Capture clean pinned VIEW-backed random streams before lifting admission."""
from pathlib import Path
import hashlib,json,re,subprocess,sys
out,java,jar=map(Path,sys.argv[1:4]);out.mkdir(parents=True,exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
index={'jar_sha256':sha(jar),'java_sha256':sha(java),'pinned_source':'94d0c5070852b37b801bb8998c436383ab7ee37b','scope':'Clean primary; no dumpTrace; workers1; explicit seed/fp; independently captured direct/configured routes','cases':[]}
assert index['jar_sha256']=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
views={'none':None,'scalar':'n','tuple':'<<n>>','record':'[phase |-> n]','full-tuple':'<<n,x>>','folded':'n % 2','conditional':'IF n % 2 = 0 THEN n ELSE n+100'}
for name,view in views.items():
 for route in ['direct','configured']:
  d=out/name/route;d.mkdir(parents=True)
  rhs='RandomElement(1..100)' if route=='direct' else 'C(1..100)'
  source=f"---- MODULE ViewStream ----\nEXTENDS Naturals,TLC\nCONSTANT C(_)\nVARIABLES n,x\nInit == n=0 /\\ x=0\nNext == n<3 /\\ n'=n+1 /\\ x'={rhs}\nInv == n<3\n"+(f"Projection == {view}\n" if view else '')+"====\n"
  cfg='CONSTANT C <- RandomElement\nINIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n'+('VIEW Projection\n' if view else '')
  (d/'ViewStream.tla').write_text(source);(d/'ViewStream.cfg').write_text(cfg)
  for seed in [0,7,14]:
   for fp in [0,7]:
    ident=f'seed-{seed}-fp-{fp}'
    args=[str(java),'-cp',str(jar),'tlc2.TLC','-tool','-seed',str(seed),'-fp',str(fp),'-workers','1','-cleanup','-metadir',str(d/f'meta-{ident}'),'-config','ViewStream.cfg','ViewStream']
    r=subprocess.run(args,cwd=d,capture_output=True,timeout=90)
    (d/f'{ident}.stdout').write_bytes(r.stdout);(d/f'{ident}.stderr').write_bytes(r.stderr)
    text=r.stdout.decode();stats=re.findall(r'([\d,]+) states generated, ([\d,]+) distinct states found, ([\d,]+) states left on queue',text)
    assert len(stats)==1 and r.returncode in [0,12],(name,route,seed,fp,r.returncode,text)
    states=[{key:int(value) for key,value in re.findall(r'/\\ (n|x) = (-?\d+)',part)} for part in re.findall(r'@!@!@STARTMSG 2217:4 @!@!@\n(.*?)@!@!@ENDMSG 2217',text,re.S)]
    depth=re.findall(r'The depth of the complete state graph search is (\d+)',text)
    index['cases'].append({'case':name,'route':route,'seed':seed,'fp':fp,'exit':r.returncode,'stats':[int(v.replace(',','')) for v in stats[0]],'depth':int(depth[0]) if depth else None,'states':states})
index['files']={str(p.relative_to(out)):sha(p) for p in out.rglob('*') if p.is_file()}
(out/'index.json').write_text(json.dumps(index,indent=2)+'\n')
print(f'Captured {len(index["cases"])} clean pinned VIEW stream executions')
