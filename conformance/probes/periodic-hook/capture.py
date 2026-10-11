from pathlib import Path
import subprocess,json,hashlib,itertools,time
base=Path(__file__).resolve().parent
cache=Path('/Users/jeff/code/dive-periodic-hook-cache')
java=cache/'jre/190480874ccceb358cbc840393207f77ac3e63a4c5f8129d0e23e9518b96ad05/jdk-17.0.20.1+1-jre/Contents/Home/bin/java'
javac=Path('/tmp/claude-501/-Users-jeff-code-dive/7ef721df-a0bf-48ac-94b6-f7c0c8e766e7/scratchpad/jdk/jdk-17.0.20.1+1/Contents/Home/bin/javac')
jar=cache/'sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
classes=base/'classes';classes.mkdir()
command=[str(javac),'-cp',str(jar),'-d',str(classes),str(base/'HookProbe.java')]
r=subprocess.run(command,capture_output=True);(base/'compile.stdout').write_bytes(r.stdout);(base/'compile.stderr').write_bytes(r.stderr);assert r.returncode==0,r.stderr.decode()
expressions=[('true','TRUE'),('false','FALSE'),('integer','1'),('string','"text"'),('set','{1}'),('tuple','<<1>>'),('record','[a |-> 1]'),('divide-zero','1 \\div 0'),('state','x=0'),('action',"x'=0"),('temporal','<>(x=0)'),('effect','PrintT("HOOK_MARKER") /\\ TRUE'),('short-state','FALSE /\\ x=0'),('model-value','Token')]
controls=[('invoke',False,True,0,False),('suspend-false',False,False,0,False),('live-error-first',True,True,2116,True),('checkpoint-after',False,True,0,True)]
selection=[dict(id=n+'-'+cn,expr=e,liveness=l,suspended=s,check_code=c,checkpoint=cp) for n,e in expressions for cn,l,s,c,cp in controls]
selection += [dict(id='undefined',expr='TRUE',role='Missing',liveness=False,suspended=True,check_code=0,checkpoint=False),dict(id='arity',expr='TRUE',role='HookArg',liveness=False,suspended=True,check_code=0,checkpoint=False),dict(id='constant-role',expr='TRUE',role='Token',liveness=False,suspended=True,check_code=0,checkpoint=False)]
(base/'selection.json').write_text(json.dumps(selection,indent=2)+'\n')
rows=[]
for i,c in enumerate(selection):
 d=base/f'case-{i:03d}';d.mkdir();meta=d/'meta';meta.mkdir()
 src="---- MODULE Periodic ----\nEXTENDS Integers,TLC\nCONSTANT Token\nVARIABLE x\nInit == x=0\nNext == x'=1-x\nLive == []<>(x=1)\nHook == "+c['expr']+"\nHookArg(v) == v\n====\n"
 cfg="CONSTANT Token=Atom\nINIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\n"+('PROPERTY Live\n' if c['liveness'] else '')+'_PERIODIC '+c.get('role','Hook')+'\n'
 (d/'Periodic.tla').write_text(src);(d/'Periodic.cfg').write_text(cfg)
 command=[str(java),'-cp',str(classes)+':'+str(jar),'HookProbe',str(meta),'0.0','true','false',str(c['checkpoint']).lower(),str(c['suspended']).lower(),str(c['check_code'])]
 start=time.monotonic()
 try:r=subprocess.run(command,cwd=d,capture_output=True,timeout=15);code=r.returncode;out=r.stdout;err=r.stderr;timeout=False
 except subprocess.TimeoutExpired as e:code=None;out=e.stdout or b'';err=e.stderr or b'';timeout=True
 (d/'stdout').write_bytes(out);(d/'stderr').write_bytes(err)
 events=[l[6:] for l in out.decode(errors='replace').splitlines() if l.startswith('EVENT\t')]
 rows.append(dict(case=c,exit=code,timeout=timeout,wall_seconds=time.monotonic()-start,events=events,command=command,files_sha256={f:sha(d/f) for f in ['Periodic.tla','Periodic.cfg','stdout','stderr']}))
 if (i+1)%10==0:print(str(i+1)+'/'+str(len(selection)),flush=True)
record=dict(oracle_sha256=sha(jar),java_sha256=sha(java),javac_sha256=sha(javac),source_sha256={f:sha(base/f) for f in ['HookProbe.java','capture.py','selection.json']},compile_exit=0,rows=rows,model_proof=False,ordinary_schedule_certified=False,scope='Actual pinned doPeriodicWork and configured hook evaluation with controlled liveness/queue/checkpoint callbacks; not a real clock schedule or checkpoint transaction.')
(base/'capture.json').write_text(json.dumps(record,indent=2)+'\n');print('closed '+str(len(rows)),flush=True)
