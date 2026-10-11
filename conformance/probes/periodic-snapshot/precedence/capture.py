from pathlib import Path
import subprocess,json,hashlib,time
base=Path(__file__).resolve().parent
cache=Path('/Users/jeff/code/dive-periodic-snapshot-cache')
java=cache/'jre/190480874ccceb358cbc840393207f77ac3e63a4c5f8129d0e23e9518b96ad05/jdk-17.0.20.1+1-jre/Contents/Home/bin/java'
javac=Path('/tmp/claude-501/-Users-jeff-code-dive/7ef721df-a0bf-48ac-94b6-f7c0c8e766e7/scratchpad/jdk/jdk-17.0.20.1+1/Contents/Home/bin/javac')
jar=cache/'sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
classes=base/'classes';classes.mkdir()
cmd=[str(javac),'-cp',str(jar),'-d',str(classes),str(base/'SnapshotProbe.java')]
r=subprocess.run(cmd,capture_output=True);(base/'compile.stdout').write_bytes(r.stdout);(base/'compile.stderr').write_bytes(r.stderr);assert r.returncode==0,r.stderr.decode()
common='---- MODULE Snapshot ----\nEXTENDS Integers,TLC\nCONSTANT Token\nVARIABLE x\nInit == x=0\nNext == x\'=1-x\n'
cfg='CONSTANT Token=Atom\nINIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n'
rows=[];selection=[]
def add(id,defs='Hook == TRUE\n',role='Hook',extra='',lib=None):
 selection.append(dict(id=id,source=common+defs+'====\n',config=cfg+extra+('_PERIODIC '+role+'\n' if role else ''),lib=lib))
add('absent',role=None)
add('direct-true');add('direct-false','Hook == FALSE\n')
add('alias','Base == TRUE\nHook == Base\n')
add('let-local','Hook == LET Local == TRUE IN Local\n')
add('state','Hook == x=0\n');add('action',"Hook == x'=0\n");add('temporal','Hook == <>(x=0)\n')
add('integer','Hook == 1\n');add('div-zero','Hook == 1 \\div 0\n')
add('constant-value','Hook == Token = Atom\n')
# Replace the false Token model-value reference with a proper constant expression.
selection[-1]['source']=common+'Hook == Token = Token\n====\n'
add('definition-substitution','Hook == TRUE\nReplacement == FALSE\n',extra='CONSTANT Hook <- Replacement\n')
add('definition-value','Hook == TRUE\n',extra='CONSTANT Hook = FALSE\n')
add('constant-substitution','Hook == Token\nReplacement == TRUE\n')
selection[-1]['config']=cfg.replace('CONSTANT Token=Atom\n','CONSTANT Token <- Replacement\n')+'_PERIODIC Hook\n'
add('missing',role='Missing');add('nonzero-arity','Hook(v) == v\n');add('constant-role',role='Token')
for role in ['Hook','Missing','HookArg','Token']:
 add('startup-effect-'+role,'Unused == PrintT("STARTUP_MARKER")\nHook == TRUE\nHookArg(v) == v\n',role)
add('hook-effect','Hook == PrintT("HOOK_MARKER")\n')
add('qualified-instance','S == INSTANCE Helper\nHook == S!Probe\n',lib='---- MODULE Helper ----\nProbe == TRUE\n====\n')
add('qualified-role','S == INSTANCE Helper\nHook == S!Probe\n',role='S!Probe',lib='---- MODULE Helper ----\nProbe == TRUE\n====\n')
add('instance-substitution','S == INSTANCE Helper WITH Flag <- TRUE\nHook == S!Probe\n',lib='---- MODULE Helper ----\nCONSTANT Flag\nProbe == Flag\n====\n')
add('constant-substitution-effect','Hidden == PrintT("SUBSTITUTION_MARKER")\nHook == TRUE\n')
selection[-1]['config']=cfg.replace('CONSTANT Token=Atom\n','CONSTANT Token <- Hidden\n')+'_PERIODIC Hook\n'

# Additive exact precedence/startup selection; original26 source/raw stays unchanged.
selection=[]
add('constant-missing-first',role='Missing');selection[-1]['config']=selection[-1]['config'].replace('CONSTANT Token=Atom\n','')
add('init-missing-first',role='Missing');selection[-1]['config']=selection[-1]['config'].replace('INIT Init\n','')
add('next-missing-first',role='Missing',defs='Hook == TRUE\nInv == x>=0\n');selection[-1]['config']=selection[-1]['config'].replace('NEXT Next\n','')+'INVARIANT Inv\n'
add('next-name-first',role='Missing');selection[-1]['config']=selection[-1]['config'].replace('NEXT Next\n','NEXT MissingNext\n')
add('constraint-name-first',role='Missing',extra='CONSTRAINT MissingConstraint\n')
add('reward-name-first',role='Missing',extra='_RL_REWARD MissingReward\n')
add('view-name-later',role='Missing',extra='VIEW MissingView\n')
add('possible-name-later',role='Missing',extra='_POSSIBLE MissingPossible\n')
add('small-subset-startup','Unused == SUBSET (1..4)\nHook == TRUE\n')
add('small-quantifier-startup','Unused == \\A i \\in 1..4 : i=i\nHook == TRUE\n')
add('state-witness-not-cached','Danger == IF x=0 THEN PrintT("WITNESS_NOT_CACHED") ELSE TRUE\nHook == TRUE\n')
add('replacement-erases-state','StateHelper == x=0\nDanger == IF StateHelper THEN PrintT("ERASED_STATE_MARKER") ELSE TRUE\nHook == TRUE\n',extra='CONSTANT StateHelper=TRUE\n')
(base/'selection.json').write_text(json.dumps(selection,indent=2)+'\n')
for i,c in enumerate(selection):
 d=base/f'case-{i:03d}';d.mkdir();(d/'Snapshot.tla').write_text(c['source']);(d/'Snapshot.cfg').write_text(c['config'])
 files=['Snapshot.tla','Snapshot.cfg']
 if c['lib']:(d/'Helper.tla').write_text(c['lib']);files.append('Helper.tla')
 cmd=[str(java),'-cp',str(classes)+':'+str(jar),'SnapshotProbe'];start=time.monotonic()
 try:r=subprocess.run(cmd,cwd=d,capture_output=True,timeout=15);code=r.returncode;out=r.stdout;err=r.stderr;timeout=False
 except subprocess.TimeoutExpired as e:code=None;out=e.stdout or b'';err=e.stderr or b'';timeout=True
 (d/'stdout').write_bytes(out);(d/'stderr').write_bytes(err)
 rows.append(dict(id=c['id'],directory=d.name,exit=code,timeout=timeout,wall_seconds=time.monotonic()-start,command=cmd,events=[l[6:] for l in out.decode(errors='replace').splitlines() if l.startswith('EVENT\t')],files_sha256={f:sha(d/f) for f in files+['stdout','stderr']}))
 print(c['id']+': '+str(code),flush=True)
record=dict(oracle_sha256=sha(jar),java_sha256=sha(java),javac_sha256=sha(javac),compile_exit=0,source_sha256={f:sha(base/f) for f in ['SnapshotProbe.java','capture.py','selection.json']},rows=rows,model_proof=False,ordinary_schedule_certified=False,scope='Actual FastTool startup and processPeriodic pre-constant definition snapshot/body lookup, with optional actual empty-state body evaluation; not an ordinary model-checking run.')
(base/'capture.json').write_text(json.dumps(record,indent=2)+'\n')
