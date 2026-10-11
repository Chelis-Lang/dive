from pathlib import Path
import json,subprocess,os,hashlib,sys,re
root,binary,target,out=map(lambda p:Path(p).resolve(),sys.argv[1:]);out.mkdir(parents=True,exist_ok=False)
source=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip();assert source=='d7c4f5911a1e68d21a10f67bb3bee0546d6448c6'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();binary_sha=sha(binary)
env=os.environ.copy()
for k in ['DIVE_COMPAT','DIVE_CHAOS']:env.pop(k,None)
env.update(CARGO_TARGET_DIR=str(target),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0')
rows=[]
def call(directory,label,args,exit):
 r=subprocess.run([str(binary),*args],cwd=directory,env=env,capture_output=True,timeout=180)
 (directory/(label+'.stdout')).write_bytes(r.stdout);(directory/(label+'.stderr')).write_bytes(r.stderr)
 d=json.loads(r.stdout);assert r.returncode==exit,(label,r.returncode,d)
 row={'label':label,'args':args,'exit':r.returncode,'stdout_sha256':sha(directory/(label+'.stdout')),'stderr_sha256':sha(directory/(label+'.stderr'))};rows.append(row);return d
for name in ['RuntimeStrings','RuntimeSequence','RuntimeSymView','RuntimeDuplicates','RuntimeChoose']:
 for profile in ['tlc','dive']:
  for width in [1,4,8]:
   directory=out/f'{name}-{profile}-{width}';directory.mkdir();model=root/'conformance/probes/native-checkpoint-runtime-names/specs'/name/(name+'.tla')
   args=['check',str(model),'--seed','7','--fp','0','--workers',str(width),'--frontier-memory-bytes','0','--predecessor-memory-bytes','0','--output','json']+(['--compat=tlc']if profile=='tlc'else[])
   full=call(directory,'full',args,12 if profile=='tlc' else 10)
   assert full['verdict']=='invariant-violation' and full['counterexample'] is not None
   actual=1 if profile=='tlc'else width
   assert full['options']['workers']['used']==actual
   cp=directory/'state.cp'
   paused=call(directory,'pause',args+['--checkpoint',str(cp),'--checkpoint-after-levels','1'],3)
   assert paused['verdict']=='resource-limit' and paused['checkpoint']['status']=='paused-incomplete' and paused['counterexample'] is None and cp.is_file()
   assert paused['options']['workers']['used']==actual
   saved=sha(cp)
   for label,counters in [('retain','retain'),('repeat','retain'),('reset','tlc')]:
    resumed=call(directory,label,args+['--recover',str(cp),'--checkpoint-counters',counters],12 if profile=='tlc' else 10)
    assert sha(cp)==saved
    assert resumed['verdict']==full['verdict'] and resumed['counterexample']==full['counterexample']
    assert resumed['options']['workers']['used']==actual
    assert resumed['checkpoint']['transient_storage_cleanup_completed'] is True
    assert resumed['options']['checkpoint']['independent_certificate'] is False
    for key in ['distinct','depth']:assert resumed['statistics'][key]==full['statistics'][key],(name,key)
    if counters=='retain':assert resumed['statistics']==full['statistics'],name
    else:
     assert resumed['statistics']['generated']==full['statistics']['generated']-paused['statistics']['generated']+paused['statistics']['distinct'],name
     assert resumed['statistics']['initial_generated']==paused['statistics']['distinct'],name
   trace=full['counterexample']['trace'];trace_path=directory/'trace.json';trace_path.write_text(json.dumps(trace)+'\n')
   val=['validate-trace',str(model),'--trace',str(trace_path),'--seed','7','--fp','0','--output','json']+(['--compat=tlc']if profile=='tlc'else[])
   proof=call(directory,'independent-witness',val,3 if profile=='tlc'else 0)
   if profile=='tlc':
    assert proof['result']=='unsupported' and proof['unsupported']=='runtime string encounter order: the independent validator cannot establish names created outside the supplied trace (#124)'
   else:
    assert proof['result']=='valid'
    mutant=json.loads(json.dumps(trace));mutant['states'][0]['values']['n']=99;bad=directory/'mutant.json';bad.write_text(json.dumps(mutant)+'\n')
    val[val.index('--trace')+1]=str(bad);rejection=call(directory,'mutant-init',val,1);assert rejection['result']=='invalid'
   (out/'progress.json').write_text(json.dumps(rows,indent=2)+'\n')
# Exact documented API fence; Cargo owns resolution and a fresh directory owns archives.
section=(root/'docs/native-checkpoint.md').read_text().split('## Pure strings and recovery encounter order',1)[1]
code=re.search(r'```rust\n(.*?)\n```',section,re.S)[1]
caller=out/'caller';(caller/'src').mkdir(parents=True)
(caller/'Cargo.toml').write_text('[package]\nname="runtime-name-publication-guide"\nversion="0.0.0"\nedition="2024"\npublish=false\n[dependencies]\ndive-api={path='+json.dumps(str(root/'crates/dive-api'))+'}\n')
(caller/'src/main.rs').write_text('fn main() -> Result<(), dive_api::ApiError> {\n'+re.sub(r'^# ','',code,flags=re.M)+'\n}\n')
runtime=out/'guide-runtime';runtime.mkdir();(runtime/'conformance').symlink_to(root/'conformance',target_is_directory=True)
r=subprocess.run(['cargo','run','--offline','--manifest-path',str(caller/'Cargo.toml')],cwd=runtime,env=env,capture_output=True,timeout=600)
(out/'guide.stdout').write_bytes(r.stdout);(out/'guide.stderr').write_bytes(r.stderr);assert r.returncode==0,r.stderr.decode()[-5000:]
assert source==subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip();assert sha(binary)==binary_sha
receipt={'schema_version':1,'source_commit':source,'binary_sha256':binary_sha,'controls':rows,'exact_cargo_owned_guide':True,'guide_source_sha256':sha(caller/'src/main.rs'),'family_certified':False,'independent_tlc_runtime_name_certificate':False}
receipt['evidence_sha256']={str(p.relative_to(out)):sha(p)for p in out.rglob('*')if p.is_file() and not p.is_symlink() and 'conformance' not in p.relative_to(out).parts}
(out/'verification.json').write_text(json.dumps(receipt,indent=2)+'\n');print('195 fresh runtime-name controls and exact Cargo-owned guide passed')
