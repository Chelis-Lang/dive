from pathlib import Path
from collections import Counter
import subprocess,sys,shutil,json,hashlib,gzip,os,re,tempfile
root=Path.cwd();base=root/'crates/dive-pluscal/recursive-procedure-acceptance';out=root/'conformance/probes/native-parity-integration/current-controls/recursive-final-integration';out.mkdir(parents=True,exist_ok=True)
target=Path('/Users/jeff/code/dive-pluscal-procedures-target');example=target/'release/examples/translate';native=target/'release/dive';harness=target/'release/dive-conformance'
cache=Path('/Users/jeff/code/dive-shipping-cache');jar=cache/'sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar';java=cache/'jre/190480874ccceb358cbc840393207f77ac3e63a4c5f8129d0e23e9518b96ad05/jdk-17.0.20.1+1-jre/Contents/Home/bin/java'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def run(label,cmd,cwd=root,env=None,expected=0):
 print('RUN',label,flush=True);p=subprocess.run([str(x) for x in cmd],cwd=cwd,capture_output=True,env=env);(out/(label+'.stdout.gz')).write_bytes(gzip.compress(p.stdout,mtime=0));(out/(label+'.stderr.gz')).write_bytes(gzip.compress(p.stderr,mtime=0));print('RESULT',p.returncode,label,flush=True)
 if p.returncode!=expected: print(p.stderr.decode()[-2500:],p.stdout.decode()[-2500:]);raise SystemExit(p.returncode or 1)
 return p
copy=out/'acceptance';copy.mkdir(exist_ok=True)
for name in ('cases','fixtures'):shutil.copytree(base/name,copy/name,dirs_exist_ok=True)
shutil.copy2(base/'selection.json',copy/'selection.json')
for artifact in base.glob('*.java'):shutil.copy2(artifact,copy/artifact.name)
for script in base.glob('*.py'):shutil.copy2(script,copy/script.name)
(copy/'budgets').mkdir(exist_ok=True);shutil.copy2(base/'budgets/probe.rs',copy/'budgets/probe.rs')
run('compare',[sys.executable,copy/'compare.py','--manifest',copy/'fixtures/manifest.json','--inputs',copy/'cases','--native',example,'--output',copy/'comparison.json'])
run('validate-strict',[sys.executable,copy/'validate.py','--java',java,'--jar',jar,'--native',native,'--example',example,'--report',copy/'comparison.json','--inputs',copy/'cases','--fixtures',copy/'fixtures','--output',copy/'validation'],expected=1)
run('qualified-replay',[sys.executable,copy/'qualified-replay.py','--native',native,'--output',copy/'qualified-replay'])
run('invalid-admission',[sys.executable,copy/'check-admission.py','--native',native])
run('document-mutants',[sys.executable,copy/'test-comparison.py','--native',example])
# Execute unchanged API/budget/guide script functions with an explicit evidence destination.
# Source digest stays tied to the original script; no historical output is overwritten.
runner=out/'run-script.py';runner.write_text('''import runpy,sys\nfrom pathlib import Path\nfrom argparse import Namespace\nsource,base,root,kind,target,native,java,jar=sys.argv[1:]\nn=runpy.run_path(source,run_name="captured_controls")\nf=n["main"];g=f.__globals__;g["BASE"]=Path(base);g["ROOT"]=Path(root)\na=Namespace(target=Path(target),native=Path(native),java=Path(java),jar=Path(jar))\nf(a if kind!="guide-cli" else Path(native))\n''')
for kind,script in [('api-admission','check-api-admission.py'),('budgets','check-budgets.py'),('guide-api','check-guide-api.py'),('guide-cli','check-guide-cli.py')]:
 run(kind,[sys.executable,runner,base/script,copy,root,kind,target,native,java,jar])
# Earlier complete source selections are retained verbatim and re-enumerated.
for kind,directory in [('single','procedure-acceptance'),('graphs','procedure-graph-acceptance')]:
 original=root/'crates/dive-pluscal'/directory
 run('earlier-'+kind+'-compare',[sys.executable,original/'compare.py','--manifest',original/'fixtures/manifest.json','--inputs',original/'cases','--native',example,'--output',out/(kind+'-comparison.json')])
env=os.environ.copy();env['DIVE_CACHE_DIR']=str(cache);env['HARNESS']=str(harness)
run('cm1',['bash',root/'scripts/conformance-gate.sh',root/'conformance/gates/cm1.txt','--workers','1,4,8','--dive-bin',native],env=env);env['DIVE_GATE_CORPUS']='validation-smoke'
run('validation-smoke',['bash',root/'scripts/conformance-gate.sh',root/'conformance/gates/validation-smoke.txt','--workers','1,4,8','--dive-bin',native],env=env)
reports={name:sha(copy/name) for name in ['comparison.json','validation/report.json','qualified-replay/report.json','admission/report.json','api-admission/report.json','budgets/report.json','guide-api/report.json','guide-cli/report.json','mutation-controls.json']}
(out/'run-summary.json').write_text(json.dumps({'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'source_tree':subprocess.check_output(['git','rev-parse','HEAD^{tree}'],text=True).strip(),'native_sha256':sha(native),'example_sha256':sha(example),'reports_sha256':reports,'family_certified':False,'tlc_violation_certified':False,'scope':'Full recursive selection and raw controls on current integrated source; earlier complete source selections re-enumerated, earlier model-runtime receipts retain their original sources.'},indent=2)+'\n')
shutil.copy2('/tmp/dive-recursive-integration-controls.py',out/'controls.py')
print('ALL CURRENT RECURSIVE CONTROLS ACCOUNTED; strict witness refusals retained, canonical replay separate',flush=True)
