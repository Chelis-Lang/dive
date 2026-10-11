"""Capture exact former known-user refusal controls before updating expectations."""
from pathlib import Path
import hashlib,json,subprocess,tempfile,argparse,re
p=argparse.ArgumentParser();p.add_argument('--java',required=True);p.add_argument('--jar',required=True);p.add_argument('--binary',required=True);a=p.parse_args()
b=Path(__file__).resolve().parent;d=b/'former-refusal-controls';assert not list(d.iterdir())
models={'Refusal': ('---- MODULE Refusal ----\nEXTENDS TLC\nVARIABLE x\nInit == x = TRUE\nNext == x\' = ~x\nF(a) == WF_x(a)\nFair == F(Next)\nEffect == []Print(x,x)\nRead == [](TLCGet("level") = 0)\nT(p) == []p\nParam == T(x)\nH(G(_),p) == G(p)\nHigher == H(T,x)\nBad == <>1\n====\n','INIT Init\nNEXT Next\nPROPERTY Higher\n'), 'OpaqueTemporal':('---- MODULE OpaqueTemporal ----\nVARIABLE x\nInit == x = FALSE\nNext == x\' = ~x\nT(p) == []<>p\nH(F(_),p) == F(p)\nP == H(T,x)\n====\n','INIT Init\nNEXT Next\nPROPERTY P\n')}
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert sha(a.jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
rows=[]
for name,(source,cfg) in models.items():
 folder=d/name;folder.mkdir();(folder/(name+'.tla')).write_text(source);(folder/(name+'.cfg')).write_text(cfg)
 with tempfile.TemporaryDirectory() as meta:
  cmd=[a.java,'-cp',a.jar,'tlc2.TLC','-tool','-workers','1','-seed','0','-fp','0','-lncheck','final','-cleanup','-metadir',meta,name];c=subprocess.run(cmd,cwd=folder,capture_output=True,timeout=30)
 (folder/'primary-stdout').write_bytes(c.stdout);(folder/'primary-stderr').write_bytes(c.stderr)
 n=subprocess.run([a.binary,'check',str(folder/(name+'.tla')),'--compat=tlc','--lncheck=final','--lasso=tlc','--output=json','--seed=0','--fp=0'],capture_output=True,timeout=30);(folder/'native-stdout').write_bytes(n.stdout);(folder/'native-stderr').write_bytes(n.stderr)
 rows.append({'name':name,'primary_exit':c.returncode,'native_exit':n.returncode,'primary_codes':[int(x) for x in re.findall(rb'STARTMSG (\d+):',c.stdout)],'files':{p.name:sha(p) for p in folder.iterdir()},'native_verdict':json.loads(n.stdout).get('verdict')});print(name,rows[-1]['primary_exit'],rows[-1]['native_exit'],rows[-1]['native_verdict'])
(d/'capture.json').write_text(json.dumps({'oracle_sha256':sha(a.jar),'binary_sha256':sha(a.binary),'reason':'Independently pin exact formerly refused known-user forwarding fixtures after failed939/941 gate; original gate remains failed','flags':dict(workers=1,seed=0,fp=0,lncheck='final'),'capture_source_sha256':sha(__file__),'rows':rows},indent=2)+'\n')
