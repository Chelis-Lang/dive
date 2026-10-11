import pathlib,subprocess,hashlib,json,itertools,copy,re,os,time
root=pathlib.Path('/Users/jeff/code/dive-trace-representation-work');out=pathlib.Path('/Users/jeff/code/dive-storage-tuple-function-artifacts/seen-replay-current/replay-runtime');cli=out.parent/'current-binaries/dive'
producer='881b75992d0ef5dc11539018da526e5e55066027'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==producer
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root)
for k in ['CARGO_TARGET_DIR','CARGO_BUILD_JOBS','DIVE_CACHE_DIR','CARGO_INCREMENTAL']:assert os.environ.get(k)
out.mkdir(exist_ok=False);rows=[];start=time.monotonic()
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def fixture(name):
 family='errors/specs' if name.startswith('Er') else 'disk-seen'
 return root/'crates/dive-check/tests/fixtures'/family/name/(name+'.tla')
def command(name,profile,store,width,chaos,budgets,disk):
 file=fixture(name);args=['check',str(file),'--config',str(file.with_suffix('.cfg')),'--compat',profile,'--store',store,'--workers',str(width),'--seed','7','--fp','0','--output','json']
 if chaos is not None:args+=['--chaos',str(chaos)]
 if budgets:args+=['--frontier-memory-bytes','0','--predecessor-memory-bytes','0']
 if disk:args+=['--seen-disk-entries','1000']
 else:args+=['--sequential']
 return args
def run(label,args):
 r=subprocess.run([str(cli),*args],cwd=root,capture_output=True,timeout=60)
 (out/(label+'.stdout')).write_bytes(r.stdout);(out/(label+'.stderr')).write_bytes(r.stderr)
 rows.append({'case':label,'command':[str(cli),*args],'exit':r.returncode,'stdout_sha256':sha(out/(label+'.stdout')),'stderr_sha256':sha(out/(label+'.stderr'))})
 (out/'running-receipt.json').write_text(json.dumps({'producer':producer,'binary_sha256':sha(cli),'rows':rows},indent=2)+'\n')
 return r.returncode,json.loads(r.stdout)
def stable(doc,disk):
 d=copy.deepcopy(doc);d.pop('elapsed_ms');d['options'].pop('workers')
 if disk:d.pop('seen_storage');d['options'].pop('seen_disk')
 return d
pure=['ReplayMembership','ReplayNew','ReplayLaterViolation','ErInvDivZero','ErInvReplayViolation','ErInvReplayError']
prints=['ReplayPrint','ReplayPrintInit','ReplayPrintEmpty','ReplayPrintInvariant','ReplayPrintWrapper','ReplayPrintAlias','ReplayPrintReplayOnly']
references={};primary=[]
for name,profile,store,budgets in itertools.product(pure+prints,['tlc','dive'],['fp','exact'],[False,True]):
 code,d=run(f'memory-{name}-{profile}-{store}-{budgets}',command(name,profile,store,1,None,budgets,False));references[(name,profile,store,budgets)]=(code,d)
 if profile=='tlc' and store=='fp' and not budgets:
  raw=(root/'conformance/probes/fallible-invariant-replay/primary'/(name+'.stdout')).read_text();exit_=int((root/'conformance/probes/fallible-invariant-replay/primary'/(name+'.exit')).read_text())
  counts=re.search(r'(\d+) states generated, (\d+) distinct states found, (\d+) states left on queue\.',raw);depth=re.search(r'The depth of the complete state graph search is (\d+)\.',raw)
  assert code==exit_ and counts and depth,name
  assert tuple(map(int,counts.groups()))==tuple(d['statistics'][k] for k in ['generated','distinct','left_on_queue']) and int(depth[1])==d['statistics']['depth'],name
  primary.append({'case':name,'reference_exit':exit_,'count_depth_equal':True,'native_error_class_verified':d['tlc']['verified'],'output_contract':'Print replay is explicitly refused on disk; no output/class agreement claim' if name in prints else 'Pure positioned key/count continuation; unknown native class remains unverified'})
for name,profile,store,width,chaos,budgets in itertools.product(pure+prints,['tlc','dive'],['fp','exact'],[1,4,8],[None,0,7],[False,True]):
 label=f'disk-{name}-{profile}-{store}-{width}-{chaos}-{budgets}'
 code,d=run(label,command(name,profile,store,width,chaos,budgets,True));refcode,ref=references[(name,profile,store,budgets)]
 w=d['options']['workers'];assert w['requested']==width and w['used']==1 and w['sequential'] is True,label
 seen=d['seen_storage'];assert seen['first_failure'] is None and seen['after_cleanup']['cleanup_completed'] is True,label
 if name in prints:
  assert code==3 and d['verdict']=='unsupported' and 'Print/PrintT' in d['message'],label
  assert d['tlc'] is None and d['counterexample'] is None,label
  assert d['statistics']==ref['statistics'],label
  r=seen['replay'];assert r['unproved_print_output'] is True and 'EvaluationError' in r['original_error'] and 'Unsupported' in r['halted_by'],label
  assert r['generated']==2 and r['admitted_keys']==2 and r['queued']==2 and seen['admitted_keys']==4,label
 else:
  assert code==refcode and stable(d,True)==stable(ref,False),label
  if seen['replay'] is not None:assert seen['replay']['unproved_print_output'] is False,label
 print(label,flush=True)
(out/'receipt.json').write_text(json.dumps({'producer':producer,'binary_sha256':sha(cli),'rows':rows,'primary':primary,'reference_selected':104,'pure_disk_selected':432,'print_refusal_selected':504,'metadata_allowlist':['elapsed_ms','options.workers','options.seen_disk','seen_storage'],'elapsed_seconds':time.monotonic()-start,'family_certified':False},indent=2)+'\n')
print('CLOSED104 references432pure504Printrefusals',flush=True)
