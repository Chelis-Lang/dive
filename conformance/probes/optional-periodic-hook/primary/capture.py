from pathlib import Path
import hashlib,json,subprocess,time
b=Path(__file__).resolve().parent
cache=Path('/Users/jeff/code/dive-periodic-snapshot-cache')
jar=cache/'sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar'
java=cache/'jre/190480874ccceb358cbc840393207f77ac3e63a4c5f8129d0e23e9518b96ad05/jdk-17.0.20.1+1-jre/Contents/Home/bin/java'
javac=Path('/tmp/claude-501/-Users-jeff-code-dive/7ef721df-a0bf-48ac-94b6-f7c0c8e766e7/scratchpad/jdk/jdk-17.0.20.1+1/Contents/Home/bin/javac')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
classes=b/'classes';classes.mkdir()
cmd=[str(javac),'-cp',str(jar),'-d',str(classes),str(b/'SnapshotProbe.java'),str(b/'OptionalServiceProbe.java')]
r=subprocess.run(cmd,capture_output=True);(b/'compile.stdout').write_bytes(r.stdout);(b/'compile.stderr').write_bytes(r.stderr)
(b/'compile.json').write_text(json.dumps({'command':cmd,'exit':r.returncode},indent=2)+'\n')
assert r.returncode==0,r.stderr.decode()
head='---- MODULE Snapshot ----\nEXTENDS Integers,TLC\nCONSTANT Token\nVARIABLE x\nInit == x=0\nNext == x\'=1-x\n'
cfg='CONSTANT Token=Atom\nINIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n'
sel=[]
def add(id,defs='',role=None,extra='',lib=None,service=False,force=False):
 sel.append(dict(id=id,source=head+defs+'====\n',config=cfg+extra+('_PERIODIC '+role+'\n' if role else ''),lib=lib,service=service,force=force))
add('absent-no-definition');add('absent-unused-false','Hook == FALSE\n');add('absent-unused-arg','Hook(v) == v\n')
add('present-true','Hook == TRUE\n','Hook');add('present-false','Hook == FALSE\n','Hook');add('alias','Base == FALSE\nHook == Base\n','Hook')
add('state','Hook == x=0\n','Hook');add('action',"Hook == x'=0\n",'Hook');add('temporal','Hook == <>(x=0)\n','Hook')
add('integer','Hook == 1\n','Hook');add('error','Hook == 1 \\div 0\n','Hook')
add('missing',role='Missing');add('arity','Hook(v) == v\n','Hook');add('constant-role',role='Token')
add('qualified','S == INSTANCE Helper\n','S!Probe',lib='---- MODULE Helper ----\nProbe == FALSE\n====\n')
add('substitution','Hook == TRUE\nReplacement == FALSE\n','Hook',extra='CONSTANT Hook <- Replacement\n')
add('absent-startup','Unused == PrintT("STARTUP_MARKER")\n')
for id,defs,role,force in [('absent', '', None,False),('absent-forced','',None,True),('unused-false','Hook == FALSE\n',None,False),('true','Hook == TRUE\n','Hook',False),('false','Hook == FALSE\n','Hook',False),('integer','Hook == 1\n','Hook',False),('error','Hook == 1 \\div 0\n','Hook',False),('state','Hook == x=0\n','Hook',False),('failed-init','Hook == TRUE\n','Hook',False)]:
 add('service-'+id,defs,role,service=True,force=force)
 if id=='failed-init':sel[-1]['source']=sel[-1]['source'].replace('Init == x=0','Init == x = (1 \\div 0)')
(b/'selection.json').write_text(json.dumps(sel,indent=2)+'\n')
rows=[]
for i,s in enumerate(sel):
 d=b/f'case-{i:03d}';d.mkdir();(d/'Snapshot.tla').write_text(s['source']);(d/'Snapshot.cfg').write_text(s['config'])
 if s['lib']:(d/'Helper.tla').write_text(s['lib'])
 cmd=[str(java),'-cp',str(classes)+':'+str(jar),'OptionalServiceProbe' if s['service'] else 'SnapshotProbe']+([str(s['force']).lower()] if s['service'] else [])
 start=time.monotonic();timeout=False
 try:r=subprocess.run(cmd,cwd=d,capture_output=True,timeout=15);out,err,code=r.stdout,r.stderr,r.returncode
 except subprocess.TimeoutExpired as e:out,err,code=e.stdout or b'',e.stderr or b'',None;timeout=True
 (d/'stdout').write_bytes(out);(d/'stderr').write_bytes(err)
 rows.append(dict(id=s['id'],directory=d.name,command=cmd,exit=code,timeout=timeout,wall_seconds=time.monotonic()-start,events=[x[6:] for x in out.decode(errors='replace').splitlines() if x.startswith('EVENT\t')],artifact_sha256={str(p.relative_to(d)):sha(p) for p in d.rglob('*') if p.is_file()}))
 print(s['id'],code,flush=True)
rec=dict(oracle_sha256=sha(jar),java_sha256=sha(java),javac_sha256=sha(javac),source_sha256={p.name:sha(p) for p in [b/'SnapshotProbe.java',b/'OptionalServiceProbe.java',b/'capture.py',b/'selection.json']},rows=rows,ordinary_schedule_certified=False,model_proof=False,scope='Actual optional processPeriodic lookup and actual ModelChecker.doPeriodicWork with real state queue, selected force request and queue-dequeue-only helper; not TLCWorker scheduling or model-proof certification. Observer cleanup is separate.')
(b/'capture.json').write_text(json.dumps(rec,indent=2)+'\n')
