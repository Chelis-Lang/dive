"""Capture actual FastTool/processLiveness projections for every selected model."""
import argparse, hashlib, json, subprocess, tempfile, datetime, os
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--java',required=True);p.add_argument('--javac',required=True);p.add_argument('--jar',required=True);p.add_argument('--reason',required=True);a=p.parse_args()
root=Path(__file__).resolve().parent
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(a.jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
rows=[]
with tempfile.TemporaryDirectory(prefix='dive-planning-models-') as tmp:
 c=subprocess.run([a.javac,'-cp',a.jar,'-d',tmp,str(root/'PlanningProbe.java')],capture_output=True,text=True)
 if c.returncode:raise RuntimeError(c.stderr)
 compiler=dict(exit=c.returncode,stdout=c.stdout,stderr=c.stderr)
 for name in json.loads((root/'models.json').read_text()):
  model=root/'models'/name
  command=[a.java,'-cp',tmp+':'+a.jar,'tlc2.tool.liveness.PlanningProbe','--model',name,name]
  out=root/'model-raw'/name;out.mkdir(parents=True,exist_ok=True)
  try:
   r=subprocess.run(command,cwd=model,capture_output=True,timeout=20);code=r.returncode;stdout=r.stdout;stderr=r.stderr;timeout=False
  except subprocess.TimeoutExpired as e:
   code=None;stdout=e.stdout or b'';stderr=e.stderr or b'';timeout=True
  (out/'stdout').write_bytes(stdout);(out/'stderr').write_bytes(stderr)
  fields={}
  for line in stdout.decode().splitlines():
   if '\t' in line:
    key,value=line.split('\t',1);assert key not in fields;fields[key]=value
  source={str(s.relative_to(root)):sha(s) for s in model.iterdir() if s.suffix in ['.tla','.cfg']}
  rows.append(dict(name=name,exit=code,timeout=timeout,fields=fields,source_sha256=source,stdout_sha256=sha(out/'stdout'),stderr_sha256=sha(out/'stderr')))
record=dict(schema_version=1,oracle='tlc-2026.10.06.014338',oracle_sha256=sha(a.jar),reason=a.reason,timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(),source_sha256=sha(root/'PlanningProbe.java'),selection_sha256=sha(root/'models.json'),compiler=compiler,command=['java','-cp','<temporary compiled probe>:<pinned jar>','tlc2.tool.liveness.PlanningProbe','--model','<module>','<config>'],rows=rows)
(root/'model-capture.json').write_text(json.dumps(record,indent=2)+'\n')
print('captured',len(rows),'real models;',sum(r['timeout'] or r['exit']!=0 for r in rows),'process failures retained')
