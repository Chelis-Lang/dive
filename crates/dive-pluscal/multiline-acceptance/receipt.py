#!/usr/bin/env python3
"""Bind the exact multiline source, immutable binaries and complete controls."""
import argparse,hashlib,json,re
from pathlib import Path
BASE=Path(__file__).resolve().parent;ROOT=BASE.parents[2]
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main(a):
 gates=BASE/'gates';gates.mkdir(exist_ok=True)
 logs={'full-gate':'full-gate','full-controls':'full-controls-3','partial-controls':'partial-controls-3-final','models':'models-3','cm1':'cm1','validation-smoke':'validation-smoke'}
 for name,suffix in logs.items():
  source=a.logs/('dive-pluscal-multiline-'+suffix+'.log');assert source.is_file(),name;(gates/(name+'.log')).write_bytes(source.read_bytes())
 gate=(gates/'full-gate.log').read_text();matched=re.search(r'(\d+) tests run: \1 passed',gate);assert matched and 'advisories ok, bans ok, licenses ok, sources ok' in gate
 assert gate.count('STATUS 0\n')==12
 for name,count in [('cm1',20),('validation-smoke',16)]:
  text=(gates/(name+'.log')).read_text();assert f'gate: {count} of {count} models' in text and 'gate: passed' in text
 full=json.loads((BASE/'full-controls/report.json').read_text());partial=json.loads((BASE/'partial-controls/report.json').read_text())
 for report,admitted in [(full,'full-derivation-and-admission'),(partial,'body-derivation-and-admission')]:
  assert report['selected']==208 and report['native_sany_and_validator_checks']==260 and report['primary_sany_accepted']==164
  assert report['native_sha256']==sha(a.native) and report['editor_sha256']==sha(a.editor) and report['cst_sha256']==sha(a.cst)
  for profile in ['dive','tlc']:assert report['summary'][profile]=={admitted:130,'rejected':20,'primary-invalid-rejected':24,'named-refusal':34}
 assert len(full['corruption_controls'])==12
 models=json.loads((BASE/'finite-models/report.json').read_text());assert models['selected']==28 and models['worker_store_controls']==336 and models['valid_witness_controls']==14 and models['rejected_witness_mutants']==56 and models['native_sha256']==sha(a.native)
 assert json.loads((BASE/'mutation-controls.json').read_text())['controls']==37
 negatives=json.loads((BASE/'partial-negative-controls/report.json').read_text());assert len(negatives['mutations'])==35 and len(negatives['corruptions'])==6
 assert json.loads((BASE/'bounds-controls.json').read_text())['controls']==512
 guide=json.loads((BASE/'guide/report.json').read_text());assert guide['controls']==20 and guide['api_example_sha256']==sha(a.api) and guide['guide_sha256']==sha(ROOT/'docs/pluscal-multiline.md') and guide['cargo_source_sha256']==sha(ROOT/guide['cargo_source'])
 paths=['AGENTS.md','Cargo.toml','Cargo.lock','rust-toolchain.toml','CHANGELOG.md','conformance/parity-features.json','docs/pluscal-multiline.md','docs/pluscal-translation.md','docs/pluscal-rewriting.md','docs/pluscal-checksum-expressions.md','docs/decisions/0047-bounded-translation-block-editing.md','docs/divergences.md','docs/tlc-differences.md','docs/provenance.md','docs/tlc-parity.md','crates/dive-api/tests/multiline.rs','crates/dive-api/tests/translation.rs','crates/dive-api/examples/pluscal_multiline.rs','scripts/conformance-gate.sh','conformance/gates/cm1.txt','conformance/gates/validation-smoke.txt']
 sources={path:sha(ROOT/path) for path in paths}
 for folder in ['crates/dive-pluscal/src','crates/dive-api/src','crates/dive-cli/src','crates/dive-conformance/src']:
  for path in (ROOT/folder).rglob('*.rs'):sources[str(path.relative_to(ROOT))]=sha(path)
 evidence={str(path.relative_to(ROOT)):sha(path) for path in BASE.rglob('*') if path.is_file() and '__pycache__' not in path.parts and path.name!='delivery.json'}
 result={'schema_version':1,'parent_commit':'27071918969e2cd1e4d1fa71077c31a8874f6105','scope':'Bounded non-junction sequential multiline native generation; exact significant operator CST, supported post-Explode CRC derivation and artifact admission. Macro/process/procedure, binders and junction multiline generation remain named refusals. No full translator/formatter/integrity/model/family certification.','family_certified':False,'fair_header_crc_only_limits_source_identity':True,'gate_status':'passed','rust_tests':int(matched.group(1)),'capture_rows':208,'full_summary':full['summary'],'partial_summary':partial['summary'],'sany_cst_validator_checks':520,'full_corruptions':12,'partial_corruptions':6,'full_forged_reports_rejected':37,'partial_forged_reports_rejected':35,'finite_models':28,'worker_store_controls':336,'strict_valid_witnesses':14,'invalid_witnesses':56,'byte_span_controls':512,'guide_controls':20,'ancestry_cm1_models':20,'validation_smoke_models':16,'harness_scope':'Immutable unchanged ancestor harness copied from frozen270719 delivery; explicit native candidate path, private verified corpus cache. Current-main CM1 additions belong to parent integration.','native_sha256':sha(a.native),'editor_sha256':sha(a.editor),'cst_sha256':sha(a.cst),'checksum_sha256':sha(a.checksum),'api_example_sha256':sha(a.api),'harness_sha256':sha(a.harness),'checked_sources':sources,'archived_evidence':evidence}
 (BASE/'delivery.json').write_text(json.dumps(result,indent=2)+'\n');print('Bound',len(sources),'sources and',len(evidence),'evidence files')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for key in ['native','editor','cst','checksum','api','harness']:p.add_argument('--'+key,type=Path,required=True)
 p.add_argument('--logs',type=Path,default=Path('/tmp'));main(p.parse_args())
