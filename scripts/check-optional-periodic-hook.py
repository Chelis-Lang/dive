"""Exact optional-hook primary/history integrity; no ordinary proof promotion."""
from pathlib import Path
import gzip,hashlib,json
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/optional-periodic-hook'
load=lambda p:json.loads(p.read_text())
sha=lambda data:hashlib.sha256(data).hexdigest()
primary=BASE/'primary'
index=load(primary/'closed-source-and-artifact-index.json')
for path,digest in index['files'].items():
    assert sha((primary/path).read_bytes())==digest,path
capture=load(primary/'capture.json')
assert capture['oracle_sha256']=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
selection=load(primary/'selection.json')
assert len(selection)==len(capture['rows'])==26
assert [r['id']for r in capture['rows']]==[r['id']for r in selection]
count=0
for row in capture['rows']:
    assert not row['timeout']
    for path,digest in row['artifact_sha256'].items():
        assert sha((primary/row['directory']/path).read_bytes())==digest,(row['id'],path)
        count+=1
    if row['id'].startswith('absent-')or row['id']in['service-absent','service-absent-forced','service-unused-false']:
        assert '_PERIODIC'not in(primary/row['directory']/'Snapshot.cfg').read_text()
        assert 'body-present:false'in row['events']
assert count==141
rows={r['id']:r for r in capture['rows']}
assert 'exception-class:java.lang.ClassCastException'in rows['constant-role']['events']
assert 'STARTUP_MARKER'in(primary/rows['absent-startup']['directory']/'stderr').read_text()
assert 'actual-stop-observed:false'in rows['service-absent']['events']
assert 'actual-stop-observed:true'in rows['service-absent-forced']['events']
assert 'force-retained:true'in rows['service-absent-forced']['events']
assert 'service-result:2104'in rows['service-false']['events']
assert 'service-exception-code:1000'in rows['service-integer']['events']
assert 'service-exception-code:2179'in rows['service-error']['events']
assert 'service-enter'not in rows['service-failed-init']['events']
record=load(BASE/'focused6/receipt.json')
assert record['source_unchanged']and len(record['steps'])==5
for path,digest in record['source_sha256'].items():
    assert sha((BASE/'focused6/source'/path).read_bytes())==digest,path
assert [step['exit']for step in record['steps']]==[0,0,0,0,101]
for step in record['steps']:
    assert sha((BASE/'focused6'/(step['name']+'.log')).read_bytes())==step['log_sha256']
fresh=load(BASE/'focused13/receipt.json')
assert fresh['source_unchanged']and len(fresh['steps'])==4
for path,digest in fresh['source_sha256'].items():
    assert sha((BASE/'focused13/source'/path).read_bytes())==digest,path
for step in fresh['steps']:
    assert step['exit']==0
    assert sha((BASE/'focused13'/(step['name']+'.log')).read_bytes())==step['log_sha256']
init=load(BASE/'init-error-primary/capture.json')
assert init['exit']==75
for path,digest in init['artifact_sha256'].items():
    assert sha((BASE/'init-error-primary'/path).read_bytes())==digest,path
raw=(BASE/'init-error-primary/stdout').read_text()
assert raw.index('STARTMSG 1000:1')<raw.index('STARTMSG 2179:0')<raw.index('STARTMSG 2103:1')
example=(ROOT/'crates/dive-api/examples/optional_periodic_candidate.rs').read_text().rstrip()
assert '```rust\n'+example+'\n```'in(ROOT/'docs/optional-periodic-hook.md').read_text()
assert '_PERIODIC Hook\\n'not in example
matrix=load(BASE/'source-matrix.json')
assert not matrix['model_proof']and not matrix['ordinary_schedule_certified']and not matrix['family_certified']
for path,digest in matrix['source_sha256'].items():assert sha((ROOT/path).read_bytes())==digest,path
history=load(BASE/'history-before-optional/manifest.json')
for path,digest in history['matrices_sha256'].items():
    raw=(BASE/'history-before-optional/matrices'/path).read_bytes()
    assert sha(raw)==digest,path
    assert json.loads(raw)['source_sha256']==history['source_by_matrix'][path]
for digest,path in history['source_archive_paths'].items():
    assert sha(gzip.decompress((ROOT/path).read_bytes()))==digest,path
ledger=load(ROOT/'conformance/parity-features.json')
f=next(f for f in ledger['families']if f['id']=='F-023')
assert f['implementation_status']=='missing'and f['certification']=='audit-required'
assert 220 in f['issues']
print('Verified26 original optional-role primary rows/141 invocation hashes, immutable focused/history sources and genuine-absence nonproof contract')
