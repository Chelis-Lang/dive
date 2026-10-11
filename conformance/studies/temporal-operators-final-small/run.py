"""Complete unchanged final-TLC selection; source/binary facts never promoted from history."""
import hashlib,json,os,subprocess,time,sys
from pathlib import Path
root=Path(sys.argv[1]).resolve();base=Path(__file__).resolve().parent;release=Path(sys.argv[2]).resolve();cache=Path(os.environ['DIVE_CACHE_DIR']).resolve()
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
inputs=json.loads((root/'conformance/probes/temporal-operators/gate-current/before.json').read_text())['source_sha256']
assert all(sha(root/p)==d for p,d in inputs.items())
before=dict(schema_version=1,source_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),source_sha256=inputs,selection=dict(corpus='examples',tier='small',filters=[],workers=[1],timeout_seconds=600,jobs=1),implementation='dive-final-tlc',binary_sha256={n:sha(release/n)for n in ['dive','dive-conformance']},launcher_sha256=sha(base/'capture-cli.py'),scope='Fresh unchanged140 small selection after bounded known-user operator forwarding; all refusals/failures/timeouts retained, final-only and fixed bounds, no ordinary/family/aggregate-memory credit')
(base/'before.json').write_text(json.dumps(before,indent=2)+'\n');env=os.environ|{'DIVE_CACHE_DIR':str(cache),'DIVE_CAPTURE_BINARY':str(release/'dive')}
args=[str(release/'dive-conformance'),'run','--impl','dive-final-tlc','--impl-profile','tlc','--corpus','examples','--tier','small','--workers','1','--jobs','1','--timeout','600','--no-sany','--no-summary','--dive-bin',str(base/'capture-cli.py'),'--report-dir',str(base/'reports')]
start=time.monotonic()
with(base/'run.log').open('wb')as out:code=subprocess.run(args,cwd=root,env=env,stdout=out,stderr=subprocess.STDOUT).returncode
assert all(sha(root/p)==d for p,d in inputs.items())
assert all(sha(release/n)==d for n,d in before['binary_sha256'].items())
compare=subprocess.run([sys.executable,str(base/'compare.py'),str(root)],cwd=root);assert compare.returncode==0
artifacts={str(p.relative_to(base)):sha(p)for p in sorted(base.rglob('*'))if p.is_file()and p.name not in ['after.json','launcher.log']}
(base/'after.json').write_text(json.dumps(dict(source_and_binaries_unchanged=True,harness_exit=code,elapsed_seconds=time.monotonic()-start,artifact_sha256=artifacts),indent=2)+'\n')
print('Full immutable final small study complete; harness exit',code)
