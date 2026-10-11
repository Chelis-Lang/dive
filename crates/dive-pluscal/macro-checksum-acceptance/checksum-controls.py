"""Actual guide CST/SANY/Validator and independent checksum corruption controls."""
from pathlib import Path
import subprocess,json,gzip,hashlib,re,tempfile
import argparse
p=argparse.ArgumentParser()
for key in ['base','binaries','out','java','jar','adapter']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();G=a.base;B=a.binaries;O=a.out;O.mkdir(exist_ok=False);rows=[];J=a.java;JAR=a.jar
def sha(b):return hashlib.sha256(b).hexdigest()
def run(argv,name):
 q=subprocess.run(list(map(str,argv)),capture_output=True,timeout=30)
 for s in ['stdout','stderr']:(O/(name+'.'+s+'.gz')).write_bytes(gzip.compress(getattr(q,s),mtime=0))
 return q
def java(argv,name):return run([J,'-Xmx256m','-cp',str(a.adapter)+':'+str(JAR),*argv],name)
manifest=json.loads((G/'primary/manifest.json').read_text())
for r in manifest['rows']:
 name=r['id'];source=(G/'cases'/(name+'.tla')).read_text();module=re.search(r'MODULE\s+(\w+)',source).group(1)
 with tempfile.TemporaryDirectory(prefix='dive-macro-crc-')as tmp:
  d=Path(tmp);file=d/(module+'.tla');file.write_text(source);q=run([B/'dive','translate',file,'--rewrite','--output','json'],name+'.generate');doc=json.loads(q.stdout);assert q.returncode==0 and doc['accepted']and doc['model_proved']is False;native=doc['replacement']['module_source'];file.write_text(native);reference=d/'primary'/file.name;reference.parent.mkdir();reference.write_bytes((G/'primary'/(name+'.translated.tla')).read_bytes())
  trees=[]
  for label,path in [('native',file),('primary',reference)]:
   sany=java(['tla2sany.SANY','-error-codes',path],name+'.'+label+'-sany');assert sany.returncode==0
   cst=run([B/'cst',path],name+'.'+label+'-cst');tree=json.loads(cst.stdout);assert cst.returncode==0 and tree['accepted'];trees.append(tree['tree'])
  assert trees[0]==trees[1]
  for kind in ['unchanged','algorithm-source','algorithm-header','body','body-header','both']:
   altered=native
   if kind=='algorithm-source':altered=altered.replace('Set(x, 1 + 2)', 'Set(x, 1 + 3)')
   if kind in ['algorithm-header','both']:altered=re.sub(r'(chksum\(pcal\) = ")[0-9a-f]+',r'\g<1>00000000',altered,count=1)
   if kind=='body':altered=altered.replace("x' = 1 + 2","x' = 1 + 3")
   if kind in ['body-header','both']:altered=re.sub(r'(chksum\(tla\) = ")[0-9a-f]+',r'\g<1>00000000',altered,count=1)
   if kind!='unchanged':assert altered!=native
   file.write_text(altered);v=java(['pcal.OptionValidatorProbe',file],name+'.'+kind+'.validator');m=re.search(rb'VALIDATION\t([^\r\n]*)',v.stdout);assert v.returncode==0 and m;status=m[1].decode();q=run([B/'dive','translate',file,'--rewrite','--output','json'],name+'.'+kind+'.native');doc=json.loads(q.stdout)
   if kind=='unchanged':assert q.returncode==0 and doc['accepted']and 'NO_DIVERGENCE'in status;result='valid-unchanged'
   else:assert q.returncode!=0 and not doc['accepted']and doc['result']!='internal-error'and'replacement'not in doc and 'NO_DIVERGENCE'not in status;result='corruption-refused'
   rows.append({'id':name+'.'+kind,'outcome':result,'validator':status,'model_proved':False})
report={'schema_version':1,'selected':len(rows),'script_sha256':sha(Path(__file__).read_bytes()),'binary_sha256':sha((B/'dive').read_bytes()),'cst_sha256':sha((B/'cst').read_bytes()),'oracle_sha256':sha(JAR.read_bytes()),'family_certified':False,'rows':rows};(O/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Closed',len(rows),'unchanged/corrupt CRCcontrols plus2whole CST/SANYguidepairs')
