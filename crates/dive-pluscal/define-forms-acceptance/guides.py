"""Actual Markdown C/P sources through compiled CLI and Cargo-owned Rust APIs."""
from pathlib import Path
import subprocess,json,gzip,hashlib,re,tempfile
import argparse
p=argparse.ArgumentParser()
for key in ['base','binaries','out']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();G=a.base;B=a.binaries;O=a.out;O.mkdir(exist_ok=False);rows=[]
def sha(b):return hashlib.sha256(b).hexdigest()
def run(argv,name):
 q=subprocess.run(list(map(str,argv)),capture_output=True,timeout=30)
 for s in ['stdout','stderr']:(O/(name+'.'+s+'.gz')).write_bytes(gzip.compress(getattr(q,s),mtime=0))
 return q
manifest=json.loads((G/'primary/manifest.json').read_text())
for r in manifest['rows']:
 original_source=G/'cases'/(r['id']+'.tla');primary=(G/'primary'/(r['id']+'.translated.ast')).read_text();module=re.search(rb'MODULE\s+(\w+)',original_source.read_bytes()).group(1).decode();source=O/(module+'.tla');source.write_bytes(original_source.read_bytes())
 for profile in ['dive','tlc']:
  artifacts=[]
  for mode in ['cli','rust']:
   n=r['id']+'.'+profile+'.'+mode
   argv=[B/'dive','translate',source,'--rewrite','--output','json',*(['--compat=tlc']if profile=='tlc'else[])]if mode=='cli'else[B/'define-guide',source,*(['tlc']if profile=='tlc'else[])]
   q=run(argv,n);doc=json.loads(q.stdout);assert q.returncode==0 and doc['accepted']is True and doc['result']=='replacement'and doc['model_proved']is False and doc['exhaustive']is False and doc['algorithm_checksum_verified']is True and doc['body_checksum_verified']is True
   ck=doc['replacement']['algorithm_checksum'];assert ck['serialized_ast']==primary and ck['checksum']==r['header'][0];artifacts.append(doc['replacement']['module_source']);rows.append({'id':n,'outcome':'admitted-no-model-proof','profile':profile,'mode':mode,'source_sha256':sha(source.read_bytes())})
  assert artifacts[0]==artifacts[1]
 q=run([B/'checksum',source],r['id']+'.raw');doc=json.loads(q.stdout);assert q.returncode==0 and doc['model_proved']is False and doc['artifact_admitted']is False and doc['family_certified']is False and doc['checksum']['Ok']['serialized_ast']==primary;rows.append({'id':r['id']+'.raw','outcome':'exact-raw-no-admission'})
report={'schema_version':1,'selected':len(rows),'script_sha256':sha(Path(__file__).read_bytes()),'binaries':{n:sha((B/n).read_bytes())for n in ['dive','define-guide','checksum']},'family_certified':False,'rows':rows};(O/'report.json').write_text(json.dumps(report,indent=2)+'\n');print('Executed',len(rows),'actual guide controls')
