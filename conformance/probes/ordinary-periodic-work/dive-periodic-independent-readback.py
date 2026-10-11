from pathlib import Path
import json,hashlib,math
base=Path('/tmp/dive-ordinary-schedule-primary-candidate2')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
record=json.loads((base/'capture.json').read_text());assert len(record['rows'])==296
checks=[]
for i,row in enumerate(record['rows']):
 c=row['case'];d=base/f'case-{i:03}';out=(d/'stdout').read_text();err=(d/'stderr').read_text();assert sha(d/'stdout')==row['stdout_sha256'];assert sha(d/'stderr')==row['stderr_sha256'];assert row['exit']==0 and not row['timeout'],(i,row)
 r=float(c['ratio']);events=['start'];force=c['force'];result=0;periodic_called=False
 if c['liveness'] and not r>0.2:events.append('growth')
 skip=(not c['liveness'] or r>0.2 or not c['growth']) and not force and not c['checkpoint'] and c['periodic']=='none'
 if skip:events.append('ratio-update:0')
 else:
  events.append('suspend')
  if c['suspended']:
   if c['liveness'] and (r<0.2 or force):
    events.append('check-force:'+str(force).lower())
    if c['check_code']!=0:result=c['check_code']
    else:force=False;events.append('ratio-update:elapsed')
   elif r>0.2:events.append('ratio-update:0')
   if result==0:
    if c['periodic']!='none':periodic_called=True;events.append('periodic')
    if c['periodic']=='false':result=2104
    elif c['checkpoint']:events.append('checkpoint')
    else:events.append('resume')
 events.extend(['result:'+str(result),'force-retained:'+str(force).lower()])
 assert [line[6:] for line in out.splitlines() if line.startswith('EVENT\t')]==row['events']
 observed=[line[6:] if line.startswith('EVENT\t') else 'periodic' for line in out.splitlines() if line.startswith('EVENT\t') or line=='\"DIVE_PERIODIC\"']
 for n,event in enumerate(events):
  if event=='ratio-update:elapsed':
   actual=observed[n];assert actual.startswith('ratio-update:') and int(actual.split(':')[1])>=0,(i,actual);observed[n]=event
 assert observed==events,(i,observed,events)
 assert err.count('"DIVE_PERIODIC"')==1 and out.count('"DIVE_PERIODIC"')==int(periodic_called),(i,periodic_called,err)
 checks.append({'row':i,'expected_ordered_events':events,'ordered_callbacks_exact':True,'force_retention_exact':True,'periodic_effect_count_exact':True,'message_code':result,'clock_normalization':'Only the actual nonnegative elapsed ratio-update argument is abstracted; no clock-ratio math claim.'})
files={str(p.relative_to(base)):sha(p) for p in sorted(base.rglob('*')) if p.is_file() and p.name!='independent-readback.json'}
receipt={'selected':296,'fully_checked':len(checks),'disagreements':0,'family_certified':False,'scope':record['scope'],'checks':checks,'artifact_sha256':files}
(base/'independent-readback.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps({'selected':296,'fully_checked':len(checks),'artifacts':len(files),'disagreements':0}))
