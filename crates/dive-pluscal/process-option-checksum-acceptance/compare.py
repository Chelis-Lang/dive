#!/usr/bin/env python3
"""Raw option AST equality is independent from artifact admission and proof."""
import argparse,collections,gzip,hashlib,json,subprocess,zlib
from pathlib import Path
BASE=Path(__file__).resolve().parent
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def options(flags):
 d={'parser':{'max_bytes':1048576,'max_tokens':100000,'max_nodes':50000,'max_depth':128},'insert_missing_labels':any(f in flags for f in ['-label','-reportLabels']),'label_root':flags[flags.index('-labelRoot')+1] if '-labelRoot' in flags else 'Lbl_','fairness':'Declared','max_actions':10000,'max_output_bytes':4194304}
 for f,v in [('-nof','None'),('-wf','Weak'),('-sf','Strong'),('-wfNext','Next')]:
  if f in flags:d['fairness']=v
 return d

def check(doc,row,base):
 assert type(doc.get('schema_version')) is int and doc['schema_version']==1
 assert doc.get('model_proved') is False and doc.get('artifact_admitted') is False and type(doc.get('accepted')) is bool
 if not doc['accepted']:
  assert doc['result']=='refused';code=doc['error']['diagnostic']['code'];assert code in ['Lexical','Syntax','Expression','Label','Resource','Unsupported']
  assert code in ['Unsupported','Resource'] or not row['primary_translated']
  return 'named-refusal' if code in ['Unsupported','Resource'] else 'rejection-accounted'
 assert row['primary_translated'] and doc['result']=='checksum'
 expected=(base/(row['id']+'.translated.ast')).read_bytes().decode();actual=doc['checksum']
 assert actual['serialized_ast']==expected, 'serialized AST bytes differ'
 assert actual['checksum']==row['primary_crc']==row['stages']['translated']['crc']
 assert actual['fair_header_crc_only']==('_fair_' in row['id'])
 assert format(zlib.crc32(('fair' if actual['fair_header_crc_only'] else expected).encode()),'x')==actual['checksum']
 return 'exact-post-translation-serialization'
def main(a):
 out=BASE/a.name;out.mkdir();manifests={};selected=[];rows=[]
 for folder in ['primary','context','label-options']:
  manifest=BASE/folder/'manifest.json';m=json.loads(manifest.read_text());manifests[folder]=sha(manifest);selected.extend((BASE/folder,row) for row in m['rows'])
 for base,row in selected:
  # Invalid simultaneous CLI choices have no typed API counterpart. Retain the
  # clean rejection; do not silently choose one or credit native matching.
  if row['flags'].count('-wf')+row['flags'].count('-sf')>1:
   assert not row['primary_translated'];rows.append({'id':row['id'],'outcome':'reference-invalid-options-no-api-counterpart'});continue
  source=base/(row['id']+'.tla');assert sha(source)==row['source_sha256']
  p=subprocess.run([str(a.binary)],input=json.dumps({'source':source.read_bytes().decode(),'options':options(row['flags'])}).encode(),capture_output=True,timeout=30)
  for stream in ['stdout','stderr']:(out/(row['id']+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(p,stream),mtime=0))
  try:assert p.returncode==0;outcome=check(json.loads(p.stdout),row,base)
  except (AssertionError,KeyError,TypeError,ValueError) as error:outcome='disagree';print(row['id'],error,flush=True)
  rows.append({'id':row['id'],'outcome':outcome,'stdout_sha256':hashlib.sha256(p.stdout).hexdigest()})
 summary=dict(collections.Counter(r['outcome'] for r in rows));report={'schema_version':1,'binary_sha256':sha(a.binary),'script_sha256':sha(Path(__file__)),'manifests':manifests,'selected':len(rows),'family_certified':False,'artifact_admitted':False,'summary':summary,'rows':rows};(out/'report.json').write_text(json.dumps(report,indent=2)+'\n');print(summary);return 1 if summary.get('disagree') else 0
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binary',type=Path,required=True);p.add_argument('--name',required=True);raise SystemExit(main(p.parse_args()))
