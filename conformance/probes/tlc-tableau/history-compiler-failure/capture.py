"""Regenerate every selected clean pinned tableau projection, retaining failures."""
import argparse,hashlib,json,subprocess,tempfile,datetime,os
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--java',required=True);p.add_argument('--javac',required=True);p.add_argument('--jar',required=True);p.add_argument('--reason',required=True);a=p.parse_args()
root=Path(__file__).resolve().parent
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(a.jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
sources=['TableauProbe.java','../tlc-planning/PlanningProbe.java','capture.py']
rows=[];selection=json.loads((root/'selection.json').read_text())
with tempfile.TemporaryDirectory(prefix='dive-tableau-probe-') as tmp:
 compiler=subprocess.run([a.javac,'-cp',a.jar,'-d',tmp,str(root/'TableauProbe.java'),str(root/'../tlc-planning/PlanningProbe.java')],capture_output=True,text=True)
 if compiler.returncode:raise RuntimeError(compiler.stderr)
 for case in selection:
  name=case['kind']+'.'+case['name'];command=[a.java,'-cp',tmp+':'+a.jar,'tlc2.tool.liveness.TableauProbe'];cwd=root;source={}
  if case['kind']=='planning':command+=[';'.join(case['fairness']),';'.join(case['properties'])]
  elif case['kind']=='direct':command+=['--direct',case['formula']]
  else:
   cwd=(root/case['source']).parent;command+=['--model',case['name'],case['name']]
   source={os.path.relpath(s,root):sha(s) for s in cwd.iterdir() if s.suffix in ['.tla','.cfg']}
  out=root/'raw'/name;out.mkdir(parents=True,exist_ok=True)
  try:
   r=subprocess.run(command,cwd=cwd,capture_output=True,timeout=20);code=r.returncode;stdout=r.stdout;stderr=r.stderr;timeout=False
  except subprocess.TimeoutExpired as e:code=None;stdout=e.stdout or b'';stderr=e.stderr or b'';timeout=True
  (out/'stdout').write_bytes(stdout);(out/'stderr').write_bytes(stderr)
  fields={}
  for line in stdout.decode().splitlines():
   if '\t' in line:
    key,value=line.split('\t',1);assert key not in fields;fields[key]=value
  rows.append(dict(name=name,exit=code,timeout=timeout,fields=fields,source_sha256=source,stdout_sha256=sha(out/'stdout'),stderr_sha256=sha(out/'stderr')))
record=dict(schema_version=1,oracle='tlc-2026.10.06.014338',oracle_sha256=sha(a.jar),reason=a.reason,timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(),source_sha256={s:sha(root/s) for s in sources},selection_sha256=sha(root/'selection.json'),compiler=dict(exit=compiler.returncode,stdout=compiler.stdout,stderr=compiler.stderr),command=['java','-cp','<temporary compiled probes>:<pinned jar>','tlc2.tool.liveness.TableauProbe','<complete selected arguments>'],rows=rows)
(root/'capture.json').write_text(json.dumps(record,indent=2)+'\n')
print('captured',len(rows),'selected tableaux;',sum(r['timeout'] or r['exit']!=0 for r in rows),'process failures retained')
