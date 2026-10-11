import os,pathlib,json,subprocess,hashlib,time,shutil
root=pathlib.Path('/Users/jeff/code/dive-trace-representation-work');base=pathlib.Path('/Users/jeff/code/dive-storage-tuple-function-artifacts/seen-roles-current');target=pathlib.Path('/Users/jeff/code/dive-trace-representation-target');producer='7fae387c11675b6a72119cc0013dd18f4eac33cb'
for k,v in {'CARGO_TARGET_DIR':str(target),'CARGO_BUILD_JOBS':'2','CARGO_INCREMENTAL':'0','DIVE_CACHE_DIR':'/Users/jeff/code/dive-trace-representation-cache'}.items():assert os.environ.get(k)==v,k
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==producer and not subprocess.check_output(['git','status','--porcelain'],cwd=root)
gate=json.loads((base/'gate/receipt.json').read_text());assert len(gate['rows'])==8 and all(r['exit']==0 for r in gate['rows']) and gate['producer']==producer
build=base/'release-build';build.mkdir(exist_ok=False);rows=[];start=time.monotonic()
commands=[['cargo','build','--release','--locked','-p','dive-cli','-p','dive-conformance'],['cargo','build','--release','--locked','-p','dive-api','--example','disk_seen_roles'],['cargo','build','--release','--locked','--manifest-path','conformance/probes/disk-safety-roles/controls/Cargo.toml']]
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source=json.loads((base/'source-index.json').read_text())
for i,command in enumerate(commands):
 log=build/f'stage-{i}.log';t=time.monotonic()
 with log.open('wb') as stream:r=subprocess.run(command,cwd=root,stdout=stream,stderr=subprocess.STDOUT)
 rows.append({'stage':i,'command':command,'exit':r.returncode,'seconds':time.monotonic()-t,'log_sha256':sha(log)})
 (build/'receipt.json').write_text(json.dumps({'producer':producer,'jobs':2,'incremental':False,'rows':rows,'elapsed_seconds':time.monotonic()-start},indent=2)+'\n')
 assert r.returncode==0,command
 assert all(sha(root/name)==row['sha256'] for name,row in source['inputs'].items())
bins=base/'current-binaries';bins.mkdir(exist_ok=False);binaries=[]
for name,path in [('dive',target/'release/dive'),('dive-conformance',target/'release/dive-conformance'),('disk_seen_roles',target/'release/examples/disk_seen_roles'),('dive-disk-role-controls',target/'release/dive-disk-role-controls')]:
 destination=bins/name;shutil.copy2(path,destination);assert sha(path)==sha(destination);binaries.append({'name':name,'runtime_source_commit':producer,'sha256':sha(destination),'path':name})
(bins/'producer.json').write_text(json.dumps({'runtime_source_commit':producer,'source_index_sha256':sha(base/'source-index.json'),'binaries':binaries,'jobs':2,'incremental':False,'qualification':'All copied executables are built after the exact-source complete gate. Selected input provenance is not a hermetic dependency/toolchain claim.'},indent=2)+'\n');print('CLOSED release',producer,json.dumps(binaries))
