from pathlib import Path
import os,subprocess,json,hashlib,time,shutil
ROOT=Path('/Users/jeff/code/dive-trace-representation-work');ART=Path('/Users/jeff/code/dive-trace-representation-artifacts');TARGET=Path('/Users/jeff/code/dive-trace-representation-target');BIN=ART/'current-binaries'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
producer=json.loads((BIN/'producer.json').read_text());assert producer['source_commit']==subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
out=ART/'current-guide-gates';out.mkdir(exist_ok=False)
env={**os.environ,'CARGO_BUILD_JOBS':'2','CARGO_TARGET_DIR':str(TARGET),'DIVE_CACHE_DIR':'/Users/jeff/code/dive-trace-representation-cache','HARNESS':str(BIN/'dive-conformance')}
rows=[]
commands=[('rust-guide',['cargo','run','--locked','-p','dive-api','--example','trace_representation'],{}),('cm1',['bash','scripts/conformance-gate.sh','conformance/gates/cm1.txt','--workers','1,4,8','--dive-bin',str(BIN/'dive')],{}),('validation-smoke',['bash','scripts/conformance-gate.sh','conformance/gates/validation-smoke.txt','--workers','1,4,8','--dive-bin',str(BIN/'dive')],{'DIVE_GATE_CORPUS':'validation-smoke'})]
for label,cmd,extra in commands:
 t=time.monotonic()
 with (out/(label+'.stdout')).open('wb')as f,(out/(label+'.stderr')).open('wb')as e:r=subprocess.run(cmd,cwd=ROOT,env={**env,**extra},stdout=f,stderr=e)
 row={'name':label,'command':cmd,'exit':r.returncode,'seconds':time.monotonic()-t,'stdout_sha256':sha(out/(label+'.stdout')),'stderr_sha256':sha(out/(label+'.stderr'))};rows.append(row);(out/'receipt.json').write_text(json.dumps({'runtime_producer':producer,'rows':rows,'driver_sha256':sha(Path(__file__))},indent=2)+'\n');print(label,r.returncode,flush=True)
 if label=='rust-guide'and r.returncode==0:shutil.copy2(TARGET/'debug/examples/trace_representation',out/'trace_representation');row['compiled_example_sha256']=sha(out/'trace_representation')
assert all(r['exit']==0 for r in rows),rows
(out/'receipt.json').write_text(json.dumps({'runtime_producer':producer,'rows':rows,'driver_sha256':sha(Path(__file__))},indent=2)+'\n');print('CLOSEDguide +21/16 models1/4/8',flush=True)
