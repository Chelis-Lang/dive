#!/usr/bin/env python3
"""Record clean SANY disposition for every translated primary artifact."""
import argparse,gzip,hashlib,json,re,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main(a):
 assert sha(a.jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
 out=BASE/'branch-separator-sany';out.mkdir();rows=[]
 for folder in ['separator-controls','empty-else-controls']:
  b=BASE/folder;m=json.loads((b/'manifest.json').read_text())
  for r in m['rows']:
   if not r['primary_translated']:rows.append({'id':r['id'],'outcome':'primary-source-rejected'});continue
   source=(b/(r['id']+'.translated.tla')).read_bytes();module=re.search(rb'MODULE (\w+)',source).group(1).decode()
   with tempfile.TemporaryDirectory(prefix='dive-crc-sany-') as tmp:
    f=Path(tmp)/(module+'.tla');f.write_bytes(source)
    p=subprocess.run([str(a.java.resolve()),'-cp',str(a.jar.resolve()),'tla2sany.SANY','-error-codes',str(f)],cwd=tmp,capture_output=True,timeout=30)
    for name in ['stdout','stderr']:(out/(r['id']+'.'+name+'.gz')).write_bytes(gzip.compress(getattr(p,name),mtime=0))
    rows.append({'id':r['id'],'artifact_sha256':hashlib.sha256(source).hexdigest(),'exit_code':p.returncode,'outcome':'sany-accepted' if p.returncode==0 else 'primary-invalid-artifact'})
   print(r['id'],p.returncode,flush=True)
 report={'schema_version':1,'selected':len(rows),'jar_sha256':sha(a.jar),'script_sha256':sha(Path(__file__)),'family_certified':False,'model_proved':False,'rows':rows}
 (out/'report.json').write_text(json.dumps(report,indent=2)+'\n')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for key in ['java','jar']:p.add_argument('--'+key,type=Path,required=True)
 main(p.parse_args())
