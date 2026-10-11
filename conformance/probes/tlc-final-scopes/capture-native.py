"""Record every selected final CLI outcome at every requested width, without promotion."""
import argparse,hashlib,json,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--binary',required=True);p.add_argument('--out',required=True);a=p.parse_args()
base=Path(__file__).resolve().parent;root=base.parents[2];out=Path(a.out);out.mkdir(exist_ok=False)
binary=Path(a.binary).resolve();sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();binary_sha=sha(binary)
inputs={str(p.relative_to(root)):sha(p) for p in (root/'crates').rglob('*') if p.is_file() and p.suffix in ['.rs','.toml']}
rows=[]
for case in json.loads((base/'selection.json').read_text()):
 for width in [1,3,4,8]:
  name=case['name'];d=out/f'{name}-{width}';d.mkdir();model=base/'models'/name
  command=[str(binary),'check',str(model/(name+'.tla')),'--compat=tlc','--lncheck=final','--lasso=tlc','--seed=0','--fp=0','--workers',str(width),'--output=json']
  run=subprocess.run(command,capture_output=True,timeout=30);(d/'stdout').write_bytes(run.stdout);(d/'stderr').write_bytes(run.stderr)
  report=json.loads(run.stdout);rows.append(dict(model=name,requested_workers=width,exit=run.returncode,verdict=report['verdict'],stdout_sha256=sha(d/'stdout'),stderr_sha256=sha(d/'stderr'),ordinary_periodic_certification=False))
assert binary_sha==sha(binary)
assert all(sha(root/p)==h for p,h in inputs.items())
(out/'capture.json').write_text(json.dumps(dict(schema_version=1,scope='Complete selected current final-only CLI census; pure supported declared fields do not certify ordinary periodic scheduling/families; all reference errors remain uncredited',binary_sha256=binary_sha,source_sha256=inputs,selection_sha256=sha(base/'selection.json'),source_and_binary_unchanged=True,rows=rows),indent=2)+'\n')
print('retained complete',len(rows),'native rows')
