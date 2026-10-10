"""Retain whole clean TLC model runs; these are raw controls, not edited goldens."""
import argparse,datetime,hashlib,json,subprocess,tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--java',required=True);p.add_argument('--jar',required=True);p.add_argument('--reason',required=True);a=p.parse_args()
root=Path(__file__).resolve().parent
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert sha(a.jar)=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
rows=[]
for case in json.loads((root/'tlc-selection.json').read_text()):
 model=root/'models'/case['name'];out=root/'raw-tlc'/case['name'];out.mkdir(parents=True,exist_ok=True)
 with tempfile.TemporaryDirectory(prefix='dive-phase-l-clean-tlc-') as tmp:
  command=[a.java,'-cp',a.jar,'tlc2.TLC','-tool','-workers','1','-seed','0','-fp','0','-lncheck','final','-cleanup','-metadir',tmp,'-config',case['name'],case['name']]
  try:
   r=subprocess.run(command,cwd=model,capture_output=True,timeout=20);code=r.returncode;stdout=r.stdout;stderr=r.stderr;timeout=False
  except subprocess.TimeoutExpired as e:code=None;stdout=e.stdout or b'';stderr=e.stderr or b'';timeout=True
 (out/'stdout').write_bytes(stdout);(out/'stderr').write_bytes(stderr)
 rows.append(dict(name=case['name'],exit=code,timeout=timeout,stdout_sha256=sha(out/'stdout'),stderr_sha256=sha(out/'stderr'),model_source_sha256={str(p.relative_to(root)):sha(p) for p in model.iterdir() if p.suffix in ['.tla','.cfg']}))
record=dict(schema_version=1,scope='clean pinned TLC end-to-end controls, no native integrated verdict claim',oracle='tlc-2026.10.06.014338',oracle_sha256=sha(a.jar),reason=a.reason,timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(),source_sha256={'capture-tlc.py':sha(root/'capture-tlc.py')},selection_sha256=sha(root/'tlc-selection.json'),command=['java','-cp','<pinned jar>','tlc2.TLC','-tool','-workers','1','-seed','0','-fp','0','-lncheck','final','-cleanup','-metadir','<private temp>','-config','<model>','<model>'],rows=rows)
(root/'tlc-capture.json').write_text(json.dumps(record,indent=2)+'\n');print('captured all',len(rows),'clean TLC models',sum(r['timeout'] for r in rows),'timeouts')
