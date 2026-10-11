from pathlib import Path
import json,hashlib,subprocess,os,sys
root=Path(sys.argv[1]);binary=Path(sys.argv[2]);out=Path(sys.argv[3]);out.mkdir(exist_ok=False,parents=True);cache=Path(sys.argv[4]);sha=lambda b:hashlib.sha256(b).hexdigest();env=dict(os.environ)
for name in ['DIVE_COMPAT','DIVE_CHAOS']:env.pop(name,None)
jar=cache/'sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar';assert sha(jar.read_bytes())=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d';java=next(cache.glob('jre/*/jdk*/Contents/Home/bin/java'))
index={'producer_commit':'475ef10ce38f3c45037ea90437b9d12e326b3916','binary_sha256':sha(binary.read_bytes()),'oracle_sha256':sha(jar.read_bytes()),'runs':[]}
for case,live,inv,primary_exit in [('safe','[]<> (n>=0)','x # 74',0),('missed','[]<> (n>=0)','x # 80',12),('unfair','<> (n=3)','x # 74',13)]:
 d=out/case;d.mkdir();old=root/'conformance/probes/configured-random-admission/backend-before';source=(old/'SafeFinal.tla').read_text().replace('Live == []<> (n>=0)','Live == '+live).replace('Inv == x # 74','Inv == '+inv);(d/'SafeFinal.tla').write_text(source);(d/'SafeFinal.cfg').write_bytes((old/'SafeFinal.cfg').read_bytes())
 args=[str(java),'-cp',str(jar),'tlc2.TLC','-tool','-seed','7','-fp','0','-workers','1','-lncheck','final','-cleanup','-metadir',str(d/'tlc-meta'),'-config','SafeFinal.cfg','SafeFinal.tla'];p=subprocess.run(args,cwd=d,capture_output=True,timeout=30);(d/'primary.stdout').write_bytes(p.stdout);(d/'primary.stderr').write_bytes(p.stderr);assert p.returncode==primary_exit,(case,p.returncode);index['runs'].append({'case':case,'kind':'primary','exit':p.returncode,'args':['pinned-java','-cp','pinned-jar',*args[3:]]})
 for compat in [False,True]:
  for store in ['fp','exact']:
   for width in [1,4,8]:
    for chaos in [None,19]:
     for lasso in ['short','tlc']:
      key=f'{"tlc" if compat else "dive"}-{store}-w{width}-c{chaos}-{lasso}';args=[str(binary),'check','SafeFinal.tla','--lncheck','final','--lasso',lasso,'--seed','7','--fp','0','--workers',str(width),'--store',store,'--output','json']
      if compat:args+=['--compat=tlc']
      if chaos is not None:args+=['--chaos',str(chaos)]
      p=subprocess.run(args,cwd=d,capture_output=True,env=env,timeout=90);(d/(key+'.stdout')).write_bytes(p.stdout);(d/(key+'.stderr')).write_bytes(p.stderr);j=json.loads(p.stdout);assert p.returncode==3 and j['verdict']=='unsupported',(case,key,j)
      assert j['statistics'] is None and j['counterexample'] is None and j['options']['workers']['used']==0,(case,key,j)
      assert j['output']==[] and j['warnings']==[],(case,key,j)
      if lasso=='short':assert j['message']=='graph capture with configured native random replacements',(case,key,j)
      index['runs'].append({'case':case,'kind':'native','setting':key,'exit':p.returncode,'message':j['message'],'args':args[1:]})
index['files']={str(p.relative_to(out)):sha(p.read_bytes()) for p in out.rglob('*') if p.is_file()};(out/'index.json').write_text(json.dumps(index,indent=2)+'\n');print('3 clean primary and144 property-bearing native refusal controls passed')
