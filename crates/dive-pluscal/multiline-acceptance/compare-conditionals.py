#!/usr/bin/env python3
"""Preserve every conditional-layout artifact and significant-CST disagreement."""
import argparse,collections,gzip,hashlib,json,re,subprocess,tempfile
from pathlib import Path
BASE=Path(__file__).resolve().parent
p=argparse.ArgumentParser();p.add_argument('--native',type=Path,required=True);p.add_argument('--cst',type=Path,required=True);p.add_argument('--name',required=True);a=p.parse_args();directory=BASE/'conditional-controls';m=json.loads((directory/'manifest.json').read_text());out=BASE/a.name;out.mkdir();rows=[]
for row in m['rows']:
 ident=row['id'];source=(directory/(ident+'.tla')).read_bytes();module=re.search(rb'MODULE (\w+)',source)[1].decode()
 with tempfile.TemporaryDirectory() as tmp:
  file=Path(tmp)/(module+'.tla');file.write_bytes(source);q=subprocess.run([str(a.native),'translate',str(file),'--rewrite','--compat=tlc','--output','json'],capture_output=True,timeout=30);doc=json.loads(q.stdout)
  for stream in ['stdout','stderr']:(out/(ident+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
  if q.returncode==0:
   assert doc['accepted'] is True and doc['model_proved'] is False and doc['exhaustive'] is False
   trees=[]
   for tag,data in [('primary',(directory/(ident+'.translated.tla')).read_bytes()),('native',doc['replacement']['module_source'].encode())]:
    t=subprocess.run([str(a.cst),'--cst'],input=data,capture_output=True,timeout=30);tree=json.loads(t.stdout);trees.append(tree);(out/(ident+'.'+tag+'.cst.json.gz')).write_bytes(gzip.compress(t.stdout,mtime=0))
   outcome='operator-tree-match' if trees[0]['accepted'] and trees[1]['accepted'] and trees[0]['structure']==trees[1]['structure'] else 'operator-tree-disagreement'
  else:outcome='named-refusal' if q.returncode==3 else 'rejected'
  rows.append({'id':ident,'outcome':outcome,'exit_code':q.returncode,'source_sha256':hashlib.sha256(source).hexdigest()});print(ident,outcome,flush=True)
summary=dict(collections.Counter(r['outcome'] for r in rows));report={'schema_version':1,'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'native_sha256':hashlib.sha256(a.native.read_bytes()).hexdigest(),'cst_sha256':hashlib.sha256(a.cst.read_bytes()).hexdigest(),'selection_sha256':hashlib.sha256((directory/'manifest.json').read_bytes()).hexdigest(),'selected':len(rows),'family_certified':False,'summary':summary,'rows':rows};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(summary)
raise SystemExit(1 if summary.get('operator-tree-disagreement') else 0)
