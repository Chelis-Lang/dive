"""Preserve the pinned parent authorities before a source-integrity-only refresh.

Run once from the repository root after this admission increment is settled.
This does not regenerate or recertify oracle/runtime captures.
"""
import gzip
import hashlib
import json
from pathlib import Path
import subprocess

root = Path.cwd()
parent = 'a3b1ee365335e64c04f9109fe839443da6b0dc8d'
base = root/'conformance/probes/configured-random-admission'
paths = [f'conformance/probes/{p}' for p in (
    'native-checkpoint/current-integration-source.json',
    'predecessor-spill/current-integration-source.json',
    'tlc-planning/public-ir-admission-source-matrix.json',
    'tlc-planning/combined-context-tableau-source-matrix.json',
    'tlc-phase-l-graph/source-matrix.json',
    'tlc-final-check/source-matrix.json',
    'tlc-phase-execution/source-matrix.json',
    'tlc-state-if/source-matrix.json',
    'tlc-temporal-if/source-matrix.json',
    'tlc-temporal-if-final/source-matrix.json',
    'tlc-final-scopes/source-matrix.json',
    'native-parity-integration/current-source.json')]
history = base/'before-authorities'
history.mkdir(exist_ok=False)
sha = lambda b: hashlib.sha256(b).hexdigest()
read = lambda p: subprocess.check_output(['git','show',f'{parent}:{p}'],cwd=root)
index = {'parent_commit':parent,'scope':'immutable parent source authorities; current admission integrity is separate from original runtime/oracle claims','matrices':{},'sources':{}}
for path in paths:
    raw = read(path)
    old = json.loads(raw)
    dest = history/'matrices'/path
    dest.parent.mkdir(parents=True,exist_ok=True)
    dest.write_bytes(raw)
    index['matrices'][path] = sha(raw)
    for source,digest in old['source_sha256'].items():
        data = read(source)
        assert sha(data) == digest,(source,digest)
        dest = history/'sources'/(source+'.gz')
        dest.parent.mkdir(parents=True,exist_ok=True)
        dest.write_bytes(gzip.compress(data,mtime=0))
        index['sources'][source] = digest
(history/'index.json').write_text(json.dumps(index,indent=2)+'\n')
pending = set(paths)
while pending:
    progressed = False
    for path in sorted(pending):
        p = root/path
        value = json.loads(p.read_text())
        if set(value['source_sha256']) & (pending-{path}):
            continue
        value['configured_native_rng_previous_authority'] = {
            'matrix':str((history/'matrices'/path).relative_to(root)),
            'matrix_sha256':index['matrices'][path],
            'source_archive':str((history/'sources').relative_to(root))}
        value['configured_native_rng_scope_addendum'] = 'Configured native RNG authority independently refuses before ordinary safety binding/output/search; historical producer evidence and certification status remain unchanged.'
        value['source_sha256'] = {name:sha((root/name).read_bytes()) for name in value['source_sha256']}
        p.write_text(json.dumps(value,indent=2)+'\n')
        pending.remove(path)
        progressed = True
        break
    assert progressed,pending
print(f'Archived {len(paths)} parent authorities and {len(index["sources"])} exact source inputs; current hashes refreshed separately')
