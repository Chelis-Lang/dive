"""Complete method selection, artifact/source integrity and nonproof boundary."""
from pathlib import Path
import hashlib,json,tarfile,gzip
root=Path(__file__).resolve().parent.parent
base=root/'conformance/probes/progress-queue'
sha=lambda b:hashlib.sha256(b).hexdigest()
load=lambda p:json.loads(p.read_text())
capture=load(base/'capture.json');index=load(base/'primary-index.json')
pin='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
assert capture['oracle_sha256']==index['oracle_sha256']==pin
for name,digest in index['artifact_sha256'].items():assert sha((base/name).read_bytes())==digest,name
with tarfile.open(base/'primary.tar.gz') as archive:
    raw={}
    for member in archive:
        assert not member.name.startswith('/') and '..' not in Path(member.name).parts
        assert member.isfile() or member.isdir()
        if member.isfile():raw[member.name]=archive.extractfile(member).read()
assert raw['capture.json']==(base/'capture.json').read_bytes()
for name,digest in capture['sources'].items():assert sha(raw[name])==digest==sha((base/name).read_bytes()),name
cases=['natural','natural-safe','resume','resume-safe','false-hook','hook-error','worker-error','finish-during','final-violation','final-eof','failed-init']
assert [(r['width'],r['scenario'])for r in capture['rows']]==[(width,case)for width in [1,4,8]for case in cases]
for row in capture['rows']:
    width=row['width'];case=row['scenario'];prefix=f'{case}-{width}/'
    assert not row['timeout'] and row['exit']==(2 if case=='hook-error' else 0)
    for name,digest in row['sha256'].items():assert sha(raw[prefix+name])==digest,(prefix,name)
    events=[line[6:]for line in raw[prefix+'stdout'].decode().splitlines()if line.startswith('EVENT\t')]
    assert events==row['events'] and not any(e.startswith('injector-error')for e in events)
    if case=='failed-init':
        assert 'init-result:2107'in events and 'failed-init-no-start'in events and 'start-workers'not in events
        continue
    assert 'init-result:0'in events and 'start-workers'in events
    if case not in ['natural','natural-safe','final-eof']:
        assert 'controlled-request-in-flight'in events and 'suspend-enter'in events
    if case in ['resume','final-violation','false-hook']:
        code=2104 if case=='false-hook'else 2116
        assert f'runTLC-result:{code}'in events and f'alive-at-return:{width}'in events
        assert 'resumeAll'not in events and not any(e.startswith('actual-final-check')for e in events)
    if case=='hook-error':assert 'exception-class:tlc2.tool.EvalException'in events and 'exception-code:2179'in events
    if case=='worker-error':
        assert 'runTLC-result:0'in events and 'error-code-at-return:2179'in events and 'keep-call-stack-at-return:true'in events
        assert not any(e.startswith('actual-final-check')for e in events)
    if case=='resume-safe':assert 'resumeAll'in events and 'actual-final-check:0'in events
    if case=='finish-during':assert 'controlled-finish-during'in events and 'suspend-return:false'in events
    if case!='hook-error':assert 'cleanup-joined'in events and events.index('cleanup-joined')>events.index(next(e for e in events if e.startswith('runTLC-result:')))
example=(root/'crates/dive-api/examples/progress_requests.rs').read_text().rstrip()
assert '```rust\n'+example+'\n```'in(root/'docs/producer-progress-requests.md').read_text()
history=base/'history-before-progress/manifest.json'
if history.exists():
    historical=load(history)
    for name,digest in historical['matrices_sha256'].items():
        matrix=base/'history-before-progress/matrices'/name
        assert sha(matrix.read_bytes())==digest,name
        assert load(matrix)['source_sha256']==historical['source_by_matrix'][name]
    for digest,name in historical['source_archive_paths'].items():
        assert sha(gzip.decompress((root/name).read_bytes()))==digest,name
current=base/'source-matrix.json'
if current.exists():
    record=load(current)
    assert not record['model_proof']and not record['ordinary_schedule_certified']
    for name,digest in record['source_sha256'].items():assert sha((root/name).read_bytes())==digest,name
print('Verified33 actual local worker/queue method rows and separate bounded native nonproof contract')
