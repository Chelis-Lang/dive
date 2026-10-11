#!/usr/bin/env python3
"""Bind final native source, immutable executables and the complete evidence."""
import argparse,hashlib,json,re
from pathlib import Path
BASE=Path(__file__).resolve().parent
ROOT=BASE.parents[2]
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main(args):
 gate=BASE/'gates';gate.mkdir(exist_ok=True)
 for name in ['full-gate','release','comparison','validation','models','guide','mutations','bounds','cm1','validation-smoke']:
  source=args.logs/('dive-pluscal-rewrite-'+name+'.log');assert source.is_file(),name;(gate/(name+'.log')).write_bytes(source.read_bytes())
 full=(gate/'full-gate.log').read_text();matched=re.search(r'(\d+) tests run: \1 passed',full);assert matched and 'advisories ok, bans ok, licenses ok, sources ok' in full
 assert 'gate: 20 of 20 models' in (gate/'cm1.log').read_text() and 'gate: 16 of 16 models' in (gate/'validation-smoke.log').read_text()
 comparison=json.loads((BASE/'comparison.json').read_text());assert comparison['selected']==258 and all(not summary.get('disagree') for summary in comparison['summary'].values())
 assert json.loads((BASE/'artifact-validation/report.json').read_text())['selected']==292
 models=json.loads((BASE/'finite-models/report.json').read_text());assert models['selected']==8 and models['worker_store_controls']==48
 assert json.loads((BASE/'mutation-controls.json').read_text())['controls']==27
 assert json.loads((BASE/'guide/report.json').read_text())['controls']==11
 assert json.loads((BASE/'bounds-controls.json').read_text())['controls']==512
 paths=['README.md','Cargo.toml','Cargo.lock','crates/dive-api/src/rewrite.rs','crates/dive-api/src/lib.rs','crates/dive-api/tests/rewrite.rs','crates/dive-cli/src/translate.rs','crates/dive-cli/src/main.rs','crates/dive-pluscal/src/rewrite.rs','crates/dive-pluscal/src/lib.rs','crates/dive-pluscal/tests/rewrite.rs','crates/dive-pluscal/examples/rewrite.rs','docs/pluscal-rewriting.md','docs/decisions/0047-bounded-translation-block-editing.md','docs/divergences.md','docs/tlc-differences.md','docs/provenance.md','docs/tlc-parity.md','conformance/parity-features.json','CHANGELOG.md']
 sources={p:sha(ROOT/p) for p in paths}
 # Bind complete translator/parser implementation, not merely the new editor.
 for folder in ['crates/dive-pluscal/src','crates/dive-api/src','crates/dive-cli/src']:
  for p in (ROOT/folder).rglob('*.rs'):sources[str(p.relative_to(ROOT))]=sha(p)
 evidence={str(p.relative_to(ROOT)):sha(p) for p in BASE.rglob('*') if p.is_file() and '__pycache__' not in p.parts and p.name!='delivery.json'}
 receipt={'schema_version':1,'parent_commit':'5d1ad4d0eee22754e7f7f9424a8a2457e49d32f3','scope':'Bounded in-memory block editor and explicit body-only CRC. No full AST CRC/file-write/generator-byte/family certification.','family_certified':False,'algorithm_checksum_verified':False,'native_sha256':sha(args.native),'example_sha256':sha(args.example),'harness_sha256':sha(args.harness),'gate_status':'passed','rust_tests':int(matched.group(1)),'capture_rows':258,'primary_rejections':93,'primary_invalid_artifacts':3,'native_admitted_per_profile':146,'exact_primary_editor_bytes':48,'native_artifact_checks':292,'finite_models':8,'workers_stores':48,'independent_strict_witnesses':4,'invalid_witnesses':16,'integrity_mutations':27,'byte_budget_controls':512,'ancestry_cm1_models':20,'validation_smoke_models':16,'checked_sources':sources,'archived_evidence':evidence}
 (BASE/'delivery.json').write_text(json.dumps(receipt,indent=2)+'\n');print('Bound',len(sources),'sources and',len(evidence),'evidence files')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__)
 for name in ['native','example','harness']:p.add_argument('--'+name,type=Path,required=True)
 p.add_argument('--logs',type=Path,default=Path('/tmp'));main(p.parse_args())
