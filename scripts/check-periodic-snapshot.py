"""Verify all actual snapshot rows, preserved epochs and no ordinary proof credit."""
from pathlib import Path
import gzip
import hashlib
import json
import sys
import tarfile

ROOT = Path(__file__).resolve().parent.parent
BASE = ROOT / 'conformance/probes/periodic-snapshot'
sha = lambda data: hashlib.sha256(data).hexdigest()
load = lambda path: json.loads(path.read_text())
PIN = '7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
primary_errors = {5:2280,6:2280,7:2261,9:2179,12:-1,14:2229,15:2228,16:-1,18:2229,19:2228,20:-1,23:5006}
precedence_errors = {0:2222,1:2231,2:2232,3:2229,4:2229,5:2229,6:2229,7:2229}
readback = {'scope':'Actual FastTool snapshot/startup/empty-state body projections only; not ordinary checking.', 'ordinary_schedule_certified':False,'model_proof':False,'groups':[]}
for group, count, errors in [('',26,primary_errors),('precedence',12,precedence_errors)]:
    directory = BASE / group
    capture = load(directory/'capture.json')
    selection = load(directory/'selection.json')
    assert capture['oracle_sha256'] == PIN and capture['compile_exit'] == 0
    assert not capture['model_proof'] and not capture['ordinary_schedule_certified']
    assert len(capture['rows']) == len(selection) == count
    assert [r['id'] for r in capture['rows']] == [s['id'] for s in selection]
    with tarfile.open(directory/'primary.tar.gz','r:gz') as archive:
        raw = {}
        for member in archive:
            assert member.isfile() or member.isdir(), member.name
            assert not member.name.startswith('/') and '..' not in Path(member.name).parts
            if member.isfile(): raw[member.name.removeprefix('./')] = archive.extractfile(member).read()
        assert raw['capture.json'] == (directory/'capture.json').read_bytes()
        for name,digest in capture['source_sha256'].items():
            assert sha(raw[name]) == digest == sha((directory/name).read_bytes()),name
        files = 0
        for index,(row,selected) in enumerate(zip(capture['rows'],selection)):
            assert row['timeout'] is False
            prefix=f'case-{index:03}/'
            assert raw[prefix+'Snapshot.tla'].decode()==selected['source']
            assert raw[prefix+'Snapshot.cfg'].decode()==selected['config']
            if selected['lib']: assert raw[prefix+'Helper.tla'].decode()==selected['lib']
            for name,digest in row['files_sha256'].items():
                assert sha(raw[prefix+name])==digest,(group,index,name)
                if name.endswith(('.tla','.cfg')):
                    assert sha((directory/'models'/f'case-{index:03}'/name).read_bytes())==digest
                files+=1
            events=[line[6:] for line in raw[prefix+'stdout'].decode().splitlines() if line.startswith('EVENT\t')]
            assert events==row['events'],(group,index)
            code=errors.get(index)
            assert row['exit']==(2 if code is not None else 0)
            actual_errors=[e for e in events if e.startswith('exception-code:')]
            assert actual_errors==([] if code is None else [f'exception-code:{code}'])
            if code is not None:
                cls='java.lang.ClassCastException' if code==-1 else ('tlc2.tool.ConfigFileException' if code==5006 else ('tlc2.tool.EvalException' if code==2179 else ('util.Assert$TLCDetailedRuntimeException' if code in [2280,2261] else 'util.Assert$TLCRuntimeException')))
                assert f'exception-class:{cls}' in events
                if group or index in [12,14,15,16,18,19,20,23]: assert 'setup-complete' not in events
            if 'body-present:true' in events:
                assert 'snapshot-class:tla2sany.semantic.OpDefNode' in events
                assert 'definition-arity:0' in events and 'body-identical:true' in events
                assert f'definition-name:{"Replacement" if not group and index==11 else "Hook"}' in events
                if code is None:
                    value='FALSE' if not group and index in [2,11] else ('1' if not group and index==8 else 'TRUE')
                    assert f'value:{value}' in events
            if not group and index==0: assert 'body-present:false' in events
            stderr=raw[prefix+'stderr'].decode()
            assert 'EVENT\tsetup-start' in stderr
            if not group and index in [17,18,19,20]: assert stderr.count('"STARTUP_MARKER"')==1
            if not group and index==21:
                assert stderr.count('"HOOK_MARKER"')==1 and raw[prefix+'stdout'].decode().count('"HOOK_MARKER"')==1
            if not group and index==25: assert stderr.count('"SUBSTITUTION_MARKER"')==1
            if group and index==10: assert '"WITNESS_NOT_CACHED"' not in stderr
            if group and index==11: assert stderr.count('"ERASED_STATE_MARKER"')==1
    readback['groups'].append({'name':group or 'original','rows':count,'files_verified':files,'capture_sha256':sha((directory/'capture.json').read_bytes()),'archive_sha256':sha((directory/'primary.tar.gz').read_bytes())})
receipt=BASE/'independent-readback.json'
if '--write-readback' in sys.argv: receipt.write_text(json.dumps(readback,indent=2)+'\n')
assert load(receipt)==readback
history=BASE/'history-before-snapshot'
if (history/'manifest.json').exists():
    manifest=load(history/'manifest.json')
    for name,digest in manifest['matrices_sha256'].items():assert sha((history/'matrices'/name).read_bytes())==digest,name
    for digest,name in manifest['source_archive_paths'].items():assert sha(gzip.decompress((ROOT/name).read_bytes()))==digest,name
    for sources in manifest['source_by_matrix'].values():assert all(d in manifest['source_archive_paths'] for d in sources.values())
for development in ['development-replay-module']:
    d=BASE/development;record=load(d/'record.json')
    assert sha(gzip.decompress((d/'test.rs.gz').read_bytes()))==record['test_sha256']
    assert record['exit']==101 and 'FAILED' in (d/'focused-8.log').read_text()
example=(ROOT/'crates/dive-api/examples/periodic_snapshot.rs').read_text().rstrip()
assert '```rust\n'+example+'\n```' in (ROOT/'docs/configured-periodic-snapshot.md').read_text()
if (BASE/'source-matrix.json').exists():
    current=load(BASE/'source-matrix.json')
    assert not current['ordinary_schedule_certified'] and not current['model_proof']
    for name,digest in current['source_sha256'].items():assert sha((ROOT/name).read_bytes())==digest,name
native=BASE/'native-current'
if (native/'manifest.json').exists():
    manifest=load(native/'manifest.json')
    assert len(manifest['rows'])==38 and len(manifest['replay_rows'])==4
    for name,digest in manifest['artifact_sha256'].items():assert sha((native/name).read_bytes())==digest,name
    for name in manifest['rows']:
        row=load(native/name)
        assert not row['model_proof'] and not row['ordinary_schedule_certified']
    for name in manifest['replay_rows']:
        row=load(native/name)
        assert row['memory']==row['json']=='Valid'
        assert row['wrong_init'].startswith('Invalid(') and row['wrong_closing_next'].startswith('Invalid(')
        assert 'Next' in row['wrong_closing_next']
        assert row['wrong_property']!='Valid' and row['malformed_ending']!='Valid'
        assert not row['model_proof'] and not row['ordinary_schedule_certified']
print('Verified38 actual configured snapshot observations, bounded native diagnostic epochs and no ordinary proof')
