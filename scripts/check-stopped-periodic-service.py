"""Complete actual stopped-worker references; source integrity is not ordinary proof."""
from pathlib import Path
import json,hashlib,gzip
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/stopped-periodic-service'
load=lambda p:json.loads(p.read_text())
sha=lambda b:hashlib.sha256(b).hexdigest()
PIN='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
for name in ['primary-development0','primary-rejected1','primary-rejected2','primary']:
    folder=BASE/name
    for path,digest in load(folder/'closed-index.json')['files'].items():assert sha((folder/path).read_bytes())==digest,(name,path)
    capture=load(folder/'capture.json');selection=load(folder/'selection.json')
    assert len(capture['rows'])==len(selection)==60
    assert [r['id']for r in capture['rows']]==[r['id']for r in selection]
    assert capture['oracle_sha256']==PIN and capture['ordinary_native_proof']is False
    for row in capture['rows']:
        assert not row['timeout']
        for path,digest in row['artifact_sha256'].items():assert sha((folder/row['id']/path).read_bytes())==digest,(name,row['id'],path)
expected={'init-division':(2103,75),'next-division':(2179,255),'invariant':(2110,12),'action-property':(2112,13),'incomplete':(2109,75),'deadlock':(2114,11),'next-division-false-hook':(2179,255),'invariant-false-hook':(2110,12),'safe':(0,0),'empty-init':(0,0)}
rows={r['id']:r for r in load(BASE/'primary/capture.json')['rows']}
for w in [1,4,8]:
    for case,(returned,exitcode) in expected.items():
        observed=rows[f'{case}-w{w}-observed'];cli=rows[f'{case}-w{w}-cli'];events=observed['events']
        assert observed['exit']==0 and cli['exit']==exitcode,(case,w)
        assert [e for e in events if e.startswith('modelCheck-return-code:')]==[f'modelCheck-return-code:{returned}']
        assert events.index(f'modelCheck-return-code:{returned}')<events.index('observer-cleanup-enter')
        assert sum(e.startswith('worker-at-return:')and e.endswith(':false')for e in events)==w
        if case not in ['safe','empty-init']:
            assert not any(e.startswith('live-finalCheck-enter')for e in events)
        else:assert 'live-finalCheck-return:0'in events
        if case not in ['safe','empty-init','init-division']:
            assert sum(e.startswith('setErrState-return:true:')for e in events)==1,(case,w,events)
            if case in ['next-division','next-division-false-hook','invariant','invariant-false-hook']:
                assert any(e.startswith('setErrState-return:false:')for e in events),(case,w)
            for event in events:
                if event.startswith('setErrState-return:false:'):assert f'code={returned}:done=true'in event
example=(ROOT/'crates/dive-api/examples/stopped_periodic_candidate.rs').read_text().rstrip()
assert '```rust\n'+example+'\n```'in(ROOT/'docs/stopped-periodic-service.md').read_text()
matrix=load(BASE/'source-matrix.json')
assert not matrix['model_proof']and not matrix['ordinary_schedule_certified']and not matrix['family_certified']
for path,digest in matrix['source_sha256'].items():assert sha((ROOT/path).read_bytes())==digest,path
history=load(BASE/'history-before-stopped/manifest.json')
for path,digest in history['matrices_sha256'].items():
    raw=(BASE/'history-before-stopped/matrices'/path).read_bytes();assert sha(raw)==digest,path
    assert json.loads(raw)['source_sha256']==history['source_by_matrix'][path]
for digest,path in history['source_archive_paths'].items():assert sha(gzip.decompress((ROOT/path).read_bytes()))==digest,path
family=next(x for x in load(ROOT/'conformance/parity-features.json')['families']if x['id']=='F-023')
assert family['implementation_status']=='missing'and family['certification']=='audit-required'and 224 in family['issues']
print('Complete four60-row stopped-worker reference histories, original first-error/method/CLI identities and exact current-source qualification; no ordinary proof')
