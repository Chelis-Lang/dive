import pathlib,os,json,subprocess,time,hashlib
root=pathlib.Path('/Users/jeff/code/dive-trace-representation-work');base=pathlib.Path('/Users/jeff/code/dive-storage-tuple-function-artifacts/seen-roles-current');producer='7fae387c11675b6a72119cc0013dd18f4eac33cb'
for k,v in {'CARGO_TARGET_DIR':'/Users/jeff/code/dive-trace-representation-target','CARGO_BUILD_JOBS':'2','CARGO_INCREMENTAL':'0','DIVE_CACHE_DIR':'/Users/jeff/code/dive-trace-representation-cache'}.items():assert os.environ.get(k)==v,k
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==producer and not subprocess.check_output(['git','status','--porcelain'],cwd=root)
assert json.loads((base/'current-binaries/producer.json').read_text())['runtime_source_commit']==producer
out=base/'runtime-driver';out.mkdir(exist_ok=False);rows=[];start=time.monotonic();sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();source=json.loads((base/'source-index.json').read_text())
commands=[['python3','-I','/tmp/dive-role-current-runtime.py'],['python3','-I','/tmp/dive-role-original-runtime.py'],['python3','-I','/tmp/dive-role-replay-property-runtime.py'],[str(base/'current-binaries/dive-disk-role-controls'),str(root),str(base/'runtime')]]
for i,command in enumerate(commands):
 stdout=out/f'stage-{i}.stdout';stderr=out/f'stage-{i}.stderr';t=time.monotonic()
 with stdout.open('wb') as a,stderr.open('wb') as b:r=subprocess.run(command,cwd=root,stdout=a,stderr=b)
 rows.append({'stage':i,'command':command,'exit':r.returncode,'stdout_sha256':sha(stdout),'stderr_sha256':sha(stderr),'seconds':time.monotonic()-t})
 (out/'receipt.json').write_text(json.dumps({'producer':producer,'rows':rows,'elapsed_seconds':time.monotonic()-start,'family_certified':False},indent=2)+'\n');print('runtime stage',i,'exit',r.returncode,'seconds',rows[-1]['seconds'],flush=True)
 assert r.returncode==0,command
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==producer and not subprocess.check_output(['git','status','--porcelain'],cwd=root)
 assert all(sha(root/name)==row['sha256'] for name,row in source['inputs'].items())
api=base/'api-controls';api.mkdir(exist_ok=False)
api.joinpath('stdout.json').write_bytes((out/'stage-3.stdout').read_bytes());api.joinpath('stderr.log').write_bytes((out/'stage-3.stderr').read_bytes())
controls=json.loads((api/'stdout.json').read_text())['rows'];assert len(controls)==2088
words=[r for r in controls if 'positive' in r];assert len(words)==432 and all(r['positive']=='valid' and r['mutants'] and all(m['validation']=='invalid' for m in r['mutants']) for r in words)
receipt={'producer':producer,'consumer_sha256':sha(base/'current-binaries/dive-disk-role-controls'),'cli_sha256':sha(base/'current-binaries/dive'),'consumer_source_sha256':sha(root/'conformance/probes/disk-safety-roles/controls/src/main.rs'),'public_selected':len(controls),'independent_valid':len(words),'actual_named_mutants_invalid':sum(len(r['mutants']) for r in words),'stdout_sha256':sha(api/'stdout.json'),'stderr_sha256':sha(api/'stderr.log'),'family_certified':False};(api/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print('CLOSED runtime+public',json.dumps(receipt),flush=True)
