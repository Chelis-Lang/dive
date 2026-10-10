"""Verify direct pinned temporal action order controls and retained failures."""
import gzip, hashlib, json
from pathlib import Path
root=Path(__file__).resolve().parent.parent/'conformance/probes/temporal-action-order'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
record=json.loads((root/'capture.json').read_text())
assert record['oracle_sha256']=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
assert record['source_commit']=='94d0c5070852b37b801bb8998c436383ab7ee37b' and record['exit']==0 and record['reason']
for table in ['source_sha256','raw_sha256']:
 for name,digest in record[table].items():assert sha(root/name)==digest,name
expected=[f'{mode}:{before}:{after}' for mode in ['angle','box','bad-sub','runtime'] for before in range(3) for after in range(3)]
assert list(record['fields'])==expected
assert record['fields']==dict(line.split('\t',1) for line in (root/'stdout').read_text().splitlines())
assert record['fields']['angle:1:1']=='false' and record['fields']['box:1:1']=='true' and record['fields']['runtime:1:1']=='error:2179'
history=root/'history-compiler-failure';old=json.loads((history/'manifest.json').read_text())
assert old['compiler_exit']==1 and old['executed_rows']==0
for name,digest in old['source_sha256'].items():assert sha(history/name)==digest
history=root/'native-before';old=json.loads((history/'manifest.json').read_text())
for name,digest in old['sha256'].items():assert hashlib.sha256(gzip.decompress((history/name).read_bytes())).hexdigest()==digest
print('verified all 36 pinned direct controls, compiler failure and native before observations')
