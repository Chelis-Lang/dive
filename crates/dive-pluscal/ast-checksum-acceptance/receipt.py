#!/usr/bin/env python3
"""Bind final AST/rewrite sources, immutable executables and complete evidence."""
import argparse,hashlib,json,re
from pathlib import Path
BASE=Path(__file__).resolve().parent;ROOT=BASE.parents[2]
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main(a):
 gate=BASE/'gates';gate.mkdir(exist_ok=True)
 for name in ['full-gate','release','comparison','full-controls','models','guide','mutations','bounds','label-root','cm1','validation-smoke']:
  path=a.logs/('dive-pluscal-ast-checksum-'+name+'.log');assert path.is_file(),name;(gate/(name+'.log')).write_bytes(path.read_bytes())
 full=(gate/'full-gate.log').read_text();matched=re.search(r'(\d+) tests run: \1 passed',full);assert matched and 'advisories ok, bans ok, licenses ok, sources ok' in full
 assert 'gate: 20 of 20 models' in (gate/'cm1.log').read_text() and 'gate: passed' in (gate/'cm1.log').read_text()
 assert 'gate: 16 of 16 models' in (gate/'validation-smoke.log').read_text() and 'gate: passed' in (gate/'validation-smoke.log').read_text()
 ast=json.loads((BASE/'comparison.json').read_text());assert ast['selected']==216 and ast['summary']=={'exact-post-translation-serialization':162,'rejection-accounted':22,'named-refusal':32} and ast['binary_sha256']==sha(a.checksum)
 report=json.loads((BASE/'full-controls/report.json').read_text());assert report['selected']==216 and report['native_sha256']==sha(a.native) and report['editor_sha256']==sha(a.editor) and len(report['corruption_controls'])==12
 models=json.loads((BASE/'finite-models/report.json').read_text());assert models['selected']==8 and models['worker_store_controls']==48 and models['native_sha256']==sha(a.native)
 assert json.loads((BASE/'mutation-controls.json').read_text())['controls']==37
 assert json.loads((BASE/'bounds-controls.json').read_text())['controls']==512
 assert json.loads((BASE/'guide/report.json').read_text())['controls']==13
 assert json.loads((BASE/'label-root-controls/report.json').read_text())['selected']==12
 paths=['Cargo.toml','Cargo.lock','README.md','CHANGELOG.md','conformance/parity-features.json','docs/pluscal-rewriting.md','docs/decisions/0047-bounded-translation-block-editing.md','docs/divergences.md','docs/tlc-differences.md','docs/provenance.md','docs/tlc-parity.md','crates/dive-pluscal/tests/algorithm_checksum.rs','crates/dive-pluscal/tests/rewrite.rs','crates/dive-api/tests/rewrite.rs','crates/dive-pluscal/examples/rewrite.rs','crates/dive-pluscal/examples/algorithm_checksum.rs']
 sources={p:sha(ROOT/p) for p in paths}
 for folder in ['crates/dive-pluscal/src','crates/dive-api/src','crates/dive-cli/src']:
  for p in (ROOT/folder).rglob('*.rs'):sources[str(p.relative_to(ROOT))]=sha(p)
 evidence={str(p.relative_to(ROOT)):sha(p) for p in BASE.rglob('*') if p.is_file() and '__pycache__' not in p.parts and p.name!='delivery.json'}
 receipt={'schema_version':1,'parent_commit':'2443cba12e19a0ff6c9424f21f02dee12cec191d','scope':'Bounded exact post-translation AST CRC derivation and Full in-memory rewriting. No full AST-shape/formatter/file/map/family certification or checksum-integrity/model proof.','family_certified':False,'reference_crc_derivation_verified_in_supported_scope':True,'fair_header_crc_only_limits_source_identity':True,'native_sha256':sha(a.native),'editor_sha256':sha(a.editor),'checksum_sha256':sha(a.checksum),'harness_sha256':sha(a.harness),'gate_status':'passed','rust_tests':int(matched.group(1)),'capture_rows':216,'option_controls':12,'ast_summary':ast['summary'],'full_summary':report['summary'],'native_sany_validator_checks':report['native_sany_and_validator_checks'],'validator_corruptions':12,'finite_models':8,'worker_store_controls':48,'independent_strict_witnesses':4,'invalid_witnesses':16,'integrity_mutations':37,'byte_budget_controls':512,'guide_controls':13,'ancestry_cm1_models':20,'validation_smoke_models':16,'checked_sources':sources,'archived_evidence':evidence}
 (BASE/'delivery.json').write_text(json.dumps(receipt,indent=2)+'\n');print('Bound',len(sources),'sources and',len(evidence),'evidence files')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for key in ['native','editor','checksum','harness']:p.add_argument('--'+key,type=Path,required=True)
 p.add_argument('--logs',type=Path,default=Path('/tmp'));main(p.parse_args())
