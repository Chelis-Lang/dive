#!/usr/bin/env python3
"""Separate pinned SANY/body-checksum checks on every admitted native document."""
import argparse,gzip,hashlib,json,re,subprocess,tempfile,zlib
from pathlib import Path
BASE=Path(__file__).resolve().parent
JAR='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
def sha(data):return hashlib.sha256(data).hexdigest()
def main(args):
 assert sha(args.jar.read_bytes())==JAR
 comparison=json.loads((BASE/'comparison.json').read_text());rows=[];out=BASE/'artifact-validation';out.mkdir(exist_ok=True)
 for row in comparison['rows']:
  directory=BASE/row['campaign']/'native'
  for profile in ['dive','tlc']:
   entry=row[profile]
   if entry['outcome']!='native-admitted-partial-checksum':continue
   raw=gzip.decompress((directory/entry['capture']['stdout']).read_bytes());assert sha(raw)==entry['capture']['stdout_sha256'];doc=json.loads(raw);source=doc['replacement']['module_source'];module=re.search(r'MODULE (\w+)',source).group(1)
   with tempfile.TemporaryDirectory(prefix='dive-rewrite-admission-') as tmp:
    file=Path(tmp)/(module+'.tla');file.write_text(source)
    commands={'sany':[str(args.java),'-cp',str(args.jar),'tla2sany.SANY','-error-codes',str(file)],'validator':[str(args.java),'-cp',str(args.jar)+':'+str(args.adapter),'pcal.ChecksumProbe','validate',str(file)]}
    controls={}
    for operation,command in commands.items():
     p=subprocess.run(command,cwd=tmp,capture_output=True,timeout=30);assert p.returncode==0,(row['id'],profile,operation,p.stdout,p.stderr)
     if operation=='validator':assert b'VALIDATION\t[NO_DIVERGENCE]' in p.stdout,(row['id'],p.stdout)
     captured={'exit_code':p.returncode}
     for stream in ['stdout','stderr']:
      data=getattr(p,stream);name=row['id']+'.'+profile+'.'+operation+'.'+stream+'.gz';(out/name).write_bytes(gzip.compress(data,mtime=0));captured[stream]={'path':name,'sha256':sha(data)}
     controls[operation]=captured
    # Changing body bytes without changing the header must be detected. The
    # algorithm remains deliberately unchecked in this partial header policy.
    if row['id'] in ['P_fresh','Corrected_C_fresh']:
     mutant=source.replace("x' = 1","x' = 2");assert mutant!=source;file.write_text(mutant)
     p=subprocess.run(commands['validator'],capture_output=True,timeout=30);assert p.returncode==0 and b'TLA_DIVERGENCE' in p.stdout
     name=row['id']+'.'+profile+'.body-mutant.stdout.gz';(out/name).write_bytes(gzip.compress(p.stdout,mtime=0));controls['body-mutant']={'outcome':'TLA_DIVERGENCE','source_sha256':sha(mutant.encode()),'stdout':{'path':name,'sha256':sha(p.stdout)}}
   rows.append({'id':row['id'],'campaign':row['campaign'],'profile':profile,'native_document_sha256':sha(raw),'module_sha256':sha(source.encode()),'controls':controls,'artifact_admitted':True,'model_proved':False,'algorithm_checksum_verified':False})
   if len(rows)%40==0:print('Validated',len(rows),'native artifacts',flush=True)
 report={'schema_version':1,'script_sha256':sha(Path(__file__).read_bytes()),'comparison_sha256':sha((BASE/'comparison.json').read_bytes()),'oracle_sha256':JAR,'adapter_sha256':sha((BASE/'ChecksumProbe.java').read_bytes()),'selected':len(rows),'family_certified':False,'rows':rows}
 (out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Validated all',len(rows),'native artifacts separately through pinned SANY and Validator')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for n in ['java','jar','adapter']:p.add_argument('--'+n,type=Path,required=True)
 main(p.parse_args())
