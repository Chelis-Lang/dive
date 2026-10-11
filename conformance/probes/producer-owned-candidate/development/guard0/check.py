"""Pinned primary and immutable failed epochs; no ordinary scheduling credit."""
from pathlib import Path
import hashlib,json,gzip
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/producer-owned-candidate'
sha=lambda b:hashlib.sha256(b).hexdigest()
load=lambda p:json.loads(p.read_text())
pin='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
for folder,expected in [('primary-final',[('violating',13),('holding',13),('multiple',13),('fair',13),('holding-initial',0)]),('material-primary',[('tuple-before',150),('string',13),('tuple',13),('record',13),('set',13)])]:
    capture=load(BASE/folder/'capture.json');assert capture['oracle_sha256']==pin
    key='case' if folder=='material-primary' else 'label'
    assert [(r[key],r['exit'])for r in capture['rows']]==expected
    for row in capture['rows']:
        artifacts=row.get('artifact_sha256',row.get('sha256',{}));assert artifacts
        for name,digest in artifacts.items():assert sha((BASE/folder/row[key]/name).read_bytes())==digest,(folder,row[key],name)
for epoch in ['focused0','focused1']:
    directory=BASE/'development'/epoch;record=load(directory/'receipt.json')
    assert record['source_unchanged'] and any(step['exit'] for step in record['steps'])
    for step in record['steps']:assert sha((directory/(step['name']+'.log')).read_bytes())==step['log_sha256']
    for name,digest in record['uncommitted_source_sha256'].items():
        assert sha((directory/'source'/name).read_bytes())==digest,(epoch,name)
example=(ROOT/'crates/dive-api/examples/periodic_candidate.rs').read_text().rstrip()
assert '```rust\n'+example+'\n```'in(ROOT/'docs/producer-owned-temporal-candidate.md').read_text()
api=(ROOT/'crates/dive-api/Cargo.toml').read_text()
assert 'test-rendezvous'not in api.split('[dev-dependencies]',1)[0]
assert 'features = ["test-rendezvous"]'in api.split('[dev-dependencies]',1)[1]
current=BASE/'source-matrix.json'
if current.exists():
    record=load(current);assert not record['model_proof']and not record['ordinary_schedule_certified']and not record['family_certified']
    for name,digest in record['source_sha256'].items():assert sha((ROOT/name).read_bytes())==digest,name
history=BASE/'history-before-candidate/manifest.json'
if history.exists():
    record=load(history)
    for name,digest in record['matrices_sha256'].items():
        data=(BASE/'history-before-candidate/matrices'/name).read_bytes();assert sha(data)==digest,name
        assert json.loads(data)['source_sha256']==record['source_by_matrix'][name]
    for digest,name in record['source_archive_paths'].items():assert sha(gzip.decompress((ROOT/name).read_bytes()))==digest,name
print('Verified complete5 final+5 material primary controls,2 failed development epochs and private candidate nonproof source contract')
