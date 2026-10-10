from pathlib import Path
import subprocess,sys,shutil,json,hashlib,gzip,os
root=Path.cwd();base=root/'crates/dive-pluscal/macro-acceptance';out=root/'conformance/probes/native-parity-integration/current-controls/macros-ordered-execution-integration';out.mkdir(parents=True,exist_ok=True)
example=Path('/Users/jeff/code/dive-pluscal-macros-shipping-target/release/examples/translate');native=example.parent.parent/'dive';harness=native.parent/'dive-conformance'
cache=Path('/Users/jeff/code/dive-shipping-cache');jar=cache/'sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar';java=cache/'jre/190480874ccceb358cbc840393207f77ac3e63a4c5f8129d0e23e9518b96ad05/jdk-17.0.20.1+1-jre/Contents/Home/bin/java'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def run(label,cmd,cwd=root,env=None):
 print('RUN',label,flush=True);p=subprocess.run([str(x) for x in cmd],cwd=cwd,capture_output=True,env=env);(out/(label+'.stdout.gz')).write_bytes(gzip.compress(p.stdout,mtime=0));(out/(label+'.stderr.gz')).write_bytes(gzip.compress(p.stderr,mtime=0));print('RESULT',p.returncode,label,flush=True)
 if p.returncode:print(p.stderr.decode()[-2500:],p.stdout.decode()[-2500:]);raise SystemExit(p.returncode)
for folder in ('','extra','process','scope','lexical','origins'):
 d=base/folder;target=out/(folder or 'main');target.mkdir(exist_ok=True)
 run('compare-'+(folder or 'main'),[sys.executable,base/'compare.py','--manifest',d/'fixtures/manifest.json','--inputs',d/'cases','--native',example,'--output',target/'comparison.json'])
 run('validate-'+(folder or 'main'),[sys.executable,base/'validate.py','--java',java,'--jar',jar,'--native',native,'--example',example,'--report',target/'comparison.json','--inputs',d/'cases','--fixtures',d/'fixtures','--output',target/'validation'])
# These scripts have fixed output paths; run identical copies against copied immutable inputs.
temporary=Path('/tmp/dive-macro-ordered-current-admission-copy');shutil.copytree(base,temporary,dirs_exist_ok=True)
run('admission',[sys.executable,temporary/'check-admission.py','--native',native]);shutil.copytree(temporary/'admission',out/'admission',dirs_exist_ok=True)
run('document-mutants',[sys.executable,temporary/'test-comparison.py','--native',example]);shutil.copy2(temporary/'mutation-controls.json',out/'mutation-controls.json')
env=os.environ.copy();env['DIVE_CACHE_DIR']=str(cache);env['HARNESS']=str(harness)
run('cm1',['bash',root/'scripts/conformance-gate.sh',root/'conformance/gates/cm1.txt','--workers','1,4,8','--dive-bin',native],env=env);env['DIVE_GATE_CORPUS']='validation-smoke'
run('validation-smoke',['bash',root/'scripts/conformance-gate.sh',root/'conformance/gates/validation-smoke.txt','--workers','1,4,8','--dive-bin',native],env=env)
rows=[]
for filename in ('MacroExample.tla','MacroExampleC.tla'):
 for profile in ('dive','tlc'):
  args=[native,'translate',root/'docs/user/examples'/filename,'--output','json']+(['--compat=tlc'] if profile=='tlc' else [])
  run('guide-'+filename+'-'+profile,args);doc=json.loads(gzip.decompress((out/('guide-'+filename+'-'+profile+'.stdout.gz')).read_bytes()));assert doc['accepted'] is True and doc['model_proved'] is False and doc['exhaustive'] is False
  rows.append({'filename':filename,'profile':profile,'passed':True})
(out/'guide.json').write_text(json.dumps({'rows':rows,'source_sha256':{x:sha(root/'docs/user/examples'/x) for x in ('MacroExample.tla','MacroExampleC.tla')},'native_sha256':sha(native)},indent=2)+'\n')
shutil.copy2('/tmp/dive-macro-ordered-integration-controls.py',out/'controls.py')
print('ALL CURRENT MACRO INTEGRATION CONTROLS PASSED',flush=True)
