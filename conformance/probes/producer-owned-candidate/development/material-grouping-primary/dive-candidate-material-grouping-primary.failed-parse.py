from pathlib import Path
import subprocess,json,hashlib,time
b=Path('/tmp/dive-candidate-material-grouping-primary');b.mkdir();sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
c=Path('/Users/jeff/code/dive-periodic-snapshot-cache');jar=c/'sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar';java=c/'jre/190480874ccceb358cbc840393207f77ac3e63a4c5f8129d0e23e9518b96ad05/jdk-17.0.20.1+1-jre/Contents/Home/bin/java';assert sha(jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
rows=[]
for kind in ['string','tuple','record','set']:
 old=json.loads((Path('/tmp/dive-producer-candidate-observer1/raw')/(kind+'-w4.json')).read_text())
 for corrected in [False,True]:
  d=b/(kind+('-corrected'if corrected else'-before'));d.mkdir();source=old['source'];source=source.replace("x'=IF x=0 THEN 1 ELSE 1 /\\ s'=s","x'=(IF x=0 THEN 1 ELSE 1) /\\ s'=s")if corrected else source
  (d/'RuntimeCandidate.tla').write_text(source);(d/'RuntimeCandidate.cfg').write_text(old['configuration']);cmd=[str(java),'-cp',str(jar),'tlc2.TLC','-tool','-workers','1','-seed','0','-fp','0','-lncheck','final','-cleanup','-metadir',str(d/'meta'),'RuntimeCandidate'];start=time.monotonic();p=subprocess.run(cmd,cwd=d,capture_output=True,timeout=15);(d/'stdout').write_bytes(p.stdout);(d/'stderr').write_bytes(p.stderr)
  rows.append(dict(id=d.name,exit=p.returncode,command=cmd,wall_seconds=time.monotonic()-start,artifact_sha256={str(f.relative_to(d)):sha(f)for f in d.rglob('*')if f.is_file()}));print(d.name,p.returncode,flush=True)
(b/'capture.json').write_text(json.dumps(dict(oracle_sha256=sha(jar),rows=rows,scope='Actual clean pin incomplete unparenthesized Next versus separately parenthesized finite material witness source; no ordinary scheduler/native proof'),indent=2)+'\n')
assert all(x['exit']==(13 if x['id'].endswith('-corrected')else75)for x in rows),rows
