from pathlib import Path
import hashlib,json,subprocess,sys
root=Path('/Users/jeff/code/dive-phase-l-graph-shipping-work')
bin=Path('/Users/jeff/code/dive-phase-l-graph-work/target/release/dive')
out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
old=json.loads((root/'docs/user/examples/observations.json').read_text())
receipt={'schema_version':1,'family_certified':False,'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'source_tree':subprocess.check_output(['git','rev-parse','HEAD^{tree}'],cwd=root,text=True).strip(),'binary_sha256':sha(bin),'source_sha256':{str(p.relative_to(root)):sha(p) for p in (root/'docs/user/examples').glob('*') if p.suffix in ['.tla','.cfg']},'rows':[]}
def run(name,args,expected):
 r=subprocess.run([str(bin),*args],cwd=root,capture_output=True,timeout=60)
 (out/(name+'.stdout.json')).write_bytes(r.stdout);(out/(name+'.stderr')).write_bytes(r.stderr)
 v=json.loads(r.stdout);actual=v.get('result',v.get('verdict',v.get('accepted')))
 receipt['rows'].append({'id':name,'args':args,'exit':r.returncode,'result':actual,'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()})
 (out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
 assert (r.returncode,actual)==expected,(name,r.returncode,actual,expected)
 return v
for i,row in enumerate(old['rows']):run('guide-'+str(i),row['args'],(row['exit'],row['result']))
archive=out/'guide.cp';args=['check','docs/user/examples/CheckpointCounter.tla','--compat=tlc','--seed','7','--fp','0','--output','json']
pause=run('checkpoint-pause',args+['--checkpoint',str(archive),'--checkpoint-after-levels','2'],(3,'resource-limit'))
assert pause['checkpoint']['status']=='paused-incomplete'
recovered=run('checkpoint-recover',args+['--recover',str(archive)],(0,'success'))
assert recovered['statistics']['distinct']==5 and recovered['statistics']['generated']==6
run('original-checkpoint-refusal',['check','docs/user/examples/Counter.tla','--compat=tlc','--checkpoint',str(out/'refused.cp'),'--checkpoint-after-levels','2','--output','json'],(3,'unsupported'))
assert not (out/'refused.cp').exists()
print('Guide commands and pure recovery/refusal passed:',len(receipt['rows']))
