from pathlib import Path
import subprocess,sys,shutil,json,hashlib,gzip,os,re,tempfile
root=Path.cwd();base=root/'crates/dive-pluscal/procedure-acceptance';out=root/'conformance/probes/native-parity-integration/current-controls/procedures-integration';out.mkdir(parents=True,exist_ok=True)
target=Path('/Users/jeff/code/dive-pluscal-procedures-target');example=target/'release/examples/translate';native=target/'release/dive';harness=target/'release/dive-conformance'
cache=Path('/Users/jeff/code/dive-shipping-cache');jar=cache/'sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar';java=cache/'jre/190480874ccceb358cbc840393207f77ac3e63a4c5f8129d0e23e9518b96ad05/jdk-17.0.20.1+1-jre/Contents/Home/bin/java'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def run(label,cmd,cwd=root,env=None,expected=0):
 print('RUN',label,flush=True);p=subprocess.run([str(x) for x in cmd],cwd=cwd,capture_output=True,env=env);(out/(label+'.stdout.gz')).write_bytes(gzip.compress(p.stdout,mtime=0));(out/(label+'.stderr.gz')).write_bytes(gzip.compress(p.stderr,mtime=0));print('RESULT',p.returncode,label,flush=True)
 if p.returncode!=expected: print(p.stderr.decode()[-2500:],p.stdout.decode()[-2500:]);raise SystemExit(p.returncode or 1)
 return p
# Run unchanged scripts against private inputs, never overwrite historical campaigns.
copy=out/'acceptance';copy.mkdir(exist_ok=True)
for name in ('cases','fixtures'):shutil.copytree(base/name,copy/name,dirs_exist_ok=True)
for script in base.glob('*.py'):shutil.copy2(script,copy/script.name)
run('compare',[sys.executable,copy/'compare.py','--manifest',copy/'fixtures/manifest.json','--inputs',copy/'cases','--native',example,'--output',out/'comparison.json'])
run('validate-strict',[sys.executable,copy/'validate.py','--java',java,'--jar',jar,'--native',native,'--example',example,'--report',out/'comparison.json','--inputs',copy/'cases','--fixtures',copy/'fixtures','--output',copy/'validation'],expected=1)
# This checker retains exactly thirty strict witness refusals; canonical replay earns no TLC credit.
run('qualified-replay',[sys.executable,copy/'qualified-replay.py','--native',native,'--output',copy/'qualified-replay'])
run('invalid-admission',[sys.executable,copy/'check-admission.py','--native',native])
run('main-admission',[sys.executable,copy/'check-main-admission.py','--native',native,'--example',example,'--java',java,'--jar',jar])
run('document-mutants',[sys.executable,copy/'test-comparison.py','--native',example])
env=os.environ.copy();env['DIVE_CACHE_DIR']=str(cache);env['HARNESS']=str(harness)
run('cm1',['bash',root/'scripts/conformance-gate.sh',root/'conformance/gates/cm1.txt','--workers','1,4,8','--dive-bin',native],env=env);env['DIVE_GATE_CORPUS']='validation-smoke'
run('validation-smoke',['bash',root/'scripts/conformance-gate.sh',root/'conformance/gates/validation-smoke.txt','--workers','1,4,8','--dive-bin',native],env=env)
rows=[]
for filename in ('ProcedureExample.tla','ProcedureExampleC.tla'):
 for profile in ('dive','tlc'):
  p=run('guide-'+filename+'-'+profile,[native,'translate',root/'docs/user/examples'/filename,'--output','json']+(['--compat=tlc'] if profile=='tlc' else []));doc=json.loads(p.stdout);assert doc['accepted'] is True and doc['model_proved'] is False and doc['exhaustive'] is False
  rows.append({'filename':filename,'profile':profile,'passed':True})
# Compile and execute the actual documented Rust snippet for both syntax examples.
guide=(root/'docs/pluscal-procedures.md').read_text();snippets=re.findall(r'```rust\n(.*?)\n```',guide,re.S);assert len(snippets)==1
deps=target/'debug/deps';library=max(deps.glob('libdive_api-*.rlib'),key=lambda p:p.stat().st_mtime_ns)
for filename in ('ProcedureExample.tla','ProcedureExampleC.tla'):
 with tempfile.TemporaryDirectory(prefix='dive-current-procedure-guide-') as tmp:
  work=Path(tmp);shutil.copy2(root/'docs/user/examples'/filename,work/filename);program=snippets[0].replace('ProcedureExample.tla',filename);(work/'main.rs').write_text(program)
  run('guide-api-compile-'+filename,['rustc','--edition=2024',work/'main.rs','--extern','dive_api='+str(library),'-L','dependency='+str(deps),'-o',work/'example'])
  p=run('guide-api-run-'+filename,[work/'example'],cwd=work);doc=json.loads(p.stdout);assert doc['accepted'] is True and doc['model_proved'] is False and doc['exhaustive'] is False
  rows.append({'filename':filename,'entry':'compiled-rust-api','program_sha256':hashlib.sha256(program.encode()).hexdigest(),'passed':True})
(out/'guide.json').write_text(json.dumps({'rows':rows,'guide_sha256':sha(root/'docs/pluscal-procedures.md'),'source_sha256':{x:sha(root/'docs/user/examples'/x) for x in ('ProcedureExample.tla','ProcedureExampleC.tla')},'native_sha256':sha(native),'api_library_sha256':sha(library)},indent=2)+'\n')
shutil.copy2('/tmp/dive-procedure-integration-controls.py',out/'controls.py')
print('ALL CURRENT PROCEDURE CONTROLS ACCOUNTED: strict witness refusals retained, canonical replay separate',flush=True)
