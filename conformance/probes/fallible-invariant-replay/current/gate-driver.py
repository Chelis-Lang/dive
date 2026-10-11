import os,pathlib,subprocess,json,hashlib,time,gzip,re
root=pathlib.Path('/Users/jeff/code/dive-trace-representation-work')
out=pathlib.Path('/Users/jeff/code/dive-storage-tuple-function-artifacts/seen-replay-current')
for k in ['CARGO_TARGET_DIR','CARGO_BUILD_JOBS','DIVE_CACHE_DIR','CARGO_INCREMENTAL']:assert os.environ.get(k),k
assert os.environ['CARGO_TARGET_DIR']=='/Users/jeff/code/dive-trace-representation-target'
assert os.environ['CARGO_BUILD_JOBS']=='2' and os.environ['CARGO_INCREMENTAL']=='0'
producer=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root)
def sha(data):return hashlib.sha256(data).hexdigest()
paths=subprocess.check_output(['git','ls-files'],cwd=root,text=True).splitlines()
paths=[p for p in paths if p.endswith('.rs') or pathlib.Path(p).name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','deny.toml'] or p in ['.config/nextest.toml','xtask/src/layering.rs']]
inputs={}
for p in paths:
 data=(root/p).read_bytes();assert data==subprocess.check_output(['git','show',producer+':'+p],cwd=root)
 archive=out/'source'/ (p+'.gz');archive.parent.mkdir(parents=True,exist_ok=True);archive.write_bytes(gzip.compress(data,mtime=0));inputs[p]={'sha256':sha(data),'archive':str(archive.relative_to(out))}
(out/'source-index.json').write_text(json.dumps({'producer':producer,'scope':'Actual immutable gate and runtime Rust/Cargo inputs; no source-only conformance claim','inputs':inputs},indent=2)+'\n')
py=[]
for cmd in re.findall(r'^\s+(?:run: )?(python3[^\n]+)$',(root/'.github/workflows/ci.yml').read_text(),re.M):
 if cmd not in py:py.append(cmd)
py.append('python3 -I scripts/check-fallible-seen-engine.py')
stages=[['cargo fmt --all --check',*py],['cargo clippy --workspace --all-targets --locked -- -D warnings'],['cargo nextest run --workspace --locked --profile ci --test-threads 2'],['cargo test --workspace --doc --locked'],['cargo doc --workspace --no-deps --locked'],['cargo xtask check-layering'],['cargo build -p dive-trace --locked'],['cargo deny --locked check']]
gate=out/'gate';gate.mkdir(exist_ok=False);rows=[];start=time.monotonic()
for i,commands in enumerate(stages):
 t=time.monotonic();code=0;log=gate/f'stage-{i}.log'
 with log.open('wb') as f:
  for cmd in commands:
   f.write(('COMMAND: '+cmd+'\n').encode());f.flush();env=os.environ.copy()
   if i==4:env['RUSTDOCFLAGS']='-D warnings'
   r=subprocess.run(cmd.split(),cwd=root,stdout=f,stderr=subprocess.STDOUT,env=env)
   if r.returncode:code=r.returncode;break
 rows.append({'stage':i,'commands':commands,'exit':code,'seconds':time.monotonic()-t,'log_sha256':sha(log.read_bytes())})
 assert all(sha((root/p).read_bytes())==v['sha256'] for p,v in inputs.items())
 (gate/'receipt.json').write_text(json.dumps({'producer':producer,'source_index_sha256':sha((out/'source-index.json').read_bytes()),'rows':rows,'jobs':2,'test_threads':2,'profile':'ci','per_test_timeout_unchanged':True,'elapsed_seconds':time.monotonic()-start},indent=2)+'\n')
 print('stage',i,'exit',code,'seconds',rows[-1]['seconds'],flush=True)
 if code:raise SystemExit(code)
print('CLOSED full8gate',producer,flush=True)
