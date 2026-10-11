#!/usr/bin/env python3
"""Preserve pinned Validator's failure to detect fair-algorithm source edits."""
import argparse,gzip,hashlib,json,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
JAR='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
def sha(data):return hashlib.sha256(data).hexdigest()
def main(args):
 assert sha(args.jar.read_bytes())==JAR
 study=BASE/'fair-checksum';out=study/'source-edit-controls';out.mkdir(exist_ok=True);m=json.loads((study/'primary/manifest.json').read_text());rows=[]
 for syntax in ['C','P']:
  original=next(r for r in m['rows'] if r['id']==syntax+'_fair_initial_0');source=(study/'primary'/original['translated']['path']).read_bytes();assert sha(source)==original['translated']['sha256']
  edited=source.replace(b'x=0',b'x=9');assert edited!=source
  for name,data in [('original',source),('changed-algorithm-only',edited)]:
   filename=syntax+'.'+name+'.tla';(out/filename).write_bytes(data)
   with tempfile.TemporaryDirectory(prefix='dive-fair-crc-control-') as tmp:
    file=Path(tmp)/'Reference.tla';file.write_bytes(data)
    p=subprocess.run([str(args.java),'-cp',str(args.jar)+':'+str(args.adapter),'pcal.ChecksumProbe','validate',str(file)],capture_output=True,timeout=30)
   assert p.returncode==0 and b'VALIDATION\t[NO_DIVERGENCE]' in p.stdout
   captures={}
   for stream in ['stdout','stderr']:
    content=getattr(p,stream);f=filename+'.'+stream+'.gz';(out/f).write_bytes(gzip.compress(content,mtime=0));captures[stream]={'path':f,'sha256':sha(content)}
   rows.append({'syntax':syntax,'control':name,'source':filename,'source_sha256':sha(data),'observed':'NO_DIVERGENCE','exit_code':p.returncode,'capture':captures,'model_proved':False,'algorithm_change_detected':False})
 report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'oracle_sha256':JAR,'controls':len(rows),'rows':rows,'native_full_checksum_certification':False}
 (out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Retained four pinned fair-algorithm validator controls; edited sources remain undetected')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for n in ['java','jar','adapter']:p.add_argument('--'+n,type=Path,required=True)
 main(p.parse_args())
