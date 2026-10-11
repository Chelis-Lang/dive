"""Actual native and pinned Validator corruption decisions for procedure headers."""
from pathlib import Path
import argparse,subprocess,json,gzip,hashlib,re,tempfile
p=argparse.ArgumentParser()
for key in ['base','binaries','out','java','jar','adapter']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();a.out.mkdir(exist_ok=False);rows=[]
sha=lambda b:hashlib.sha256(b).hexdigest()
def run(argv,name):
 q=subprocess.run(list(map(str,argv)),capture_output=True,timeout=30)
 for stream in ['stdout','stderr']:(a.out/(name+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
 return q
for row in json.loads((a.base/'guide/primary/manifest.json').read_text())['rows']:
 source=(a.base/'guide/cases'/(row['id']+'.tla')).read_text()
 for profile in ['dive','tlc']:
  with tempfile.TemporaryDirectory(prefix='dive-procedure-crc-')as tmp:
   file=Path(tmp)/(row['id']+'.tla');file.write_text(source);flags=['--compat=tlc']if profile=='tlc'else[]
   q=run([a.binaries/'dive','translate',file,'--rewrite','--output','json',*flags],row['id']+'.'+profile+'.generate');doc=json.loads(q.stdout);assert q.returncode==0 and doc['accepted']and doc['model_proved']is False;native=doc['replacement']['module_source']
   for kind in ['unchanged','source','algorithm-header','body','body-header','both']:
    modified=native
    if kind=='source':modified=modified.replace('Put(q)','Put(q+1)')
    if kind in ['algorithm-header','both']:modified=re.sub(r'(chksum\(pcal\) = ")[0-9a-f]+',r'\g<1>00000000',modified,count=1)
    if kind=='body':modified=modified.replace("x' = a","x' = a + 1")
    if kind in ['body-header','both']:modified=re.sub(r'(chksum\(tla\) = ")[0-9a-f]+',r'\g<1>00000000',modified,count=1)
    if kind!='unchanged':assert modified!=native
    file.write_text(modified);ident=row['id']+'.'+profile+'.'+kind
    q=run([a.java,'-Xmx256m','-cp',str(a.adapter)+':'+str(a.jar),'pcal.OptionValidatorProbe',file],ident+'.validator');m=re.search(rb'VALIDATION\t([^\r\n]*)',q.stdout);assert q.returncode==0 and m;status=m[1].decode()
    q=run([a.binaries/'dive','translate',file,'--rewrite','--output','json',*flags],ident+'.native');doc=json.loads(q.stdout)
    if kind=='unchanged':assert q.returncode==0 and doc['accepted']and 'NO_DIVERGENCE'in status
    else:assert q.returncode!=0 and not doc['accepted']and doc['result']!='internal-error'and'replacement'not in doc and 'NO_DIVERGENCE'not in status
    rows.append({'id':ident,'outcome':'valid-unchanged'if kind=='unchanged'else'corruption-refused','validator':status,'model_proved':False})
(a.out/'report.json').write_text(json.dumps({'schema_version':1,'selected':len(rows),'script_sha256':sha(Path(__file__).read_bytes()),'native_sha256':sha((a.binaries/'dive').read_bytes()),'oracle_sha256':sha(a.jar.read_bytes()),'family_certified':False,'rows':rows},indent=2)+'\n');print('Closed',len(rows),'native/Validator unchanged+corruption controls')
