from pathlib import Path
import os,json,subprocess,time,hashlib,shutil
r=Path.cwd();g=Path('/tmp/dive-terminal-classification-fullgate-retry1');b=g/'runtime-current';b.mkdir(exist_ok=False);before=json.loads((g/'before.json').read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert all(sha(r/p)==h for p,h in before['source_sha256'].items());assert json.loads((g/'after.json').read_text())['all_steps_passed']
env=os.environ.copy();env.update(CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_TARGET_DIR='/Users/jeff/code/dive-periodic-snapshot-target',DIVE_CACHE_DIR='/Users/jeff/code/dive-periodic-snapshot-cache')
t=Path(env['CARGO_TARGET_DIR']);frozen=Path('/tmp/dive-terminal-classification-frozen-native');frozen.mkdir(exist_ok=False);steps=[]
def run(cmd,log,override=None):
 start=time.monotonic()
 with(b/log).open('w')as f:p=subprocess.run(cmd,cwd=r,env=override or env,stdout=f,stderr=subprocess.STDOUT)
 steps.append(dict(command=cmd,log=log,exit=p.returncode,wall_seconds=time.monotonic()-start));(b/'runtime-steps.json').write_text(json.dumps(steps,indent=2)+'\n');assert p.returncode==0,(cmd,p.returncode)
for p in ['dive-api','dive-cli','dive-conformance','dive-trace']:
 run(['cargo','tree','--locked','-p',p,'--edges','normal,build,features'],p+'-default-feature-scope.log');assert 'test-rendezvous'not in(b/(p+'-default-feature-scope.log')).read_text()
run(['cargo','rustc','--release','--locked','-p','dive-check','--lib','--no-default-features','--','--print','cfg'],'default-check-cfg.log');assert 'feature="test-rendezvous"'not in(b/'default-check-cfg.log').read_text()
run(['cargo','build','--release','--locked','-p','dive-api','-p','dive-cli','-p','dive-conformance','--message-format=json-render-diagnostics'],'default-release-build.log')
artifacts={}
for line in(b/'default-release-build.log').read_text().splitlines():
 try:a=json.loads(line)
 except json.JSONDecodeError:continue
 if a.get('reason')=='compiler-artifact':
  assert 'test-rendezvous'not in a.get('features',[]),a
  lib=next((f for f in a['filenames']if f.endswith('.rlib')),None)
  if lib:artifacts[a['target']['name']]=lib
runner=Path('/tmp/dive-terminal-classification-runtime.rs');shutil.copy2(runner,b/'runtime-source.rs')
cmd=['rustc','--edition=2024',str(runner),'-L','dependency='+str(t/'release/deps'),'-o',str(frozen/'terminal-outcome-runtime')]
for name in ['dive_api','dive_check','dive_liveness','dive_sema','dive_ir','dive_config','dive_trace','serde_json']:cmd+=['--extern',name+'='+artifacts[name]]
run(cmd,'default-api-runtime-compile.log')
for name in ['dive','dive-conformance']:shutil.copy2(t/'release'/name,frozen/name)
# Actual default-library producer is closed before Cargo example/dev-feature compilation.
run([str(frozen/'terminal-outcome-runtime'),str(b/'native-current')],'default-api-runtime.log')
selection=json.loads(Path('/tmp/dive-terminal-classification-runtime-selection.json').read_text());shutil.copy2('/tmp/dive-terminal-classification-runtime-selection.json',b/'selection.json')
assert 'production_commit'not in selection
selection['production_commit']=before['producer'];(b/'selection.json').write_text(json.dumps(selection,indent=2)+'\n')
assert selection['runner_sha256']==sha(b/'runtime-source.rs')
rows=[]
for p in sorted((b/'native-current').glob('*.json')):
 x=json.loads(p.read_text());assert x['model_proof']is False and x['ordinary_schedule_certified']is False
 rows.append(dict(file=p.name,kind=x['kind'],sha256=sha(p)))
assert len(rows)==206
assert {x['file'].removesuffix('.json')for x in rows}=={x['id']for x in selection['rows']}
(b/'native-manifest.json').write_text(json.dumps(dict(production_commit=before['producer'],rows=rows,runner_source_sha256=sha(b/'runtime-source.rs'),compiler_extern_rlib_sha256={p:sha(Path(v))for p,v in artifacts.items()},model_proof=False,ordinary_schedule_certified=False),indent=2)+'\n')
run(['cargo','build','--release','--locked','-p','dive-api','--example','terminal_classification'],'cargo-guide-build.log')
shutil.copy2(t/'release/examples/terminal_classification',frozen/'terminal-outcome-guide');run([str(frozen/'terminal-outcome-guide')],'cargo-guide.log')
assert(b/'cargo-guide.log').read_text().splitlines()==[
 'terminal=EofWitness graph_states=2 final_checks=1/1 post_passed=false model_proof=false',
 'terminal=PostconditionFailure { code: Some(2405) } graph_states=2 final_checks=1/1 post_passed=false model_proof=false',
 'terminal=FinalSelectionsComplete graph_states=2 final_checks=1/1 post_passed=true model_proof=false']
# Test-only controlled actual RequestPrefix is separate from default release graph.
run(['cargo','test','--locked','-p','dive-api','--lib','periodic_candidate::tests::','--','--test-threads=2'],'controlled-request-and-authority.log')
assert '5 passed; 0 failed'in(b/'controlled-request-and-authority.log').read_text()
run(['cargo','test','--locked','-p','dive-api','--test','stopped_periodic_candidate','--','--test-threads=2'],'fresh-api-controls.log');assert '4 passed; 0 failed'in(b/'fresh-api-controls.log').read_text()
run(['cargo','test','--locked','-p','dive-check','--test','stopped_observer','--','--test-threads=2'],'fresh-core-stopped-controls.log');assert '3 passed; 0 failed'in(b/'fresh-core-stopped-controls.log').read_text()
run(['cargo','test','--locked','-p','dive-api','--test','terminal_periodic_candidate','--','--test-threads=2'],'fresh-legacy-terminal-controls.log');assert '4 passed; 0 failed'in(b/'fresh-legacy-terminal-controls.log').read_text()
run(['cargo','test','--locked','-p','dive-api','--test','periodic_candidate','--','--test-threads=2'],'mandatory-api-controls.log');assert '7 passed; 0 failed'in(b/'mandatory-api-controls.log').read_text()
run(['cargo','test','--locked','-p','dive-api','--test','terminal_classification','--','--test-threads=2'],'fresh-final-classification-controls.log');assert '8 passed; 0 failed'in(b/'fresh-final-classification-controls.log').read_text()
run(['cargo','test','--locked','-p','dive-check','--test','deferred_postconditions','--','--test-threads=2'],'fresh-token-ownership-controls.log');assert '3 passed; 0 failed'in(b/'fresh-token-ownership-controls.log').read_text()
run(['cargo','test','--locked','-p','dive-check','--lib','evidence_controls','--','--test-threads=2'],'fresh-capture-finalization-controls.log');assert '2 passed; 0 failed'in(b/'fresh-capture-finalization-controls.log').read_text()
run(['cargo','test','--locked','-p','dive-liveness','--lib','final_records','--','--test-threads=2'],'fresh-product-finalization-controls.log');assert '1 passed; 0 failed'in(b/'fresh-product-finalization-controls.log').read_text()
for corpus,listname in [('examples','cm1'),('validation-smoke','validation-smoke')]:
 selected=[s.split('#')[0].strip()for s in(r/'conformance/gates'/f'{listname}.txt').read_text().splitlines()];selected=[s for s in selected if s];assert len(selected)==(21 if corpus=='examples'else 16)
 out=b/(corpus+'-reports');out.mkdir(exist_ok=False);cmd=[str(frozen/'dive-conformance'),'run','--impl','dive','--impl-profile','tlc','--dive-bin',str(frozen/'dive'),'--corpus',corpus,'--tier','all','--no-sany','--no-summary','--report-dir',str(out),'--workers','1,4,8']
 for name in selected:cmd+=['--filter',name]
 run(cmd,corpus+'-gate.log');lines=[s for s in(b/(corpus+'-gate.log')).read_text().splitlines()if s.startswith('[')and'/'in s];assert len(lines)==len(selected)and all('] agree 'in s for s in lines),lines
bins={p.name:sha(p)for p in frozen.iterdir()if p.is_file()};assert all(sha(r/p)==h for p,h in before['source_sha256'].items())
(b/'runtime-closed.json').write_text(json.dumps(dict(production_commit=before['producer'],source_sha256=before['source_sha256'],binary_sha256=bins,steps=steps,source_and_binaries_unchanged=True,scope='Owned terminal graph/resource/postcondition composition preserves original source error and actual final selection with fresh independent words; separately controlled RequestPrefix and default-release EOF/mutant refusals;37 ordinary safety gate models1/4/8; no ordinary temporal scheduling/model proof'),indent=2)+'\n')
print('Closed terminal-finalization runtime, guide, mandatory compatibility and37 required gate models1/4/8')
