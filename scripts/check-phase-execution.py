"""Verify the full execution selection and every preserved source/raw family."""
from pathlib import Path
import importlib.util,json,hashlib
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/tlc-phase-execution'
spec=importlib.util.spec_from_file_location('phase_l',ROOT/'scripts/check-phase-l.py')
phase_l=importlib.util.module_from_spec(spec);spec.loader.exec_module(phase_l)
phase_l.verify(BASE,'capture.json','selection.json','raw',26)
phase_l.verify(BASE/'historical-before-record-order','capture.json','selection.json','raw',26)
first=BASE/'historical-first-compiler';record=json.loads((first/'capture.json').read_text())
assert record['compiler']['exit']!=0 and record['rows']==[]
assert len(json.loads((first/'selection.json').read_text()))==26
assert phase_l.sha(first/'selection.json')==record['selection_sha256']
for name,digest in record['source_sha256'].items():assert phase_l.sha(first/name)==digest,name
for stream in ['stdout','stderr']:assert phase_l.sha(first/'raw'/('compiler.'+stream))==record['compiler'][stream+'_sha256']
rows=json.loads((BASE/'capture.json').read_text())['rows']
assert sum(r['exit']==0 and r['fields'].get('groups')!='0' for r in rows)==15
assert sum(r['exit']==0 and r['fields'].get('groups')=='0' for r in rows)==3
assert sum(r['exit']!=0 for r in rows)==8
clean=BASE/'clean-tlc-reference';record=json.loads((clean/'capture.json').read_text())
assert record['oracle_jar_sha256']=='7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d'
assert [r['model'] for r in record['rows']]==json.loads((clean/'selection.json').read_text())
assert len(record['rows'])==8
for row in record['rows']:
 model=row['model'];case=clean/model
 for stream in ['stdout','stderr']:assert phase_l.sha(case/stream)==row[stream+'_sha256']
 assert phase_l.sha(case/(model+'.tla'))==row['source_sha256']
 assert phase_l.sha(case/(model+'.cfg'))==row['config_sha256']
 assert '-dumpTrace' not in row['flags'] and row['flags'][row['flags'].index('-workers')+1]=='1'
for family in ['tlc-planning','tlc-tableau','tlc-phase-l','tlc-phase-l-graph']:
 base=ROOT/'conformance/probes'/family/'historical-before-execution';matrix=json.loads((base/'source-matrix.json').read_text())
 for name,digest in matrix['source_sha256'].items():assert phase_l.sha(base/name)==digest,name
matrix=json.loads((BASE/'source-matrix.json').read_text())
for name,digest in matrix['source_sha256'].items():assert phase_l.sha(ROOT/name)==digest,name
print('verified all26 execution rows:15 completed graph fields,3 no-OOS boundaries,8 retained failures;8 clean TLC precedence controls and preserved producer histories; no strict verdict claim')
