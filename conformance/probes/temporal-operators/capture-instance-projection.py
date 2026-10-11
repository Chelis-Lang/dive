"""Retain the complete actual LiveCheck model-bound invocation selection and raw failures."""
import argparse, datetime, hashlib, json, subprocess, tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--java',required=True);p.add_argument('--javac',required=True);p.add_argument('--jar',required=True);p.add_argument('--reason',required=True);a=p.parse_args()
root=Path(__file__).resolve().parent
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(a.jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
selection=json.loads((root/'selection-instance.json').read_text());rows=[]
with tempfile.TemporaryDirectory(prefix='dive-phase-l-slots-') as tmp:
 c=subprocess.run([a.javac,'-cp',a.jar,'-d',tmp,str(root/'IfIdentityProbe.java')],capture_output=True)
 raw=root/'raw-instance';raw.mkdir(exist_ok=True)
 (raw/'compiler.stdout').write_bytes(c.stdout);(raw/'compiler.stderr').write_bytes(c.stderr)
 if c.returncode==0:
  for case in selection:
   out=raw/case['name'];out.mkdir(exist_ok=True)
   model=root/'models'/case['model']
   command=[a.java,'-cp',tmp+':'+a.jar,'tlc2.tool.liveness.IfIdentityProbe','--model',case['model'],case['model']]
   (Path(tmp)/case['name']).mkdir(exist_ok=True)
   try:
    r=subprocess.run(command,cwd=model,capture_output=True,timeout=20);code=r.returncode;stdout=r.stdout;stderr=r.stderr;timeout=False
   except subprocess.TimeoutExpired as e:code=None;stdout=e.stdout or b'';stderr=e.stderr or b'';timeout=True
   (out/'stdout').write_bytes(stdout);(out/'stderr').write_bytes(stderr)
   import shutil
   shutil.rmtree(Path(tmp)/case['name'])
   fields={}
   for line in stdout.decode().splitlines():
    if '\t' in line:
     key,value=line.split('\t',1);assert key not in fields;fields[key]=value
   rows.append(dict(name=case['name'],model_source_sha256={str(p.relative_to(root)):sha(p) for p in model.iterdir() if p.suffix in ['.tla','.cfg']},exit=code,timeout=timeout,fields=fields,stdout_sha256=sha(out/'stdout'),stderr_sha256=sha(out/'stderr')))
record=dict(schema_version=1,scope='actual parseLiveness shared object identity, tag visit count, normalization, OOS bins and numbered tableau; diagnostic projection only, no model proof',oracle='tlc-2026.10.06.014338',oracle_sha256=sha(a.jar),reason=a.reason,timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(),source_sha256={s:sha(root/s) for s in ['IfIdentityProbe.java','capture.py']},selection_sha256=sha(root/'selection-instance.json'),compiler=dict(exit=c.returncode,stdout_sha256=sha(raw/'compiler.stdout'),stderr_sha256=sha(raw/'compiler.stderr')),rows=rows)
(root/'capture-instance-projection.json').write_text(json.dumps(record,indent=2)+'\n')
print('selected',len(selection),'executed',len(rows),'failures',sum(r['timeout'] or r['exit']!=0 for r in rows),'compiler',c.returncode)
if c.returncode:raise SystemExit(c.returncode)
