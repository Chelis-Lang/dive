from pathlib import Path
import hashlib,json,subprocess,time
base=Path(__file__).resolve().parent
cache=Path('/Users/jeff/code/dive-periodic-snapshot-cache')
java=cache/'jre/190480874ccceb358cbc840393207f77ac3e63a4c5f8129d0e23e9518b96ad05/jdk-17.0.20.1+1-jre/Contents/Home/bin/java'
javac=Path('/tmp/claude-501/-Users-jeff-code-dive/7ef721df-a0bf-48ac-94b6-f7c0c8e766e7/scratchpad/jdk/jdk-17.0.20.1+1/Contents/Home/bin/javac')
jar=cache/'sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
r=subprocess.run([str(javac),'-cp',str(jar),'-d',str(base/'classes'),str(base/'ProgressProbe.java'),str(base/'Progress.java')],capture_output=True)
(base/'compile.stdout').write_bytes(r.stdout);(base/'compile.stderr').write_bytes(r.stderr)
assert r.returncode==0,r.stderr.decode()
rows=[]
for width in [1,4,8]:
 for scenario in ['natural','natural-safe','resume','resume-safe','false-hook','hook-error','worker-error','finish-during','final-violation','final-eof','failed-init']:
  d=base/f'{scenario}-{width}';d.mkdir(exist_ok=True)
  source=f"---- MODULE Progress ----\nEXTENDS Integers,TLC\nVARIABLE x\nInit == x \\in 0..{width-1}\nGate(v) == TRUE\nNext == Gate(x) /\\ x' = IF x < {width*3} THEN x+{width} ELSE x\nLive == <>(x >= {width*2})\nBad == <>(x=999)\nInv == x < 0\nHook == {'FALSE' if scenario=='false-hook' else 'TRUE'}\n====\n"
  if scenario=='hook-error':source=source.replace('Hook == TRUE','Hook == 1 \\div 0')
  if scenario=='worker-error':source=source.replace("Next == Gate(x) /\\ x' = IF", "Next == Gate(x) /\\ x' = 1 \\div 0 /\\ x' = IF")
  config='INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\nPROPERTY '+('Bad' if scenario in ['final-violation','final-eof'] else 'Live')+'\n_PERIODIC Hook\n'
  if scenario in ['natural-safe','resume-safe','false-hook','hook-error','worker-error']:config=config.replace('PROPERTY Live\n','')
  if scenario=='failed-init':config+='INVARIANT Inv\n'
  (d/'Progress.tla').write_text(source);(d/'Progress.cfg').write_text(config)
  cmd=[str(java),'-cp',str(base/'classes')+':'+str(jar),'ProgressProbe',str(width),'natural' if scenario=='failed-init' else scenario]
  start=time.monotonic()
  try:r=subprocess.run(cmd,cwd=d,capture_output=True,timeout=18);code=r.returncode;out=r.stdout;err=r.stderr;timeout=False
  except subprocess.TimeoutExpired as e:code=None;out=e.stdout or b'';err=e.stderr or b'';timeout=True
  (d/'stdout').write_bytes(out);(d/'stderr').write_bytes(err)
  rows.append(dict(scenario=scenario,width=width,exit=code,timeout=timeout,seconds=time.monotonic()-start,command=cmd,events=[line[6:] for line in out.decode(errors='replace').splitlines() if line.startswith('EVENT\t')],sha256={name:sha(d/name) for name in ['Progress.tla','Progress.cfg','stdout','stderr']}))
  print(scenario,width,code,timeout,flush=True)
record=dict(oracle_sha256=sha(jar),java_sha256=sha(java),javac_sha256=sha(javac),sources={name:sha(base/name) for name in ['ProgressProbe.java','Progress.java','capture.py']},rows=rows,scope='Actual local Worker, AbstractChecker.runTLC, ModelChecker.doPeriodicWork and actual StateQueue; Native Progress.Gate gates first actual Next predicate under Worker; actual FastTool is unchanged; controlled force request except natural/failed-init. Cleanup join after runTLC is explicitly probe-owned. No native clock/count/verdict certificate.')
(base/'capture.json').write_text(json.dumps(record,indent=2)+'\n')
