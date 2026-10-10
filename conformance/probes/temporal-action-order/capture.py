"""Retain direct pinned temporal/runtime action evaluation order controls."""
import argparse, hashlib, json, subprocess, tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--java',required=True);p.add_argument('--javac',required=True);p.add_argument('--jar',required=True);p.add_argument('--reason',required=True);a=p.parse_args()
root=Path(__file__).resolve().parent
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
pin='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
assert sha(a.jar)==pin
with tempfile.TemporaryDirectory(prefix='dive-action-order-') as temp:
 compiler=subprocess.run([a.javac,'-cp',a.jar,'-d',temp,str(root/'ActionOrderProbe.java')],capture_output=True)
 (root/'compiler.stdout').write_bytes(compiler.stdout);(root/'compiler.stderr').write_bytes(compiler.stderr)
 if compiler.returncode:raise SystemExit(compiler.returncode)
 command=[a.java,'-cp',temp+':'+a.jar,'tlc2.tool.liveness.ActionOrderProbe']
 r=subprocess.run(command,cwd=root,capture_output=True,timeout=30)
 (root/'stdout').write_bytes(r.stdout);(root/'stderr').write_bytes(r.stderr)
 fields=dict(line.split('\t',1) for line in r.stdout.decode().splitlines() if '\t' in line)
 expected=[f'{mode}:{before}:{after}' for mode in ['angle','box','bad-sub','runtime'] for before in range(3) for after in range(3)]
 assert r.returncode==0 and list(fields)==expected
 record=dict(schema_version=1,oracle_sha256=pin,source_commit='94d0c5070852b37b801bb8998c436383ab7ee37b',reason=a.reason,exit=r.returncode,fields=fields,scope='direct actual LNAction.eval versus Tool.eval, not model verdicts',source_sha256={name:sha(root/name) for name in ['ActionOrder.tla','ActionOrder.cfg','ActionOrderProbe.java','capture.py']},raw_sha256={name:sha(root/name) for name in ['stdout','stderr','compiler.stdout','compiler.stderr']})
 (root/'capture.json').write_text(json.dumps(record,indent=2)+'\n')
 print('captured',len(fields),'direct controls')
