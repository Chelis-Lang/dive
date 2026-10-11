from pathlib import Path
import subprocess,json,hashlib,os,sys,re
root=Path(sys.argv[1]).resolve();binary=Path(sys.argv[2]).resolve();target=Path(sys.argv[3]).resolve();out=Path(sys.argv[4]).resolve()
source=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
assert source=='fbaa81a11fc4a0f7a7606f7a951fb2b91b84384f'
out.mkdir(parents=True,exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
env=os.environ.copy()
for key in ['DIVE_COMPAT','DIVE_CHAOS']:env.pop(key,None)
env.update(CARGO_TARGET_DIR=str(target),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0')
records=[]
old=json.loads((root/'conformance/probes/native-checkpoint-spill/current-shipping/guide/index.json').read_text())
for entry in old['controls']:
 args=[str(out/'cp-spill.cp') if a=='/tmp/dive-checkpoint-spill-shipping-guide/cp-spill.cp' else a for a in entry['args']]
 result=subprocess.run([str(binary),*args],cwd=root,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=180)
 stem=entry['id'];(out/(stem+'.stdout.json')).write_bytes(result.stdout);(out/(stem+'.stderr')).write_bytes(result.stderr)
 record={'id':stem,'args':args,'exit':result.returncode,'stdout':stem+'.stdout.json','stderr':stem+'.stderr'};records.append(record)
 assert result.returncode==entry['exit'],record
 value=json.loads(result.stdout)
 assert value['options']['workers']['used']==(4 if stem=='pause' else 8),value
 assert value['checkpoint']['transient_storage_cleanup_completed'] is True,value
 assert value['options']['checkpoint']['independent_certificate'] is False,value
 if stem=='pause':
  assert value['checkpoint']['status']=='paused-incomplete' and value['verdict']=='resource-limit',value
 else:
  assert value['verdict']=='invariant-violation',value
  for k,v in {'generated':5,'distinct':3,'depth':3}.items():assert value['statistics'][k]==v,value
  assert value['counterexample'] is not None,value
 record['stdout_sha256']=sha(out/record['stdout']);record['stderr_sha256']=sha(out/record['stderr'])
 (out/'cli-progress.json').write_text(json.dumps(records,indent=2)+'\n')
section=(root/'docs/native-checkpoint.md').read_text().split('## Recovery with transient disk storage',1)[1]
code=re.search(r'```rust,no_run\n(.*?)\n```',section,re.S)[1]
caller=out/'caller';(caller/'src').mkdir(parents=True)
(caller/'Cargo.toml').write_text('[package]\nname="spill-publication-guide"\nversion="0.0.0"\nedition="2024"\npublish=false\n[dependencies]\ndive-api={path='+json.dumps(str(root/'crates/dive-api'))+'}\n')
wrapped='fn main() -> Result<(), dive_api::ApiError> {\n'+re.sub(r'^# ', '',code,flags=re.M)+'\n}\n'
(caller/'src/main.rs').write_text(wrapped)
runtime=out/'rust-runtime';runtime.mkdir()
(runtime/'conformance').symlink_to(root/'conformance',target_is_directory=True)
result=subprocess.run(['cargo','run','--manifest-path',str(caller/'Cargo.toml'),'--offline'],cwd=runtime,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=600)
(out/'rust-run.stdout').write_bytes(result.stdout);(out/'rust-run.stderr').write_bytes(result.stderr)
assert result.returncode==0,result.stderr.decode()[-3000:]
record={'schema_version':1,'source_commit':source,'binary_sha256':sha(binary),'guide_sha256':sha(root/'docs/native-checkpoint.md'),'code_extraction':'Exact documented Rust fence, hidden rustdoc prefixes removed, main Result wrapper added. Paths resolved through a read-only conformance symlink in the private run directory.','cli_controls':records,'rust_example_executed':True,'rust_exit':result.returncode,'independent_archive_certificate':False,'family_certified':False}
record['files_sha256']={str(p.relative_to(out)):sha(p) for p in out.rglob('*') if p.is_file() and not p.is_symlink() and 'conformance' not in p.relative_to(out).parts}
(out/'verification.json').write_text(json.dumps(record,indent=2)+'\n')
print('Three current-source CLI controls and the exact wrapped Rust guide passed')
