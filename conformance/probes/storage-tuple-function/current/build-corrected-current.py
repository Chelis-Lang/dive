from pathlib import Path
import os
REQUIRED_ENV = {'CARGO_TARGET_DIR':'/Users/jeff/code/dive-trace-representation-target', 'CARGO_BUILD_JOBS':'2', 'DIVE_CACHE_DIR':'/Users/jeff/code/dive-trace-representation-cache'}
for name, value in REQUIRED_ENV.items():
    assert os.environ.get(name) == value, (name, os.environ.get(name), value)
import subprocess,os,json,hashlib,shutil,time
ROOT=Path('/Users/jeff/code/dive-trace-representation-work')
ART=Path('/Users/jeff/code/dive-storage-tuple-function-artifacts')
TARGET=Path('/Users/jeff/code/dive-trace-representation-target')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
gate=json.loads((ART/'corrected-full-gate/index.json').read_text())
assert len(gate['stages'])==8 and all(c['exit']==0 for s in gate['stages'] for c in s['commands'])
commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip();assert commit==gate['producer_commit']
assert not subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True).strip()
source=json.loads((ART/'corrected-full-gate/source/index.json').read_text())
for n,row in source['inputs'].items():assert sha(ROOT/n)==row['sha256'],n
cmd=['cargo','build','--release','--locked','-p','dive-cli','-p','dive-conformance']
start=time.monotonic()
with (ART/'current-release-build.log').open('wb')as f:
 r=subprocess.run(cmd,cwd=ROOT,env={**os.environ,'CARGO_TARGET_DIR':str(TARGET),'CARGO_BUILD_JOBS':'2'},stdout=f,stderr=subprocess.STDOUT)
assert r.returncode==0,r.returncode
out=ART/'current-binaries';out.mkdir(exist_ok=False)
for name in ['dive','dive-conformance']:shutil.copy2(TARGET/'release'/name,out/name);assert sha(out/name)==sha(TARGET/'release'/name)
for n,row in source['inputs'].items():assert sha(ROOT/n)==row['sha256'],n
j={'source_commit':commit,'producer_commit':commit,'source_index_sha256':sha(ART/'corrected-full-gate/source/index.json'),'binaries_sha256':{n:sha(out/n)for n in ['dive','dive-conformance']},'build':{'command':cmd,'exit':r.returncode,'jobs':2,'seconds':time.monotonic()-start,'log_sha256':sha(ART/'current-release-build.log')}}
(out/'producer.json').write_text(json.dumps(j,indent=2)+'\n');print(json.dumps(j),flush=True)
