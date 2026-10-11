"""Capture clean pinned single-worker TLC configured-RandomElement controls."""
from pathlib import Path
import hashlib,json,re,subprocess,sys
out,java,jar=map(Path,sys.argv[1:4]);out.mkdir(parents=True,exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
direct=len(sys.argv)>4 and sys.argv[4]=='direct'
base={'decl':'CONSTANT C(_)','extra':'','init':'n=0 /\\ x=0','rhs':'C(1..100)'}
cases={
 'natural':{},'redirect':{'decl':'C(S) == CHOOSE a \\in S : TRUE'},
 'visible-unused':{'extra':'Unused == RandomElement(1..100)'},
 'instance-import':{'extra':'I == INSTANCE TLC'},
 'two-draws':{'rhs':'C(1..100)+C(1..100)'},
 'discarded-branch':{'rhs':'IF FALSE THEN RandomElement(1..100) ELSE C(1..100)'},
 'assume-source':{'extra':'ASSUME RandomElement(1..100)>0'},
 'assume-configured':{'extra':'ASSUME C(1..100)>0'},
 'print-source':{'extra':'ASSUME Print(RandomElement(1..100),TRUE)'},
 'print-configured':{'extra':'ASSUME Print(C(1..100),TRUE)'},
 'cached-source':{'extra':'Cached == RandomElement(1..100)'},
 'cached-configured':{'extra':'Cached == C(1..100)'},
 'cached-print':{'extra':'Cached == Print(C(1..100),TRUE)'},
 'random-init':{'init':'n=0 /\\ x=C(1..100)'},
 'random-init-after-assume':{'extra':'ASSUME C(1..100)>0','init':'n=0 /\\ x=C(1..100)'},
 'conditional-random-init':{'init':'n=0 /\\ x=IF TRUE THEN C(1..100) ELSE RandomElement(1..100)'},
 'wrong-verdict-success':{'invariant':'x # 74'},'wrong-verdict-violation':{'invariant':'x # 80'},
}
index={'java_sha256':sha(java),'jar_sha256':sha(jar),'route':'direct-source' if direct else 'configured', 'scope':'Clean unperturbed TLC primary; no dumpTrace, worker1, fixed fp0 and seed. Runtime source captures will be qualified separately.','cases':[]}
assert index['jar_sha256']=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
for name,overrides in cases.items():
 c=base|overrides;d=out/name;d.mkdir()
 if direct:
  for field in ('extra','init','rhs'):c[field]=c[field].replace('C(', 'RandomElement(')
 (d/'Stream.tla').write_text(f'---- MODULE Stream ----\nEXTENDS Naturals,TLC\n{c["decl"]}\nVARIABLES n,x\n{c["extra"]}\nInit == {c["init"]}\nNext == n<3 /\\ n\'=n+1 /\\ x\'={c["rhs"]}\nInv == {c.get("invariant","n<3")}\n====\n')
 (d/'Stream.cfg').write_text('CONSTANT C <- RandomElement\nINIT Init\nNEXT Next\nINVARIANT Inv\nCHECK_DEADLOCK FALSE\n')
 for seed in [0,7,14]:
  args=[str(java),'-cp',str(jar),'tlc2.TLC','-tool','-seed',str(seed),'-fp','0','-workers','1','-cleanup','-metadir',str(d/f'meta-{seed}'),'-config','Stream.cfg','Stream']
  r=subprocess.run(args,cwd=d,capture_output=True,timeout=90)
  (d/f'seed-{seed}.stdout').write_bytes(r.stdout);(d/f'seed-{seed}.stderr').write_bytes(r.stderr)
  text=r.stdout.decode(); stats=re.findall(r'([\d,]+) states generated, ([\d,]+) distinct states found, ([\d,]+) states left on queue',text)
  assert len(stats)==1,(name,seed,r.returncode,text)
  stat=[int(x.replace(',','')) for x in stats[0]]
  depth=re.findall(r'The depth of the complete state graph search is (\d+)',text)
  states=[{key:int(value) for key,value in re.findall(r'/\\ (n|x) = (-?\d+)',part)} for part in re.findall(r'@!@!@STARTMSG 2217:4 @!@!@\n(.*?)@!@!@ENDMSG 2217',text,re.S)]
  states=[x for x in states if 'n' in x and 'x' in x]
  assert r.returncode in [0,12],(name,seed,r.returncode,text)
  index['cases'].append({'case':name,'seed':seed,'exit':r.returncode,'stats':stat,'depth':int(depth[0]) if depth else None,'trace_states':states})
index['files']={str(p.relative_to(out)):sha(p) for p in out.rglob('*') if p.is_file()}
(out/'index.json').write_text(json.dumps(index,indent=2)+'\n')
print(f'Captured {len(index["cases"])} clean pinned primary executions')
