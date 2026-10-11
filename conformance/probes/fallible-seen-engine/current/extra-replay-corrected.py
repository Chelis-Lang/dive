import itertools,json,subprocess,os,hashlib,pathlib
root=pathlib.Path('/Users/jeff/code/dive-trace-representation-work');base=pathlib.Path('/Users/jeff/code/dive-storage-tuple-function-artifacts/seen-engine-development')
out=base.parent/'seen-engine-current/extra';cli=out.parent/'current-binaries/dive'
assert os.environ['CARGO_TARGET_DIR']=='/Users/jeff/code/dive-trace-representation-target' and os.environ['CARGO_BUILD_JOBS']=='2' and os.environ['DIVE_CACHE_DIR']=='/Users/jeff/code/dive-trace-representation-cache'
producer=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip();assert producer=='da0f857161c57b52e8cb1d6c18ef0cebb0021ea4'
out.mkdir(parents=True,exist_ok=False);rows=[]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def run(label,args):
 command=[str(cli),*args];r=subprocess.run(command,cwd=root,capture_output=True,timeout=60)
 for channel in ['stdout','stderr']:(out/(label+'.'+channel)).write_bytes(getattr(r,channel))
 row={'case':label,'command':command,'exit':r.returncode,'stdout_sha256':sha(out/(label+'.stdout')),'stderr_sha256':sha(out/(label+'.stderr'))};rows.append(row)
 (out/'running-receipt.json').write_text(json.dumps({'producer':producer,'binary_sha256':sha(cli),'rows':rows},indent=2)+'\n')
 return r.returncode,json.loads(r.stdout) if r.stdout else None
repro=root/'crates/dive-check/tests/fixtures/disk-seen/ReplayMembership/ReplayMembership.tla'
for profile,store in itertools.product(['tlc','dive'],['fp','exact']):
 args=['check',str(repro),'--compat',profile,'--seed','7','--fp','0','--store',store,'--workers','1','--sequential','--output','json']
 code,d=run(f'replay-memory-{profile}-{store}',args)
 assert code==(76 if profile=='tlc' else 13) and d['verdict']=='evaluation-error'
 assert [d['statistics'][k] for k in ['generated','distinct','left_on_queue']]==[3,3,1]
 assert d['counterexample']['length']==2 and d['counterexample']['trace'] is not None
for profile,store,width,chaos in itertools.product(['tlc','dive'],['fp','exact'],[1,4,8],[None,0,7]):
 args=['check',str(repro),'--compat',profile,'--seed','7','--fp','0','--store',store,'--workers',str(width),'--seen-disk-entries','1000','--output','json']
 if chaos is not None:args+=['--chaos',str(chaos)]
 code,d=run(f'replay-disk-{profile}-{store}-{width}-{chaos}',args)
 assert code==3 and d['verdict']=='unsupported'
 assert [d['statistics'][k] for k in ['generated','distinct','left_on_queue']]==[3,3,1]
 assert d['counterexample']['length']==2 and d['counterexample']['trace'] is None
 assert [s['values']['x'] for s in d['counterexample']['states']]==[0,1]
 assert any('EvaluationError' in n and 'Invariant' in n and 'stopping prefix' in n for n in d['notes'])
 assert d['options']['workers']['requested']==width and d['options']['workers']['used']==1
 facts=d['seen_storage'];assert facts['first_failure'] is None and facts['admitted_keys']==3 and facts['after_cleanup']['cleanup_completed'] is True
# Public guide is four independently captured commands, not interpreted Markdown.
for profile,store in itertools.product(['tlc','dive'],['fp','exact']):
 code,d=run(f'guide-{profile}-{store}',['check','crates/dive-api/examples/seen-safety/Counter.tla','--compat',profile,'--seed','7','--fp','0','--store',store,'--workers','8','--seen-disk-entries','1000','--output','json'])
 assert code==0 and d['verdict']=='success' and d['options']['workers']['used']==1
 assert d['seen_storage']['after_cleanup']['cleanup_completed'] is True
# Small admitted limit is resource evidence, never fake absence or success.
file=root/'crates/dive-check/tests/fixtures/disk-seen/DuplicateNext/DuplicateNext.tla'
for profile,store,width in itertools.product(['tlc','dive'],['fp','exact'],[1,4,8]):
 code,d=run(f'entry-bound-{profile}-{store}-{width}',['check',str(file),'--compat',profile,'--seed','7','--fp','0','--store',store,'--workers',str(width),'--seen-disk-entries','1','--output','json'])
 assert code==3 and d['verdict']=='resource-limit' and d['statistics']['distinct']==1
 assert d['seen_storage']['first_failure']['phase']=='Insertion'
 assert d['seen_storage']['after_cleanup']['cleanup_completed'] is True
# Configured roles are rejected before startup Print/ASSUME effects.
models=out/'models';models.mkdir();f=models/'Role.tla';f.write_text("---- MODULE Role ----\nEXTENDS TLC\nVARIABLE x\nUnused == PrintT(\"must not bind\")\nASSUME PrintT(\"must not assume\")\nInit == x=0\nNext == x'=x\nSpec == Init /\\ [][Next]_x\nP == []<>(x=0)\n====\n")
for role in ['PROPERTY P','SPECIFICATION Spec']:
 cfg=models/(role.split()[0]+'.cfg');cfg.write_text(('INIT Init\nNEXT Next\n' if role.startswith('PROPERTY') else '')+role+'\nCHECK_DEADLOCK FALSE\n')
 for profile in ['tlc','dive']:
  code,d=run(f'role-{profile}-{role.split()[0]}',['check',str(f),'--config',str(cfg),'--compat',profile,'--seed','7','--fp','0','--seen-disk-entries','1000','--output','json'])
  assert code==3 and d['verdict']=='unsupported' and d['output']==[] and d['statistics'] is None and d.get('seen_storage') is None
(out/'receipt.json').write_text(json.dumps({'producer':producer,'binary_sha256':sha(cli),'replay_disk_selected':36,'replay_memory_selected':4,'rows':rows,'family_certified':False},indent=2)+'\n');print('Closed40 replay controls,4CLI guide controls,12tiny limits,4pre-effect role refusals',flush=True)
