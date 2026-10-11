#!/usr/bin/env python3
"""Retain the unchanged emitter's complete current refusal campaign."""
import argparse,gzip,hashlib,json,subprocess,tempfile,re
from pathlib import Path
BASE=Path(__file__).resolve().parent
p=argparse.ArgumentParser();p.add_argument('--native',type=Path,required=True);a=p.parse_args();out=BASE/'baseline';out.mkdir();rows=[]
for folder in ['primary','layout-controls']:
 directory=BASE/folder;m=json.loads((directory/'manifest.json').read_text())
 for row in m['rows']:
  source=(directory/(row['id']+'.tla')).read_bytes();module=re.search(rb'MODULE (\w+)',source)[1].decode()
  with tempfile.TemporaryDirectory(prefix='dive-multiline-baseline-') as tmp:
   file=Path(tmp)/(module+'.tla');file.write_bytes(source)
   for profile in ['dive','tlc']:
    for policy in ['full','tla']:
     q=subprocess.run([str(a.native),'translate',str(file),'--rewrite','--checksums',policy,'--output','json']+(['--compat=tlc'] if profile=='tlc' else []),capture_output=True,timeout=30);doc=json.loads(q.stdout);assert doc['accepted'] is False and doc['model_proved'] is False and doc['exhaustive'] is False and file.read_bytes()==source
     for stream in ['stdout','stderr']:(out/(row['id']+'.'+profile+'.'+policy+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
     rows.append({'id':row['id'],'source_sha256':hashlib.sha256(source).hexdigest(),'profile':profile,'policy':policy,'exit_code':q.returncode,'document':doc})
report={'schema_version':1,'native_sha256':hashlib.sha256(a.native.read_bytes()).hexdigest(),'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'selected_models':len(rows)//4,'selected_runs':len(rows),'family_certified':False,'rows':rows}
(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Preserved',len(rows),'baseline rejected/refused runs')
