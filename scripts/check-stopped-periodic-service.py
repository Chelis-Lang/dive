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

# Closed compiled/runtime producer is distinct from delivery-only Python/docs/maps.
gate=BASE/'gate-current';before=load(gate/'before.json');after=load(gate/'after.json')
assert before['producer']=='8820b20158e48ff0ee98ab582ed3eb87cc1d3238'
assert before['source_sha256']==after['source_sha256']and after['all_steps_passed']and after['build_inputs_unchanged']
assert len(before['source_sha256'])==487 and len(before['literal_include_sources'])==50 and not before['unresolved_include_macros']
for path,digest in before['source_sha256'].items():
    raw=gzip.decompress((gate/'source'/(path+'.gz')).read_bytes());assert sha(raw)==digest,path
compiled=load(gate/'compiled-source-readback.json')
assert compiled['producer']==before['producer']and compiled['source_sha256']==before['source_sha256']
assert len(compiled['source_git_blob_sha1'])==487
# Receiving integrations validate their own source map; they cannot retag the historical compiled producer.
steps=load(gate/'steps.json');assert len(steps)==48 and all(s['exit']==0 for s in steps)
assert '1087 tests run: 1087 passed, 0 skipped'in(gate/'workspace.log').read_text()
assert sha((gate/'workspace.log').read_bytes())==after['log_sha256']
runtime=gate/'runtime-current';closed=load(runtime/'runtime-closed.json');selection=load(runtime/'selection.json');manifest=load(runtime/'native-manifest.json')
assert closed['production_commit']==before['producer']==selection['production_commit']==manifest['production_commit']
assert closed['source_and_binaries_unchanged']and closed['source_sha256']==before['source_sha256']
assert len(closed['steps'])==17 and all(s['exit']==0 for s in closed['steps'])
assert sha((runtime/'runtime-source.rs').read_bytes())==selection['runner_sha256']==manifest['runner_source_sha256']
assert len(selection['rows'])==len(manifest['rows'])==146
assert {r['id']for r in selection['rows']}=={r['file'].removesuffix('.json')for r in manifest['rows']}
native=[]
for row in manifest['rows']:
    path=runtime/'native-current'/row['file'];assert sha(path.read_bytes())==row['sha256'];record=load(path)
    assert not record['model_proof']and not record['ordinary_schedule_certified'];native.append(record)
assert sum(r.get('stopped_service')is not None for r in native)==80
assert sum(r.get('terminal_service')is not None for r in native)==40
assert sum(not r['execution_retained']for r in native)==6
words=[r for r in native if 'document'in r];assert len(words)==12
for row in words:
    assert 'validation: Valid'in row['memory_validation']and 'validation: Valid'in row['json_validation']
    assert len(row['mutants'])==4
    for mutant in row['mutants']:
        assert 'validation: Invalid'in mutant['memory_validation']and 'validation: Invalid'in mutant['json_validation']
for corpus,count in [('examples',21),('validation-smoke',16)]:
    reports=list((runtime/(corpus+'-reports')).glob('*.json'));assert len(reports)==1
    models=load(reports[0])['models'];assert len(models)==count
    for model in models:
        assert model['outcome']=='agree'and {run['workers']for run in model['runs']}=={1,4,8}
        assert all(run['outcome']=='agree'and not run['diffs']for run in model['runs'])
for name in ['dive-api','dive-cli','dive-conformance','dive-trace']:
    assert 'test-rendezvous'not in(runtime/(name+'-default-feature-scope.log')).read_text()
assert 'feature="test-rendezvous"'not in(runtime/'default-check-cfg.log').read_text()
assert 'original=invariant-violation code=2110 source=0 workers=4 joined=true hook_calls=0 model_proof=false'==(runtime/'cargo-guide.log').read_text().strip()
current_history=load(BASE/'history-production-8820/manifest.json')
for path,digest in current_history['matrices_sha256'].items():
    raw=gzip.decompress((BASE/'history-production-8820/matrices'/(path+'.gz')).read_bytes());assert sha(raw)==digest
    assert json.loads(raw)['source_sha256']==current_history['source_by_matrix'][path]
for digest,path in current_history['source_archive_paths'].items():assert sha(gzip.decompress((ROOT/path).read_bytes()))==digest,path
print('Closed exact8820:48gate/1087zero-skip,487compiled inputs,146native rows/80stopped/40terminal/6preexecution,12Valid+48Invalid both routes;37gate models1/4/8; no ordinary proof')
