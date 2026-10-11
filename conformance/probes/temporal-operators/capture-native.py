"""Preserve every selected invocation on an immutable native/source producer."""
import argparse,hashlib,json,subprocess,time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--binary',required=True);p.add_argument('--out',required=True);p.add_argument('--selection',default='all');p.add_argument('--source-root');a=p.parse_args()
base=Path(__file__).resolve().parent;root=Path(a.source_root).resolve()if a.source_root else base.parents[2]
out=Path(a.out);out.mkdir(exist_ok=False);binary=Path(a.binary).resolve();sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();binary_sha=sha(binary)
files=['selection.json','selection-instance.json']if a.selection=='all'else[a.selection];selection=[]
for f in files:selection.extend(json.loads((base/f).read_text()))
inputs={str(p.relative_to(root)):sha(p)for p in(root/'crates').rglob('*')if p.is_file()and p.suffix in['.rs','.toml']}
rows=[]
for case in selection:
 for width in[1,3,4,8]:
  name=case['name'];d=out/f'{name}-{width}';d.mkdir();model=base/'models'/name
  command=[str(binary),'check',str(model/(name+'.tla')),'--compat=tlc','--lncheck=final','--lasso=tlc','--seed=0','--fp=0','--workers',str(width),'--output=json'];start=time.monotonic()
  try:r=subprocess.run(command,capture_output=True,timeout=30);code=r.returncode;stdout=r.stdout;stderr=r.stderr;timeout=False
  except subprocess.TimeoutExpired as e:code=None;stdout=e.stdout or b'';stderr=e.stderr or b'';timeout=True
  (d/'stdout').write_bytes(stdout);(d/'stderr').write_bytes(stderr)
  try:report=json.loads(stdout)
  except (ValueError,UnicodeDecodeError):report={}
  rows.append(dict(model=name,requested_workers=width,exit=code,timeout=timeout,elapsed_seconds=time.monotonic()-start,verdict=report.get('verdict'),stdout_sha256=sha(d/'stdout'),stderr_sha256=sha(d/'stderr'),model_source_sha256={str(p.relative_to(base)):sha(p)for p in model.iterdir()if p.suffix in['.cfg','.tla']},ordinary_periodic_certification=False))
assert sha(binary)==binary_sha and all(sha(root/p)==h for p,h in inputs.items())
record=dict(schema_version=1,scope='Complete bounded final/TLC selection and all refusals/failures; supported fields only, no ordinary/family certification',binary_sha256=binary_sha,source_sha256=inputs,selections_sha256={f:sha(base/f)for f in files},capture_source_sha256=sha(Path(__file__)),source_and_binary_unchanged=True,rows=rows)
(out/'capture.json').write_text(json.dumps(record,indent=2)+'\n');print('Preserved',len(rows),'rows;',sum(r['timeout']for r in rows),'timeouts')
