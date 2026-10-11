import pathlib,os,subprocess,json,hashlib,copy,itertools,time,re
root=pathlib.Path('/Users/jeff/code/dive-trace-representation-work');base=pathlib.Path('/Users/jeff/code/dive-storage-tuple-function-artifacts/seen-roles-current');out=base/'replay-property-current';producer='7fae387c11675b6a72119cc0013dd18f4eac33cb';cli=base/'current-binaries/dive'
for k,v in {'CARGO_TARGET_DIR':'/Users/jeff/code/dive-trace-representation-target','CARGO_BUILD_JOBS':'2','CARGO_INCREMENTAL':'0','DIVE_CACHE_DIR':'/Users/jeff/code/dive-trace-representation-cache'}.items():assert os.environ.get(k)==v,k
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==producer and not subprocess.check_output(['git','status','--porcelain'],cwd=root)
out.mkdir(exist_ok=False);rows=[];refs={};start=time.monotonic();sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();file=root/'crates/dive-check/tests/fixtures/errors/specs/ErInvReplayProp/ErInvReplayProp.tla'
def stable(d,disk=False):
 d=copy.deepcopy(d);d.pop('elapsed_ms');d['options'].pop('workers')
 if disk:d.pop('seen_storage');d['options'].pop('seen_disk')
 return d
for profile,store,budget in itertools.product(['tlc','dive'],['fp','exact'],[False,True]):
 args=['check',str(file),'--config',str(file.with_suffix('.cfg')),'--compat',profile,'--store',store,'--seed','7','--fp','0','--output','json']
 if budget:args+=['--frontier-memory-bytes','0','--predecessor-memory-bytes','0']
 for width,chaos in [(None,None),*itertools.product([1,4,8],[None,0,7])]:
  label=f'{profile}-{store}-{budget}-{width}-{chaos}';cmd=[str(cli),*args,'--workers',str(width or 1)]
  if width is None:cmd+=['--sequential']
  else:cmd+=['--seen-disk-entries','1000']
  if chaos is not None:cmd+=['--chaos',str(chaos)]
  r=subprocess.run(cmd,cwd=root,capture_output=True,timeout=60);(out/(label+'.stdout')).write_bytes(r.stdout);(out/(label+'.stderr')).write_bytes(r.stderr);d=json.loads(r.stdout)
  assert r.returncode==(76 if profile=='tlc' else 13) and d['verdict']=='evaluation-error',label
  assert d['tlc']['verified'] is True and d['tlc']['code']==2111 and d['tlc']['exit_status']==76,label
  stats=d['statistics'];assert tuple(stats[k] for k in ['generated','distinct','left_on_queue','depth'])==(4,4,1,3),label
  assert d['counterexample']['length']==3 and d['output']==[],label
  if width is None:refs[(profile,store,budget)]=d
  else:
   assert stable(d,True)==stable(refs[(profile,store,budget)]),label
   seen=d['seen_storage'];assert seen['first_failure'] is None and seen['deciding_event_before_failure'] is None and seen['after_cleanup']['cleanup_completed'] is True
   assert seen['admitted_keys']==4 and seen['before_cleanup']['physical_entries']==4 and seen['before_cleanup']['resident_key_collection_bytes']==0
   assert d['options']['workers']['requested']==width and d['options']['workers']['used']==1 and d['options']['workers']['sequential'] is True
  rows.append({'case':label,'command':cmd,'exit':r.returncode,'stdout_sha256':sha(out/(label+'.stdout')),'stderr_sha256':sha(out/(label+'.stderr')),'diagnostic_word_length':3,'certificate_credit':False})
  (out/'running-receipt.json').write_text(json.dumps({'producer':producer,'binary_sha256':sha(cli),'rows':rows},indent=2)+'\n')
assert len(rows)==80
(out/'receipt.json').write_text(json.dumps({'producer':producer,'binary_sha256':sha(cli),'selection':'ErInvReplayProp supplemental current-release error contract;8 resident refs+72diskrows','selected':80,'source_sha256':sha(file),'config_sha256':sha(file.with_suffix('.cfg')),'rows':rows,'metadata_allowlist':['elapsed_ms','options.workers','options.seen_disk','seen_storage'],'elapsed_seconds':time.monotonic()-start,'certificate_credit':False,'family_certified':False},indent=2)+'\n');print('CLOSED80 supplemental diagnostic replay rows')
