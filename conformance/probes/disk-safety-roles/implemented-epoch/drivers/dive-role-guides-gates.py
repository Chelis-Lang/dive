import pathlib,json,os,subprocess,hashlib,time,re,shutil
root=pathlib.Path('/Users/jeff/code/dive-trace-representation-work');base=pathlib.Path('/Users/jeff/code/dive-storage-tuple-function-artifacts/seen-roles-current');bins=base/'current-binaries';target=pathlib.Path('/Users/jeff/code/dive-trace-representation-target');out=base/'guides-gates';producer='7fae387c11675b6a72119cc0013dd18f4eac33cb'
for k,v in {'CARGO_TARGET_DIR':str(target),'CARGO_BUILD_JOBS':'2','CARGO_INCREMENTAL':'0','DIVE_CACHE_DIR':'/Users/jeff/code/dive-trace-representation-cache'}.items():assert os.environ.get(k)==v,k
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==producer and not subprocess.check_output(['git','status','--porcelain'],cwd=root)
out.mkdir(exist_ok=False);rows=[];start=time.monotonic();sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def run(name,command,timeout=600,env=None):
 t=time.monotonic();r=subprocess.run(command,cwd=root,env=env,capture_output=True,timeout=timeout);stdout=out/(name+'.stdout');stderr=out/(name+'.stderr');stdout.write_bytes(r.stdout);stderr.write_bytes(r.stderr);rows.append({'name':name,'command':command,'exit':r.returncode,'stdout_sha256':sha(stdout),'stderr_sha256':sha(stderr),'seconds':time.monotonic()-t});(out/'running-receipt.json').write_text(json.dumps({'producer':producer,'rows':rows},indent=2)+'\n');assert r.returncode==0,(name,r.returncode);return r
# Exact current documentation invocation, not just a copied-example binary.
r=run('cargo-rust-guide',['cargo','run','-p','dive-api','--example','disk_seen_roles','--locked'])
expected={f'profile={profile},store={store},actual_workers=1,cleanup_completed=true' for profile in ['tlc','dive'] for store in ['exact','fingerprint']};assert set(r.stdout.decode().splitlines())==expected
actual=target/'debug/examples/disk_seen_roles';copied=bins/'disk_seen_roles-debug-guide';shutil.copy2(actual,copied);assert sha(actual)==sha(copied)
file=root/'crates/dive-api/examples/seen-roles/Counter.tla'
for profile in ['tlc','dive']:
 for store in ['fp','exact']:
  r=run('cli-guide-'+profile+'-'+store,[str(bins/'dive'),'check',str(file),'--compat',profile,'--seed','7','--fp','0','--workers','8','--store',store,'--seen-disk-entries','1000','--output','json'])
  d=json.loads(r.stdout);assert d['verdict']=='success' and tuple(d['statistics'][k] for k in ['generated','distinct','left_on_queue','depth'])==(4,4,0,4)
  assert d['options']['workers']['requested']==8 and d['options']['workers']['used']==1 and d['options']['workers']['sequential'] is True
  assert d['seen_storage']['after_cleanup']['cleanup_completed'] is True and d['seen_storage']['before_cleanup']['resident_key_collection_bytes']==0
for corpus,listname,expected in [('examples','cm1',21),('validation-smoke','validation-smoke',16)]:
 model_list=root/'conformance/gates'/(listname+'.txt');selected=[]
 for line in model_list.read_text().splitlines():
  name=line.split('#')[0].strip()
  if name:selected.append(name)
 assert len(selected)==expected
 command=[str(bins/'dive-conformance'),'run','--impl','dive','--impl-profile','tlc','--corpus',corpus,'--tier','all','--no-sany','--no-summary','--report-dir',str(out/('report-'+corpus)),'--dive-bin',str(bins/'dive'),'--workers','1,4,8']
 for name in selected:command+=['--filter',name]
 r=run('gate-'+corpus,command,timeout=1200);lines=re.findall(r'^\[\d+/\d+\] (\S+) ',(r.stdout+r.stderr).decode(),re.M);assert len(lines)==expected and set(lines)=={'agree'},(corpus,lines)
 assert sha(model_list)==hashlib.sha256(model_list.read_bytes()).hexdigest()
receipt={'producer':producer,'cli_sha256':sha(bins/'dive'),'harness_sha256':sha(bins/'dive-conformance'),'actual_cargo_guide_binary_sha256':sha(copied),'guide_source_sha256':sha(root/'crates/dive-api/examples/disk_seen_roles.rs'),'guide_model_sha256':sha(file),'guide_config_sha256':sha(file.with_suffix('.cfg')),'requested_widths':[1,4,8],'cm1_selected':21,'vts_selected':16,'rows':rows,'elapsed_seconds':time.monotonic()-start,'family_certified':False}
(out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print('CLOSED actual Cargo guide+4CLI+37conformance models at1/4/8')
