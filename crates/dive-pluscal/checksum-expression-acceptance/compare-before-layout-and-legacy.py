#!/usr/bin/env python3
"""Compare native serialization with each retained reference post-translation AST."""
import argparse,collections,gzip,hashlib,json,subprocess,zlib
from pathlib import Path
BASE=Path(__file__).resolve().parent
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def check(doc,row,base):
 assert doc.get('schema_version')==1 and doc.get('model_proved') is False and doc.get('artifact_admitted') is False and isinstance(doc.get('accepted'),bool)
 if not doc['accepted']:
  assert doc['result']=='refused';code=doc['error']['diagnostic']['code'];assert code in ['Lexical','Syntax','Expression','Label','Resource','Unsupported'];assert code in ['Resource','Unsupported'] or not row['primary_translated'];return 'named-refusal' if code in ['Resource','Unsupported'] else 'rejection-accounted'
 assert doc['result']=='checksum' and row['primary_translated']
 stage=row['stages']['translated'];expected=(base/(row['id']+'.translated.ast')).read_text();checksum=doc['checksum'];assert checksum['serialized_ast']==expected
 assert checksum['checksum']==stage['crc']==row['primary_crc'];assert checksum['fair_header_crc_only']==('_fair_' in row['id'])
 assert format(zlib.crc32(('fair' if checksum['fair_header_crc_only'] else expected).encode()),'x')==checksum['checksum'];return 'exact-post-translation-serialization'
def main(a):
 out=BASE/a.name;out.mkdir();rows=[];manifests={}
 selected=[]
 for folder in ['primary','c-fixed-controls']:
  directory=BASE/folder;manifest=json.loads((directory/'manifest.json').read_text());manifests[folder]=sha(directory/'manifest.json');selected.extend((directory,row) for row in manifest['rows'])
 for directory,row in selected:
  source=directory/(row['id']+'.tla');assert sha(source)==row['source_sha256']
  p=subprocess.run([str(a.binary)],input=json.dumps({'source':source.read_text()}).encode(),capture_output=True,timeout=30)
  for name,data in [('stdout',p.stdout),('stderr',p.stderr)]:(out/(row['id']+'.'+name+'.gz')).write_bytes(gzip.compress(data,mtime=0))
  try:assert p.returncode==0;outcome=check(json.loads(p.stdout),row,directory)
  except (AssertionError,KeyError,ValueError,TypeError) as err:outcome='disagree';print(row['id'],err,flush=True)
  rows.append({'id':row['id'],'outcome':outcome,'stdout_sha256':hashlib.sha256(p.stdout).hexdigest()})
 summary=dict(collections.Counter(r['outcome'] for r in rows));report={'schema_version':1,'binary_sha256':sha(a.binary),'script_sha256':sha(Path(__file__)),'manifests':manifests,'selected':len(rows),'family_certified':False,'artifact_admitted':False,'summary':summary,'rows':rows};(BASE/(a.name+'-comparison.json')).write_text(json.dumps(report,indent=2)+'\n');print(summary);return 1 if summary.get('disagree') else 0
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binary',type=Path,required=True);p.add_argument('--name',default='native');raise SystemExit(main(p.parse_args()))
