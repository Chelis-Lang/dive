#!/usr/bin/env python3
"""Native source option failures remain selected, then refuse every public surface."""
import argparse,gzip,hashlib,json,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
p=argparse.ArgumentParser();p.add_argument('--native',type=Path,required=True);p.add_argument('--checksum',type=Path,required=True);a=p.parse_args();out=BASE/'source-guard';out.mkdir();rows=[]
for name in ['weak','strong','labels','root']:
 ident='SourceOptions_'+name;source=(BASE/'validator-context'/(ident+'.tla')).read_bytes()
 with tempfile.TemporaryDirectory(prefix='dive-source-options-guard-') as tmp:
  file=Path(tmp)/(ident+'.tla');file.write_bytes(source)
  for profile in ['dive','tlc']:
   for surface in ['translate','full-rewrite','body-rewrite','raw-checksum']:
    cmd=[str(a.native),'translate',str(file),'--output','json'] if surface!='raw-checksum' else [str(a.checksum)]
    if surface in ['full-rewrite','body-rewrite']:cmd+=['--rewrite','--checksums','full' if surface=='full-rewrite' else 'tla']
    if profile=='tlc' and surface!='raw-checksum':cmd+=['--compat=tlc']
    q=subprocess.run(cmd,input=json.dumps({'source':source.decode()}).encode() if surface=='raw-checksum' else None,capture_output=True,timeout=30);d=json.loads(q.stdout)
    assert d['accepted'] is False and d['model_proved'] is False and file.read_bytes()==source
    if surface=='raw-checksum':assert q.returncode==0 and d['error']['diagnostic']['code']=='Unsupported' and 'checksum' not in d
    else:
     assert q.returncode==3 and d['result']=='unsupported' and d['exhaustive'] is False and 'replacement' not in d and 'artifact' not in d
     cause=d.get('generation',d);assert cause['diagnostic']['code']=='Unsupported' and 'source option' in cause['diagnostic']['message']
    for stream in ['stdout','stderr']:(out/(ident+'.'+profile+'.'+surface+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
    rows.append({'id':ident,'profile':profile,'surface':surface,'outcome':'named-source-option-refusal','stdout_sha256':hashlib.sha256(q.stdout).hexdigest()})
report={'schema_version':1,'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'native_sha256':hashlib.sha256(a.native.read_bytes()).hexdigest(),'checksum_sha256':hashlib.sha256(a.checksum.read_bytes()).hexdigest(),'controls':len(rows),'family_certified':False,'rows':rows};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Passed',len(rows),'source guard controls')
