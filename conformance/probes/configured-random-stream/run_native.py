"""Compare an immutable native executable with the separate clean primary rows."""
from pathlib import Path
import copy,hashlib,json,os,subprocess,sys
root,binary,primary,out=map(Path,sys.argv[1:5]);producer=sys.argv[5]
out.mkdir(parents=True,exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
rows=json.loads((primary/'index.json').read_text())['cases']
env=os.environ.copy()
for key in ('DIVE_COMPAT','DIVE_CHAOS'):env.pop(key,None)
index={'producer_commit':producer,'binary_sha256':sha(binary),'primary_index_sha256':sha(primary/'index.json'),'scope':'Bound ordinary RandomElement controls; no transformed, capture, other-family or recovery parity claim. First-successor replay refusals and startup-prefix proof refusals stay explicit.','cases':[],'validation':[]}
for row in rows:
 case,seed=row['case'],row['seed'];d=out/case/f'seed-{seed}';d.mkdir(parents=True)
 for filename in ('Stream.tla','Stream.cfg'):(d/filename).write_bytes((primary/case/filename).read_bytes())
 for compat in [False,True]:
  for store in ['fp','exact']:
   baseline=None
   for width in [1,4,8]:
    for chaos in [None,19]:
     ident=f'{"tlc" if compat else "dive"}-{store}-w{width}-c{chaos}'
     args=[str(binary),'check',str(d/'Stream.tla'),'--seed',str(seed),'--fp','0','--workers',str(width),'--store',store,'--output','json']
     if compat:args+=['--compat=tlc']
     if chaos is not None:args+=['--chaos',str(chaos)]
     r=subprocess.run(args,capture_output=True,env=env,timeout=90)
     (d/(ident+'.stdout')).write_bytes(r.stdout);(d/(ident+'.stderr')).write_bytes(r.stderr)
     j=json.loads(r.stdout);w=j['options']['workers'];assert w['requested']==width and w['used']==1 and w['sequential'],(case,seed,ident,j)
     assert w['chaos']==chaos and 'RandomElement' in w['reason']
     if compat:
      assert [j['statistics'][k] for k in ('generated','distinct','left_on_queue')]==row['stats'],(case,seed,ident,j,row)
      assert j['statistics']['depth']==row['depth']
      if row['exit']==0:assert j['verdict']=='success' and r.returncode==0,(case,seed,ident,j)
      elif row['trace_states'] and len(row['trace_states']) < row['depth']:
       assert j['verdict']=='unsupported' and r.returncode==3 and j['counterexample'] is None
       assert j['message']=='TLC initial-predecessor random counterexample replay is not proved'
      else:
       assert j['verdict']=='invariant-violation' and r.returncode==12,(case,seed,ident,j)
       actual=[] if j['counterexample'] is None else [st['values'] for st in j['counterexample']['states']]
       assert actual==row['trace_states'],(case,seed,ident,actual,row)
     else:
      assert j['verdict'] in ['success','invariant-violation','unsupported'],(case,seed,ident,j)
      if j['verdict']=='unsupported':assert j['message']=='TLC initial-predecessor random counterexample replay is not proved' and j['counterexample'] is None
     normalized=copy.deepcopy(j);normalized['elapsed_ms']=0
     for key in ('requested','chaos'):normalized['options']['workers'][key]=None
     if baseline is None:baseline=(normalized,r.stderr)
     else:assert baseline==(normalized,r.stderr),(case,seed,ident,baseline,normalized)
     if width==1 and chaos is None and j['counterexample'] is not None:
      original=j['counterexample']['trace'];assert original is not None
      for label,trace in [('original',original),('invalid-init',copy.deepcopy(original))]:
       if label=='invalid-init':trace['states'][0]['values']['n']=999
       path=d/(ident+'.'+label+'.trace.json');path.write_text(json.dumps(trace))
       va=[str(binary),'validate-trace',str(d/'Stream.tla'),'--trace',str(path),'--seed',str(seed),'--fp','0','--output','json']
       if compat:va+=['--compat=tlc']
       v=subprocess.run(va,capture_output=True,env=env,timeout=90)
       (d/(ident+'.'+label+'.stdout')).write_bytes(v.stdout);(d/(ident+'.'+label+'.stderr')).write_bytes(v.stderr)
       result=json.loads(v.stdout)['result']
       if label=='original':assert result in ['valid','unsupported'],(case,seed,ident,label,result,v.stdout)
       else:assert result in ['invalid','unsupported'],(case,seed,ident,label,result,v.stdout)
       index['validation'].append({'case':case,'seed':seed,'setting':ident,'control':label,'result':result,'exit':v.returncode})
     index['cases'].append({'case':case,'seed':seed,'setting':ident,'exit':r.returncode,'verdict':j['verdict'],'trace_validation':None if j['counterexample'] is None else j['counterexample'].get('validation')})
index['files']={str(p.relative_to(out)):sha(p) for p in out.rglob('*') if p.is_file()}
index['source_sha256']={str(p.relative_to(root)):sha(p) for p in root.glob('crates/*/src/**/*.rs')}
for p in [root/'Cargo.toml',root/'Cargo.lock',root/'rust-toolchain.toml']:index['source_sha256'][str(p.relative_to(root))]=sha(p)
(out/'index.json').write_text(json.dumps(index,indent=2)+'\n')
print(f'Passed {len(index["cases"])} checker and {len(index["validation"])} standalone validation controls')
