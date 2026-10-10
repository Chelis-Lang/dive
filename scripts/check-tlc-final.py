"""Verify every selected final projection, native outcome and preserved source."""
from pathlib import Path
import importlib.util,json,hashlib
ROOT=Path(__file__).resolve().parent.parent
BASE=ROOT/'conformance/probes/tlc-final-check'
spec=importlib.util.spec_from_file_location('phase_l',ROOT/'scripts/check-phase-l.py')
phase=importlib.util.module_from_spec(spec);spec.loader.exec_module(phase)
phase.verify(BASE,'capture.json','selection.json','raw',30)
rows=json.loads((BASE/'capture.json').read_text())['rows']
assert sum(r['exit']==0 for r in rows)==19
assert sum(r['exit']!=0 for r in rows)==11
assert sum(r['fields'].get('final_check_code')=='0' for r in rows)==1
for name in ['historical-before-witness-fields','historical-missing-graph-cache','historical-development-final']:
 base=BASE/name;manifest=json.loads((base/'manifest.json').read_text())
 for path,digest in manifest.get('files',manifest.get('sha256',{})).items():assert phase.sha(base/path)==digest,path
for family in ['tlc-planning','tlc-tableau','tlc-phase-l','tlc-phase-l-graph','tlc-phase-execution']:
 base=ROOT/'conformance/probes'/family/'historical-before-final-check'
 matrix=json.loads((base/'source-matrix.json').read_text())
 for path,digest in matrix['source_sha256'].items():assert phase.sha(base/path)==digest,path
native=json.loads((BASE/'native-outcomes.json').read_text());assert native['selection_rows']==30 and native['widths']==[1,3,4,8]
assert len(native['rows'])==120
for i,row in enumerate(rows):
 for w,width in enumerate(native['widths']):
  n=native['rows'][4*i+w]
  assert n['model']==row['name'] and n['requested_workers']==width
  assert n['reference_exit']==row['exit'] and n['reference_failure_agreement_credit'] is False
  assert n['ordinary_strict_parity_credit'] is False
  if row['exit']==0:
   expected='final-only-holds' if row['fields']['final_check_code']=='0' else 'unvalidated-candidate'
   assert n['native']['status']==expected
  else:assert n['native']['status'] in ['binding-failure','refusal-or-error']
identity=BASE/'historical-before-report-identity'
history=json.loads((identity/'manifest.json').read_text())
for group in ['source_sha256','artifact_sha256']:
 for path,digest in history[group].items():assert phase.sha(identity/path)==digest,path
before=BASE/'historical-before-cli'
old=json.loads((before/'source-matrix.json').read_text())
for path,digest in old['source_sha256'].items():assert phase.sha(before/path)==digest,path
for path,digest in old['artifact_sha256'].items():assert phase.sha(before/path)==digest,path
matrix=json.loads((BASE/'source-matrix.json').read_text())
for path,digest in matrix['source_sha256'].items():assert phase.sha(ROOT/path)==digest,path
for path,digest in matrix['artifact_sha256'].items():assert phase.sha(BASE/path)==digest,path
print('verified all30 reference rows/120 native outcomes:19 completed final projections,11 failures without error agreement; exact source histories retained; ordinary strict parity remains unclaimed')

# Preserve current authority and the legacy final producer separately.
for family in ['tlc-planning','tlc-tableau','tlc-phase-l','tlc-phase-l-graph','tlc-phase-execution']:
 base=ROOT/'conformance/probes'/family/'historical-before-final-cli-integration'
 record=json.loads((base/'manifest.json').read_text())
 assert phase.sha(base/'source-matrix.json')==record['original_matrix_sha256']
 for path,digest in record['head_source_sha256'].items():assert phase.sha(base/path)==digest,path
base=BASE/'historical-before-current-source-integration'
record=json.loads((base/'source-matrix.json').read_text())
for path,digest in record['source_sha256'].items():assert phase.sha(base/path)==digest,path
for path,digest in record['artifact_sha256'].items():assert phase.sha(base/path)==digest,path
study=ROOT/'conformance/studies/tlc-final-cli-first'
record=json.loads((study/'after.json').read_text())
assert record['source_and_binaries_unchanged'] and record['harness_exit'] is None
for path,digest in record['artifact_sha256'].items():assert phase.sha(study/path)==digest,path
old=json.loads((study/'before.json').read_text());assert old['source_commit']=='72f8ae9023603f00f9ac7b5e181b1960212ccd71'
for path,digest in old['source_matrix']['source_sha256'].items():assert phase.sha(base/path)==digest,path
reports=list((study/'reports').glob('*.json'));assert len(reports)==1
report=json.loads(reports[0].read_text());assert report['summary']['models_selected']==140
assert len(report['models'])==140 and all(m['outcome']=='unsupported' for m in report['models'])
raw=list((study/'raw').iterdir());assert len(raw)==138
for row in raw:
 invocation=json.loads((row/'invocation.json').read_text())
 assert invocation['real_binary_sha256']==old['binary_sha256']['dive']
 assert (row/'stdout.txt').is_file() and (row/'stderr.txt').is_file()
print('verified complete140 historical refusal rows/138 raw subprocesses; no broad corpus credit')
