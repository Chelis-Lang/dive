import os,pathlib,subprocess,json,hashlib,time,gzip,re,posixpath
root=pathlib.Path('/Users/jeff/code/dive-trace-representation-work')
out=pathlib.Path('/Users/jeff/code/dive-storage-tuple-function-artifacts/seen-roles-current')
expected={'CARGO_TARGET_DIR':'/Users/jeff/code/dive-trace-representation-target','CARGO_BUILD_JOBS':'2','DIVE_CACHE_DIR':'/Users/jeff/code/dive-trace-representation-cache','CARGO_INCREMENTAL':'0'}
for k,v in expected.items():assert os.environ.get(k)==v,k
producer=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root)
def sha(data):return hashlib.sha256(data).hexdigest()
tracked=subprocess.check_output(['git','ls-files'],cwd=root,text=True).splitlines()
selected={p for p in tracked if p.endswith('.rs') or pathlib.Path(p).name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','deny.toml'] or p in ['.config/nextest.toml','.github/workflows/ci.yml','xtask/src/layering.rs'] or p.startswith('scripts/') and p.endswith(('.py','.sh'))}
compiled={p for p in selected if p.endswith('.rs') and (re.match(r'^crates/[^/]+/(src|tests|examples|benches)/',p) or p.startswith('xtask/src/') or re.match(r'^crates/[^/]+/build\.rs$',p))}
references=[];embedded=set()
for name in sorted(compiled):
 text=(root/name).read_text();matches=list(re.finditer(r'\b(include_str|include_bytes|include)!\s*\(\s*"([^"\\]+)"\s*\)',text))
 assert len(matches)==len(list(re.finditer(r'\b(?:include_str|include_bytes|include)!\s*\(',text))),name
 for m in matches:
  target=posixpath.normpath(str(pathlib.Path(name).parent/m.group(2)));assert not target.startswith('../') and not pathlib.Path(target).is_absolute();assert target in tracked,target
  references.append({'source':name,'macro':m.group(1),'literal':m.group(2),'target':target,'sha256':sha((root/target).read_bytes())});embedded.add(target)
selected.update(embedded)
inputs={};out.mkdir(parents=True,exist_ok=True)
for p in sorted(selected):
 data=(root/p).read_bytes();assert data==subprocess.check_output(['git','show',producer+':'+p],cwd=root)
 archive=out/'source'/(p+'.gz');archive.parent.mkdir(parents=True,exist_ok=True);archive.write_bytes(gzip.compress(data,mtime=0));inputs[p]={'sha256':sha(data),'archive':str(archive.relative_to(out))}
index={'producer':producer,'scope':'Selected tracked Rust/Cargo/gate-script inputs, with literal embedded targets derived from actual workspace Rust source paths before gate. Not a hermetic toolchain/dependency archive; historical Rust controls are selected but not thereby claimed compiled.','inputs':inputs,'workspace_rust_paths':sorted(compiled),'embedded_references':references,'embedded_inputs':sorted(embedded),'dynamic_includes':0,'family_certified':False}
(out/'source-index.json').write_text(json.dumps(index,indent=2)+'\n')
py=[]
for cmd in re.findall(r'^\s+(?:run: )?(python3[^\n]+)$',(root/'.github/workflows/ci.yml').read_text(),re.M):
 if cmd not in py:py.append(cmd)
assert 'python3 -I scripts/check-fallible-seen-engine.py' in py and 'python3 -I scripts/test-fallible-seen-git-proof.py' in py
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
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==producer
 (gate/'receipt.json').write_text(json.dumps({'producer':producer,'source_index_sha256':sha((out/'source-index.json').read_bytes()),'rows':rows,'jobs':2,'test_threads':2,'profile':'ci','per_test_timeout_unchanged':True,'ci_python_commands':py,'elapsed_seconds':time.monotonic()-start},indent=2)+'\n')
 print('stage',i,'exit',code,'seconds',rows[-1]['seconds'],flush=True)
 if code:raise SystemExit(code)
print('CLOSED full8gate',producer,flush=True)
