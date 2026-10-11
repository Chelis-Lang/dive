from pathlib import Path
import hashlib,subprocess,json,os,time
r=Path('/Users/jeff/code/dive-view-normalization-work');a=Path('/Users/jeff/code/dive-view-normalization-artifacts/final-metadata-verification');a.mkdir(exist_ok=False);sha=lambda b:hashlib.sha256(b).hexdigest();p=json.loads((r/'conformance/probes/view-concrete-normalization/current/runtime-producer.json').read_text());proof={}
for f,h in p['compiled_source_sha256'].items():
 assert sha((r/f).read_bytes())==h,f;assert sha(subprocess.check_output(['git','show',p['source_commit']+':'+f],cwd=r))==h,f;proof[f]=h
commands=[['cargo','fmt','--all','--check']]
for line in (r/'.github/workflows/ci.yml').read_text().splitlines():
 s=line.strip().removeprefix('run: ')
 if s.startswith('python3 '):commands.append(s.split())
env=os.environ.copy();env['CARGO_TARGET_DIR']='/Users/jeff/code/dive-view-random-stream-target';env['CARGO_BUILD_JOBS']='2';rows=[]
for i,cmd in enumerate(commands):
 t=time.time();v=subprocess.run(cmd,cwd=r,env=env,capture_output=True);(a/f'{i}.stdout').write_bytes(v.stdout);(a/f'{i}.stderr').write_bytes(v.stderr);rows.append(dict(command=cmd,exit=v.returncode,elapsed_seconds=time.time()-t));print(cmd,v.returncode,flush=True)
(a/'receipt.json').write_text(json.dumps(dict(runtime_source_commit=p['source_commit'],compiled_live_and_git_source_equality=proof,rows=rows,scope='Delivery docs/evidence/Python verification only; compiled060 source and full gate are unchanged'),indent=2)+'\n');assert all(row['exit']==0 for row in rows)
