"""Record the unchanged frozen native admission failures before material support."""
from pathlib import Path
import argparse,json,hashlib,subprocess,gzip,datetime
p=argparse.ArgumentParser();p.add_argument('--binary',required=True);p.add_argument('--reason',required=True);p.add_argument('--source-root',required=True);a=p.parse_args()
base=Path(__file__).resolve().parent;root=Path(a.source_root);out=base/'native-general-before';assert not out.exists();out.mkdir()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();binary=sha(a.binary);selection=json.loads((base/'selection-general.json').read_text())
paths=[p for p in (root/'crates').rglob('*')if p.is_file()and p.suffix in ['.rs','.toml']]+[root/'Cargo.toml',root/'Cargo.lock',root/'rust-toolchain.toml']
sources={str(p.relative_to(root)):sha(p)for p in paths};rows=[]
for path,d in sources.items():
 dst=out/'source'/(path+'.gz');dst.parent.mkdir(parents=True,exist_ok=True);dst.write_bytes(gzip.compress((root/path).read_bytes(),mtime=0))
for row in selection:
 for w in [1,4]:
  name=row['name'];folder=out/f'{name}-{w}';folder.mkdir();command=[a.binary,'check',str(base/'models'/name/(name+'.tla')),'--compat=tlc','--lncheck=final','--lasso=tlc','--seed=0','--fp=0',f'--workers={w}','--output=json']
  try:c=subprocess.run(command,cwd=root,capture_output=True,timeout=30);code=c.returncode;stdout=c.stdout;stderr=c.stderr;timeout=False
  except subprocess.TimeoutExpired as e:code=None;stdout=e.stdout or b'';stderr=e.stderr or b'';timeout=True
  (folder/'stdout').write_bytes(stdout);(folder/'stderr').write_bytes(stderr);rows.append(dict(name=name,requested_workers=w,exit=code,timeout=timeout,stdout_sha256=sha(folder/'stdout'),stderr_sha256=sha(folder/'stderr')))
record=dict(schema_version=1,source_commit='f27fbe5cf3931a773d2b9678ca3a0c4883581840',binary_sha256=binary,reason=a.reason,timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(),selection_sha256=sha(base/'selection-general.json'),source_sha256=sources,capture_source_sha256=sha(__file__),rows=rows,source_and_binary_unchanged=sha(a.binary)==binary and all(sha(root/p)==d for p,d in sources.items()))
(out/'capture.json').write_text(json.dumps(record,indent=2)+'\n');assert record['source_and_binary_unchanged']
print('Preserved',len(rows),'unchanged native-before rows; exits',sorted(set(r['exit']for r in rows)))
