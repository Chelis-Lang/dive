#!/usr/bin/env python3
"""Bind the final immutable selection, source, executables and gate evidence."""
import argparse,hashlib,json
from pathlib import Path
BASE=Path(__file__).resolve().parent
ROOT=BASE.parents[2]
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main(args):
    gate=BASE/'gates';gate.mkdir(exist_ok=True)
    for name in ['gate','final-runtime','final-controls','cm1','validation-smoke']:
        source=args.logs/('dive-pluscal-multiple-procedures-'+name+'.log')
        assert source.is_file(),name
        (gate/(name+'.log')).write_bytes(source.read_bytes())
    complete=(gate/'gate.log').read_text()
    assert '686 tests run: 686 passed' in complete
    assert 'CHECKS PASSED' in complete
    controls=(gate/'final-controls.log').read_text()
    assert controls.count('EXIT 0')==7
    runtime=(gate/'final-runtime.log').read_text()
    assert runtime.count('EXIT 0')==2 and runtime.count('EXIT 1')==1
    assert 'gate: 20 of 20 models' in (gate/'cm1.log').read_text()
    assert 'gate: 16 of 16 models' in (gate/'validation-smoke.log').read_text()
    source_paths=['CHANGELOG.md','conformance/parity-features.json','crates/dive-api/src/translate.rs','crates/dive-pluscal/src/lib.rs','crates/dive-pluscal/src/translate/process.rs','crates/dive-pluscal/examples/translate.rs','crates/dive-pluscal/tests/multiple_procedures.rs','docs/decisions/0045-pluscal-shared-process-procedures.md','docs/divergences.md','docs/pluscal-process-procedures.md','docs/pluscal-multiple-procedures.md','docs/provenance.md','docs/tlc-differences.md','docs/tlc-parity.md']
    sources={p:sha(ROOT/p) for p in source_paths}
    for file in BASE.rglob('*'):
        if file.is_file() and file.suffix in ['.py','.rs','.java','.md'] and '__pycache__' not in file.parts:
            sources[str(file.relative_to(ROOT))]=sha(file)
    evidence={str(p.relative_to(ROOT)):sha(p) for p in BASE.rglob('*') if p.is_file() and '__pycache__' not in p.parts and p.name!='delivery.json'}
    reports={p:h for p,h in evidence.items() if p.endswith('.json')}
    logs={str(p.relative_to(ROOT)):sha(p) for p in gate.glob('*.log')}
    receipt={'schema_version':1,'scope':'Bounded shared procedures in disjoint finite process blocks; no full PlusCal certification. Canonical witness replay is qualified, never strict TLC credit.','parent_commit':'8b27ab2d11826a24c34a1e7906b4a1bf0b6a7a92','native_sha256':sha(args.native),'example_sha256':sha(args.example),'harness_sha256':sha(args.harness),'family_certified':False,'tlc_violation_certified':False,'gate_status':'passed','source_selection':166,'artifacts':94,'invalid_primary_artifacts':11,'model_rows':142,'worker_store_controls':852,'strict_complete':74,'strict_invariant_pending':66,'strict_deadlock_pending':2,'canonical_valid':68,'canonical_invalid_mutants':272,'rust_tests':686,'ancestry_cm1_models':20,'validation_smoke_models':16,'reports':reports,'checked_sources':sources,'gate_logs':logs,'archived_evidence':evidence}
    (BASE/'delivery.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print('Bound',len(reports),'reports,',len(sources),'sources and',len(evidence),'evidence files')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    for name in ['native','example','harness']:p.add_argument('--'+name,type=Path,required=True)
    p.add_argument('--logs',type=Path,default=Path('/tmp'))
    main(p.parse_args())
