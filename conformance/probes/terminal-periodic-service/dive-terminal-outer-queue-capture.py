from pathlib import Path
import subprocess,shutil,json,hashlib,time
b=Path('/tmp/dive-terminal-outer-queue-candidate2');source=Path('/tmp/dive-terminal-periodic-primary-candidate1');selection=json.loads((source/'selection.json').read_text());(b/'selection.json').write_text(json.dumps(selection,indent=2)+'\n');rows=[]
java='/Users/jeff/code/dive-periodic-snapshot-cache/jre/190480874ccceb358cbc840393207f77ac3e63a4c5f8129d0e23e9518b96ad05/jdk-17.0.20.1+1-jre/Contents/Home/bin/java';jar=Path('/Users/jeff/code/dive-periodic-snapshot-cache/sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar');sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();assert sha(jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
for row in selection:
 d=b/row['id']
 if d.exists():
  # The initial probe is immutable; additional complete selection gets its own directory.
  d=b/(row['id']+'-selected')
 d.mkdir()
 for n in ['Terminal.tla','Terminal.cfg']:shutil.copy2(source/row['id']/n,d/n)
 cmd=[java,'-cp',str(b)+':'+str(jar),'TerminalOuterProbe',str(row['width'])];(d/'command.json').write_text(json.dumps(cmd,indent=2)+'\n');start=time.monotonic();timeout=False
 with(d/'stdout').open('wb')as o,(d/'stderr').open('wb')as e:
  try:r=subprocess.run(cmd,cwd=d,stdout=o,stderr=e,timeout=30);exit=r.returncode
  except subprocess.TimeoutExpired:timeout=True;exit=None
 events=[x[len('EVENT\t'):]for x in(d/'stdout').read_text(errors='replace').splitlines()if x.startswith('EVENT\t')]
 record=dict(**row,directory=d.name,exit=exit,timeout=timeout,wall_seconds=time.monotonic()-start,events=events,scope='Actual ModelChecker.modelCheck; transparent delegating queue/liveness observer; controlled instrumentation timing, not natural clock proof',artifact_sha256={str(p.relative_to(d)):sha(p)for p in d.rglob('*')if p.is_file()})
 rows.append(record);(b/'capture.json').write_text(json.dumps(dict(rows=rows,oracle_sha256=sha(jar),ordinary_native_proof=False,observer_sha256=sha(b/'TerminalOuterProbe.java')),indent=2)+'\n');print(row['id'],exit,[x for x in events if 'suspendAll-return' in x or 'finalCheck-return' in x or 'live-check-return' in x or 'modelCheck-'in x],flush=True)
assert len(rows)==60
print('Closed60 actual outer/queue selected controls; no ordinary native proof',flush=True)
