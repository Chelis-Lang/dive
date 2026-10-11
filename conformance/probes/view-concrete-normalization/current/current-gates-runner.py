from pathlib import Path
import subprocess,hashlib,json,re,time
r=Path('/Users/jeff/code/dive-view-normalization-work');a=Path('/Users/jeff/code/dive-view-normalization-artifacts/current-conformance');a.mkdir(exist_ok=False);b=a.parent/'current-binaries';sha=lambda v:hashlib.sha256(v).hexdigest();producer=json.loads((b/'producer.json').read_text());rows=[]
for corpus,name,expected in [('examples','cm1',21),('validation-smoke','validation-smoke',16)]:
 p=a/name;p.mkdir();f=r/f'conformance/gates/{name}.txt';filters=[s.split('#')[0].strip()for s in f.read_text().splitlines()];filters=[s for s in filters if s];assert len(filters)==expected
 cmd=[str(b/'dive-conformance'),'run','--impl','dive','--impl-profile','tlc','--corpus',corpus,'--tier','all','--no-sany','--no-summary','--report-dir',str(p/'reports'),'--dive-bin',str(b/'dive'),'--workers','1,4,8']
 for s in filters:cmd.extend(['--filter',s])
 import os
 env=os.environ.copy();env['DIVE_CACHE_DIR']='/Users/jeff/code/dive-view-random-stream-cache';start=time.time();v=subprocess.run(cmd,cwd=r,env=env,capture_output=True,timeout=1200);(p/'stdout').write_bytes(v.stdout);(p/'stderr').write_bytes(v.stderr);lines=re.findall(rb'^\[\d+/\d+\] (\S+) ',v.stdout+v.stderr,re.M);assert v.returncode==0 and len(lines)==expected and set(lines)=={b'agree'},(v.returncode,lines,v.stderr[-1000:]);rows.append(dict(corpus=corpus,selection_sha256=sha(f.read_bytes()),expected_models=expected,workers=[1,4,8],exit=v.returncode,elapsed_seconds=time.time()-start,files_sha256={str(q.relative_to(p)):sha(q.read_bytes())for q in p.rglob('*')if q.is_file()}));print(name,expected,'agree',flush=True)
(a/'receipt.json').write_text(json.dumps(dict(runtime_producer=producer,runner_sha256=sha(Path(__file__).read_bytes()),rows=rows),indent=2)+'\n')
