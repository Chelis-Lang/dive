#!/usr/bin/env python3
"""Preserve all native process checksum refusals before the implementation changes."""
import argparse,collections,gzip,hashlib,json,re,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--native',type=Path,required=True);a=p.parse_args();out=BASE/'baseline';out.mkdir();rows=[]
for folder in ['primary','corrected-c','corrected-modifiers']:
 directory=BASE/folder;manifest=json.loads((directory/'manifest.json').read_text())
 for selected in manifest['rows']:
  source=(directory/(selected['id']+'.tla')).read_bytes();module=re.search(rb'MODULE (\w+)',source).group(1).decode();ident=folder+'__'+selected['id']
  with tempfile.TemporaryDirectory(prefix='dive-process-crc-baseline-') as tmp:
   file=Path(tmp)/(module+'.tla');file.write_bytes(source)
   for profile in ['dive','tlc']:
    for policy in ['full','tla']:
     q=subprocess.run([str(a.native),'translate',str(file),'--rewrite','--checksums',policy,'--output','json']+(['--compat=tlc'] if profile=='tlc' else []),capture_output=True,timeout=30);doc=json.loads(q.stdout)
     assert file.read_bytes()==source and doc['schema_version']==1 and doc['exit_code']==q.returncode and doc['model_proved'] is False and doc['exhaustive'] is False and q.returncode in [0,1,3]
     if policy=='full':assert q.returncode in [1,3] and doc['accepted'] is False
     for stream in ['stdout','stderr']:(out/(ident+'.'+profile+'.'+policy+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
     rows.append({'id':ident,'profile':profile,'policy':policy,'exit_code':q.returncode,'result':doc['result'],'stdout_sha256':hashlib.sha256(q.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(q.stderr).hexdigest()})
  print('Accounted',ident,flush=True)
report={'schema_version':1,'scope':'Unchanged process Full refusal dispositions; BodyOnly generation is a separate stage, not Full or family credit.','family_certified':False,'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'native_sha256':hashlib.sha256(a.native.read_bytes()).hexdigest(),'controls':len(rows),'selected':len(rows)//4,'summary':dict(collections.Counter((r['policy']+':'+r['result']) for r in rows)),'rows':rows};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(report['summary'])
