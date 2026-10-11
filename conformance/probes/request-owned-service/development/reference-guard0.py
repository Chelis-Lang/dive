"""Complete controlled reference epochs and request-owned nonproof source contract."""
from pathlib import Path
import gzip, hashlib, json, tarfile
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/request-owned-service'
sha=lambda b:hashlib.sha256(b).hexdigest()
load=lambda p:json.loads(p.read_text())
index=load(BASE/'primary-index.json');readback=load(BASE/'primary-independent-readback.json')
assert sha((BASE/'primary-history.tar.gz').read_bytes())==index['archive_sha256']==readback['reference_archive_sha256']
assert readback['all926_archived_live_pairs_equal'] and readback['qualified_rows']==27
with tarfile.open(BASE/'primary-history.tar.gz')as archive:
    raw={}
    for member in archive:
        assert not member.name.startswith('/') and '..'not in Path(member.name).parts
        assert member.isfile()or member.isdir()
        if member.isfile():raw[member.name]=archive.extractfile(member).read()
pin='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
verified=0
for epoch,files in index['artifacts'].items():
    for name,digest in files.items():assert sha(raw[epoch+'/'+name])==digest,(epoch,name);verified+=1
assert verified==926
for category,expected in [('empty',18),('nonempty',9)]:
    epoch,count=index['qualified_primary'][category];assert count==expected
    capture=json.loads(raw[epoch+'/capture.json']);assert capture['oracle_sha256']==pin
    assert len(capture['rows'])==count
    cases=['empty-hook-true','empty-hook-false','empty-hook-false-repeat','empty-hook-error','empty-candidate','empty-failed-init']if category=='empty'else['nonempty-hook-false-repeat','nonempty-hook-error','nonempty-candidate-before-hook']
    assert [(r['width'],r['scenario'])for r in capture['rows']]==[(w,c)for w in [1,4,8]for c in cases]
    for name,digest in capture['sources'].items():assert sha(raw[epoch+'/'+name])==digest
    for row in capture['rows']:
        scenario=row['scenario'];assert not row['timeout']
        assert row['exit']==(2 if scenario.endswith('hook-error')else 0)
        for name,digest in row['sha256'].items():assert sha(raw[f'{epoch}/{scenario}-{row["width"]}/{name}'])==digest
        events=row['events']
        assert not any(e.startswith('injector-error')for e in events)
        if scenario=='empty-failed-init':assert 'failed-init-no-start'in events and 'start-workers'not in events;continue
        assert 'suspend-return:true'in events
        if category=='empty':
            assert 'first-next-gate-at-suspend-return:false'in events
            assert not any(e.startswith('before-doNextCheckLiveness-write:')for e in events[:events.index('suspend-return:true')])
        else:assert 'actual-root0-written-before-request'in events
        if scenario.endswith('hook-false-repeat'):
            assert ('second-service-result:2104'in events)if category=='empty'else('second-periodic:2104'in events)
        if scenario=='nonempty-candidate-before-hook':assert 'runTLC-result:2116'in events and 'resumeAll'not in events
        if scenario.endswith('hook-error'):assert any('2179'in e and ('code:'in e)for e in events)
pair=BASE/'paired-property-primary';d=load(pair/'capture.json');assert d['oracle_sha256']==pin
assert [(r['label'],r['exit'])for r in d['rows']]==[('violating',13),('holding',0)]
for row in d['rows']:
    for name,digest in row['sha256'].items():assert sha((pair/row['label']/name).read_bytes())==digest
example=(ROOT/'crates/dive-api/examples/request_owned_service.rs').read_text().rstrip()
assert '```rust\n'+example+'\n```'in(ROOT/'docs/request-owned-periodic-service.md').read_text()
history=BASE/'history-before-service/manifest.json'
if history.exists():
    record=load(history)
    for name,digest in record['matrices_sha256'].items():
        data=(BASE/'history-before-service/matrices'/name).read_bytes();assert sha(data)==digest,name
        assert json.loads(data)['source_sha256']==record['source_by_matrix'][name]
    for digest,name in record['source_archive_paths'].items():assert sha(gzip.decompress((ROOT/name).read_bytes()))==digest,name
current=BASE/'source-matrix.json'
if current.exists():
    record=load(current);assert not record['model_proof']and not record['ordinary_schedule_certified']and not record['family_certified']
    for name,digest in record['source_sha256'].items():assert sha((ROOT/name).read_bytes())==digest,name
native=BASE/'native-current/manifest.json'
if native.exists():
    record=load(native)
    for name,digest in record['artifact_sha256'].items():assert sha((BASE/name).read_bytes())==digest,name
    for name in record['rows']:
        row=load(BASE/name);assert not row['model_proof']and not row['ordinary_schedule_certified']
        if 'memory'in row:
            assert row['memory']==row['json']=='Valid'
            assert all(row[k].startswith('Invalid(')for k in ['wrong_property','wrong_init','wrong_closing_next','malformed_ending'])
            assert 'Next'in row['wrong_closing_next'];continue
        assert row['legacy_ordinal_invocations']==row['legacy_hook_evaluations']==0
        for service,pause in zip(row['services'],row['pauses'],strict=True):
            assert service['request_sequence']==pause['request_sequence']
            assert service['committed']==pause['frontier_appended']==pause['committed']
            assert service['stats']==pause['stats']and service['pool']==pause['pool']
            assert not service['model_proof']
        if name.split('/')[-1].startswith('nonempty-safe-'):
            assert any(s['committed']['end']>s['committed']['start']for s in row['services'])
print('Verified27 qualified controlled method rows/926 artifacts, paired final property controls and separate request-owned diagnostic contract')
