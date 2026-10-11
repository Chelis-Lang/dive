from pathlib import Path
import subprocess, json, hashlib, os, shutil, time
r=Path.cwd();b=r/'conformance/probes/committed-prefix/gate-current';before=json.loads((b/'runtime-before.json').read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert all(sha(r/p)==h for p,h in before['source_sha256'].items())
env=os.environ.copy();env.update(CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_TARGET_DIR='/Users/jeff/code/dive-periodic-snapshot-target',DIVE_CACHE_DIR='/Users/jeff/code/dive-periodic-snapshot-cache')
target=Path(env['CARGO_TARGET_DIR']);frozen=Path('/tmp/dive-committed-prefix-frozen-native');frozen.mkdir(exist_ok=True);steps=[]
def run(cmd,log):
 t=time.monotonic()
 with(b/log).open('w')as f:p=subprocess.run(cmd,cwd=r,env=env,stdout=f,stderr=subprocess.STDOUT)
 steps.append({'command':cmd,'log':log,'exit':p.returncode,'seconds':time.monotonic()-t});(b/'runtime-steps.json').write_text(json.dumps(steps,indent=2)+'\n');assert p.returncode==0,(cmd,p.returncode)
run(['cargo','build','--release','--locked','-p','dive-cli','-p','dive-conformance'],'binary-build.log')
run(['cargo','build','--release','--locked','-p','dive-api','--example','committed_prefix'],'cargo-example-build.log')
for name,source in [('dive',target/'release/dive'),('dive-conformance',target/'release/dive-conformance'),('committed-prefix-guide',target/'release/examples/committed_prefix')]:shutil.copy2(source,frozen/name)
bins={p.name:sha(p)for p in frozen.iterdir()if p.is_file()}
run([str(frozen/'committed-prefix-guide')],'cargo-guide.log')
assert (b/'cargo-guide.log').read_text().strip()=='generated=4234 distinct=65 safety_eof=true model_proof=false'
env.update(CARGO_PROFILE_TEST_DEBUG='0',DIVE_PERIODIC_SNAPSHOT_CAPTURE=str(r/'conformance/probes/committed-prefix/native-runtime-snapshot'),DIVE_PROGRESS_QUEUE_NATIVE_CAPTURE=str(r/'conformance/probes/committed-prefix/native-runtime'))
run(['cargo','test','--locked','-p','dive-liveness','--test','periodic_snapshot','--','--test-threads=2'],'native-controls.log')
assert '29 passed; 0 failed' in (b/'native-controls.log').read_text()
for test,label in [('configured_candidate_precedes_false_hook_and_replays_on_fresh_binding','old-timer'),('real_request_pause_retains_uncommitted_facts_and_existing_candidate_replay','new-request')]:
 env['DIVE_PERIODIC_SNAPSHOT_CAPTURE']=str(r/'conformance/probes/committed-prefix'/('replay-'+label))
 run(['cargo','test','--locked','-p','dive-liveness','--test','periodic_snapshot',test,'--','--exact','--test-threads=2'],'replay-'+label+'.log')
 assert '1 passed; 0 failed' in (b/('replay-'+label+'.log')).read_text()
for corpus,listname in [('examples','cm1'),('validation-smoke','validation-smoke')]:
 selected=[s.split('#')[0].strip()for s in (r/'conformance/gates'/f'{listname}.txt').read_text().splitlines()];selected=[s for s in selected if s];assert len(selected)==(21 if corpus=='examples'else 16)
 out=b/(corpus+'-reports');out.mkdir(exist_ok=True)
 cmd=[str(frozen/'dive-conformance'),'run','--impl','dive','--impl-profile','tlc','--dive-bin',str(frozen/'dive'),'--corpus',corpus,'--tier','all','--no-sany','--no-summary','--report-dir',str(out),'--workers','1,4,8']
 for name in selected:cmd+=['--filter',name]
 run(cmd,corpus+'-gate.log')
 lines=[s for s in(b/(corpus+'-gate.log')).read_text().splitlines()if s.startswith('[')and'/'in s];assert len(lines)==len(selected)and all('] agree 'in s for s in lines),lines
unchanged=all(sha(r/p)==h for p,h in before['source_sha256'].items())and all(sha(frozen/p)==h for p,h in bins.items());assert unchanged
(b/'runtime-closed.json').write_text(json.dumps({'production_commit':before['producer'],'source_sha256':before['source_sha256'],'binary_sha256':bins,'source_and_binaries_unchanged':unchanged,'steps':steps,'scope':'37 required safety-conformance models at1/4/8; actual committed returned contiguous prefixes and retained later outputs under real joined pause, separately recorded graph-before-frontier callbacks and successful physical append; no ordinary scheduling/verdict credit'},indent=2)+'\n')
print('Closed three immutable binaries, Cargo guide and37 required models all1/4/8')
