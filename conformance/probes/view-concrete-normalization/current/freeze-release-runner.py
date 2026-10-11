from pathlib import Path
import subprocess,json,hashlib,shutil,time
r=Path('/Users/jeff/code/dive-view-normalization-work');a=Path('/Users/jeff/code/dive-view-normalization-artifacts');p=json.loads((a/'current-gate/producer.json').read_text());sha=lambda b:hashlib.sha256(b).hexdigest();assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=r,text=True).strip()==p['source_commit'];assert not subprocess.check_output(['git','status','--porcelain'],cwd=r,text=True)
for f,h in p['compiled_source_sha256'].items():
 assert sha((r/f).read_bytes())==h,f;assert sha(subprocess.check_output(['git','show',p['source_commit']+':'+f],cwd=r))==h,f
b=a/'current-binaries';b.mkdir(exist_ok=False);ident={}
for n in ['dive','dive-conformance']:
 q=Path('/Users/jeff/code/dive-view-random-stream-target/release')/n;shutil.copyfile(q,b/n);(b/n).chmod(0o755);ident[n]=sha((b/n).read_bytes())
out=dict(p);out['binaries_sha256']=ident;out['build_profile']='release';out['build_jobs']=2;out['build_command']=['cargo','build','--release','--locked','-p','dive-cli','-p','dive-conformance'];out['build_log_sha256']=sha((a/'current-release-build.log').read_bytes());out['freeze_runner_sha256']=sha(Path(__file__).read_bytes());out['source_readback']='Every compiled source Git blob, live byte hash and full gate map compared before copying executables';(b/'producer.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(ident))
