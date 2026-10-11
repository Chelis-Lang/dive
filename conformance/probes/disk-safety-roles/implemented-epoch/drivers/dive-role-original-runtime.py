import copy,hashlib,itertools,json,os,pathlib,re,subprocess,time
root=pathlib.Path('/Users/jeff/code/dive-trace-representation-work')
base=pathlib.Path('/Users/jeff/code/dive-storage-tuple-function-artifacts/seen-engine-development')
out=pathlib.Path('/Users/jeff/code/dive-storage-tuple-function-artifacts/seen-roles-current/original-runtime')
binroot=out.parent/'current-binaries'
for k,v in {'CARGO_TARGET_DIR':'/Users/jeff/code/dive-trace-representation-target','CARGO_BUILD_JOBS':'2','DIVE_CACHE_DIR':'/Users/jeff/code/dive-trace-representation-cache','CARGO_INCREMENTAL':'0'}.items():assert os.environ.get(k)==v,k
producer=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
assert producer=='7fae387c11675b6a72119cc0013dd18f4eac33cb'
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root)
cli=binroot/'dive';out.mkdir(parents=True,exist_ok=False)
rows=[];start=time.monotonic()
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def run(label,args):
 cmd=[str(cli),*args]
 r=subprocess.run(cmd,cwd=root,capture_output=True,timeout=60)
 (out/(label+'.stdout')).write_bytes(r.stdout);(out/(label+'.stderr')).write_bytes(r.stderr)
 row={'case':label,'command':cmd,'exit':r.returncode,'stdout_sha256':sha(out/(label+'.stdout')),'stderr_sha256':sha(out/(label+'.stderr'))};rows.append(row)
 (out/'running-receipt.json').write_text(json.dumps({'producer':producer,'binary_sha256':sha(cli),'elapsed_seconds':time.monotonic()-start,'rows':rows},indent=2)+'\n')
 return r.returncode,json.loads(r.stdout) if r.stdout else None
models=base/'memory-baseline/models'
def command(model,profile,store,width,chaos,inline=False,budgets=False):
 args=['check',str(models/model/(model+'.tla')),'--config',str(models/model/(model+'.cfg')),'--compat',profile,'--store',store,'--seed','7','--fp','0','--workers',str(width),'--output','json']
 if inline:args+=['--sequential']
 if chaos is not None:args+=['--chaos',str(chaos)]
 if budgets:args+=['--frontier-memory-bytes','0','--predecessor-memory-bytes','0']
 return args
def stable(doc):
 d=copy.deepcopy(doc);d.pop('elapsed_ms');return d
# Preserve original memory outputs with no new field or metadata allowlist.
for model,profile,store,width,chaos in itertools.product(sorted(p.name for p in models.iterdir()),['tlc','dive'],['fp','exact'],['inline','1','4','8'],[None,0,7]):
 label=f'{model}-{profile}-{store}-{width}-{chaos}'
 code,d=run('memory-'+label,command(model,profile,store,1 if width=='inline' else width,chaos,inline=width=='inline'))
 old=json.loads((base/'memory-baseline'/(label+'.stdout')).read_text());oldrow=next(r for r in json.loads((base/'memory-baseline/receipt.json').read_text())['rows'] if r['case']==label)
 assert code==oldrow['exit'] and stable(d)==stable(old),label
 print('memory',label,flush=True)
reference={};disk=[]
for model,profile,store,budgets in itertools.product(sorted(p.name for p in models.iterdir()),['tlc','dive'],['fp','exact'],[False,True]):
 code,d=run(f'reference-{model}-{profile}-{store}-{budgets}',command(model,profile,store,1,None,inline=True,budgets=budgets));reference[(model,profile,store,budgets)]=(code,d)
for model,profile,store,width,chaos,budgets in itertools.product(sorted(p.name for p in models.iterdir()),['tlc','dive'],['fp','exact'],[1,4,8],[None,0,7],[False,True]):
 label=f'disk-{model}-{profile}-{store}-{width}-{chaos}-{budgets}'
 code,d=run(label,command(model,profile,store,width,chaos,budgets=budgets)+['--seen-disk-entries','1000'])
 expected_code,ref=reference[(model,profile,store,budgets)];assert code==expected_code,(label,code,expected_code)
 w=d['options']['workers'];assert w['requested']==width and w['used']==1 and w['sequential'] is True and 'immutable disk read views' in w['reason']
 facts=d['seen_storage'];assert facts['first_failure'] is None and facts['deciding_event_before_failure'] is None
 assert facts['admitted_keys']==d['statistics']['distinct']==facts['before_cleanup']['physical_entries']
 assert facts['before_cleanup']['resident_key_collection_bytes']==0 and facts['after_cleanup']['cleanup_completed'] is True and facts['after_cleanup']['expected_disk_bytes']==0
 actual=stable(d);expected=stable(ref)
 # Only named opt-in metadata and requested/actual execution differ. All existing
 # concrete, storage, trace, diagnostics, counters and collision fields remain.
 actual.pop('seen_storage');actual['options'].pop('seen_disk');actual['options'].pop('workers');expected['options'].pop('workers')
 assert actual==expected,label
 disk.append(label);print(label,flush=True)
# All eight raw primary verdict/count/depth controls remain independently sourced.
primary=[]
for model in sorted(p.name for p in models.iterdir()):
 raw=(base/'primary/tlc-2026.10.06.014338'/(model+'.stdout')).read_text()
 code,d=reference[(model,'tlc','fp',False)]
 oracle_code=int((base/'primary/tlc-2026.10.06.014338'/(model+'.exit')).read_text())
 assert code==oracle_code,(model,code,oracle_code)
 stats=re.search(r'@!@!@STARTMSG 2199:0 @!@!@\n(\d+) states generated, (\d+) distinct states found, (\d+) states left on queue\.',raw)
 depth=re.search(r'The depth of the complete state graph search is (\d+)\.',raw)
 assert stats and depth,model
 assert tuple(map(int,stats.groups()))==tuple(d['statistics'][k] for k in ['generated','distinct','left_on_queue']),model
 assert int(depth[1])==d['statistics']['depth'],model
 verified=d['tlc']['verified'];assert verified is (model!='DeterministicError')
 primary.append({'case':model,'exit':code,'count_depth_equal':True,'native_error_class_verified':verified,'qualification':'The GENERAL/nested evaluation error preserves native verified:false and receives no exact class credit.' if not verified else 'Pinned verdict/count/depth contract; witness proof is separately recorded.'})
(out/'receipt.json').write_text(json.dumps({'producer':producer,'binary_sha256':sha(cli),'baseline_producer':'7dfa4f52ece9b71f8b86df61ced461b2c0655e59','memory_selected':384,'disk_selected':576,'reference_selected':64,'primary':primary,'rows':rows,'elapsed_seconds':time.monotonic()-start,'metadata_allowlist':['elapsed_ms','options.workers','options.seen_disk','seen_storage'],'family_certified':False},indent=2)+'\n')
print('CLOSED384memory576disk64references8primary',flush=True)
