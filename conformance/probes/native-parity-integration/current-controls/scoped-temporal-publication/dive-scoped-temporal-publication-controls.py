from pathlib import Path
import os,json,hashlib,subprocess,re,sys
root,binary,target,out=map(lambda p:Path(p).resolve(),sys.argv[1:]);out.mkdir(parents=True,exist_ok=False)
source=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
assert source=='70afaa8997ecca6145fb38df2077d3688a010c91',source
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
inputs={str(p.relative_to(root)):sha(p) for p in (root/'crates').rglob('*') if p.is_file() and p.suffix in ['.rs','.toml']}
env=os.environ.copy()
for k in ['DIVE_COMPAT','DIVE_CHAOS']:env.pop(k,None)
env.update(CARGO_TARGET_DIR=str(target),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0')
studies=[]
for study,expected in [('tlc-temporal-if-final',{'liveness-violation':76,'success':4,'unsupported':40}),('tlc-final-scopes',{'liveness-violation':28,'success':32,'invariant-violation':4,'unsupported':8})]:
 base=root/'conformance/probes'/study;dest=out/study
 r=subprocess.run(['python3','-I',str(base/'capture-native.py'),'--binary',str(binary),'--out',str(dest)],cwd=root,env=env,capture_output=True)
 (out/(study+'.capture.stdout')).write_bytes(r.stdout);(out/(study+'.capture.stderr')).write_bytes(r.stderr);assert r.returncode==0,r.stderr
 capture=json.loads((dest/'capture.json').read_text());assert capture['source_sha256']==inputs
 selections=json.loads((base/'selection.json').read_text());assert len(capture['rows'])==4*len(selections)
 assert [(r['model'],r['requested_workers'])for r in capture['rows']]==[(r['name'],w)for r in selections for w in [1,3,4,8]]
 projections={r['name']:r for r in json.loads((base/'final-projection/capture.json').read_text())['rows']}
 clean={r['name']:r for r in json.loads((base/'clean-tlc/capture.json').read_text())['rows']}
 counts={}
 for row in capture['rows']:
  name=row['model'];folder=dest/f"{name}-{row['requested_workers']}";d=json.loads((folder/'stdout').read_text())
  for stream in ['stdout','stderr']:assert sha(folder/stream)==row[stream+'_sha256']
  counts[d['verdict']]=counts.get(d['verdict'],0)+1
  assert d['schema_version']==2 and d['options']['ordinary_temporal_schedule_supported'] is False
  assert d['options']['workers']['requested']==row['requested_workers']
  if projections[name]['exit']!=0:
   assert row['exit']==3 and d['verdict']=='unsupported' and d['counterexample'] is None
   assert d['tlc'] is None or d['tlc']['verified'] is False
   continue
  reference=clean[name]
  assert d['tlc']['verified'] is True and row['exit']==reference['exit']==d['tlc']['exit_status']
  assert d['tlc']['code'] in reference['message_codes']
  if name in ['TfEnabledFails','TfEnabledHolds']:
   assert d['execution']['route']=='bounded-safety-only' and d['execution']['product_constructed'] is False and d['execution']['temporal_certificate'] is False
  else:
   assert d['execution']['schedule']=='final-only' and d['execution']['capture_completed'] is True
   assert d['options']['workers']['used']==row['requested_workers']
   for field in ['distinct','generated']:assert str(d['statistics'][field])==projections[name]['fields'][field],(study,name,field)
   depth=re.search(r'depth of the complete state graph search is (\d+)',(base/'clean-tlc'/name/'stdout').read_text())
   assert depth and d['statistics']['depth']==int(depth[1])
  if d['verdict'] in ['liveness-violation','invariant-violation']:
   assert d['counterexample']['validation']=='valid'
   if d['verdict']=='liveness-violation':assert d['counterexample']['trace']['properties']==d['properties']
 assert counts==expected,(study,counts)
 studies.append({'study':study,'rows':len(capture['rows']),'counts':counts,'capture_sha256':sha(dest/'capture.json')})
 guides=[]
for study,expected in [('tlc-temporal-if-final','["Live"]'),('tlc-final-scopes','["P", "Q"]')]:
 dest=out/(study+'-rust-guide');(dest/'src').mkdir(parents=True)
 example=root/'conformance/probes'/study/'user-example.rs';code=example.read_bytes();(dest/'src/main.rs').write_bytes(code)
 deps=['dive-api','dive-check','dive-sema','dive-ir','dive-config','dive-liveness','dive-trace']
 manifest='[package]\nname="'+study+'-publication-guide"\nversion="0.0.0"\nedition="2024"\npublish=false\n[dependencies]\n'
 manifest+=''.join(name+'={path='+json.dumps(str(root/'crates'/name))+'}\n'for name in deps)
 (dest/'Cargo.toml').write_text(manifest)
 runtime=dest/'runtime';runtime.mkdir();(runtime/'conformance').symlink_to(root/'conformance',target_is_directory=True)
 r=subprocess.run(['cargo','run','--offline','--manifest-path',str(dest/'Cargo.toml')],cwd=runtime,env=env,capture_output=True,timeout=600)
 (dest/'stdout').write_bytes(r.stdout);(dest/'stderr').write_bytes(r.stderr)
 assert r.returncode==0,r.stderr.decode()[-5000:]
 assert r.stdout.decode().strip()==expected,(study,r.stdout)
 guides.append({'study':study,'exact_source_sha256':sha(example),'compile_and_run_exit':r.returncode,'expected_stdout':expected})
assert all(sha(root/p)==h for p,h in inputs.items());assert source==subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
receipt={'schema_version':1,'source_commit':source,'binary_sha256':sha(binary),'source_sha256':inputs,'studies':studies,'cargo_owned_exact_guides':guides,'ordinary_periodic_certification':False,'family_certified':False}
receipt['evidence_sha256']={str(p.relative_to(out)):sha(p)for p in out.rglob('*')if p.is_file() and not p.is_symlink() and 'conformance' not in p.relative_to(out).parts}
(out/'verification.json').write_text(json.dumps(receipt,indent=2)+'\n');print('Fresh source-bound 192 CLI outcomes and two exact Cargo-owned guides passed')
