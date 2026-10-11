from pathlib import Path
import subprocess,os,shutil,json,hashlib
root=Path('/Users/jeff/code/dive-pluscal-rewrite-publication-work');source='b8fb13cdab443e7ad0206a467d9a392075f31397';target=Path('/Users/jeff/code/dive-pluscal-rewrite-target');out=Path('/Users/jeff/code/dive-full-runtime-cst-final-artifacts')
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==source
assert not subprocess.check_output(['git','status','--porcelain','-uno'],cwd=root,text=True).strip()
env=os.environ.copy();env.update(CARGO_TARGET_DIR=str(target),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0')
for cmd in [['cargo','build','-p','dive-cli','-p','dive-conformance','--locked'],['cargo','build','-p','dive-pluscal','--example','rewrite','--example','translate','--locked']]:
 p=subprocess.run(cmd,cwd=root,env=env);assert p.returncode==0
out.mkdir(exist_ok=False)
for name,src in [('dive',target/'debug/dive'),('dive-conformance',target/'debug/dive-conformance'),('editor',target/'debug/examples/rewrite'),('cst',target/'debug/examples/translate')]:shutil.copy2(src,out/name);(out/name).chmod(0o555)
receipt={'source_commit':source,'binary_sha256':{name:hashlib.sha256((out/name).read_bytes()).hexdigest()for name in ['dive','dive-conformance','editor','cst']}}
(out/'producer.json').write_text(json.dumps(receipt,indent=2)+'\n');print('Built exact frozen final producer',source)
