#!/usr/bin/env python3
"""Run the entire ancestor workspace gate with two compile/test threads."""
import hashlib,json,os,subprocess,time
from pathlib import Path
ROOT=Path.cwd();BASE=Path(__file__).resolve().parent
assert ROOT.joinpath('Cargo.toml').exists()
env=dict(os.environ,CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_TARGET_DIR='/Users/jeff/code/dive-pluscal-process-option-checksum-target',DIVE_CACHE_DIR='/Users/jeff/code/dive-pluscal-process-option-checksum-cache')
steps=[('fmt',['cargo','fmt','--all','--check']),('validation-python',['python3','scripts/test-validation-import.py']),('frontend-python',['python3','-I','scripts/test-frontend-campaign.py']),('clippy',['cargo','clippy','--workspace','--all-targets','--locked','--','-D','warnings']),('nextest',['cargo','nextest','run','--workspace','--locked','--test-threads','2']),('doctests',['cargo','test','--workspace','--doc','--locked']),('rustdoc',['cargo','doc','--workspace','--no-deps','--locked']),('layering',['cargo','xtask','check-layering']),('trace-standalone',['cargo','build','-p','dive-trace','--locked']),('deny',['cargo','deny','--locked','check'])]
rows=[]
for name,args in steps:
 print('Running',name,flush=True);start=time.monotonic();chosen=dict(env)
 if name=='rustdoc':chosen['RUSTDOCFLAGS']='-D warnings'
 with (BASE/(name+'.log')).open('wb') as log:p=subprocess.run(args,cwd=ROOT,env=chosen,stdout=log,stderr=subprocess.STDOUT)
 rows.append({'stage':name,'exit_code':p.returncode,'seconds':time.monotonic()-start,'log_sha256':hashlib.sha256((BASE/(name+'.log')).read_bytes()).hexdigest()})
 (BASE/'report.json').write_text(json.dumps({'schema_version':1,'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'compile_jobs':2,'test_threads':2,'status':'passed' if len(rows)==len(steps) and all(r['exit_code']==0 for r in rows) else 'incomplete-or-failed','rows':rows},indent=2)+'\n')
 if p.returncode:raise SystemExit(p.returncode)
print('Full workspace gate passed',flush=True)
