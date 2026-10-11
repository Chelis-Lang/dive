from pathlib import Path
import hashlib,json,subprocess,sys,copy,os
root=Path(sys.argv[1]); binary=Path(sys.argv[2]); out=Path(sys.argv[3]);out.mkdir(parents=True,exist_ok=False)
sha=lambda b:hashlib.sha256(b).hexdigest()
env=os.environ.copy()
for key in ('DIVE_COMPAT','DIVE_CHAOS'):env.pop(key,None)
cases={
'direct':('ASSUME PrintT("must not print")\nStartup == PrintT("must not print")','RandomElement(1..100)','VIEW V\n',False),
'configured':('CONSTANT C(_)\nASSUME PrintT("must not print")\nStartup == PrintT("must not print")','C(1..100)','CONSTANT C <- RandomElement\nVIEW V\n',False),
'pure':('','n','VIEW V\n',True),
'random':('','RandomElement(1..100)','',True)}
index={'producer_commit':'4ecaa79d33a47e54126feaf171a854a2c77b641b','binary_sha256':sha(binary.read_bytes()),'scope':'Fresh VIEW/RNG admission and genuine pure VIEW/untransformed source-call RNG positives; not configured-only untransformed stream equivalence or transformed replay.','cases':[],'source_sha256':{},'files':{}}
for name,(defs,nextval,cfg,positive) in cases.items():
 d=out/name;d.mkdir(); source=d/'Guard.tla'; config=d/'Guard.cfg'
 source.write_text(f'---- MODULE Guard ----\nEXTENDS Naturals,TLC\nVARIABLES n,x\n{defs}\nInit == n=0 /\\ x=0\nNext == n\'=n+1 /\\ x\'={nextval}\nInv == n<3\nV == n\n====\n'); config.write_text('INIT Init\nNEXT Next\nINVARIANT Inv\n'+cfg)
 for compat in [False,True]:
  for store in ['fp','exact']:
   baseline=None
   for width in [1,4,8]:
    for chaos in [None,19]:
     ident=f'{"tlc" if compat else "dive"}-{store}-w{width}-c{chaos}'
     args=[str(binary),'check',str(source),'--seed','7','--fp','0','--workers',str(width),'--store',store,'--output','json']
     if compat:args+=['--compat=tlc']
     if chaos is not None:args+=['--chaos',str(chaos)]
     run=subprocess.run(args,capture_output=True,timeout=90,env=env)
     (d/(ident+'.stdout')).write_bytes(run.stdout);(d/(ident+'.stderr')).write_bytes(run.stderr)
     j=json.loads(run.stdout);assert j['options']['profile']==('tlc' if compat else 'dive');workers=j['options']['workers'];assert workers['requested']==width and workers['chaos']==chaos,(ident,workers)
     if not positive:
      assert run.returncode==3 and j['verdict']=='unsupported',(ident,run.returncode,j)
      assert workers['used']==0 and j['statistics'] is None and j['counterexample'] is None and j['collision'] is None
      assert j['output']==[] and j['warnings']==[] and b'must not print' not in run.stderr
      assert 'randomness under VIEW needs transformed-fingerprint seeding and independent replay' in j['message']
     else:
      assert run.returncode==(12 if compat else 10) and j['verdict']=='invariant-violated',(ident,run.returncode,j)
      assert tuple(j['statistics'][k] for k in ('generated','distinct','depth'))==(4,4,4)
      assert j['property']=='Inv' and len(j['counterexample']['states'])==4
      assert j['output']==[] and j['warnings']==[]
      if name=='random':
       assert workers['used']==1 and workers['sequential'] and 'RandomElement' in workers['reason']
       if compat:assert [state['values']['x'] for state in j['counterexample']['states']]==[0,80,94,100]
      else:assert workers['used']==width and not workers['sequential'] and workers['reason'] is None
      normalized=copy.deepcopy(j);del normalized['elapsed_ms']
      for key in ('requested','used','chaos'):del normalized['options']['workers'][key]
      if baseline is None:baseline=(normalized,run.stderr)
      else:assert baseline==(normalized,run.stderr),(name,ident,baseline,normalized)
     if positive:
      trace=j['counterexample']['trace']; valid=d/(ident+'.trace.json');valid.write_text(json.dumps(trace))
      invalid=copy.deepcopy(trace); invalid['states'][0]['values']['x']=1
      broken=d/(ident+'.broken.json');broken.write_text(json.dumps(invalid))
      for label,path,expected in [('valid',valid,0),('invalid-init',broken,1)]:
       vargs=[str(binary),'validate-trace',str(source),'--trace',str(path),'--seed','7','--fp','0','--output','json']
       if compat:vargs+=['--compat=tlc']
       vrun=subprocess.run(vargs,capture_output=True,timeout=90,env=env)
       (d/(ident+'.'+label+'.stdout')).write_bytes(vrun.stdout);(d/(ident+'.'+label+'.stderr')).write_bytes(vrun.stderr)
       assert vrun.returncode==expected,(name,ident,label,vrun.returncode,vrun.stdout)
       assert json.loads(vrun.stdout)['result']==('valid' if expected==0 else 'invalid')
     index['cases'].append({'case':name,'setting':ident,'exit_code':run.returncode,'args':[a.replace(str(binary),'<immutable-current-native>').replace(str(out),'<current-controls>') for a in args]})
for p in out.rglob('*'):
 if p.is_file():index['files'][str(p.relative_to(out))]=sha(p.read_bytes())
for p in root.glob('crates/*/src/**/*.rs'):index['source_sha256'][str(p.relative_to(root))]=sha(p.read_bytes())
for p in [root/'Cargo.toml',root/'Cargo.lock',root/'rust-toolchain.toml']:index['source_sha256'][str(p.relative_to(root))]=sha(p.read_bytes())
(out/'index.json').write_text(json.dumps(index,indent=2)+'\n')
print(f'passed {len(index["cases"])} checker controls and96 independent witness controls')
