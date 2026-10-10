"""Verify the complete pinned conditional conversion selection and failures."""
import gzip, hashlib, json
from pathlib import Path

root=Path(__file__).resolve().parent.parent
probe=root/'conformance/probes/state-if-planning'
pin='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
sha=lambda data:hashlib.sha256(data).hexdigest()
names=['StateGuardShortFalse','StateGuardShortTrue','StateGuardNegated',
       'StateGuardImplies','StateGuardEquiv','TemporalGuard','StateThenError',
       'StateElseError','StateThenFalse','StateValid','ConstantSkipThen',
       'ConstantSkipElse','ConstantChosenThen','ConstantChosenElse',
       'StateBothErrors','ConstantBadGuard','StateNonBoolThen','StateNonBoolElse',
       'NestedState','LetState','CallState','ActionThenError','ActionValid',
       'StateGuardError']
def verify(directory, expected):
    capture=json.loads((directory/'capture.json').read_text())
    assert capture['oracle_sha256']==pin and capture['reason']
    assert capture['source_commit']=='94d0c5070852b37b801bb8998c436383ab7ee37b'
    assert [row['id'] for row in capture['rows']]==expected
    for table in ['source_sha256','raw_sha256']:
        for name,digest in capture[table].items():
            assert sha((directory/name).read_bytes())==digest,name
    for row in capture['rows']:
        assert row['exit']==0
        fields={}
        for line in (directory/'raw'/row['id']/'stdout').read_text().splitlines():
            key,value=line.split('\t',1);assert key not in fields;fields[key]=value
        assert fields==row['fields']
    return {row['id']:row['fields'] for row in capture['rows']}
current=verify(probe,names)
verify(probe/'historical-original-18',names[6:])
assert current['StateThenError']['error_code']=='2179'
assert current['CallState']['completion']=='planned'
assert current['StateGuardEquiv']['completion']=='planned'
assert current['StateGuardImplies']['error_code']=='2179'
assert current['TemporalGuard']['g0.tableau']=='true'
assert current['ActionThenError']['phase']=='binding'
history=probe/'historical-action-wrapper-rejection'
manifest=json.loads((history/'manifest.json').read_text())
for name,digest in manifest['sha256'].items():assert sha((history/name).read_bytes())==digest
history=probe/'native-before';manifest=json.loads((history/'manifest.json').read_text())
assert set(manifest['sha256'])=={str(p.relative_to(history)) for p in history.rglob('*.gz')}
for name,digest in manifest['sha256'].items():assert sha(gzip.decompress((history/name).read_bytes()))==digest
assert b'pinned TLC raises2179' in gzip.decompress((history/'baseline-failure.log.gz').read_bytes())
print('verified all24 current and18 historical conditional controls, original capture failure and native regressions')
