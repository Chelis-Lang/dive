"""Capture every selected material fixture at four widths without editing prior runs."""
import argparse,datetime,hashlib,json,subprocess,time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--binary',required=True);p.add_argument('--output',required=True);p.add_argument('--reason',required=True);a=p.parse_args()
base=Path(__file__).resolve().parent;root=base.parents[2];out=Path(a.output)
assert not out.exists(),'choose a new output; preserve prior observations'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
paths=[p for p in (root/'crates').rglob('*')if p.is_file()and p.suffix in ['.rs','.toml']]+[root/'Cargo.toml',root/'Cargo.lock',root/'rust-toolchain.toml']
sources={str(p.relative_to(root)):sha(p)for p in paths};binary=sha(a.binary);selection=json.loads((base/'selection.json').read_text());rows=[];out.mkdir(parents=True)
for r in selection:
 for w in [1,3,4,8]:
  name=r['name'];b=out/f'{name}-{w}';b.mkdir();start=time.monotonic()
  command=[a.binary,'check',str(base/'models'/name/(name+'.tla')),'--compat=tlc','--lncheck=final','--lasso=tlc','--seed=0','--fp=0',f'--workers={w}','--output=json']
  try:
   c=subprocess.run(command,cwd=root,capture_output=True,timeout=60);stdout=c.stdout;stderr=c.stderr;code=c.returncode;timeout=False
  except subprocess.TimeoutExpired as e:stdout=e.stdout or b'';stderr=e.stderr or b'';code=None;timeout=True
  (b/'stdout').write_bytes(stdout);(b/'stderr').write_bytes(stderr)
  rows.append(dict(model=name,requested_workers=w,exit=code,timeout=timeout,elapsed_seconds=time.monotonic()-start,stdout_sha256=sha(b/'stdout'),stderr_sha256=sha(b/'stderr')))
unchanged=sha(a.binary)==binary and all(sha(root/p)==d for p,d in sources.items())
record=dict(schema_version=1,scope='Callback-free stored-value admission with fixed 50k/128 capture proof; bounded final-only results, no ordinary/full-family credit',reason=a.reason,timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(),binary_sha256=binary,source_sha256=sources,selection_sha256=sha(base/'selection.json'),capture_source_sha256=sha(__file__),rows=rows,source_and_binary_unchanged=unchanged)
(out/'capture.json').write_text(json.dumps(record,indent=2)+'\n');assert unchanged
print('Retained',len(rows),'selected rows;',sum(r['timeout']for r in rows),'timeouts')
