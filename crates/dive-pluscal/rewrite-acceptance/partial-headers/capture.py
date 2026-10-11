#!/usr/bin/env python3
"""Capture pinned clean rewrite bytes and separately labelled public callback controls."""
import argparse,gzip,hashlib,json,re,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
JAR='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
def sha(data):return hashlib.sha256(data).hexdigest()
def run(command,cwd,out,stem):
 p=subprocess.run(command,cwd=cwd,capture_output=True,timeout=30);c={'exit_code':p.returncode}
 for name in ['stdout','stderr']:
  data=getattr(p,name);f=stem+'.'+name+'.gz';(out/f).write_bytes(gzip.compress(data,mtime=0));c[name]=f;c[name+'_sha256']=sha(data)
 return c,p.stdout

def capture(args):
 assert sha(args.jar.read_bytes())==JAR
 selection=json.loads((BASE/'selection.json').read_text());assert set(r['input'] for r in selection['rows'])==set(p.name for p in (BASE/'cases').glob('*.tla'))
 out=BASE/'primary';out.mkdir(exist_ok=True);rows=[]
 prefix=[str(args.java),'-Xmx256m','-cp',str(args.jar)]
 for original in selection['rows']:
  source=(BASE/'cases'/original['input']).read_bytes();assert sha(source)==original['input_sha256'];row=dict(original)
  module=re.search(rb'MODULE (\w+)',source).group(1).decode()
  with tempfile.TemporaryDirectory(prefix='dive-rewrite-reference-') as tmp:
   work=Path(tmp);file=work/(module+'.tla');file.write_bytes(source)
   capture,stdout=run(prefix+['pcal.trans','-nocfg',file.name],work,out,row['id']+'.translation');row['primary']=capture
   row['primary']['status']='translated' if capture['exit_code']==0 and b'Translation completed.' in stdout else 'rejected'
   if row['primary']['status']=='translated':
    data=file.read_bytes();f=row['id']+'.translated.tla';(out/f).write_bytes(data);row['translated']={'path':f,'sha256':sha(data)}
    old=work/(module+'.old');assert old.read_bytes()==source;row['backup_matches_input']=True
    val,vstdout=run([str(args.java),'-Xmx256m','-cp',str(args.jar)+':'+str(args.adapter),'pcal.ChecksumProbe','validate',str(file)],work,out,row['id']+'.validator');row['validator_reference_adapter']=val
    sany,sstdout=run(prefix+['tla2sany.SANY','-error-codes',file.name],work,out,row['id']+'.sany');row['sany']=sany
   else:row['unchanged_on_failure']=file.read_bytes()==source
   file.write_bytes(source);target=work/'callback-output.tla'
   cb,cstdout=run([str(args.java),'-Xmx256m','-cp',str(args.jar)+':'+str(args.adapter),'pcal.ChecksumProbe',row['mode'],str(file),str(target)],work,out,row['id']+'.callback');row['callback_reference_adapter']=cb
   row['callback_reference_adapter']['status']='translated' if b'OUTCOME\ttranslated' in cstdout else 'rejected'
   if target.exists():
    data=target.read_bytes();f=row['id']+'.callback.tla';(out/f).write_bytes(data);row['callback_output']={'path':f,'sha256':sha(data)}
  rows.append(row)
  if len(rows)%20==0:print('Captured',len(rows),flush=True)
 manifest={'schema_version':1,'oracle_id':'tlc-2026.10.06.014338','oracle_sha256':JAR,'capture_script_sha256':sha(Path(__file__).read_bytes()),'selection_sha256':sha((BASE/'selection.json').read_bytes()),'adapter_source_sha256':sha((BASE/'ChecksumProbe.java').read_bytes()),'java_sha256':sha(args.java.read_bytes()),'model_proved':False,'rows':rows}
 (out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');print('Captured all',len(rows),'primary rows')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for name in ['java','jar','adapter']:p.add_argument('--'+name,type=Path,required=True)
 capture(p.parse_args())
