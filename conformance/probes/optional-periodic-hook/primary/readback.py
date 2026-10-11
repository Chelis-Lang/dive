from pathlib import Path
import hashlib,json
b=Path(__file__).resolve().parent
r=json.loads((b/'capture.json').read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
for f,h in r['source_sha256'].items():assert sha(b/f)==h,f
rows={x['id']:x for x in r['rows']};assert len(rows)==26
count=0
for x in rows.values():
 assert not x['timeout'],x['id']
 for f,h in x['artifact_sha256'].items():assert sha(b/x['directory']/f)==h,(x['id'],f);count+=1
 def event(s):assert s in x['events'],(x['id'],s,x['events'])
 if x['id'].startswith('absent-'):
  assert x['exit']==0;event('configured-name:');event('body-present:false');event('snapshot-class:null')
 if x['id'] in ['missing','arity','constant-role','qualified']:
  assert x['exit']==2
  event({'missing':'exception-code:2229','arity':'exception-code:2228','constant-role':'exception-class:java.lang.ClassCastException','qualified':'exception-code:5006'}[x['id']])
 if x['id'] in ['present-true','present-false','alias','substitution']:
  assert x['exit']==0;event('definition-arity:0');event('body-identical:true')
 if x['id'] in ['state','action','temporal']:
  event('body-level:'+str({'state':1,'action':2,'temporal':3}[x['id']]))
 if x['id'] in ['service-absent','service-unused-false']:
  assert x['exit']==0;event('body-present:false');event('actual-stop-observed:false');event('ratio-update:0');event('queue-at-service-return:1');event('service-result:0')
 if x['id']=='service-absent-forced':
  assert x['exit']==0;event('body-present:false');event('actual-stop-observed:true');event('force-retained:true');event('service-result:0');event('stop-after-service:false')
 if x['id']=='service-true':
  assert x['exit']==0;event('actual-stop-observed:true');event('service-result:0');event('stop-after-service:false')
 if x['id'] in ['service-false','service-integer','service-error','service-state']:
  event('actual-stop-observed:true');event('stop-after-service:true');event('queue-at-service-return:1');event('queue-helper-return:EOF')
  event({'service-false':'service-result:2104','service-integer':'service-exception-code:1000','service-error':'service-exception-code:2179','service-state':'service-exception-code:2280'}[x['id']])
  assert x['exit']==(0 if x['id']=='service-false' else 2)
 if x['id']=='service-failed-init':
  assert x['exit']==2;event('exception-code:2179');assert 'service-enter' not in x['events']
 if x['id'].startswith('service-') and x['id']!='service-failed-init':
  event('observer-cleanup-complete')
  ev=x['events'];assert ev.index('service-enter')<ev.index('observer-cleanup-start')<ev.index('observer-cleanup-complete')
# Lexical no-hook source/config are unmodified absence, not a substituted TRUE role.
for id in ['absent-no-definition','service-absent','service-absent-forced','service-unused-false']:
 d=b/rows[id]['directory'];assert '_PERIODIC' not in (d/'Snapshot.cfg').read_text()
assert 'STARTUP_MARKER' in (b/rows['absent-startup']['directory']/'stderr').read_text()
result=dict(status='pass',selected_rows=26,closed_artifact_hashes=count,capture_sha256=sha(b/'capture.json'),ordinary_schedule_certified=False,model_proof=False,qualifications=['The queue helper uses actual sDequeue but is not TLCWorker or source expansion.','Force is deliberately requested only in the separately labelled no-hook forced case.','Thread event interleaving is not a stable order/count contract.','No native producer or ordinary model verdict was executed.'])
(b/'independent-readback.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
