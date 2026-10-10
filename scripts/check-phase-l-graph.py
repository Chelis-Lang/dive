"""Verify every native graph probe, raw outcome and preserved historical source."""
import importlib.util,json,hashlib
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/tlc-phase-l-graph'
spec=importlib.util.spec_from_file_location('phase_l',ROOT/'scripts/check-phase-l.py')
phase_l=importlib.util.module_from_spec(spec);spec.loader.exec_module(phase_l)
phase_l.verify(BASE,'capture.json','selection.json','raw',15)
phase_l.verify(BASE,'collision-capture.json','collision-selection.json','raw-collision',1)
phase_l.verify(BASE,'tlc-capture.json','tlc-selection.json','raw-tlc',15,False)
phase_l.verify(BASE/'historical-first-compiler','capture.json','selection.json','raw',15)
for history in ['historical-first-collision','historical-before-forced-state']:
 phase_l.verify(BASE/history,'collision-capture.json','collision-selection.json','raw-collision',1)
for family in ['tlc-planning','tlc-tableau','tlc-phase-l']:
 base=ROOT/'conformance/probes'/family/'historical-before-graph'
 matrix=json.loads((base/'source-matrix.json').read_text())
 for name,digest in matrix['source_sha256'].items():assert phase_l.sha(base/name)==digest,name
layout=json.loads((BASE/'historical-std-helper-layout.json').read_text())
for name,digest in layout['source_sha256'].items():assert phase_l.sha(BASE/name)==digest,name
for history in ['historical-before-model-ownership','historical-before-compile-ownership']:
 before=BASE/history
 matrix=json.loads((before/'source-matrix.json').read_text())
 for name,digest in matrix['source_sha256'].items():assert phase_l.sha(before/name)==digest,name
 observation=json.loads((before/'observation.json').read_text())
 for name,digest in observation['source_sha256'].items():assert phase_l.sha(before/name)==digest,name
import gzip
original=json.loads((BASE/'historical-delivery-source/source-matrix.json').read_text())
for name,digest in original['source_sha256'].items():
 assert hashlib.sha256(gzip.decompress((BASE/'historical-delivery-source'/(name+'.gz')).read_bytes())).hexdigest()==digest,name
history=ROOT/'conformance/probes/predecessor-spill/historical-before-concrete-graph'
old=json.loads((history/'source-matrix.json').read_text())
for name,digest in old['source_sha256'].items():
 assert hashlib.sha256(gzip.decompress((history/'sources'/(name+'.gz')).read_bytes())).hexdigest()==digest,name
matrix=json.loads((BASE/'source-matrix.json').read_text())
for name,digest in matrix['source_sha256'].items():assert phase_l.sha(ROOT/name)==digest,name
rows=json.loads((BASE/'capture.json').read_text())['rows']
assert sum(r['fields'].get('completion')=='graph-inserted-no-scc-proof' for r in rows)==12
assert sum(r['exit']!=0 for r in rows)==2
assert sum(r['fields'].get('completion')=='outside-null-one-group-projection' for r in rows)==1
print('verified complete15 graph models,1 forced-collision control,15 clean TLC outcomes and all historical attempts/source matrices; no strict verdict claim')
