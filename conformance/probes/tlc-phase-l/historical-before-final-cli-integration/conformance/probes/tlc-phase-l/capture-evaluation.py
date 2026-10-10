"""Retain the complete actual LiveCheck synthetic bin diagnostic selection and raw failures."""
import argparse, datetime, hashlib, json, subprocess, tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--java',required=True);p.add_argument('--javac',required=True);p.add_argument('--jar',required=True);p.add_argument('--reason',required=True);a=p.parse_args()
root=Path(__file__).resolve().parent
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(a.jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
selection=json.loads((root/'evaluation-selection.json').read_text());rows=[]
with tempfile.TemporaryDirectory(prefix='dive-phase-l-slots-') as tmp:
 c=subprocess.run([a.javac,'-cp',a.jar,'-d',tmp,str(root/'SlotsProbe.java'),str(root/'EvaluationProbe.java')],capture_output=True)
 raw=root/'raw-evaluation';raw.mkdir(exist_ok=True)
 (raw/'compiler.stdout').write_bytes(c.stdout);(raw/'compiler.stderr').write_bytes(c.stderr)
 if c.returncode==0:
  for case in selection:
   out=raw/case['name'];out.mkdir(exist_ok=True)
   command=[a.java,'-cp',tmp+':'+a.jar,'tlc2.tool.liveness.EvaluationProbe',str(case['groups']),str(case['states']),str(case['actions']),str(case['fail']),str(case['capacity']),';'.join(case['records']),str(out/'metadir')]
   (out/'metadir').mkdir(exist_ok=True)
   try:
    r=subprocess.run(command,capture_output=True,timeout=20);code=r.returncode;stdout=r.stdout;stderr=r.stderr;timeout=False
   except subprocess.TimeoutExpired as e:code=None;stdout=e.stdout or b'';stderr=e.stderr or b'';timeout=True
   (out/'stdout').write_bytes(stdout);(out/'stderr').write_bytes(stderr)
   import shutil
   shutil.rmtree(out/'metadir')
   fields={}
   for line in stdout.decode().splitlines():
    if '\t' in line:
     key,value=line.split('\t',1);assert key not in fields;fields[key]=value
   rows.append(dict(name=case['name'],exit=code,timeout=timeout,fields=fields,stdout_sha256=sha(out/'stdout'),stderr_sha256=sha(out/'stderr')))
record=dict(schema_version=1,scope='actual LiveCheck synthetic null-tableau invocation; no model execution or SCC verdict',oracle='tlc-2026.10.06.014338',oracle_sha256=sha(a.jar),reason=a.reason,timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(),source_sha256={s:sha(root/s) for s in ['SlotsProbe.java','EvaluationProbe.java','capture-evaluation.py']},selection_sha256=sha(root/'evaluation-selection.json'),compiler=dict(exit=c.returncode,stdout_sha256=sha(raw/'compiler.stdout'),stderr_sha256=sha(raw/'compiler.stderr')),rows=rows)
(root/'evaluation-capture.json').write_text(json.dumps(record,indent=2)+'\n')
print('selected',len(selection),'executed',len(rows),'failures',sum(r['timeout'] or r['exit']!=0 for r in rows),'compiler',c.returncode)
if c.returncode:raise SystemExit(c.returncode)
