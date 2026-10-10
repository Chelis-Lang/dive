import hashlib, json, pathlib, subprocess, sys
root=pathlib.Path(__file__).resolve().parents[3]
binary=pathlib.Path(sys.argv[1]).resolve()
out=pathlib.Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True)
cases=[('dive-check','tlc-probes',n) for n in ['DuplicateSuccessors','MidLevelViolation','DeadlockMidLevel','EvalErrorMidLevel','InitViolation','ConstrainedViolation','ActionConstraintNoDeadlock','ImpliedActionSeen']]
cases += [('dive-liveness','tlc-probes',n) for n in ['SymOrder','SymViolation','SymView']]
cases += [('dive-check','tlc-module',n) for n in ['TmRandom','TmRandomStartup']]
index={'schema_version':1,'parent_commit':'7ea2ce0','native_binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'controls':[]}
for f in ['crates/dive-api/src/check.rs', 'crates/dive-api/tests/predecessor_spill.rs', 'crates/dive-api/tests/symmetry_trace.rs', 'crates/dive-check/src/capture.rs', 'crates/dive-check/src/dfid.rs', 'crates/dive-check/src/engine.rs', 'crates/dive-check/src/lib.rs', 'crates/dive-check/src/options.rs', 'crates/dive-check/src/result.rs', 'crates/dive-cli/src/main.rs', 'crates/dive-store/src/error.rs', 'crates/dive-store/src/lib.rs', 'crates/dive-store/src/predecessor.rs', 'crates/dive-store/src/trace.rs']:
 index.setdefault("source_sha256",{})[f]=hashlib.sha256((root/f).read_bytes()).hexdigest()
def observable(j):
 j=json.loads(json.dumps(j));j.pop('elapsed_ms',None);j.pop('predecessor_storage',None)
 for k in ['predecessor_memory_bytes','predecessor_path_steps_limit']:j['options'].pop(k,None)
 return j
for corpus,fixture,name in cases:
 file=f'crates/{corpus}/tests/fixtures/{fixture}/specs/{name}/{name}.tla'
 for profile in ['dive','tlc']:
  for store in ['fp','exact']:
   for workers in [1,4,8]:
    for chaos in [None,13]:
     args=['check',file,'--seed','7','--fp','0','--workers',str(workers),'--store',store,'--output','json']
     if profile=='tlc':args+=['--compat=tlc']
     if chaos is not None:args+=['--chaos',str(chaos)]
     stem=f'{name}-{profile}-{store}-w{workers}-c{chaos}'
     reference=None
     for budget in [None,0,24]:
      actual=args+([] if budget is None else ['--predecessor-memory-bytes',str(budget)])
      result=subprocess.run([str(binary),*actual],cwd=root,text=True,capture_output=True)
      j=json.loads(result.stdout)
      assert j['options']['profile']==profile, (name, 'profile environment override')
      assert j['options']['workers']['chaos']==chaos, (name, 'chaos environment override')
      leaf=f'{stem}-b{budget}'
      (out/f'{leaf}.stdout.json').write_text(result.stdout)
      (out/f'{leaf}.stderr').write_text(result.stderr)
      entry={'case':name,'profile':profile,'store':store,'workers':workers,'chaos':chaos,'budget':budget,'args':actual,'exit':result.returncode,'stdout':f'{leaf}.stdout.json','stderr':f'{leaf}.stderr','stdout_sha256':hashlib.sha256(result.stdout.encode()).hexdigest()}
      if reference is None:reference=(result.returncode,observable(j))
      else:
       assert (result.returncode,observable(j))==reference, leaf
       assert j['predecessor_storage']['cleanup_completed'],leaf
       assert not j['predecessor_storage']['failures'],leaf
       entry['agrees_with_memory']=True
      index['controls'].append(entry)
print(f"{len(index['controls'])} raw runs; all bounded runs equal memory")
(out/'index.json').write_text(json.dumps(index,indent=2)+'\n')
