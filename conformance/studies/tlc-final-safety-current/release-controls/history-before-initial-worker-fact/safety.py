"""Preserve every selected safety release row, including certified scope refusals."""
import hashlib,json,subprocess,time,sys
from pathlib import Path
root=Path(sys.argv[1]).resolve();binary=Path(sys.argv[2]).resolve();out=Path(sys.argv[3]);out.mkdir(parents=True,exist_ok=True)
primary=json.loads((root/'conformance/probes/tlc-final-safety/primary.json').read_text());rows=[]
for reference in primary['rows']:
 name=reference['model'];model=root/'conformance/probes/tlc-final-safety/models'/name
 for width in [1,3,4,8]:
  args=['check',str(model/(name+'.tla')),'--config',str(model/(name+'.cfg')),'--compat=tlc','--lncheck','final','--lasso','tlc','--seed','0','--fp','0','--workers',str(width),'--output','json']
  started=time.monotonic();p=subprocess.run([str(binary),*args],capture_output=True,timeout=120);stem=name+'.workers-'+str(width)
  (out/(stem+'.stdout')).write_bytes(p.stdout);(out/(stem+'.stderr')).write_bytes(p.stderr);(out/(stem+'.exit')).write_text(str(p.returncode)+'\n')
  v=json.loads(p.stdout);assert v['schema_version']==2
  if name.startswith('SafetyConstrained'):
   assert p.returncode==3 and v['verdict']=='unsupported';assert v['options']['workers']['used']==0
   classification='named-refusal'
  elif name=='SafetyDeadlock' and v['verdict']=='unsupported':
   assert p.returncode==3 and v['execution']['stage']=='independent-validation';classification='independent-replay-refusal'
  else:
   assert p.returncode==reference['exit'] and v['tlc']['code']==reference['code'],(name,width,v)
   assert v['options']['workers']['used']==width
   assert v['execution']['route']=='bounded-safety-only' and v['execution']['product_constructed'] is False and v['execution']['temporal_certificate'] is False
   assert v['execution']['capture_completed']==(reference['exit']==0)
   if reference['statistics'] is not None:
    for field,value in reference['statistics'].items():assert v['statistics'][field]==value,(name,width,field,v)
    assert v['statistics']['depth']==reference['depth']
   if reference['exit']!=0:assert v['counterexample']['validation']=='valid' and v['counterexample']['ending']=='finite'
   classification='supported-declared-fields'
  rows.append({'model':name,'workers':width,'classification':classification,'exit':p.returncode,'verdict':v['verdict'],'elapsed_seconds':time.monotonic()-started})
files={str(p.relative_to(out)):hashlib.sha256(p.read_bytes()).hexdigest() for p in out.iterdir() if p.is_file()}
(out/'outcomes.json').write_text(json.dumps({'selection':primary['selection'],'widths':[1,3,4,8],'rows':rows,'raw_sha256':files,'scope':'Declared code/exit/count fields and private independent replay; initial inline states are not compared to numbered-only TLC trace length. Refusals earn no agreement credit.'},indent=2)+'\n')
print('All36 selected release safety rows retained and checked; no temporal product credit')
