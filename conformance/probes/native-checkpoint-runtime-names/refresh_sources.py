"""Preserve immutable parent authorities before refreshing current source hashes.

Run once in the new increment after source/docs are settled. Histories are not
oracle results or recertification; each archived byte must match its parent map.
"""
import gzip
import hashlib
import json
from pathlib import Path
import subprocess

root=Path(__file__).resolve().parents[3]
base=root/'conformance/probes/native-checkpoint-runtime-names'
parent='425474bc61210d3a178f5f373d332c72a01d8d1c'
paths=[f'conformance/probes/{p}' for p in (
 'native-checkpoint/current-integration-source.json',
 'predecessor-spill/current-integration-source.json',
 'tlc-planning/public-ir-admission-source-matrix.json',
 'tlc-planning/combined-context-tableau-source-matrix.json',
 'tlc-phase-l-graph/source-matrix.json',
 'tlc-final-check/source-matrix.json',
 'tlc-phase-execution/source-matrix.json',
 'native-parity-integration/current-source.json')]
history=base/'before-authorities';history.mkdir(exist_ok=False)
sha=lambda b:hashlib.sha256(b).hexdigest()
read=lambda p:subprocess.check_output(['git','show',f'{parent}:{p}'],cwd=root)
index={'parent_commit':parent,'scope':'immutable parent source integrity authorities, not fresh runtime certification','matrices':{},'sources':{}}
for path in paths:
 raw=read(path); old=json.loads(raw); dest=history/'matrices'/path;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(raw)
 index['matrices'][path]=sha(raw)
 for source,digest in old['source_sha256'].items():
  data=read(source);assert sha(data)==digest,(source,digest)
  archived=history/'sources'/(source+'.gz');archived.parent.mkdir(parents=True,exist_ok=True);archived.write_bytes(gzip.compress(data,mtime=0));index['sources'][source]=digest
(history/'index.json').write_text(json.dumps(index,indent=2)+'\n')
pending=set(paths)
while pending:
 progressed=False
 for path in sorted(pending):
  p=root/path;v=json.loads(p.read_text())
  if set(v['source_sha256']) & (pending-{path}):continue
  v['runtime_name_recovery_previous_authority']={'matrix':str((history/'matrices'/path).relative_to(root)),'matrix_sha256':index['matrices'][path],'source_archive':str((history/'sources').relative_to(root))}
  v['current_scope_addendum']='Pure-runtime-name safety recovery source integration; historical producer captures and family-certification status remain unchanged.'
  v['source_sha256']={name:sha((root/name).read_bytes()) for name in v['source_sha256']}
  p.write_text(json.dumps(v,indent=2)+'\n');pending.remove(path);progressed=True
  break
 assert progressed,pending
print(f'Preserved {len(paths)} parent matrices and {len(index["sources"])} exact source inputs; current hashes separately refreshed')
