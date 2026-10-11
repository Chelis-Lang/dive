from pathlib import Path
import subprocess,json,os,hashlib,re,time,gzip
root=Path('/Users/jeff/code/dive-trace-representation-work')
out=Path('/Users/jeff/code/dive-trace-representation-artifacts/full-gate')
out.mkdir(parents=True,exist_ok=True)
producer=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True).strip()
sha=lambda b:hashlib.sha256(b).hexdigest()
inputs={}
for n in subprocess.check_output(['git','ls-files'],cwd=root,text=True).splitlines():
 if (n.startswith(('crates/','xtask/')) and (n.endswith('.rs') or n.endswith('/Cargo.toml'))) or n in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']:
  raw=(root/n).read_bytes();assert subprocess.check_output(['git','show',producer+':'+n],cwd=root)==raw,n
  p=out/'source'/(n+'.gz');p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(gzip.compress(raw,mtime=0));inputs[n]={'archive':n+'.gz','sha256':sha(raw)}
(out/'source/index.json').write_text(json.dumps({'producer_commit':producer,'scope':'All crate/xtask Rust and Cargo inputs, including tests/examples/fixtures; archive equality proves source identity, not conformance.','inputs':inputs},indent=2)+'\n')
commands=re.findall(r'(python3(?: -I)? scripts/[^\n]+)',(root/'.github/workflows/ci.yml').read_text())
stages=[('controls',['cargo fmt --all --check']+commands),('clippy',['cargo clippy --workspace --all-targets --locked -- -D warnings']),('nextest',['cargo nextest run --workspace --locked --profile ci --test-threads 2']),('doctests',['cargo test --workspace --doc --locked']),('docs',['cargo doc --workspace --no-deps --locked']),('layering',['cargo xtask check-layering']),('standalone',['cargo build -p dive-trace --locked']),('deny',['cargo deny --locked check'])]
receipt={'producer_commit':producer,'source_index_sha256':sha((out/'source/index.json').read_bytes()),'environment':{'CARGO_BUILD_JOBS':'2','nextest_threads':2,'CI_timeouts':'Unchanged ci profile per-test limits; local stages have no fabricated 15min CI completion claim'},'stages':[]}
env={**os.environ,'CARGO_TARGET_DIR':'/Users/jeff/code/dive-trace-representation-target','CARGO_BUILD_JOBS':'2'}
started=time.monotonic()
for stage,cmds in stages:
 row={'name':stage,'commands':[]};receipt['stages'].append(row)
 for i,cmd in enumerate(cmds):
  print('START',stage,i,cmd,flush=True);t=time.monotonic();log=out/f'{stage}-{i}.log'
  runenv={**env,**({'RUSTDOCFLAGS':'-D warnings'} if stage=='docs' else {})}
  with log.open('wb') as f:r=subprocess.run(cmd,cwd=root,env=runenv,shell=True,stdout=f,stderr=subprocess.STDOUT)
  row['commands'].append({'command':cmd,'exit':r.returncode,'seconds':time.monotonic()-t,'log':log.name,'sha256':sha(log.read_bytes())})
  receipt['total_seconds']=time.monotonic()-started
  (out/'index.json').write_text(json.dumps(receipt,indent=2)+'\n');print('DONE',stage,i,r.returncode,flush=True)
for n,row in inputs.items():assert sha((root/n).read_bytes())==row['sha256'],n
print('CLOSED',producer,'all8:',all(c['exit']==0 for s in receipt['stages'] for c in s['commands']),flush=True)
