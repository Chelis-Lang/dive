from pathlib import Path
import subprocess,tempfile,json,hashlib,re
base=Path('/tmp/dive-native-operator-reference');base.mkdir(exist_ok=False);sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();java='/Users/jeff/code/dive-temporal-operator-cache/jre/190480874ccceb358cbc840393207f77ac3e63a4c5f8129d0e23e9518b96ad05/jdk-17.0.20.1+1-jre/Contents/Home/bin/java';jar=Path('/Users/jeff/code/dive-temporal-operator-cache/sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar');binary=Path('/tmp/dive-temporal-operator-frozen-native/dive');binary_hash=sha(binary);rows=[]
cases=[]
for op,module,value,expression in [('Len','Sequences','<<1>>','F(x) = {expected}'),('Cardinality','FiniteSets','{1}','F(x) = {expected}'),('Head','Sequences','<<{boolean}>>','F(x)')]:
 for holds in [True,False]:
  n='Native'+op+('Holds'if holds else'Fails');cases.append((n,f'EXTENDS {module}\nVARIABLE x\nInit == x = '+value.replace('{boolean}','TRUE'if holds else'FALSE')+'\nNext == UNCHANGED x\nUse(F(_)) == []<>('+expression.format(expected=1 if holds else 2)+')\nLive == Use('+op+')\n','native-pure',holds))
cases.extend([('NativePrintUnused','EXTENDS TLC\nVARIABLE x\nInit == x=TRUE\nNext == UNCHANGED x\nUse(F(_)) == []TRUE\nLive == Use(PrintT)\n','effectful-unused',None),('NativeWrongArity','EXTENDS Sequences\nVARIABLE x\nInit == x=<<TRUE>>\nNext == UNCHANGED x\nUse(F(_,_)) == []<>F(x,x)\nLive == Use(Len)\n','arity',None)])
for name,body,kind,holds in cases:
 d=base/name;d.mkdir();source=d/(name+'.tla');source.write_text('---- MODULE '+name+' ----\n'+body+'====\n');(d/(name+'.cfg')).write_text('INIT Init\nNEXT Next\nPROPERTY Live\n')
 with tempfile.TemporaryDirectory()as meta:
  c=subprocess.run([java,'-cp',str(jar),'tlc2.TLC','-tool','-workers','1','-seed','0','-fp','0','-lncheck','final','-cleanup','-metadir',meta,name],cwd=d,capture_output=True,timeout=30)
 (d/'primary-stdout').write_bytes(c.stdout);(d/'primary-stderr').write_bytes(c.stderr)
 n=subprocess.run([str(binary),'check',str(source),'--compat=tlc','--lncheck=final','--lasso=tlc','--seed=0','--fp=0','--output=json'],capture_output=True,timeout=30);(d/'native-before-stdout').write_bytes(n.stdout);(d/'native-before-stderr').write_bytes(n.stderr)
 try:j=json.loads(n.stdout)
 except ValueError:j={}
 rows.append(dict(name=name,kind=kind,expected_holds=holds,primary_exit=c.returncode,primary_codes=[int(x)for x in re.findall(rb'STARTMSG (\d+):',c.stdout)],native_before_exit=n.returncode,native_before_verdict=j.get('verdict'),files={p.name:sha(p)for p in d.iterdir()}));print(name,c.returncode,n.returncode,j.get('verdict'),flush=True)
assert sha(binary)==binary_hash
(base/'capture.json').write_text(json.dumps(dict(oracle_sha256=sha(jar),candidate_binary_sha256=binary_hash,flags=dict(workers=1,seed=0,fp=0,lncheck='final'),reason='Capture next pure native actual boundary while current known-user delivery remains frozen; candidate native refusals/arity/effects receive no agreement credit',rows=rows),indent=2)+'\n')
