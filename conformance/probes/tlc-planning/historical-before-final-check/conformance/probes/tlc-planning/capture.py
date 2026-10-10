"""Capture all selected inert-predicate projections from the pinned TLC jar."""
import argparse, hashlib, json, subprocess, tempfile, datetime
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--java',required=True);p.add_argument('--javac',required=True);p.add_argument('--jar',required=True);p.add_argument('--reason',required=True);a=p.parse_args()
root=Path(__file__).resolve().parent
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(a.jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
rows=[]
with tempfile.TemporaryDirectory(prefix='dive-planning-probe-') as tmp:
 c=subprocess.run([a.javac,'-cp',a.jar,'-d',tmp,str(root/'PlanningProbe.java')],capture_output=True,text=True)
 if c.returncode:raise RuntimeError(c.stderr)
 compiler=dict(exit=c.returncode,stdout=c.stdout,stderr=c.stderr)
 for case in json.loads((root/'cases.json').read_text()):
  command=[a.java,'-cp',tmp+':'+a.jar,'tlc2.tool.liveness.PlanningProbe',';'.join(case['fairness']),';'.join(case['properties'])]
  out=root/'raw'/case['name'];out.mkdir(parents=True,exist_ok=True)
  try:
   r=subprocess.run(command,capture_output=True,timeout=20);code=r.returncode;stdout=r.stdout;stderr=r.stderr;timeout=False
  except subprocess.TimeoutExpired as e:
   code=None;stdout=e.stdout or b'';stderr=e.stderr or b'';timeout=True
  (out/'stdout').write_bytes(stdout);(out/'stderr').write_bytes(stderr)
  fields={}
  for line in stdout.decode().splitlines():
   if '\t' in line:
    key,value=line.split('\t',1);assert key not in fields;fields[key]=value
  rows.append(dict(name=case['name'],exit=code,timeout=timeout,fields=fields,stdout_sha256=sha(out/'stdout'),stderr_sha256=sha(out/'stderr')))
record=dict(schema_version=1,oracle='tlc-2026.10.06.014338',oracle_sha256=sha(a.jar),reason=a.reason,timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(),source_sha256=sha(root/'PlanningProbe.java'),cases_sha256=sha(root/'cases.json'),compiler=compiler,command=['java','-cp','<temporary compiled probe>:<pinned jar>','tlc2.tool.liveness.PlanningProbe','<ordered fairness>','<ordered properties>'],rows=rows)
(root/'capture.json').write_text(json.dumps(record,indent=2)+'\n')
print('captured',len(rows),'controls;',sum(r['timeout'] or r['exit']!=0 for r in rows),'process failures retained')
