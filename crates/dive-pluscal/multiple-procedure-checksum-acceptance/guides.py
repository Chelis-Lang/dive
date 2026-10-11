"""Actual C/P Markdown cases through CLI and compiled Cargo-owned APIs."""
from pathlib import Path
import argparse,hashlib,json,gzip,subprocess
p=argparse.ArgumentParser()
for key in ['base','binaries','out','worktree','target']:p.add_argument('--'+key,type=Path,required=True)
a=p.parse_args();a.out.mkdir(exist_ok=False);rows=[]
sha=lambda b:hashlib.sha256(b).hexdigest()
for row in json.loads((a.base/'guide/primary/manifest.json').read_text())['rows']:
 source=a.base/'guide/cases'/(row['id']+'.tla')
 expected=(a.base/'guide/primary'/(row['id']+'.translated.ast')).read_text()
 for profile in ['dive','tlc']:
  for mode in ['cli-full','rust-full','cli-body','rust-body','cargo-full']:
   name=row['id']+'.'+profile+'.'+mode;flags=['--compat=tlc']if profile=='tlc'else[]
   if mode.startswith('cli'):command=[a.binaries/'dive','translate',source,'--rewrite','--output','json',*(['--checksums','tla']if mode=='cli-body'else[]),*flags]
   elif mode=='cargo-full':command=['cargo','run','--locked','--release','-p','dive-api','--example','pluscal_multiple_procedure_checksum','--',source,*flags]
   else:command=[a.binaries/'multiple-guide',source,*(['--body-only']if mode=='rust-body'else[]),*flags]
   import os
   env=os.environ.copy();env.update(CARGO_BUILD_JOBS='2',CARGO_TARGET_DIR=str(a.target))
   q=subprocess.run(list(map(str,command)),cwd=a.worktree,capture_output=True,env=env,timeout=120)
   for stream in ['stdout','stderr']:(a.out/(name+'.'+stream+'.gz')).write_bytes(gzip.compress(getattr(q,stream),mtime=0))
   doc=json.loads(q.stdout)
   assert q.returncode==0 and doc['accepted']is True and doc['result']=='replacement'and doc['model_proved']is False and doc['exhaustive']is False and doc['profile']==('tlc+overrides'if profile=='tlc'and mode.endswith('body')else profile)
   if mode.endswith('body'):assert doc['algorithm_checksum_verified']is False and doc['body_checksum_verified']is True
   else:assert doc['algorithm_checksum_verified']is True and doc['body_checksum_verified']is True and doc['replacement']['algorithm_checksum']['serialized_ast']==expected and doc['replacement']['algorithm_checksum']['checksum']==row['header'][0]
   rows.append({'id':name,'outcome':'admitted-no-model-proof','stdout_sha256':sha(q.stdout),'stderr_sha256':sha(q.stderr),'source_sha256':sha(source.read_bytes())})
(a.out/'report.json').write_text(json.dumps({'schema_version':1,'selected':len(rows),'script_sha256':sha(Path(__file__).read_bytes()),'family_certified':False,'binaries_sha256':{p.name:sha(p.read_bytes())for p in a.binaries.iterdir()},'rows':rows},indent=2)+'\n');print('Executed',len(rows),'actual guide controls')
